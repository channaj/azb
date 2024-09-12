# azb
A cli tool to download files from Azure Storage Blobs and open them using the default program.

Uses default azure credentiols to access Azure Storage

## Usage


### Examples


```
// Open the latest blob in the container under the specified prefix
azb -s <storage_acount_name> -c <container_name> <prefix>
```

```
// List blobs upder a prefix
azb -s <storage_account_name> -c <container_name> <prefix> --list
```

```
// Open a blob under a prefix
azb -s <storage_account_name> -c <container_name> <prefix> -n <file_name>
```

## Opinions

Please note that this tool is still in its initial development phase and currently has some strong opinions about how it does certain things.

- Blobs are downloaded into "blobs" directory next to the installation directory and stored there. If you have sensitive data that shouldn't be stored in this way in the device please make sure to clean them up manually (`--clean` option will be included in the next version to make this easier).


