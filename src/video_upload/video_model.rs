use std::path::PathBuf;

use axum::body::Bytes;
use serde_json::{Value, json};

use crate::{
    utils::{
        common::{mime_to_extension, new_mime_id},
        storage::write_file,
    },
    video_upload::video_struct::MultipartReq,
};

const ALLOWED_MIME_TYPES: &[&str] = &[
    // images
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    // documents
    "application/pdf",
    // video
    "video/mp4",
    "video/quicktime", // .mov
];

pub async fn video_upload_basic_http_body_handler(bytes: Bytes, content_type: String) -> Result<axum::Json<Value>, String> {
    // 1. Validate MIME
    let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("upload rejected: unsupported MIME type `{}`", mime_base);
        return Err(format!("unsupported file type: {mime_base}"));
    }

    if bytes.is_empty() {
        log::error!("upload rejected: empty body");
        return Err("empty body".to_string());
    }

    // 2. Generate filename + write to disk
    let extension = mime_to_extension(&mime_base);
    let filename = format!("{}{}", new_mime_id(None), extension);
    let dest = PathBuf::from("./uploads/basic").join(&filename);

    let write_file_res = write_file(&dest, &bytes).await.map_err(|e| {
        log::error!("disk write failed for `{}`: {}", filename, e);
        "could not save file".to_string()
    });

    if let Err(err) = write_file_res {
        log::error!("Error writing file using basic HTTP body handler `{}`: {}", filename, err);
        return Err(err);
    }

    log::info!("upload saved | file={} mime={} size={}B", filename, mime_base, bytes.len());

    let full_path = match tokio::fs::canonicalize(&dest).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("failed to resolve full path for `{}`: {}", filename, e);
            return Err("could not resolve file path".to_string());
        }
    };

    let res = axum::Json(json!(
        { "filePath": full_path }
    ));

    Ok(res)
}

pub async fn video_upload_multipart_handler(req: MultipartReq) -> Result<axum::Json<Value>, String> {
    let mime_base = req.file_content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Multipart Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err(format!("unsupported file type: {mime_base}"));
    }

    if req.file.is_empty() {
        log::error!("Multipart Handler >> upload rejected: empty file, file:{}, line:{}", file!(), line!());
        return Err("empty file".to_string());
    }

    // 2. Generate filename + write to disk
    let extension = mime_to_extension(&mime_base);
    let filename = format!("{}{}", new_mime_id(None), extension);
    let dest = PathBuf::from("./uploads/multipart").join(&filename);

    let write_file_res = write_file(&dest, &req.file).await.map_err(|e| {
        log::error!("Multipart Handler >> disk write failed for `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
        "could not save file".to_string()
    });

    if let Err(err) = write_file_res {
        log::error!("Multipart Handler >> Error writing file using multipart handler `{}`: {}, file:{}, line:{}", filename, err, file!(), line!());
        return Err(err);
    }

    log::info!("Multipart Handler >> upload saved | file={} mime={} size={}B", filename, mime_base, req.file.len());

    let full_path = match tokio::fs::canonicalize(&dest).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("Multipart Handler >> failed to resolve full path for `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
            return Err("could not resolve file path".to_string());
        }
    };

    let res = axum::Json(json!(
        { "filePath": full_path }
    ));

    Ok(res)
}
