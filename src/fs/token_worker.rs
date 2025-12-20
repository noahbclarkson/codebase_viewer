//! Background worker for token counting and content caching.

use crate::{llm::token_counter::count_tokens_local, task::TaskMessage};
use crossbeam_channel::Sender;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

pub struct TokenWorkItem {
    pub id: usize,
    pub path: PathBuf,
    pub is_binary: bool,
}

pub fn start_token_calculation(
    job_id: u64,
    files: Vec<TokenWorkItem>,
    max_cache_size: u64,
    max_count_size: u64,
    sender: Sender<TaskMessage>,
    cancel_flag: Arc<AtomicBool>,
) {
    let thread_name = format!("token_worker_{job_id}");
    let result = thread::Builder::new().name(thread_name).spawn(move || {
        for file in files {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if file.is_binary {
                continue;
            }
            if max_count_size == 0 {
                continue;
            }

            let file_size = match fs::metadata(&file.path) {
                Ok(metadata) => metadata.len(),
                Err(_) => continue,
            };
            if file_size > max_count_size {
                continue;
            }

            let bytes = match fs::read(&file.path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let cache_allowed = max_cache_size > 0 && file_size <= max_cache_size;
            let content = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(err) => String::from_utf8_lossy(&err.into_bytes()).to_string(),
            };

            let count = count_tokens_local(&content);
            let cached_content = if cache_allowed { Some(content) } else { None };

            let _ = sender.send(TaskMessage::TokenUpdate {
                job_id,
                id: file.id,
                count,
                content: cached_content,
            });
        }

        let _ = sender.send(TaskMessage::TokenCalculationFinished { job_id });
    });
    if let Err(err) = result {
        log::error!("Failed to spawn token worker thread: {err}");
    }
}
