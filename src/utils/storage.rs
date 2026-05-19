use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

/// Writes bytes to disk, creating the parent directory if it does not exist.
pub async fn write_file(path: &PathBuf, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(data).await?;
    file.flush().await
}