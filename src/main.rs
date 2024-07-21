use azure_storage::prelude::*;
use azure_storage_blobs::container::operations::list_blobs::BlobItem;
use azure_storage_blobs::prelude::*;
use clap::Parser;
use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::str;
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
struct StorageAccountKey {
    value: String,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the storage account
    #[arg(
        short('s'),
        long("storage-account"),
        env("STORAGE_ACCOUNT"),
        required(true)
    )]
    storage_account: String,

    /// Storage account key
    #[arg(short('k'), long("storage-account-key"), env("STORAGE_ACCOUNT_KEY"))]
    storage_account_key: Option<String>,

    /// Name of the blob container
    #[arg(
        short('c'),
        long("container-name"),
        env("STORAGE_CONTAINER"),
        required(true)
    )]
    container: String,

    /// Prefix of the blob
    #[arg(required(true), index(1))]
    prefix: String,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let credential = azure_identity::create_credential()?;

    let storage_credentials = StorageCredentials::token_credential(credential);

    let bar = ProgressBar::new_spinner();
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {msg}").unwrap());
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_message("Finding the latest blob.");

    let blob_container_client = ClientBuilder::new(&args.storage_account, storage_credentials)
        .container_client(&args.container);
    let latest_blob = get_latest_blob(&blob_container_client, args.prefix).await;

    if let Some(blob) = latest_blob {
        bar.set_message(format!("Downloading {}", &blob.name));

        let _ = process_blob(&blob_container_client, &blob.name).await;
    }
    bar.finish();

    Ok(())
}

#[derive(Debug, Clone)]
struct Blob {
    name: String,
    last_updated: OffsetDateTime,
}

fn make_blob(blob_item: BlobItem) -> Option<Blob> {
    match blob_item {
        BlobItem::Blob(blob) => Some(Blob {
            name: blob.name,
            last_updated: blob.properties.last_modified,
        }),
        BlobItem::BlobPrefix(_) => None,
    }
}

async fn list_blobs(
    blob_container_client: &ContainerClient,
    prefix: String,
) -> Result<Vec<BlobItem>> {
    let mut list_stream = blob_container_client
        .list_blobs()
        .prefix(prefix)
        .into_stream();
    let mut ret: Vec<BlobItem> = Vec::new();

    while let Some(value) = list_stream.next().await {
        let _blobs = value.map(|list_response| ret.extend_from_slice(&list_response.blobs.items));
    }
    Ok(ret)
}

async fn get_blob(blob_container_client: &ContainerClient, blob_name: &str) -> Result<Vec<u8>> {
    let blob_client = blob_container_client.blob_client(blob_name);
    let mut stream = blob_client.get().into_stream();
    let mut result: Vec<u8> = vec![];

    while let Some(value) = stream.next().await {
        let mut body = value?.data;
        while let Some(value) = body.next().await {
            let value = value?;
            result.extend(&value);
        }
    }
    Ok(result)
}

async fn get_latest_blob(blob_container_client: &ContainerClient, prefix: String) -> Option<Blob> {
    list_blobs(blob_container_client, prefix)
        .await
        .map(|items| {
            let mut blobs: Vec<Blob> = items.into_iter().filter_map(make_blob).collect();

            blobs.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
            blobs.first().cloned()
        })
        .ok()
        .flatten()
}

async fn process_blob(blob_container_client: &ContainerClient, blob_name: &str) -> Result<()> {
    let file_name = blob_name
        .split("/")
        .last()
        .map(|x| x.trim())
        .unwrap_or("unknown");

    // Retrieve the blob
    let blob_content = get_blob(blob_container_client, blob_name)
        .await
        .map_err(|err| format!("Error retrieving blob: {}", err))?;

    // Convert the blob content to a string (handling UTF-8 errors)
    let blob_content_str = std::str::from_utf8(&blob_content)
        .map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

    // Write the content to a file
    let mut file = File::create(file_name)?;
    file.write_all(blob_content_str.as_bytes())?;

    // Open the file
    opener::open(Path::new(file_name)).map_err(|err| format!("Error opening file: {}", err))?;

    Ok(())
}
