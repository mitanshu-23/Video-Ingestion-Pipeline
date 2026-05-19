/// A tiny ID generator — swap this out for uuid::Uuid::new_v4() if you add
/// the uuid crate later. For now it uses timestamp + random suffix via std.
pub fn new_mime_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    // xor-shift for the random part — no extra deps needed
    let mut x = ts as u64 ^ 0xdeadbeefcafe;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    format!("{ts:016x}{x:016x}")
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