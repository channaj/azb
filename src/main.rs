use std::path::Path;
use std::fs::File;
use time::OffsetDateTime;
use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use azure_storage_blobs::container::operations::list_blobs::BlobItem;
use futures::stream::StreamExt;
use std::str;
use std::process::Command;
use serde::{Deserialize, Serialize};
use std::io::prelude::*;
use clap::Parser;


#[derive(Serialize, Deserialize)]
struct StorageAccountKey {
    value: String,
}


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the storage account
    #[arg(short('s'), long("storage-account"), env("STORAGE_ACCOUNT"), required(true))]
    storage_account: String,

    /// Storage account key
    #[arg(short('k'), long("storage-account-key"), env("STORAGE_ACCOUNT_KEY"))]
    storage_account_key: Option<String>,

    /// Name of the blob container
    #[arg(short('c'), long("container-name"), env("STORAGE_CONTAINER"), required(true))]
    container: String,

    /// Prefix of the blob
    #[arg(short('p'), long("prefix"), required(true))]
    prefix: String,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {

    let args = Args::parse();

    let credential = azure_identity::create_credential()?;

    let storage_credentials = StorageCredentials::token_credential(credential);
    
    // ******************************
    // let access_key =
    //     args.storage_account_key
    //     .or_else(|| get_storage_access_key(&args.storage_account).ok())
    //     .ok_or("Storage account key not found")?;

    // let storage_credentials = StorageCredentials::access_key(&args.storage_account, access_key);
   // ******************************
    
    let blob_container_client = 
        ClientBuilder::new(&args.storage_account, storage_credentials)
        .container_client(&args.container);
    
    let latest_blob = get_latest_blob(&blob_container_client, args.prefix).await;
       
    if let Some(blob) = latest_blob {
        let _ = process_blob(&blob_container_client, &blob.name).await;
    }

    Ok(())

}


#[derive(Debug)]
#[derive(Clone)]
struct Blob {
    name : String,
    last_updated : OffsetDateTime
}

fn make_blob (blob_item: BlobItem) -> Option<Blob> {
    match blob_item {
        BlobItem::Blob(blob) => {
            Some(
                Blob {
                    name: blob.name,
                    last_updated: blob.properties.last_modified
                })
        },
        BlobItem::BlobPrefix(_) => None
    }
}

async fn list_blobs (blob_container_client: &ContainerClient, prefix: String) -> Result<Vec<BlobItem>> {
    let mut list_stream = blob_container_client.list_blobs().prefix(prefix).into_stream();
    let mut ret:Vec<BlobItem> = Vec::new();

    while let Some(value) = 
        list_stream.next().await {
            let _blobs = value.map (|list_response| {
            ret.extend_from_slice(&list_response.blobs.items)
        });
    }
    Ok(ret)
}

async fn get_blob (blob_container_client: &ContainerClient, blob_name: &str) -> Result<Vec<u8>> {
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

async fn get_latest_blob (blob_container_client: &ContainerClient, prefix: String) -> Option<Blob> {
    list_blobs (blob_container_client, prefix)
        .await
        .map(|items| {
            let mut blobs: Vec<Blob> = items.into_iter()
                .filter_map(make_blob)
                .collect();
            
            blobs.sort_by(|a, b| a.last_updated.cmp(&b.last_updated));
            blobs.first().cloned()
        })
        .ok()
        .flatten()
}

fn get_storage_access_key(name: &str) -> Result<String> {
    
    let command = format!("az storage account keys list --account-name '{}'", name);
    let output = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("/C")
            .arg(&command)
            .output()
            .expect("failed to execute process")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process")
    };

    let response = str::from_utf8(&output.stdout)?;
    let keys: Vec<StorageAccountKey> = serde_json::from_str(response)?;

    keys.first()
        .map(|key| key.value.clone())
        .ok_or_else(|| "Storage account keys not found".into())
    
}

async fn process_blob(blob_container_client: &ContainerClient, blob_name: &str) -> Result<()> {

    let file_name = blob_name.split("/").last().unwrap_or("unknown");
    // Retrieve the blob
    let blob_content = get_blob(blob_container_client, blob_name).await
        .map_err(|err| format!("Error retrieving blob: {}", err))?;
    
    // Convert the blob content to a string (handling UTF-8 errors)
    let blob_content_str = std::str::from_utf8(&blob_content)
        .map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;
    
    // Write the content to a file
    let mut file = File::create(file_name)?;
    file.write_all(blob_content_str.as_bytes())?;

    // Open the file
    opener::open(Path::new(file_name))
        .map_err(|err| format!("Error opening file: {}", err))?;
    
    Ok(())
}
