# azb
A cli tool to download files from Azure Storage Blobs and open them using the default program.

Uses default azure credentiols to access Azure Storage

# Usage examples

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

