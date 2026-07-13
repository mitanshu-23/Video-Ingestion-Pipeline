use crate::video_upload::video_struct::ChunkDetails;
use std::collections::HashMap;
use tokio::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref UPLOAD_STATE: Mutex<HashMap<String, ChunkDetails>> = Mutex::new(HashMap::new());
}

pub async fn set_upload_state(transfer_id: String, details: ChunkDetails) {
    let mut upload_state = UPLOAD_STATE.lock().await;
    upload_state.insert(transfer_id, details);
}

pub async fn get_upload_state(transfer_id: &str) -> Option<ChunkDetails> {
    let upload_state = UPLOAD_STATE.lock().await;
    upload_state.get(transfer_id).cloned()
}

/// Atomically read-modify-write a single transfer's state under one lock hold,
/// so concurrent chunks for the same upload can't clobber each other's counter
/// update. Returns `None` if the transfer_id is unknown, otherwise the value
/// the closure produced.
pub async fn update_upload_state<F, R>(transfer_id: &str, f: F) -> Option<R>
where
    F: FnOnce(&mut ChunkDetails) -> R,
{
    let mut upload_state = UPLOAD_STATE.lock().await;
    upload_state.get_mut(transfer_id).map(f)
}

pub async fn remove_upload_state(transfer_id: &str) {
    let mut upload_state = UPLOAD_STATE.lock().await;
    upload_state.remove(transfer_id);
}

pub async fn get_all_upload_states() -> HashMap<String, ChunkDetails> {
    let upload_state = UPLOAD_STATE.lock().await;
    upload_state.clone()
}
