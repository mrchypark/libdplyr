//! CLI module containing command-line interface components
//!
//! This module provides various components for handling command-line operations
//! including stdin reading, output formatting, validation, and error handling.

pub mod debug_logger;
pub mod error_handler;
pub mod json_output;
pub mod output_formatter;
pub mod pipeline;
pub mod signal_handler;
pub mod stdin_reader;
pub mod validator;

/// Main CLI entry point using the processing pipeline
pub fn run_cli() -> i32 {
    // Parse command line arguments
    let args = pipeline::parse_args();

    // Create CLI configuration from arguments
    let config = CliConfig::from_args(&args);

    // Create processing pipeline
    let mut pipeline = match ProcessingPipeline::new(config) {
        Ok(pipeline) => pipeline,
        Err(error) => {
            let error_handler = ErrorHandler::new();
            return error_handler.handle_error(&error);
        }
    };

    // Process input according to configuration
    match pipeline.process() {
        Ok(output) => {
            // Write output to appropriate destination
            if let Err(error) = pipeline.write_output(&output) {
                return pipeline.handle_error(&error);
            }

            // Success
            ExitCode::SUCCESS
        }
        Err(error) => {
            // Handle error and return appropriate exit code
            pipeline.handle_error(&error)
        }
    }
}

// Re-export all modules
pub use error_handler::{ErrorCategory, ErrorHandler, ErrorInfo, ExitCode};
pub use json_output::{
    ErrorInfo as JsonErrorInfo, InputInfo, JsonOutputFormatter, MetadataBuilder, ProcessingStats,
    TranspileMetadata,
};
pub use output_formatter::{FormatConfig, OutputFormat, OutputFormatter};
pub use pipeline::{parse_args, CliArgs, CliConfig, CliMode, ProcessingPipeline, SqlDialectType};
pub use signal_handler::{
    utils, ProcessingError, SignalAwareProcessor, SignalError, SignalHandler,
};
pub use stdin_reader::StdinReader;
pub use validator::{
    DplyrValidator, ValidateResult, ValidationConfig, ValidationErrorInfo, ValidationSummary,
};
