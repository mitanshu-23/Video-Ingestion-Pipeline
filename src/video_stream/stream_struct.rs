#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoStreamRequest {
    pub file_name: String,
    pub upload_type: String,
}

// OLD APPROACH — the JSON response envelope. No longer used: the stream
// endpoint now returns the file's raw bytes as the HTTP body instead of
// wrapping them in JSON (serializing `Vec<u8>` as a `[..]` number array was
// ~3-4x on the wire and slow to build/parse). Kept commented for reference.
//
// #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
// #[serde(rename_all = "camelCase")]
// pub struct VideoStreamResponse {
//     pub file_name: String,
//     pub data: Vec<u8>,
// }

pub struct HHTPRangeReqInitResponse {
    pub content_length: u64,
    pub content_type: String,
}
