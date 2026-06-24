use axum::extract::Multipart;

// The exact shape we expect from a `multipart/form-data` body:
//   file: the video file to upload (binary part)
//   name: the name of the video (text part)
#[derive(Debug)]
pub struct MultipartReq {
    pub file: Vec<u8>,
    pub file_content_type: Option<String>,
    pub name: String,
}

impl MultipartReq {
    // Drive the multipart stream field-by-field and assemble the typed struct.
    // Unknown fields are rejected so the body must match the contract exactly.
    pub async fn from_multipart(mut multipart: Multipart) -> Result<Self, String> {
        let mut file: Option<Vec<u8>> = None;
        let mut file_content_type: Option<String> = None;
        let mut name: Option<String> = None;

        while let Some(field) = multipart.next_field().await.map_err(|e| format!("malformed multipart body: {e}"))? {
            // `field.name()` borrows, so capture what we need before consuming it.
            match field.name() {
                Some("file") => {
                    // content_type() must be read before bytes() consumes the field.
                    file_content_type = field.content_type().map(|s| s.to_string());
                    name = field.file_name().map(|s| s.to_string());
                    let bytes = field.bytes().await.map_err(|e| format!("failed to read `file` field: {e}"))?;
                    file = Some(bytes.to_vec());
                }
                Some(other) => {
                    log::info!("multipart field `{other}` is not expected; ignoring");
                },
                None => return Err("multipart field is missing a name".to_string()),
            }
        }

        Ok(MultipartReq { file: file.ok_or("missing required field `file`")?, file_content_type, name: name.ok_or("missing required field `name`")? })
    }
}
