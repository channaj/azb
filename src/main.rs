use azure_storage::prelude::*;
use azure_storage_blobs::container::operations::list_blobs::BlobItem;
use azure_storage_blobs::prelude::*;
use clap::{Parser, Subcommand};
use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::env::current_exe;
use std::fs::{self, File};
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
struct StorageArgs {
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

#[derive(Parser, Debug)]
struct OpenArgs {

    #[clap(flatten)]
    storage: StorageArgs,

    /// Name of the blob
    #[arg(long, short = 'n')]
    name: Option<String>

}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct App {
    
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Clean the 'blobs' directory (the default download location)
    Clean,

    /// List all blobs under the specified prefix.
    List(StorageArgs),

    /// Open a blob under the specified prefix.
    /// Blob updated most recently will be opened if no --name argument is provided. 
    Open(OpenArgs)
}
#[tokio::main]
async fn main() -> Result<()> {

    let app = App::parse();

    let bar = ProgressBar::new_spinner();
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {msg}").unwrap());
    bar.enable_steady_tick(Duration::from_millis(100));

    let credential = azure_identity::create_credential()?;

    let storage_credentials = StorageCredentials::token_credential(credential);

    match app.command {

        Command::Clean => {

          bar.set_message("Cleaning the 'blobs' directory.");
          let _ = clean();
          bar.set_message("Done");
          bar.finish();
        
        },

        Command::List(args) => {

          let blob_container_client = ClientBuilder::new(&args.storage_account, storage_credentials)
            .container_client(&args.container);

          bar.set_message("Finding blobs.");

          let blobs = 
              list_blobs(&blob_container_client, args.prefix).await?
              .into_iter()
              .filter_map(make_blob);
  
          for blob in blobs {
              println!("{} - {}", blob.name, blob.last_updated);
          }
          bar.finish();

        },

        Command::Open(args) => {

          let blob_container_client = ClientBuilder::new(&args.storage.storage_account, storage_credentials)
            .container_client(&args.storage.container);

          if let Some(name) = args.name {

            let blob_name = format! ("{}/{}", args.storage.prefix, name);
            bar.set_message(format!("Downloading {}", &blob_name));
            let _ = process_blob(&blob_container_client, &blob_name).await;

          } 
          else {

            bar.set_message("Finding the latest blob.");
            let latest_blob = get_latest_blob(&blob_container_client, &args.storage.prefix).await;
    
            if let Some(blob) = latest_blob {
                bar.set_message(format!("Downloading {}", &blob.name));
                let _ = process_blob(&blob_container_client, &blob.name).await;
            }
          }
          bar.finish();

        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct Blob {
    name: String,
    last_updated: OffsetDateTime,
}

fn clean() -> Result<()> {
  let current_dir = current_exe()?;

  let dir = current_dir
      .parent()
      .ok_or("Could not find parent directory")?;

  let blobs_dir = dir.join("blobs");

  if blobs_dir.exists() && blobs_dir.is_dir() {
      fn remove_dir_contents(dir: &Path) -> Result<()> {
          for entry in fs::read_dir(dir)? {
              let entry = entry?;
              let path = entry.path();
              if path.is_dir() {
                  remove_dir_contents(&path)?;
                  fs::remove_dir(&path)?;
              } else {
                  fs::remove_file(&path)?;
              }
          }
          Ok(())
      }

      remove_dir_contents(&blobs_dir)?;

  } else {
      println!("'blobs' directory does not exist.");
  }

  Ok(())
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

async fn get_latest_blob(blob_container_client: &ContainerClient, prefix: &str) -> Option<Blob> {
    list_blobs(blob_container_client, prefix.into())
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

    let current_dir = current_exe()?;

    let dir = current_dir
        .parent()
        .ok_or("Could not find parent directory")?;

    let file_path = dir
        .join("blobs")
        .join(blob_name)
        .parent()
        .ok_or("Could not find parent directory")?
        .join(file_name);

    let file_dir = file_path
        .parent()
        .ok_or("Could not find parent directory")?;
    std::fs::create_dir_all(file_dir)?;

    let mut file = File::create(file_path.clone())?;

    let open_path = file_path.as_path();

    file.write_all(blob_content_str.as_bytes())?;

    let open_result =
        opener::open(Path::new(open_path)).map_err(|err| format!("Error opening file: {}", err));

    match open_result {
        Ok(_) => (),
        Err(err) => println!("Error opening file: {}", err),
    }

    Ok(())
}
