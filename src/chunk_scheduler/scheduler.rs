use crate::{
    chunk_scheduler::channel::{CHUNK_CHANNEL, RESUMABLE_CHUNK_CHANNEL},
    utils::state::{get_all_upload_states, update_upload_state},
    video_upload::video_struct::{ChunkType, ProcessingStatus},
};

// It loops over and checks the status of all the chunks in the transfer, and if all are complete, it removes from memory and send it to channel.
pub async fn chunk_scheduler() {
    loop {
        let all_states = get_all_upload_states().await;
        log::info!("Chunk Scheduler >> Upload States Length: {:?}", all_states.len());
        log::info!("Chunk Scheduler >> Upload States: {:?}", all_states);
        for (transfer_id, chunk_details) in all_states {
            // Check the status of each chunk and update accordingly.
            // Completion is based on the number of *distinct* chunk indices
            // received, so duplicate/retried chunks can't falsely mark an
            // upload complete while a distinct chunk is still missing.
            let received = chunk_details.received_chunks.len() as u32;
            if chunk_details.status != ProcessingStatus::InProcess && received >= chunk_details.total_chunks {
                log::info!("All chunks received for transfer_id: {}. Processing the video.", transfer_id);
                // Send the video to a channel or start processing here.
                if chunk_details.chunk_type == ChunkType::Raw {
                    if let Err(err) = CHUNK_CHANNEL.0.send(chunk_details.clone()) {
                        log::error!("Chunk Scheduler - Raw Chunk - Failed to send chunk details to channel: {}, file:{}, line:{}", err, file!(), line!());
                    } else {
                        update_upload_state(&transfer_id, |cd| {
                            cd.status = ProcessingStatus::InProcess;
                        })
                        .await;
                        log::info!("Chunk Scheduler - Raw Chunk - Successfully sent chunk details for transfer_id: {} to channel.", transfer_id);
                    }
                } else if chunk_details.chunk_type == ChunkType::Resumable {
                    // Handle resumable chunk processing here
                    if let Err(err) = RESUMABLE_CHUNK_CHANNEL.0.send(chunk_details.clone()) {
                        log::error!("Chunk Scheduler - Resumable Chunk - Failed to send resumable chunk details to channel: {}, file:{}, line:{}", err, file!(), line!());
                    } else {
                        update_upload_state(&transfer_id, |cd| {
                            cd.status = ProcessingStatus::InProcess;
                        })
                        .await;
                        log::info!("Chunk Scheduler - Resumable Chunk - Successfully sent resumable chunk details for transfer_id: {} to channel.", transfer_id);
                    }
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}
