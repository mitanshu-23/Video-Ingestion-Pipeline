# axum-video-upload

A Rust/Axum server implementing multiple video upload strategies — from a basic multipart endpoint to resumable session-based uploads and pre-signed direct-to-storage flows.

Built as the ingestion layer for a video processing pipeline.

---

## What this does

This server exposes HTTP endpoints for uploading video files. Each route implements a different upload strategy, increasing in reliability and scalability:

| Route | Strategy | Notes |
|-------|----------|-------|
| `POST /video/basic/upload/v1` | Raw body | Full file in request body |
| `POST /video/multipart/upload/v1` | Multipart form | File + metadata in one request |
| `POST /video/stream/upload/v1` | Streaming body | Chunked transfer, low memory usage |
| `POST /video/stream-multipart/upload/v1` | Streaming multipart | Streamed multipart with metadata |
| `POST /video/chunked/upload/v1` | Chunked | Split file, multiple requests |
| `POST /video/resumable/upload/v1` | Resumable (TUS) | Session-based, survives interruptions |
| `POST /video/presign/upload/v1` | Pre-signed URL | Client uploads directly to storage |

---

## Upload Strategies

### Raw Body
Single `POST` with the video as raw bytes. No metadata. Useful for internal tooling and scripted uploads.

### Multipart
Standard `multipart/form-data` — carries the file alongside fields like title and description. What browsers submit natively.

### Streaming
Request body is read as a stream and written incrementally. Memory usage stays flat regardless of file size.

### Streaming Multipart
Multipart fields parsed and written as a stream. Same memory profile as streaming, retains the metadata support of multipart.

### Chunked
Client splits the file and sends each piece separately. Failed chunks can be retried without restarting the upload.

### Resumable
Session-based chunked upload compatible with the [TUS protocol](https://tus.io/). Upload state is persisted — interrupted uploads can be resumed from where they left off.

```
POST   /video/resumable/upload/v1              → create session
HEAD   /video/resumable/upload/v1/{id}         → query received bytes
PATCH  /video/resumable/upload/v1/{id}         → upload next chunk
DELETE /video/resumable/upload/v1/{id}         → abort
```

### Direct-to-Storage (Pre-signed URL)
Server generates a pre-signed URL; client uploads directly to object storage (S3 / GCS / MinIO). The server never handles the video bytes — only the metadata and completion event.

---

## Running

```bash
cargo run
# http://localhost:3000
```

---

## License

MIT