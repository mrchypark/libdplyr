//! Debug logger for CLI operations
//!
//! This module provides utilities for verbose and debug output during CLI operations.
//! It handles different verbosity levels and formats debug information appropriately.

use std::io::{self, Write};
use std::time::{Duration, Instant};
use colored::Colorize;

/// Debug logger configuration
#[derive(Debug, Clone)]
pub struct DebugLoggerConfig {
    /// Enable verbose output
    pub verbose: bool,
    /// Enable debug output
    pub debug: bool,
    /// Use colors in output
    pub use_colors: bool,
    /// Use Korean language
    pub use_korean: bool,
}

impl Default for DebugLoggerConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            debug: false,
            use_colors: true,
            use_korean: false,
        }
    }
}

/// Debug logger for CLI operations
pub struct DebugLogger {
    config: DebugLoggerConfig,
    start_time: Instant,
    step_time: Instant,
}

impl DebugLogger {
    /// Create a new debug logger with the given configuration
    pub fn new(config: DebugLoggerConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            start_time: now,
            step_time: now,
        }
    }

    /// Create a new debug logger with default configuration
    pub fn with_settings(verbose: bool, debug: bool) -> Self {
        Self::new(DebugLoggerConfig {
            verbose,
            debug,
            ..Default::default()
        })
    }

    /// Log a verbose message
    pub fn verbose(&self, message: &str) {
        if self.config.verbose || self.config.debug {
            let prefix = if self.config.use_colors {
                "[INFO]".bright_blue()
            } else {
                "[INFO]".normal()
            };
            
            eprintln!("{} {}", prefix, message);
        }
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) {
        if self.config.debug {
            let prefix = if self.config.use_colors {
                "[DEBUG]".bright_yellow()
            } else {
                "[DEBUG]".normal()
            };
            
            eprintln!("{} {}", prefix, message);
        }
    }

    /// Log a timing message with elapsed time since last timing call
    pub fn timing(&mut self, label: &str) {
        if self.config.debug {
            let elapsed = self.step_time.elapsed();
            let prefix = if self.config.use_colors {
                "[TIME]".bright_green()
            } else {
                "[TIME]".normal()
            };
            
            eprintln!("{} {} took {:.2?}", prefix, label, elapsed);
            self.step_time = Instant::now();
        }
    }

    /// Log total execution time
    pub fn total_time(&self) {
        if self.config.verbose || self.config.debug {
            let elapsed = self.start_time.elapsed();
            let prefix = if self.config.use_colors {
                "[TOTAL]".bright_magenta()
            } else {
                "[TOTAL]".normal()
            };
            
            eprintln!("{} Execution completed in {:.2?}", prefix, elapsed);
        }
    }

    /// Log AST structure
    pub fn log_ast(&self, ast: &impl std::fmt::Debug) {
        if self.config.debug {
            let prefix = if self.config.use_colors {
                "[AST]".bright_cyan()
            } else {
                "[AST]".normal()
            };
            
            eprintln!("{} Structure:\\n{:#?}", prefix, ast);
        }
    }

    /// Log SQL generation details
    pub fn log_sql_generation(&self, sql: &str, dialect: &str) {
        if self.config.debug {
            let prefix = if self.config.use_colors {
                "[SQL]".bright_green()
            } else {
                "[SQL]".normal()
            };
            
            eprintln!("{} Generated {} SQL:\\n{}", prefix, dialect, sql);
        }
    }

    /// Log processing statistics
    pub fn log_stats(&self, stats: &impl std::fmt::Display) {
        if self.config.debug {
            let prefix = if self.config.use_colors {
                "[STATS]".bright_blue()
            } else {
                "[STATS]".normal()
            };
            
            eprintln!("{} {}", prefix, stats);
        }
    }

    /// Reset step timer
    pub fn reset_step_timer(&mut self) {
        self.step_time = Instant::now();
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get elapsed time since last step
    pub fn step_elapsed(&self) -> Duration {
        self.step_time.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_logger_creation() {
        let logger = DebugLogger::new(DebugLoggerConfig::default());
        assert!(!logger.config.verbose);
        assert!(!logger.config.debug);
    }

    #[test]
    fn test_debug_logger_with_settings() {
        let logger = DebugLogger::with_settings(true, false);
        assert!(logger.config.verbose);
        assert!(!logger.config.debug);
    }

    #[test]
    fn test_elapsed_time() {
        let logger = DebugLogger::new(DebugLoggerConfig::default());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(logger.elapsed().as_millis() >= 10);
    }

    #[test]
    fn test_step_elapsed_time() {
        let logger = DebugLogger::new(DebugLoggerConfig::default());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(logger.step_elapsed().as_millis() >= 10);
    }
}