//! Multithreading module
//!
//! Provides lightweight parallel processing utilities for optimizing file I/O and compute-intensive tasks.
//! Avoids heavy dependencies (such as rayon) and uses the standard library.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

/// Parallel processing result
pub struct ParallelResult<T> {
    pub results: Vec<T>,
    pub duration_ms: u128,
}

/// Parallel map processing
///
/// Splits an input list into multiple chunks, each processed by one thread.
///
/// # Parameters
/// - `items`: List of input items
/// - `num_threads`: Number of threads (defaults to the CPU core count)
/// - `processor`: Processing function, receives one input item and returns one result
///
/// # Returns
/// The collection of all results (order may differ from the input)
pub fn parallel_map<T, F, R>(items: Vec<T>, num_threads: Option<usize>, processor: F) -> ParallelResult<R>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T) -> R + Send + Sync + 'static,
    R: Send + 'static,
{
    let start = Instant::now();
    let processor = Arc::new(processor);
    let items = Arc::new(items);
    let results = Arc::new(Mutex::new(Vec::new()));
    
    let thread_count = num_threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }).min(items.len().max(1));
    
    if thread_count <= 1 || items.is_empty() {
        // Single-threaded or empty list, process sequentially
        let mut local_results = Vec::new();
        for item in items.iter() {
            local_results.push(processor(item.clone()));
        }
        return ParallelResult {
            results: local_results,
            duration_ms: start.elapsed().as_millis(),
        };
    }
    
    // Split the tasks
    let chunk_size = (items.len() + thread_count - 1) / thread_count;
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let start_idx = thread_id * chunk_size;
        let end_idx = (start_idx + chunk_size).min(items.len());
        
        if start_idx >= items.len() {
            break;
        }
        
        let items_ref = Arc::clone(&items);
        let processor_ref = Arc::clone(&processor);
        let results_ref = Arc::clone(&results);
        
        let handle = thread::spawn(move || {
            for i in start_idx..end_idx {
                let item = &items_ref[i];
                let result = processor_ref(item.clone());
                if let Ok(mut results_vec) = results_ref.lock() {
                    results_vec.push(result);
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to finish
    for handle in handles {
        let _ = handle.join();
    }
    
    let final_results = Arc::try_unwrap(results)
        .ok()
        .and_then(|mutex| mutex.into_inner().ok())
        .unwrap_or_default();
    
    ParallelResult {
        results: final_results,
        duration_ms: start.elapsed().as_millis(),
    }
}

/// Compute file hashes in parallel
///
/// Uses multiple threads to compute the SHA-256 hash of multiple files simultaneously.
///
/// # Parameters
/// - `file_paths`: List of file paths
///
/// # Returns
/// A mapping of file paths to hash values
pub fn parallel_file_hash(file_paths: Vec<std::path::PathBuf>) -> std::collections::HashMap<String, String> {
    use sha2::Digest;
    
    let result = parallel_map(file_paths, None, |path| {
        let hash = if let Ok(content) = std::fs::read(&path) {
            let mut hasher = sha2::Sha256::new();
            hasher.update(&content);
            format!("{:x}", hasher.finalize())
        } else {
            String::new()
        };
        (path.to_string_lossy().to_string(), hash)
    });
    
    result.results.into_iter().collect()
}

/// Batch processing strategy
pub struct BatchConfig {
    /// Number of files processed per batch
    pub batch_size: usize,
    /// Delay between batches (milliseconds), used to avoid CPU overload
    pub throttle_ms: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            throttle_ms: 10,
        }
    }
}

/// Process a file list in batches
///
/// For a large number of files, batch processing provides better progress feedback and memory management.
///
/// # Parameters
/// - `items`: List of input items
/// - `config`: Batch configuration
/// - `handler`: Processing function
/// - `progress_callback`: Progress callback (current batch number, total batch count)
pub fn batch_process<T, F>(
    items: Vec<T>,
    config: &BatchConfig,
    handler: F,
    progress_callback: Option<&dyn Fn(usize, usize)>,
) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> T,
{
    let total_batches = (items.len() + config.batch_size - 1) / config.batch_size;
    let mut results = Vec::with_capacity(items.len());
    
    for (batch_idx, chunk) in items.chunks(config.batch_size).enumerate() {
        let batch_result: Vec<T> = chunk.iter().map(|item| handler(item)).collect();
        results.extend(batch_result);
        
        if let Some(cb) = progress_callback {
            cb(batch_idx + 1, total_batches);
        }
        
        if config.throttle_ms > 0 && batch_idx < total_batches - 1 {
            std::thread::sleep(std::time::Duration::from_millis(config.throttle_ms));
        }
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parallel_map_single_thread() {
        let items = vec![1, 2, 3, 4, 5];
        let result = parallel_map(items, Some(1), |x| x * 2);
        
        let mut sorted_results = result.results;
        sorted_results.sort();
        assert_eq!(sorted_results, vec![2, 4, 6, 8, 10]);
    }
    
    #[test]
    fn test_parallel_map_multi_thread() {
        let items: Vec<i32> = (0..100).collect();
        let result = parallel_map(items, Some(4), |x| x * 3);
        
        assert_eq!(result.results.len(), 100);
        for (i, &val) in result.results.iter().enumerate() {
            // Verify the results; since the order is nondeterministic, only verify the correctness of the values
            assert!(val % 3 == 0);
            assert!(val >= 0 && val <= 297);
        }
    }
    
    #[test]
    fn test_parallel_map_empty() {
        let items: Vec<i32> = vec![];
        let result = parallel_map(items, None, |x| x * 2);
        assert!(result.results.is_empty());
    }
    
    #[test]
    fn test_batch_process() {
        let items: Vec<i32> = (0..50).collect();
        let config = BatchConfig {
            batch_size: 10,
            throttle_ms: 0,
        };
        
        let results = batch_process(items, &config, |x| x + 1, None);
        assert_eq!(results.len(), 50);
        assert_eq!(results[0], 1);
        assert_eq!(results[49], 50);
    }
}
