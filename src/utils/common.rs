use std::{io::Read, path::PathBuf};
use uuid::Uuid;

/// A tiny ID generator — swap this out for uuid::Uuid::new_v4() if you add
/// the uuid crate later. For now it uses timestamp + random suffix via std.
pub fn new_mime_id(_name: Option<&str>) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    // xor-shift for the random part — no extra deps needed
    let mut x = ts as u64 ^ 0xdeadbeefcafe;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    format!("{ts:016x}{x:016x}") // example: "0000017f3b2c4d5e00000000deadbeef"
}

/// Maps common MIME types to file extensions.
/// Add entries as your allowed-list grows.
pub fn mime_to_extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "application/pdf" => ".pdf",
        "video/mp4" => ".mp4",
        "video/quicktime" => ".mov",
        _ => ".bin",
    }
}

pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Maps a stored file's extension back to a MIME type, so a streamed file can
/// be served with the right `Content-Type` and the browser renders it inline
/// (image/video) instead of downloading it. This is the reverse of
/// `mime_to_extension` — keep the two in sync as the allowed list grows.
/// Falls back to `application/octet-stream` for anything unrecognized.
pub fn content_type_from_filename(file_name: &str) -> &'static str {
    let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

pub fn is_video_file(path: &PathBuf) -> Result<bool, String> {
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(err) => {
            log::error!("Common - is_video_file - Error: {:?} opening file at path: {:?}", err, path);
            return Err("Error opening file".to_string());
        }
    };

    // We only need the first few bytes (typically 128 bytes is plenty for magic numbers)
    let mut buffer = [0; 128];
    let bytes_read = match file.read(&mut buffer) {
        Ok(bytes) => bytes,
        Err(err) => {
            log::error!("Common - is_video_file - Error: {:?} reading bytes for file at path: {:?}", err, path);
            return Err("Error reading file bytes into buffer".to_string());
        }
    };

    if let Some(kind) = infer::get(&buffer[..bytes_read]) { Ok(kind.mime_type().starts_with("video/")) } else { Ok(false) }
}

pub fn extension_to_header(extension: &String) -> &'static str {
    match extension.to_lowercase().as_str() {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "html" => "text/html",
        _ => "application/octet-stream", // Default fallback for unknown files
    }
}
