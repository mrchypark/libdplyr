//! Signal handling for cross-platform pipeline integration
//!
//! This module provides signal handling capabilities for proper integration
//! with Unix pipelines, including SIGINT, SIGTERM, and SIGPIPE handling.
//! On Windows, it provides basic interrupt handling.

#[cfg(unix)]
use signal_hook::{consts::*, iterator::Signals};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Signal handler for managing Unix signals
#[derive(Debug)]
pub struct SignalHandler {
    /// Atomic flag indicating if shutdown was requested
    shutdown_requested: Arc<AtomicBool>,
    /// Atomic flag indicating if SIGPIPE was received
    sigpipe_received: Arc<AtomicBool>,
    /// Handle to the signal monitoring thread
    _signal_thread: Option<thread::JoinHandle<()>>,
}

impl SignalHandler {
    /// Creates a new signal handler and starts monitoring signals
    pub fn new() -> Result<Self, SignalError> {
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let sigpipe_received = Arc::new(AtomicBool::new(false));

        let shutdown_clone = shutdown_requested.clone();
        let sigpipe_clone = sigpipe_received.clone();

        // Set up signal monitoring
        let signal_thread = thread::spawn(move || {
            if let Err(e) = Self::monitor_signals(shutdown_clone, sigpipe_clone) {
                eprintln!("Signal monitoring error: {e}");
            }
        });

        Ok(Self {
            shutdown_requested,
            sigpipe_received,
            _signal_thread: Some(signal_thread),
        })
    }

    /// Monitor signals in a separate thread (Unix implementation)
    #[cfg(unix)]
    fn monitor_signals(
        shutdown_flag: Arc<AtomicBool>,
        sigpipe_flag: Arc<AtomicBool>,
    ) -> Result<(), SignalError> {
        let mut signals = Signals::new([SIGINT, SIGTERM, SIGPIPE])
            .map_err(|e| SignalError::SetupError(format!("Failed to setup signal handler: {e}")))?;

        for signal in signals.forever() {
            match signal {
                SIGINT | SIGTERM => {
                    eprintln!("Received termination signal, shutting down gracefully...");
                    shutdown_flag.store(true, Ordering::Relaxed);
                    break;
                }
                SIGPIPE => {
                    // SIGPIPE indicates the output pipe was closed
                    // This is normal in Unix pipelines and should be handled gracefully
                    sigpipe_flag.store(true, Ordering::Relaxed);
                    shutdown_flag.store(true, Ordering::Relaxed);
                    break;
                }
                _ => {
                    // Ignore other signals
                }
            }
        }

        Ok(())
    }

