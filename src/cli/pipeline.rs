//! Processing pipeline for unified CLI operations
//!
//! This module provides a unified processing pipeline that handles different
//! CLI modes (file, text, stdin) and processing types (validation, transpilation).

use crate::cli::{
    debug_logger::DebugLogger,
    signal_handler::{utils, ProcessingError, SignalAwareProcessor, SignalHandler},
    DplyrValidator, ErrorHandler, ExitCode, JsonOutputFormatter, OutputFormat, OutputFormatter,
    StdinReader, TranspileMetadata, ValidateResult,
};
use crate::{
    DuckDbDialect, MySqlDialect, PostgreSqlDialect, SqlDialect, SqliteDialect, TranspileError,
    Transpiler,
};
use clap::{value_parser, Arg, ArgMatches, Command};
use std::io::{self, Write};

/// CLI arguments structure
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub input_file: Option<String>,
    pub output_file: Option<String>,
    pub dialect: SqlDialectType,
    pub pretty_print: bool,
    pub input_text: Option<String>,
    pub validate_only: bool,
    pub verbose: bool,
    pub debug: bool,
    pub compact: bool,
    pub json_output: bool,
}

/// Supported SQL dialect types
#[derive(Debug, Clone, PartialEq)]
pub enum SqlDialectType {
    PostgreSql,
    MySql,
    Sqlite,
    DuckDb,
}

impl std::fmt::Display for SqlDialectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlDialectType::PostgreSql => write!(f, "postgresql"),
            SqlDialectType::MySql => write!(f, "mysql"),
            SqlDialectType::Sqlite => write!(f, "sqlite"),
            SqlDialectType::DuckDb => write!(f, "duckdb"),
        }
    }
}

impl std::str::FromStr for SqlDialectType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" | "postgres" | "pg" => Ok(SqlDialectType::PostgreSql),
            "mysql" => Ok(SqlDialectType::MySql),
            "sqlite" => Ok(SqlDialectType::Sqlite),
            "duckdb" | "duck" => Ok(SqlDialectType::DuckDb),
            _ => Err(format!("Unsupported SQL dialect: {s}")),
        }
    }
}

