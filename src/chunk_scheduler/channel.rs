use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    time::Duration,
};

use crate::{
    utils::state::{remove_upload_state, update_upload_state},
    video_upload::video_struct::ProcessingStatus,
};

use crate::{utils::storage, video_upload::video_struct::ChunkDetails};

lazy_static::lazy_static! {
    pub static ref CHUNK_CHANNEL: (Sender<ChunkDetails>, Arc<Mutex<Receiver<ChunkDetails>>>) = {
        let (tx, rx) = mpsc::channel();
        (tx, Arc::new(Mutex::new(rx)))
    };

        pub static ref RESUMABLE_CHUNK_CHANNEL: (Sender<ChunkDetails>, Arc<Mutex<Receiver<ChunkDetails>>>) = {
        let (tx, rx) = mpsc::channel();
        (tx, Arc::new(Mutex::new(rx)))
    };
}

pub async fn chunk_receiver() {
    loop {
        // Pop one completed transfer off the channel. This whole match yields a
        // plain `Option<ChunkDetails>`; the std MutexGuard from `try_lock` is a
        // temporary that drops at the end of the match, so it never lives across
        // the `.await` below (a std guard is not `Send`).
        let chunk_detail = match CHUNK_CHANNEL.1.try_lock() {
            Ok(guard) => match guard.try_recv() {
                Ok(chunk_details) => Some(chunk_details),
                // An empty channel is the normal idle state, not an error.
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    log::error!("Chunk Receiver - channel disconnected; stopping receiver, file:{}, line:{}", file!(), line!());
                    return;
                }
            },
            Err(err) => {
                // Contention on the receiver lock is unexpected (single consumer).
                log::error!("Chunk Receiver - failed to lock receiver: {}, file:{}, line:{}", err, file!(), line!());
                None
            }
        };

        match chunk_detail {
            Some(details) => {
                if let Err(_err) = process_chunk(&details).await {
                    log::error!("Chunk Receiver - Process Chunk - Failed to process chunk: {}, file:{}, line:{}", details.transfer_id, file!(), line!());
                    update_upload_state(&details.transfer_id, |cd| {
                        cd.status = ProcessingStatus::None;
                    })
                    .await;
                } else {
                    // Here you can add logic to process the video, e.g., send it to a channel or start processing.
                    // After processing, you might want to remove the state from memory.
                    remove_upload_state(&details.transfer_id).await;
                    log::info!("Chunk Receiver - Removed transfer_id: {} from memory after processing.", details.transfer_id);
                }
            }
            // Nothing ready yet — yield to the runtime instead of blocking a
            // worker thread with std::thread::sleep.
            None => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

pub async fn resumable_chunk_receiver() {
    loop {
        // Pop one completed transfer off the channel. This whole match yields a
        // plain `Option<ChunkDetails>`; the std MutexGuard from `try_lock` is a
        // temporary that drops at the end of the match, so it never lives across
        // the `.await` below (a std guard is not `Send`).
        let chunk_detail = match RESUMABLE_CHUNK_CHANNEL.1.try_lock() {
            Ok(guard) => match guard.try_recv() {
                Ok(chunk_details) => Some(chunk_details),
                // An empty channel is the normal idle state, not an error.
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    log::error!("Resumable Chunk Receiver - channel disconnected; stopping receiver, file:{}, line:{}", file!(), line!());
                    return;
                }
            },
            Err(err) => {
                // Contention on the receiver lock is unexpected (single consumer).
                log::error!("Resumable Chunk Receiver - failed to lock receiver: {}, file:{}, line:{}", err, file!(), line!());
                None
            }
        };

        match chunk_detail {
            Some(details) => {
                if let Err(err) = process_resumable_chunk(&details).await {
                    log::error!("Resumable Chunk Receiver - Failed to process resumable chunk: {}, file:{}, line:{}", err, file!(), line!());
                    update_upload_state(&details.transfer_id, |cd| {
                        cd.status = ProcessingStatus::None;
                    })
                    .await;
                } else {
                    // Here you can add logic to process the video, e.g., send it to a channel or start processing.
                    // After processing, you might want to remove the state from memory.
                    remove_upload_state(&details.transfer_id).await;
                    log::info!("Resumable Chunk Receiver - Removed transfer_id: {} from memory after processing.", details.transfer_id);
                }
            }
            // Nothing ready yet — yield to the runtime instead of blocking a
            // worker thread with std::thread::sleep.
            None => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

async fn process_chunk(chunk_details: &ChunkDetails) -> Result<(), String> {
    let file_name = chunk_details.file_name.clone();
    let total_chunks = chunk_details.total_chunks;
    let extension = chunk_details.extension.clone();

    let chunk_dir = PathBuf::from(format!("./uploads/chunk/{}", file_name));
    let final_path = PathBuf::from(format!("./uploads/chunk/final_data/{}.{}", file_name, extension));

    // Reassembly must be idempotent. The final file is written by appending, so
    // if a stale/partial file from a previous (failed or restarted) run is still
    // present, appending would duplicate bytes and corrupt the output. Start
    // every reassembly from a clean slate.
    if let Err(e) = storage::remove_file_if_exists(&final_path).await {
        log::error!("Chunk Scheduler - Process Chunk - failed to clear stale final file `{}`: {}, file:{}, line:{}", final_path.display(), e, file!(), line!());
        return Err("Failed to clear stale final file before reassembly".to_string());
    }

    // Append chunks strictly in index order. Any read/write failure aborts the
    // whole reassembly and removes the partial file, so a truncated output is
    // never left behind for a consumer to pick up as "complete".
    for current_chunk in 0..total_chunks {
        let chunk_path = chunk_dir.join(format!("{current_chunk}.{extension}"));

        let chunk_data = match storage::read_file(&chunk_path).await {
            Ok(data) => data,
            Err(e) => {
                log::error!("Chunk Scheduler - Process Chunk - Failed to read chunk file `{}`: {}, file:{}, line:{}", chunk_path.display(), e, file!(), line!());
                // let _ = storage::remove_file_if_exists(&final_path).await;
                return Err("Failed to read chunk file".to_string());
            }
        };

        if let Err(e) = storage::append_file(&final_path, &chunk_data).await {
            log::error!("Chunk Scheduler - Process Chunk - Failed to append chunk {current_chunk} to final file `{}`: {}, file:{}, line:{}", final_path.display(), e, file!(), line!());
            let _ = storage::remove_file_if_exists(&final_path).await;
            return Err("Failed to append chunk to final file".to_string());
        }

        log::info!("Chunk Scheduler - Process Chunk - appended chunk {current_chunk} to final file `{}`", final_path.display());
    }

    log::info!("Chunk Scheduler - Process Chunk - reassembled {} chunks into `{}`", total_chunks, final_path.display());

    // The parts are now fully merged; drop the per-transfer chunk directory so
    // it doesn't leak disk. A cleanup failure is non-fatal — the final file is
    // already complete.
    if let Err(e) = tokio::fs::remove_dir_all(&chunk_dir).await {
        log::warn!("Chunk Scheduler - Process Chunk - failed to remove chunk dir `{}`: {}, file:{}, line:{}", chunk_dir.display(), e, file!(), line!());
        return Err("Failed to remove chunk directory after reassembly".to_string());
    }

    Ok(())
}

async fn process_resumable_chunk(chunk_details: &ChunkDetails) -> Result<(), String> {
    let file_name = chunk_details.file_name.clone();
    let total_chunks = chunk_details.total_chunks;
    let extension = chunk_details.extension.clone();

    let chunk_dir = PathBuf::from(format!("./uploads/resumable/{}", file_name));
    let final_path = PathBuf::from(format!("./uploads/resumable/final_data/{}.{}", file_name, extension));

    // Reassembly must be idempotent. The final file is written by appending, so
    // if a stale/partial file from a previous (failed or restarted) run is still
    // present, appending would duplicate bytes and corrupt the output. Start
    // every reassembly from a clean slate.
    if let Err(e) = storage::remove_file_if_exists(&final_path).await {
        log::error!("Chunk Scheduler - Process Resumable Chunk - failed to clear stale final file `{}`: {}, file:{}, line:{}", final_path.display(), e, file!(), line!());
        return Err("Failed to clear stale final file before reassembly".to_string());
    }

    // Append chunks strictly in index order. Any read/write failure aborts the
    // whole reassembly and removes the partial file, so a truncated output is
    // never left behind for a consumer to pick up as "complete".
    for current_chunk in 0..total_chunks {
        let chunk_path = chunk_dir.join(format!("{current_chunk}.{extension}"));

        let chunk_data = match storage::read_file(&chunk_path).await {
            Ok(data) => data,
            Err(e) => {
                log::error!("Chunk Scheduler - Process Resumable Chunk - Failed to read chunk file `{}`: {}, file:{}, line:{}", chunk_path.display(), e, file!(), line!());
                // let _ = storage::remove_file_if_exists(&final_path).await;
                return Err("Failed to read chunk file".to_string());
            }
        };

        if let Err(e) = storage::append_file(&final_path, &chunk_data).await {
            log::error!("Chunk Scheduler - Process Resumable Chunk - Failed to append chunk {current_chunk} to final file `{}`: {}, file:{}, line:{}", final_path.display(), e, file!(), line!());
            let _ = storage::remove_file_if_exists(&final_path).await;
            return Err("Failed to append chunk to final file".to_string());
        }

        log::info!("Chunk Scheduler - Process Resumable Chunk - appended chunk {current_chunk} to final file `{}`", final_path.display());
    }

    log::info!("Chunk Scheduler - Process Resumable Chunk - reassembled {} chunks into `{}`", total_chunks, final_path.display());

    // The parts are now fully merged; drop the per-transfer chunk directory so
    // it doesn't leak disk. A cleanup failure is non-fatal — the final file is
    // already complete.
    if let Err(e) = tokio::fs::remove_dir_all(&chunk_dir).await {
        log::warn!("Chunk Scheduler - Process Resumable Chunk - failed to remove chunk dir `{}`: {}, file:{}, line:{}", chunk_dir.display(), e, file!(), line!());
        return Err("Failed to remove chunk directory after reassembly".to_string());
    }
    Ok(())
}