    /// Monitor signals in a separate thread (Windows implementation)
    #[cfg(windows)]
    fn monitor_signals(
        shutdown_flag: Arc<AtomicBool>,
        _sigpipe_flag: Arc<AtomicBool>,
    ) -> Result<(), SignalError> {
        use std::sync::mpsc;
        use winapi::shared::minwindef::{BOOL, DWORD, FALSE, TRUE};
        use winapi::um::consoleapi::SetConsoleCtrlHandler;
        use winapi::um::wincon::{CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT};

        static mut SHUTDOWN_SENDER: Option<mpsc::Sender<()>> = None;

        let (tx, rx) = mpsc::channel();

        unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
            match ctrl_type {
                CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
                    eprintln!("Received interrupt signal, shutting down gracefully...");
                    if let Some(ref sender) = SHUTDOWN_SENDER {
                        let _ = sender.send(());
                    }
                    TRUE
                }
                _ => FALSE,
            }
        }

        unsafe {
            SHUTDOWN_SENDER = Some(tx);
            if SetConsoleCtrlHandler(Some(ctrl_handler), TRUE) == 0 {
                return Err(SignalError::SetupError(
                    "Failed to set console control handler".to_string(),
                ));
            }
        }

        // Wait for shutdown signal
        if rx.recv().is_ok() {
            shutdown_flag.store(true, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Monitor signals in a separate thread (fallback implementation)
    #[cfg(not(any(unix, windows)))]
    fn monitor_signals(
        _shutdown_flag: Arc<AtomicBool>,
        _sigpipe_flag: Arc<AtomicBool>,
    ) -> Result<(), SignalError> {
        // On unsupported platforms, just sleep indefinitely
        // The application will need to be terminated externally
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Check if shutdown was requested via signal
    pub fn should_shutdown(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    /// Check if SIGPIPE was received (pipe closed)
    pub fn pipe_closed(&self) -> bool {
        self.sigpipe_received.load(Ordering::Relaxed)
    }

    /// Reset the shutdown flag (useful for testing)
    pub fn reset(&self) {
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.sigpipe_received.store(false, Ordering::Relaxed);
    }

    /// Wait for a signal with timeout
    pub fn wait_for_signal(&self, timeout: Duration) -> SignalWaitResult {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.should_shutdown() {
                return if self.pipe_closed() {
                    SignalWaitResult::PipeClosed
                } else {
                    SignalWaitResult::Shutdown
                };
            }

            thread::sleep(Duration::from_millis(10));
        }

        SignalWaitResult::Timeout
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new().expect("Failed to create signal handler")
    }
}

/// Result of waiting for a signal
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalWaitResult {
    /// Shutdown signal received (SIGINT/SIGTERM)
    Shutdown,
    /// Pipe was closed (SIGPIPE)
    PipeClosed,
    /// Timeout occurred
    Timeout,
}

/// Errors that can occur during signal handling
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("Signal setup error: {0}")]
    SetupError(String),

    #[error("Signal monitoring error: {0}")]
    MonitoringError(String),
}

/// Utility functions for signal handling
pub mod utils {
    use super::*;

    /// Check if the current process is running in a pipeline
    pub fn is_in_pipeline() -> bool {
        use std::io::IsTerminal;

        // Check if stdin is piped (not a terminal)
        let stdin_piped = !std::io::stdin().is_terminal();

        // Check if stdout is piped (not a terminal)
        let stdout_piped = !std::io::stdout().is_terminal();

        stdin_piped || stdout_piped
    }

