use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

pub async fn read_file(file_path: &PathBuf) -> Result<Vec<u8>, std::io::Error> {
    match tokio::fs::read(file_path).await {
        Ok(data) => Ok(data),
        Err(e) => {
            log::error!("Storage - Read File - Failed to read file `{}`: {}, file:{}, line:{}", file_path.display(), e, file!(), line!());
            Err(e)
        }
    }
}

/// Writes bytes to disk, creating the parent directory if it does not exist.
pub async fn write_file(path: &PathBuf, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(data).await?;
    file.flush().await
}

/// Appends bytes to a file, creating it (and the parent directory) if it does
/// not yet exist. Used for chunked uploads where each chunk is added to the end
/// of the same file.
pub async fn append_file(path: &PathBuf, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open(path).await?;
    file.write_all(data).await?;
    file.flush().await
}

/// Removes a file if it exists. A missing file is treated as success, so
/// callers can use this to guarantee a clean slate before (re)writing without
/// racing on a prior existence check.
pub async fn remove_file_if_exists(path: &PathBuf) -> std::io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            log::error!("Storage - Remove File - Failed to remove file `{}`: {}, file:{}, line:{}", path.display(), e, file!(), line!());
            Err(e)
        }
    }
}

pub async fn create_file(destination: &str) -> Result<tokio::fs::File, String> {
    // Ensure the parent directory exists before creating the file, so callers
    // streaming into a fresh upload subdir (e.g. ./uploads/multipart_stream)
    // don't fail on a missing directory.
    if let Some(parent) = std::path::Path::new(destination).parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            log::error!("create_file >> failed to create parent dir for `{}`: {}, file:{}, line:{}", destination, e, file!(), line!());
            return Err("Error creating directory".to_string());
        }
    }

    // Create the file on disk to write the incoming stream to.
    match tokio::fs::File::create(&destination).await {
        Ok(f) => Ok(f),
        Err(e) => {
            log::error!("Multipart - stream_to_file >> Failed to create file `{}`: {}, file:{}, line:{}", destination, e, file!(), line!());
            return Err("Error creating file".to_string());
        }
    }
}
