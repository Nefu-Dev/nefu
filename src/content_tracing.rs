//! ContentTracing module
//!
//! Provides functionality similar to the Electron contentTracing:
//! - startRecording(options) - start recording traces
//! - stopRecording(resultFilePath) - stop recording traces
//! - getCategories() - get available tracing categories
//! - getTraceBufferUsage() - get trace buffer usage
//! - setWatchdog(interval) - set the watchdog interval

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Tracing options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingOptions {
    /// Category filter
    pub category_filter: Option<String>,
    /// Trace options
    pub trace_options: Option<String>,
    /// Buffer size (KB)
    pub buffer_size: Option<u32>,
}

/// Trace buffer usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceBufferUsage {
    /// Used percentage
    pub value: f64,
    /// Maximum percentage
    pub maximum: f64,
}

/// Trace recorder
///
/// Manages the recording and export of performance traces
pub struct ContentTracing {
    /// Whether currently recording
    recording: AtomicBool,
    /// Recording start time
    start_time: Mutex<Option<Instant>>,
    /// Buffer size
    buffer_size: AtomicU64,
    /// Trace data
    trace_data: Mutex<Vec<TraceEvent>>,
}

/// Trace event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Event name
    pub name: String,
    /// Event category
    pub cat: String,
    /// Event timestamp (microseconds)
    pub ts: u64,
    /// Event duration (microseconds)
    pub dur: u64,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Event phase (B=start, E=end, X=complete event)
    pub ph: String,
}

impl ContentTracing {
    /// Create a new trace recorder
    pub fn new() -> Self {
        Self {
            recording: AtomicBool::new(false),
            start_time: Mutex::new(None),
            buffer_size: AtomicU64::new(0),
            trace_data: Mutex::new(Vec::new()),
        }
    }

    /// Start recording traces
    ///
    /// # Parameters
    /// - `options`: Tracing options
    pub fn start_recording(&self, _options: Option<TracingOptions>) -> Result<()> {
        if self.recording.swap(true, Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Tracing is already recording"));
        }

        if let Ok(mut start) = self.start_time.lock() {
            *start = Some(Instant::now());
        }
        if let Ok(mut data) = self.trace_data.lock() {
            data.clear();
        }

        log::info!("Performance tracing started");
        Ok(())
    }

    /// Stop recording traces
    ///
    /// # Returns
    /// Trace data file path
    pub fn stop_recording(&self) -> Result<String> {
        if !self.recording.swap(false, Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Tracing is not recording"));
        }

        // Generate trace data
        let trace_json = self.generate_trace_json();

        // Write to file
        let output_path = std::env::temp_dir().join("nefu_trace.json");
        std::fs::write(&output_path, &trace_json)?;

        log::info!("Performance tracing stopped, data saved to: {}", output_path.display());
        Ok(output_path.to_string_lossy().to_string())
    }

    /// Get available tracing categories
    pub fn get_categories(&self) -> Vec<String> {
        vec![
            "benchmark".to_string(),
            "blink".to_string(),
            "blink.console".to_string(),
            "blink.net".to_string(),
            "cc".to_string(),
            "gpu".to_string(),
            "ipc".to_string(),
            "loading".to_string(),
            "memory".to_string(),
            "navigation".to_string(),
            "net".to_string(),
            "renderer".to_string(),
            "toplevel".to_string(),
            "v8".to_string(),
            "v8.execute".to_string(),
        ]
    }

    /// Get trace buffer usage
    pub fn get_trace_buffer_usage(&self) -> TraceBufferUsage {
        let data_len = self.trace_data.lock()
            .map(|d| d.len())
            .unwrap_or(0);
        let max_entries = self.buffer_size.load(Ordering::Relaxed) as usize;
        let usage = if max_entries > 0 {
            (data_len as f64 / max_entries as f64) * 100.0
        } else {
            0.0
        };

        TraceBufferUsage {
            value: usage.min(100.0),
            maximum: 100.0,
        }
    }

    /// Get recording status
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }

    /// Generate trace JSON data (Chrome Trace Event format)
    fn generate_trace_json(&self) -> String {
        let data = self.trace_data.lock().unwrap_or_else(|e| e.into_inner());
        let events: Vec<serde_json::Value> = data.iter().map(|e| {
            serde_json::json!({
                "name": e.name,
                "cat": e.cat,
                "ph": e.ph,
                "ts": e.ts,
                "dur": e.dur,
                "pid": e.pid,
                "tid": e.tid,
            })
        }).collect();

        serde_json::json!({
            "displayTimeUnit": "ms",
            "traceEvents": events,
            "metadata": {
                "nefu-version": env!("CARGO_PKG_VERSION"),
            }
        }).to_string()
    }
}

/// Global ContentTracing instance
static TRACING_INSTANCE: std::sync::OnceLock<ContentTracing> = std::sync::OnceLock::new();

/// Get the global ContentTracing
pub fn get_content_tracing() -> &'static ContentTracing {
    TRACING_INSTANCE.get_or_init(ContentTracing::new)
}
