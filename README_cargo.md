# azb
A cli tool to download files from Azure Storage Blobs and open them using the default program.

Uses default azure credentials to access Azure Storage. Alternatively, storage account key can be used.

## Usage

Please run `azb --help` for available commands and `--help` with the sub command for options (e.g `azb open --help`).

Both `STORAGE_ACCOUNT` and `STORAGE_CONTAINER` can also be read from environment variables.

If you choose to use storage account key instead of default Azure credentials, the value can be set using `STORAGE_ACCOUNT_KEY` environment variable or `-k` option.

### Examples


```
// Open the latest blob in the container under the specified prefix
azb open -s <storage_acount_name> -c <container_name> <prefix>
```

```
// List blobs under a prefix
azb list -s <storage_account_name> -c <container_name> <prefix>
```

```
// Open a blob under a prefix
azb open -s <storage_account_name> -c <container_name> <prefix> -n <blob_name>
```

## Opinions

Please note that this tool is still in its initial development phase and currently has some strong opinions about how it does certain things.

- Blobs are downloaded into "blobs" directory next to the installation directory and stored there. If you have sensitive data that shouldn't be stored in this way in the device please make sure to clean them up manually or using `clean` subcommand.
- Currently there is no directory hierarchy for downloaded blobs, all blobs opened using `azb` will be created in the "blobs" directory