/// Parses CLI arguments.
pub fn parse_args() -> CliArgs {
    let matches = Command::new("libdplyr")
        .version("0.1.0")
        .author("libdplyr contributors")
        .about("A transpiler that converts R dplyr syntax to SQL")
        .long_about("libdplyr is a Rust-based transpiler that converts R dplyr syntax to SQL queries.\n\
                     It supports multiple SQL dialects including PostgreSQL, MySQL, SQLite, and DuckDB.\n\n\
                     Examples:\n  \
                     libdplyr -t \"data %>% select(name, age) %>% filter(age > 18)\"\n  \
                     libdplyr -i input.R -o output.sql -d mysql -p\n  \
                     echo \"data %>% select(*)\" | libdplyr -d sqlite")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("Input dplyr file path")
                .long_help("Read dplyr code from the specified file. Cannot be used with -t/--text option.")
                .conflicts_with("text"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output SQL file path (stdout if not specified)")
                .long_help("Write the generated SQL to the specified file. If not provided, output goes to stdout."),
        )
        .arg(
            Arg::new("dialect")
                .short('d')
                .long("dialect")
                .value_name("DIALECT")
                .help("Target SQL dialect [possible values: postgresql, mysql, sqlite, duckdb]")
                .long_help("Specify the target SQL dialect for code generation.\n\
                           Supported dialects:\n  \
                           postgresql, postgres, pg - PostgreSQL\n  \
                           mysql - MySQL\n  \
                           sqlite - SQLite\n  \
                           duckdb, duck - DuckDB")
                .value_parser(value_parser!(SqlDialectType))
                .default_value("postgresql"),
        )
        .arg(
            Arg::new("pretty")
                .short('p')
                .long("pretty")
                .help("Pretty-format SQL output with proper indentation")
                .long_help("Format the generated SQL with proper line breaks and indentation for better readability.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("text")
                .short('t')
                .long("text")
                .value_name("DPLYR_CODE")
                .help("Direct dplyr code input")
                .long_help("Provide dplyr code directly as a command line argument. Cannot be used with -i/--input option.")
                .conflicts_with("input"),
        )
        .arg(
            Arg::new("validate-only")
                .long("validate-only")
                .help("Only validate dplyr syntax without generating SQL")
                .long_help("Perform syntax validation only without SQL generation. Returns exit code 0 for valid syntax, 1 for invalid syntax.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output with detailed processing information")
                .long_help("Display detailed information about each processing step to stderr. Useful for debugging and understanding the conversion process.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Enable debug mode with AST structure output")
                .long_help("Display detailed debug information including AST structure and conversion steps. Implies --verbose.")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("compact")
                .short('c')
                .long("compact")
                .help("Generate compact SQL output with minimal whitespace")
                .long_help("Output SQL in compact format with minimal whitespace. Conflicts with --pretty option.")
                .conflicts_with("pretty")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .short('j')
                .long("json")
                .help("Output results in JSON format with metadata")
                .long_help("Output SQL and metadata in JSON format. Includes dialect information, processing statistics, and timestamps.")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    parse_matches(&matches)
}

/// Creates CliArgs from ArgMatches.
fn parse_matches(matches: &ArgMatches) -> CliArgs {
    CliArgs {
        input_file: matches.get_one::<String>("input").cloned(),
        output_file: matches.get_one::<String>("output").cloned(),
        dialect: matches
            .get_one::<SqlDialectType>("dialect")
            .cloned()
            .unwrap_or(SqlDialectType::PostgreSql),
        pretty_print: matches.get_flag("pretty"),
        input_text: matches.get_one::<String>("text").cloned(),
        validate_only: matches.get_flag("validate-only"),
        verbose: matches.get_flag("verbose"),
        debug: matches.get_flag("debug"),
        compact: matches.get_flag("compact"),
        json_output: matches.get_flag("json"),
    }
}

/// Creates a SQL dialect instance based on the dialect type
fn create_dialect(dialect_type: &SqlDialectType) -> Box<dyn SqlDialect> {
    match dialect_type {
        SqlDialectType::PostgreSql => Box::new(PostgreSqlDialect::new()),
        SqlDialectType::MySql => Box::new(MySqlDialect::new()),
        SqlDialectType::Sqlite => Box::new(SqliteDialect::new()),
        SqlDialectType::DuckDb => Box::new(DuckDbDialect::new()),
    }
}

/// CLI operation modes
#[derive(Debug, Clone, PartialEq)]
pub enum CliMode {
    /// File-based processing mode
    FileMode {
        input_file: String,
        output_file: Option<String>,
    },
    /// Direct text processing mode
    TextMode {
        input_text: String,
        output_file: Option<String>,
    },
    /// Stdin processing mode
    StdinMode {
        validate_only: bool,
        streaming: bool,
    },
}

/// CLI configuration derived from command-line arguments
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub mode: CliMode,
    pub dialect: SqlDialectType,
    pub output_format: OutputFormat,
    pub validation_only: bool,
    pub verbose: bool,
    pub debug: bool,
}

impl CliConfig {
    /// Create CLI configuration from command-line arguments
    pub fn from_args(args: &CliArgs) -> Self {
        let mode = Self::determine_mode(args);
        let output_format = Self::determine_output_format(args);

        Self {
            mode,
            dialect: args.dialect.clone(),
            output_format,
            validation_only: args.validate_only,
            verbose: args.verbose,
            debug: args.debug,
        }
    }

    /// Determine the CLI mode based on arguments
    fn determine_mode(args: &CliArgs) -> CliMode {
        if let Some(ref input_text) = args.input_text {
            CliMode::TextMode {
                input_text: input_text.clone(),
                output_file: args.output_file.clone(),
            }
        } else if let Some(ref input_file) = args.input_file {
            CliMode::FileMode {
                input_file: input_file.clone(),
                output_file: args.output_file.clone(),
            }
        } else {
            CliMode::StdinMode {
                validate_only: args.validate_only,
                streaming: false, // Future extension
            }
        }
    }

    /// Determine output format based on arguments
    fn determine_output_format(args: &CliArgs) -> OutputFormat {
        if args.json_output {
            OutputFormat::Json
        } else if args.compact {
            OutputFormat::Compact
        } else if args.pretty_print {
            OutputFormat::Pretty
        } else {
            OutputFormat::Default
        }
    }
}

/// Processing pipeline that handles all CLI operations
pub struct ProcessingPipeline {
    config: CliConfig,
    transpiler: Transpiler,
    validator: Option<DplyrValidator>,
    output_formatter: OutputFormatter,
    json_formatter: JsonOutputFormatter,
    error_handler: ErrorHandler,
    debug_logger: DebugLogger,
    signal_handler: Option<SignalHandler>,
    signal_processor: Option<SignalAwareProcessor>,
}

impl ProcessingPipeline {
    /// Create a new processing pipeline with the given configuration
    pub fn new(config: CliConfig) -> Result<Self, TranspileError> {
        let dialect = create_dialect(&config.dialect);
        let transpiler = Transpiler::new(dialect);

        let validator = if config.validation_only {
            Some(DplyrValidator::new())
        } else {
            None
        };

        let output_formatter = OutputFormatter::with_format(config.output_format.clone());
        let json_formatter = JsonOutputFormatter::new();
        let error_handler = ErrorHandler::with_settings(false, config.verbose, false);
        let debug_logger = DebugLogger::with_settings(config.verbose, config.debug);

        // Initialize signal handling for Unix pipeline integration
        let (signal_handler, signal_processor) = if utils::is_unix_like()
            && matches!(config.mode, CliMode::StdinMode { .. })
        {
            // Enable signal handling for stdin mode on Unix-like systems
            let handler = SignalHandler::new().map_err(|e| {
                TranspileError::SystemError(format!("Failed to initialize signal handler: {e}"))
            })?;
            let processor = SignalAwareProcessor::new().map_err(|e| {
                TranspileError::SystemError(format!("Failed to initialize signal processor: {e}"))
            })?;

            // Ignore SIGPIPE to handle broken pipes gracefully
            if let Err(e) = utils::ignore_sigpipe() {
                eprintln!("Warning: Failed to ignore SIGPIPE: {e}");
            }

            (Some(handler), Some(processor))
        } else {
            (None, None)
        };

        Ok(Self {
            config,
            transpiler,
            validator,
            output_formatter,
            json_formatter,
            error_handler,
            debug_logger,
            signal_handler,
            signal_processor,
        })
    }

    /// Process input according to the configured mode
    pub fn process(&mut self) -> Result<String, TranspileError> {
        self.debug_logger.verbose("Starting processing pipeline");
        self.debug_logger.reset_step_timer();

        let input = self.read_input()?;
        self.debug_logger.timing("Input reading");

        let result = if self.config.validation_only {
            self.debug_logger.verbose("Validation mode enabled");
            self.validate_input(&input)
        } else {
            self.debug_logger.verbose("Transpilation mode enabled");
            self.transpile_input(&input)
        };

        self.debug_logger.total_time();
        result
    }

    /// Read input based on the configured mode
    fn read_input(&self) -> Result<String, TranspileError> {
        match &self.config.mode {
            CliMode::StdinMode { .. } => {
                self.debug_logger.verbose("Reading from stdin...");
                self.debug_logger.debug("Stdin mode: waiting for input");

                // Check if we're in a pipeline environment
                if utils::is_in_pipeline() {
                    self.debug_logger.debug("Pipeline environment detected");
                }

                // Use signal-aware stdin reader for Unix-like systems
                let reader = if utils::is_unix_like() {
                    self.debug_logger.debug("Using signal-aware stdin reader");
                    StdinReader::with_signal_handling().map_err(|e| {
                        TranspileError::SystemError(format!(
                            "Failed to create signal-aware stdin reader: {e}"
                        ))
                    })?
                } else {
                    StdinReader::new()
                };

                // Read input with signal handling
                let result = if let Some(ref signal_processor) = self.signal_processor {
                    self.read_stdin_with_signals(&reader, signal_processor)?
                } else {
                    reader.read_all().map_err(|e| {
                        TranspileError::IoError(format!("Failed to read from stdin: {e}"))
                    })?
                };

                self.debug_logger
                    .debug(&format!("Read {} bytes from stdin", result.len()));
                Ok(result)
            }
            CliMode::TextMode { input_text, .. } => {
                self.debug_logger.verbose("Processing direct text input...");
                self.debug_logger.debug(&format!(
                    "Text input length: {} characters",
                    input_text.len()
                ));
                Ok(input_text.clone())
            }
            CliMode::FileMode { input_file, .. } => {
                self.debug_logger
                    .verbose(&format!("Reading from file: {input_file}"));
                self.debug_logger.debug(&format!("File path: {input_file}"));

                let result = std::fs::read_to_string(input_file).map_err(|e| {
                    TranspileError::IoError(format!("Failed to read file '{input_file}': {e}"))
                })?;

                self.debug_logger
                    .debug(&format!("Read {} bytes from file", result.len()));
                Ok(result)
            }
        }
    }

    /// Validate input without transpilation
    fn validate_input(&self, input: &str) -> Result<String, TranspileError> {
        if let Some(ref validator) = self.validator {
            self.debug_logger.verbose("Validating dplyr syntax...");
            self.debug_logger
                .debug(&format!("Input to validate: {}", input.trim()));

            let result = validator.validate(input)?;

            match result {
                ValidateResult::Valid { summary } => {
                    self.debug_logger
                        .debug(&format!("Validation successful: {summary:?}"));
                    self.debug_logger.verbose("Syntax validation passed");

                    match self.config.output_format {
                        OutputFormat::Json => {
                            let metadata = TranspileMetadata::from_validation_summary(&summary);
                            Ok(self
                                .json_formatter
                                .format_validation_success(&summary, &metadata))
                        }
                        _ => Ok("Valid dplyr syntax".to_string()),
                    }
                }
                ValidateResult::Invalid { error, suggestions } => {
                    self.debug_logger
                        .debug(&format!("Validation failed: {error:?}"));
                    self.debug_logger
                        .verbose(&format!("Validation error: {}", error.message));

                    match self.config.output_format {
                        OutputFormat::Json => Ok(self
                            .json_formatter
                            .format_validation_error(&error, &suggestions)),
                        _ => {
                            let mut error_msg = format!("Validation failed: {}", error.message);
                            if !suggestions.is_empty() {
                                error_msg.push_str("\nSuggestions:");
                                for suggestion in suggestions {
                                    error_msg.push_str(&format!("\n  • {suggestion}"));
                                }
                            }
                            Err(TranspileError::ValidationError(error_msg))
                        }
                    }
                }
            }
        } else {
            Err(TranspileError::ConfigurationError(
                "Validator not configured for validation mode".to_string(),
            ))
        }
    }

    /// Transpile input to SQL
    fn transpile_input(&mut self, input: &str) -> Result<String, TranspileError> {
        self.debug_logger.verbose(&format!(
            "Transpiling dplyr to SQL (dialect: {})...",
            self.config.dialect
        ));
        self.debug_logger
            .debug(&format!("Input to transpile: {}", input.trim()));

        // Parse dplyr code to AST
        self.debug_logger.debug("Starting lexical analysis...");
        let ast = self.transpiler.parse_dplyr(input)?;
        self.debug_logger.timing("Parsing");

        // Log AST structure if debug mode is enabled
        self.debug_logger.log_ast(&ast);

        // Generate SQL from AST
        self.debug_logger.debug("Starting SQL generation...");
        let sql = self.transpiler.generate_sql(&ast)?;
        self.debug_logger.timing("SQL generation");

        self.debug_logger
            .log_sql_generation(&sql, &self.config.dialect.to_string());
        self.debug_logger
            .verbose("Transpilation completed successfully");

        match self.config.output_format {
            OutputFormat::Json => {
                let metadata = TranspileMetadata::transpilation_success(
                    &self.config.dialect,
                    self.debug_logger.elapsed(),
                    input,
                    &sql,
                );
                Ok(self.json_formatter.format_transpile_result(&sql, &metadata))
            }
            _ => Ok(self.output_formatter.format(&sql)?),
        }
    }

    /// Write output to the appropriate destination
    pub fn write_output(&self, output: &str) -> Result<(), TranspileError> {
        match &self.config.mode {
            CliMode::FileMode {
                output_file: Some(file),
                ..
            }
            | CliMode::TextMode {
                output_file: Some(file),
                ..
            } => {
                if self.config.verbose {
                    eprintln!("Writing output to file: {file}");
                }
                std::fs::write(file, output).map_err(|e| {
                    TranspileError::IoError(format!("Failed to write to file '{file}': {e}"))
                })
            }
            _ => {
                // Write to stdout
                print!("{output}");
                io::stdout()
                    .flush()
                    .map_err(|e| TranspileError::IoError(format!("Failed to flush stdout: {e}")))
            }
        }
    }

    /// Handle errors using the configured error handler
    pub fn handle_error(&self, error: &TranspileError) -> i32 {
        // If JSON output is requested, format error as JSON
        if matches!(self.config.output_format, OutputFormat::Json) {
            let error_info = crate::cli::json_output::ErrorInfo::from_transpile_error(error);
            let metadata = TranspileMetadata::transpilation_success(
                &self.config.dialect,
                std::time::Duration::from_millis(0),
                "",
                "",
            );

            let json_output = self.json_formatter.format_error(error_info, metadata);
            match json_output {
                Ok(json) => {
                    println!("{json}");
                }
                Err(_) => {
                    // Fallback to regular error handling if JSON formatting fails
                    return self.error_handler.handle_error(error);
                }
            }

            // Return appropriate exit code based on error type
            match error {
                TranspileError::LexError(_) | TranspileError::ParseError(_) => {
                    ExitCode::VALIDATION_ERROR
                }
                TranspileError::GenerationError(_) => ExitCode::TRANSPILATION_ERROR,
                TranspileError::IoError(_) => ExitCode::IO_ERROR,
                TranspileError::ValidationError(_) => ExitCode::VALIDATION_ERROR,
                TranspileError::ConfigurationError(_) => ExitCode::CONFIG_ERROR,
                TranspileError::SystemError(_) => ExitCode::SYSTEM_ERROR,
            }
        } else {
            self.error_handler.handle_error(error)
        }
    }

    /// Read stdin with signal handling support
    fn read_stdin_with_signals(
        &self,
        reader: &StdinReader,
        signal_processor: &SignalAwareProcessor,
    ) -> Result<String, TranspileError> {
        self.debug_logger
            .debug("Reading stdin with signal handling");

        signal_processor
            .execute_with_signal_check(|should_continue| {
                if !should_continue() {
                    if let Some(ref handler) = self.signal_handler {
                        if handler.pipe_closed() {
                            return Err(ProcessingError::PipeClosed);
                        } else {
                            return Err(ProcessingError::Interrupted);
                        }
                    }
                }

                reader.read_all().map_err(|e| {
                    ProcessingError::ProcessingError(format!("Failed to read from stdin: {e}"))
                })
            })
            .map_err(|e| match e {
                ProcessingError::Interrupted => {
                    TranspileError::SystemError("Reading interrupted by signal".to_string())
                }
                ProcessingError::PipeClosed => {
                    TranspileError::SystemError("Output pipe was closed".to_string())
                }
                ProcessingError::ProcessingError(msg) => TranspileError::IoError(msg),
                ProcessingError::SignalError(sig_err) => {
                    TranspileError::SystemError(format!("Signal error: {sig_err}"))
                }
            })
    }

    /// Check if processing should continue (signal handling)
    pub fn should_continue(&self) -> bool {
        if let Some(ref handler) = self.signal_handler {
            !handler.should_shutdown()
        } else {
            true
        }
    }

    /// Check if the output pipe was closed
    pub fn pipe_closed(&self) -> bool {
        if let Some(ref handler) = self.signal_handler {
            handler.pipe_closed()
        } else {
            false
        }
    }

    /// Get configuration reference
    pub fn config(&self) -> &CliConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliArgs;

    fn create_test_args() -> CliArgs {
        CliArgs {
            input_file: None,
            output_file: None,
            dialect: SqlDialectType::PostgreSql,
            pretty_print: false,
            input_text: None,
            validate_only: false,
            verbose: false,
            debug: false,
            compact: false,
            json_output: false,
        }
    }

    #[test]
    fn test_cli_config_from_args_stdin_mode() {
        let args = create_test_args();
        let config = CliConfig::from_args(&args);

        assert!(matches!(config.mode, CliMode::StdinMode { .. }));
        assert_eq!(config.dialect, SqlDialectType::PostgreSql);
        assert!(matches!(config.output_format, OutputFormat::Default));
        assert!(!config.validation_only);
    }

    #[test]
    fn test_cli_config_from_args_text_mode() {
        let mut args = create_test_args();
        args.input_text = Some("select(name)".to_string());
        args.json_output = true;

        let config = CliConfig::from_args(&args);

        if let CliMode::TextMode {
            input_text,
            output_file,
        } = config.mode
        {
            assert_eq!(input_text, "select(name)");
            assert_eq!(output_file, None);
        } else {
            panic!("Expected TextMode");
        }

        assert!(matches!(config.output_format, OutputFormat::Json));
    }

    #[test]
    fn test_cli_config_from_args_file_mode() {
        let mut args = create_test_args();
        args.input_file = Some("input.dplyr".to_string());
        args.output_file = Some("output.sql".to_string());
        args.pretty_print = true;

        let config = CliConfig::from_args(&args);

        if let CliMode::FileMode {
            input_file,
            output_file,
        } = config.mode
        {
            assert_eq!(input_file, "input.dplyr");
            assert_eq!(output_file, Some("output.sql".to_string()));
        } else {
            panic!("Expected FileMode");
        }

        assert!(matches!(config.output_format, OutputFormat::Pretty));
    }

    #[test]
    fn test_cli_config_validation_mode() {
        let mut args = create_test_args();
        args.validate_only = true;
        args.verbose = true;
        args.debug = true;

        let config = CliConfig::from_args(&args);

        assert!(config.validation_only);
        assert!(config.verbose);
        assert!(config.debug);
    }

    #[test]
    fn test_processing_pipeline_creation() {
        let args = create_test_args();
        let config = CliConfig::from_args(&args);

        let pipeline = ProcessingPipeline::new(config);
        assert!(pipeline.is_ok());
    }

    #[test]
    fn test_processing_pipeline_validation_mode() {
        let mut args = create_test_args();
        args.validate_only = true;
        let config = CliConfig::from_args(&args);

        let pipeline = ProcessingPipeline::new(config).unwrap();
        assert!(pipeline.validator.is_some());
    }
}