    /// Install a simple SIGPIPE handler that ignores the signal
    /// This prevents the process from terminating when the pipe is closed
    #[cfg(unix)]
    pub fn ignore_sigpipe() -> Result<(), SignalError> {
        unsafe {
            if libc::signal(libc::SIGPIPE, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(SignalError::SetupError(
                    "Failed to ignore SIGPIPE".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Install a simple SIGPIPE handler that ignores the signal (Windows no-op)
    /// On Windows, SIGPIPE doesn't exist, so this is a no-op
    #[cfg(not(unix))]
    pub fn ignore_sigpipe() -> Result<(), SignalError> {
        // SIGPIPE doesn't exist on Windows, so this is a no-op
        Ok(())
    }

    /// Check if we're running in a Unix-like environment
    pub const fn is_unix_like() -> bool {
        cfg!(unix)
    }

    /// Get the process ID for debugging
    pub fn get_pid() -> u32 {
        std::process::id()
    }
}

/// A wrapper that provides signal-aware operations
pub struct SignalAwareProcessor {
    signal_handler: SignalHandler,
    check_interval: Duration,
}

impl SignalAwareProcessor {
    /// Create a new signal-aware processor
    pub fn new() -> Result<Self, SignalError> {
        Ok(Self {
            signal_handler: SignalHandler::new()?,
            check_interval: Duration::from_millis(100),
        })
    }

    /// Create with custom check interval
    pub fn with_check_interval(interval: Duration) -> Result<Self, SignalError> {
        Ok(Self {
            signal_handler: SignalHandler::new()?,
            check_interval: interval,
        })
    }

    /// Execute a function with signal checking
    /// The function receives a closure that returns true if it should continue
    pub fn execute_with_signal_check<F, R>(&self, mut operation: F) -> Result<R, ProcessingError>
    where
        F: FnMut(&dyn Fn() -> bool) -> Result<R, ProcessingError>,
    {
        let should_continue = || !self.signal_handler.should_shutdown();

        operation(&should_continue)
    }

    /// Process data in chunks with signal checking between chunks
    pub fn process_chunked<T, F, R>(
        &self,
        data: Vec<T>,
        chunk_size: usize,
        mut processor: F,
    ) -> Result<Vec<R>, ProcessingError>
    where
        F: FnMut(&[T]) -> Result<Vec<R>, ProcessingError>,
    {
        let mut results = Vec::new();

        for chunk in data.chunks(chunk_size) {
            // Check for signals before processing each chunk
            if self.signal_handler.should_shutdown() {
                if self.signal_handler.pipe_closed() {
                    return Err(ProcessingError::PipeClosed);
                } else {
                    return Err(ProcessingError::Interrupted);
                }
            }

            let chunk_results = processor(chunk)?;
            results.extend(chunk_results);

            // Use the configured check interval for signal checking
            thread::sleep(self.check_interval);
        }

        Ok(results)
    }

    /// Get reference to the signal handler
    pub const fn signal_handler(&self) -> &SignalHandler {
        &self.signal_handler
    }
}

impl Default for SignalAwareProcessor {
    fn default() -> Self {
        Self::new().expect("Failed to create signal-aware processor")
    }
}

/// Errors that can occur during signal-aware processing
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error("Processing was interrupted by signal")]
    Interrupted,

    #[error("Output pipe was closed")]
    PipeClosed,

    #[error("Processing error: {0}")]
    ProcessingError(String),

    #[error("Signal handling error: {0}")]
    SignalError(#[from] SignalError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_signal_handler_creation() {
        let handler = SignalHandler::new();
        assert!(handler.is_ok());

        let handler = handler.unwrap();
        assert!(!handler.should_shutdown());
        assert!(!handler.pipe_closed());
    }

    #[test]
    fn test_signal_handler_reset() {
        let handler = SignalHandler::new().unwrap();
        handler.reset();
        assert!(!handler.should_shutdown());
        assert!(!handler.pipe_closed());
    }

    #[test]
    fn test_signal_wait_timeout() {
        let handler = SignalHandler::new().unwrap();
        let result = handler.wait_for_signal(Duration::from_millis(10));
        assert_eq!(result, SignalWaitResult::Timeout);
    }

    #[test]
    fn test_utils_is_unix_like() {
        // This test will pass on Unix-like systems
        if cfg!(unix) {
            assert!(utils::is_unix_like());
        } else {
            assert!(!utils::is_unix_like());
        }
    }

    #[test]
    fn test_utils_get_pid() {
        let pid = utils::get_pid();
        assert!(pid > 0);
    }

    #[test]
    fn test_signal_aware_processor_creation() {
        let processor = SignalAwareProcessor::new();
        assert!(processor.is_ok());
    }

    #[test]
    fn test_signal_aware_processor_with_interval() {
        let processor = SignalAwareProcessor::with_check_interval(Duration::from_millis(50));
        assert!(processor.is_ok());
    }

    #[test]
    fn test_execute_with_signal_check() {
        let processor = SignalAwareProcessor::new().unwrap();

        let result = processor.execute_with_signal_check(|should_continue| {
            if should_continue() {
                Ok("completed".to_string())
            } else {
                Err(ProcessingError::Interrupted)
            }
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "completed");
    }

    #[test]
    fn test_process_chunked() {
        let processor = SignalAwareProcessor::new().unwrap();
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let result =
            processor.process_chunked(data, 3, |chunk| Ok(chunk.iter().map(|x| x * 2).collect()));

        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results, vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
    }

    #[test]
    fn test_processing_error_display() {
        let error = ProcessingError::Interrupted;
        assert_eq!(error.to_string(), "Processing was interrupted by signal");

        let error = ProcessingError::PipeClosed;
        assert_eq!(error.to_string(), "Output pipe was closed");

        let error = ProcessingError::ProcessingError("test error".to_string());
        assert_eq!(error.to_string(), "Processing error: test error");
    }
}
