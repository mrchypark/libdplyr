# Design Document

## Overview

stdin/stdout CLI 기능은 기존 libdplyr CLI를 확장하여 파이프라인 친화적인 인터페이스를 제공합니다. 이 기능은 Unix 철학에 따라 "한 가지 일을 잘하고, 다른 도구와 잘 협력하는" 도구를 만드는 것을 목표로 합니다. 기존 파일 기반 CLI와 함께 동작하며, stdin에서 dplyr 코드를 읽고 stdout으로 SQL을 출력하는 스트리밍 방식을 지원합니다.

## Architecture

### 기존 CLI 구조와의 통합

```
┌─────────────────────────────────────────────────────────────┐
│                    CLI Entry Point                          │
│                   (src/main.rs)                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                CLI Argument Parser                          │
│                 (src/cli.rs)                               │
│  ┌─────────────────┬─────────────────┬─────────────────┐   │
│  │   File Mode     │   Text Mode     │   Stdin Mode    │   │
│  │   (-i/-o)       │   (-t)          │   (default)     │   │
│  └─────────────────┴─────────────────┴─────────────────┘   │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│              Input Processing Layer                         │
│  ┌─────────────────┬─────────────────┬─────────────────┐   │
│  │  File Reader    │  Direct Text    │  Stdin Reader   │   │
│  │                 │                 │   (Enhanced)    │   │
│  └─────────────────┴─────────────────┴─────────────────┘   │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                Transpiler Core                              │
│              (Existing libdplyr)                           │
│  ┌─────────────────┬─────────────────┬─────────────────┐   │
│  │     Lexer       │     Parser      │  SQL Generator  │   │
│  └─────────────────┴─────────────────┴─────────────────┘   │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│              Output Processing Layer                        │
│  ┌─────────────────┬─────────────────┬─────────────────┐   │
│  │  File Writer    │  Stdout Writer  │  Format Handler │   │
│  │                 │   (Enhanced)    │   (Enhanced)    │   │
│  └─────────────────┴─────────────────┴─────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 새로운 CLI 모드 구조

```rust
pub enum CliMode {
    FileMode {
        input_file: String,
        output_file: Option<String>,
    },
    TextMode {
        input_text: String,
        output_file: Option<String>,
    },
    StdinMode {
        // 새로운 모드
        validate_only: bool,
        streaming: bool,
    },
}
```

## Components and Interfaces

### 1. Enhanced CLI Argument Parser

기존 `src/cli.rs`를 확장하여 새로운 옵션들을 추가합니다:

```rust
#[derive(Debug, Clone)]
pub struct CliArgs {
    // 기존 필드들
    pub input_file: Option<String>,
    pub output_file: Option<String>,
    pub dialect: SqlDialectType,
    pub pretty_print: bool,
    pub input_text: Option<String>,
    
    // 새로운 필드들
    pub validate_only: bool,
    pub output_format: OutputFormat,
    pub verbose: bool,
    pub debug: bool,
    pub compact: bool,
    pub json_output: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Default,    // 기본 형식
    Pretty,     // --pretty (기존)
    Compact,    // --compact (새로운)
    Json,       // --json (새로운)
}
```

### 2. Stdin Reader Module

새로운 stdin 처리 모듈을 추가합니다:

```rust
// src/cli/stdin_reader.rs
use std::io::{self, BufRead, BufReader, Read};

pub struct StdinReader {
    reader: BufReader<io::Stdin>,
    buffer_size: usize,
}

impl StdinReader {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            buffer_size: 8192, // 8KB 버퍼
        }
    }
    
    /// 전체 입력을 한 번에 읽기 (기본 모드)
    pub fn read_all(&mut self) -> io::Result<String> {
        let mut input = String::new();
        self.reader.read_to_string(&mut input)?;
        Ok(input)
    }
    
    /// 스트리밍 모드로 줄 단위 읽기 (향후 확장용)
    pub fn read_lines(&mut self) -> io::Result<Vec<String>> {
        let mut lines = Vec::new();
        let mut line = String::new();
        
        while self.reader.read_line(&mut line)? > 0 {
            lines.push(line.trim_end().to_string());
            line.clear();
        }
        
        Ok(lines)
    }
    
    /// 파이프 연결 여부 확인
    pub fn is_piped() -> bool {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::isatty(io::stdin().as_raw_fd()) == 0
        }
    }
}
```

### 3. Output Formatter Module

다양한 출력 형식을 지원하는 모듈을 추가합니다:

```rust
// src/cli/output_formatter.rs
use serde_json::json;
use crate::DplyrNode;

pub struct OutputFormatter {
    format: OutputFormat,
}

impl OutputFormatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }
    
    pub fn format_sql(&self, sql: &str, metadata: Option<&TranspileMetadata>) -> String {
        match self.format {
            OutputFormat::Default => sql.to_string(),
            OutputFormat::Pretty => self.format_pretty(sql),
            OutputFormat::Compact => self.format_compact(sql),
            OutputFormat::Json => self.format_json(sql, metadata),
        }
    }
    
    fn format_pretty(&self, sql: &str) -> String {
        // 기존 format_sql 함수 로직 사용
        format_sql_pretty(sql)
    }
    
    fn format_compact(&self, sql: &str) -> String {
        sql.split_whitespace()
           .collect::<Vec<_>>()
           .join(" ")
    }
    
    fn format_json(&self, sql: &str, metadata: Option<&TranspileMetadata>) -> String {
        let mut output = json!({
            "sql": sql,
            "dialect": metadata.map(|m| m.dialect.to_string()).unwrap_or_default(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Some(meta) = metadata {
            output["metadata"] = json!({
                "operations_count": meta.operations_count,
                "complexity_score": meta.complexity_score,
                "warnings": meta.warnings,
            });
        }
        
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| sql.to_string())
    }
}

#[derive(Debug)]
pub struct TranspileMetadata {
    pub dialect: SqlDialectType,
    pub operations_count: usize,
    pub complexity_score: f32,
    pub warnings: Vec<String>,
    pub processing_time_ms: u64,
}
```

### 4. Validation Module

문법 검증 전용 기능을 추가합니다:

```rust
// src/cli/validator.rs
use crate::{Transpiler, TranspileError, DplyrNode};

pub struct DplyrValidator {
    transpiler: Transpiler,
}

impl DplyrValidator {
    pub fn new(transpiler: Transpiler) -> Self {
        Self { transpiler }
    }
    
    pub fn validate(&self, dplyr_code: &str) -> ValidationResult {
        match self.transpiler.parse_dplyr(dplyr_code) {
            Ok(ast) => ValidationResult::Valid {
                ast,
                message: "Valid dplyr syntax".to_string(),
            },
            Err(e) => ValidationResult::Invalid {
                error: e,
                suggestions: self.generate_suggestions(&e),
            },
        }
    }
    
    fn generate_suggestions(&self, error: &TranspileError) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        match error {
            TranspileError::ParseError(parse_err) => {
                let error_str = parse_err.to_string();
                if error_str.contains("Unexpected token") {
                    suggestions.push("Check function names and syntax".to_string());
                    suggestions.push("Supported functions: select, filter, mutate, arrange, group_by, summarise".to_string());
                }
                if error_str.contains("pipe") {
                    suggestions.push("Use %>% for pipe operations".to_string());
                }
            }
            TranspileError::LexError(lex_err) => {
                let error_str = lex_err.to_string();
                if error_str.contains("string") {
                    suggestions.push("Check string quotes and escaping".to_string());
                }
            }
            _ => {}
        }
        
        suggestions
    }
}

#[derive(Debug)]
pub enum ValidationResult {
    Valid {
        ast: DplyrNode,
        message: String,
    },
    Invalid {
        error: TranspileError,
        suggestions: Vec<String>,
    },
}
```

### 5. Enhanced Error Handler

더 상세한 오류 처리와 exit code 관리:

```rust
// src/cli/error_handler.rs
use std::process;
use crate::TranspileError;

pub struct ErrorHandler {
    verbose: bool,
    debug: bool,
}

impl ErrorHandler {
    pub fn new(verbose: bool, debug: bool) -> Self {
        Self { verbose, debug }
    }
    
    pub fn handle_error(&self, error: &CliError) -> ! {
        match error {
            CliError::InvalidArguments(msg) => {
                self.print_argument_error(msg);
                process::exit(2);
            }
            CliError::IoError(msg) => {
                self.print_io_error(msg);
                process::exit(1);
            }
            CliError::TranspileError(e) => {
                self.print_transpile_error(e);
                process::exit(3);
            }
            CliError::SystemError(msg) => {
                self.print_system_error(msg);
                process::exit(4);
            }
        }
    }
    
    fn print_argument_error(&self, msg: &str) {
        eprintln!("명령줄 인수 오류: {}", msg);
        if self.verbose {
            eprintln!("사용법: libdplyr [OPTIONS]");
            eprintln!("자세한 도움말: libdplyr --help");
        }
    }
    
    fn print_io_error(&self, msg: &str) {
        eprintln!("입출력 오류: {}", msg);
        if self.verbose {
            eprintln!("파일 권한과 경로를 확인하세요.");
        }
    }
    
    fn print_transpile_error(&self, error: &TranspileError) {
        eprintln!("변환 오류: {}", error);
        
        if self.debug {
            eprintln!("디버그 정보: {:#?}", error);
        }
        
        // 기존 print_transpile_error 로직 재사용
        print_transpile_error_details(error);
    }
    
    fn print_system_error(&self, msg: &str) {
        eprintln!("시스템 오류: {}", msg);
        if self.verbose {
            eprintln!("시스템 리소스를 확인하세요.");
        }
    }
}

#[derive(Debug)]
pub enum CliError {
    InvalidArguments(String),
    IoError(String),
    TranspileError(TranspileError),
    SystemError(String),
}
```

## Data Models

### CLI Configuration

```rust
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
    pub fn from_args(args: &CliArgs) -> Self {
        let mode = if args.input_text.is_some() {
            CliMode::TextMode {
                input_text: args.input_text.clone().unwrap(),
                output_file: args.output_file.clone(),
            }
        } else if args.input_file.is_some() {
            CliMode::FileMode {
                input_file: args.input_file.clone().unwrap(),
                output_file: args.output_file.clone(),
            }
        } else {
            CliMode::StdinMode {
                validate_only: args.validate_only,
                streaming: false, // 향후 확장용
            }
        };
        
        let output_format = if args.json_output {
            OutputFormat::Json
        } else if args.compact {
            OutputFormat::Compact
        } else if args.pretty_print {
            OutputFormat::Pretty
        } else {
            OutputFormat::Default
        };
        
        Self {
            mode,
            dialect: args.dialect.clone(),
            output_format,
            validation_only: args.validate_only,
            verbose: args.verbose,
            debug: args.debug,
        }
    }
}
```

### Processing Pipeline

```rust
pub struct ProcessingPipeline {
    config: CliConfig,
    transpiler: Transpiler,
    validator: Option<DplyrValidator>,
    formatter: OutputFormatter,
    error_handler: ErrorHandler,
}

impl ProcessingPipeline {
    pub fn new(config: CliConfig) -> Self {
        let dialect = create_dialect(&config.dialect);
        let transpiler = Transpiler::new(dialect);
        let validator = if config.validation_only {
            Some(DplyrValidator::new(transpiler.clone()))
        } else {
            None
        };
        let formatter = OutputFormatter::new(config.output_format.clone());
        let error_handler = ErrorHandler::new(config.verbose, config.debug);
        
        Self {
            config,
            transpiler,
            validator,
            formatter,
            error_handler,
        }
    }
    
    pub fn process(&self) -> Result<String, CliError> {
        let input = self.read_input()?;
        
        if let Some(validator) = &self.validator {
            self.validate_input(&input, validator)
        } else {
            self.transpile_input(&input)
        }
    }
    
    fn read_input(&self) -> Result<String, CliError> {
        match &self.config.mode {
            CliMode::StdinMode { .. } => {
                let mut reader = StdinReader::new();
                reader.read_all()
                    .map_err(|e| CliError::IoError(format!("stdin 읽기 실패: {}", e)))
            }
            CliMode::TextMode { input_text, .. } => {
                Ok(input_text.clone())
            }
            CliMode::FileMode { input_file, .. } => {
                std::fs::read_to_string(input_file)
                    .map_err(|e| CliError::IoError(format!("파일 읽기 실패: {}", e)))
            }
        }
    }
    
    fn validate_input(&self, input: &str, validator: &DplyrValidator) -> Result<String, CliError> {
        match validator.validate(input) {
            ValidationResult::Valid { message, .. } => Ok(message),
            ValidationResult::Invalid { error, suggestions } => {
                let mut error_msg = format!("검증 실패: {}", error);
                if !suggestions.is_empty() {
                    error_msg.push_str("\n제안사항:");
                    for suggestion in suggestions {
                        error_msg.push_str(&format!("\n  • {}", suggestion));
                    }
                }
                Err(CliError::TranspileError(error))
            }
        }
    }
    
    fn transpile_input(&self, input: &str) -> Result<String, CliError> {
        let sql = self.transpiler.transpile(input)
            .map_err(CliError::TranspileError)?;
        
        let metadata = TranspileMetadata {
            dialect: self.config.dialect.clone(),
            operations_count: 0, // TODO: AST에서 계산
            complexity_score: 0.0, // TODO: 복잡도 계산
            warnings: Vec::new(),
            processing_time_ms: 0, // TODO: 시간 측정
        };
        
        Ok(self.formatter.format_sql(&sql, Some(&metadata)))
    }
}
```

## Error Handling

### Exit Code Strategy

```rust
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_IO_ERROR: i32 = 1;
pub const EXIT_INVALID_ARGS: i32 = 2;
pub const EXIT_TRANSPILE_ERROR: i32 = 3;
pub const EXIT_SYSTEM_ERROR: i32 = 4;

pub fn determine_exit_code(error: &CliError) -> i32 {
    match error {
        CliError::InvalidArguments(_) => EXIT_INVALID_ARGS,
        CliError::IoError(_) => EXIT_IO_ERROR,
        CliError::TranspileError(_) => EXIT_TRANSPILE_ERROR,
        CliError::SystemError(_) => EXIT_SYSTEM_ERROR,
    }
}
```

### Signal Handling

```rust
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct SignalHandler {
    shutdown: Arc<AtomicBool>,
}

impl SignalHandler {
    pub fn new() -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        
        std::thread::spawn(move || {
            let mut signals = Signals::new(&[SIGINT]).unwrap();
            for _signal in signals.forever() {
                shutdown_clone.store(true, Ordering::Relaxed);
                break;
            }
        });
        
        Self { shutdown }
    }
    
    pub fn should_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stdin_reader_creation() {
        let _reader = StdinReader::new();
    }
    
    #[test]
    fn test_output_formatter_default() {
        let formatter = OutputFormatter::new(OutputFormat::Default);
        let sql = "SELECT name FROM users";
        let result = formatter.format_sql(sql, None);
        assert_eq!(result, sql);
    }
    
    #[test]
    fn test_output_formatter_compact() {
        let formatter = OutputFormatter::new(OutputFormat::Compact);
        let sql = "SELECT  name  FROM   users";
        let result = formatter.format_sql(sql, None);
        assert_eq!(result, "SELECT name FROM users");
    }
    
    #[test]
    fn test_output_formatter_json() {
        let formatter = OutputFormatter::new(OutputFormat::Json);
        let sql = "SELECT name FROM users";
        let result = formatter.format_sql(sql, None);
        assert!(result.contains("\"sql\""));
        assert!(result.contains(sql));
    }
    
    #[test]
    fn test_validator_valid_syntax() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let validator = DplyrValidator::new(transpiler);
        
        let result = validator.validate("select(name, age)");
        assert!(matches!(result, ValidationResult::Valid { .. }));
    }
    
    #[test]
    fn test_validator_invalid_syntax() {
        let transpiler = Transpiler::new(Box::new(PostgreSqlDialect::new()));
        let validator = DplyrValidator::new(transpiler);
        
        let result = validator.validate("invalid_function()");
        assert!(matches!(result, ValidationResult::Invalid { .. }));
    }
    
    #[test]
    fn test_cli_config_from_args() {
        let args = CliArgs {
            input_file: None,
            output_file: None,
            dialect: SqlDialectType::PostgreSql,
            pretty_print: false,
            input_text: None,
            validate_only: true,
            output_format: OutputFormat::Json,
            verbose: true,
            debug: false,
            compact: false,
            json_output: true,
        };
        
        let config = CliConfig::from_args(&args);
        assert!(matches!(config.mode, CliMode::StdinMode { .. }));
        assert!(config.validation_only);
        assert!(matches!(config.output_format, OutputFormat::Json));
    }
}
```

### Integration Tests

```rust
// tests/stdin_stdout_integration.rs
use std::process::{Command, Stdio};
use std::io::Write;

#[test]
fn test_stdin_stdout_basic() {
    let mut child = Command::new("target/debug/libdplyr")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr");
    
    let stdin = child.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(b"select(name, age)").expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");
    drop(stdin);
    
    let output = child.wait_with_output().expect("Failed to read stdout");
    
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("SELECT"));
    assert!(stdout.contains("name"));
    assert!(stdout.contains("age"));
}

#[test]
fn test_validation_only_mode() {
    let mut child = Command::new("target/debug/libdplyr")
        .args(&["--validate-only"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr");
    
    let stdin = child.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(b"select(name, age) %>% filter(age > 18)").expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");
    drop(stdin);
    
    let output = child.wait_with_output().expect("Failed to read stdout");
    
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    assert!(stdout.contains("Valid dplyr syntax"));
}

#[test]
fn test_json_output_format() {
    let mut child = Command::new("target/debug/libdplyr")
        .args(&["--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start libdplyr");
    
    let stdin = child.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(b"select(name)").expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");
    drop(stdin);
    
    let output = child.wait_with_output().expect("Failed to read stdout");
    
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    // JSON 형식 검증
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON");
    assert!(json["sql"].is_string());
    assert!(json["dialect"].is_string());
    assert!(json["timestamp"].is_string());
}
```

### Performance Tests

```rust
// benches/stdin_stdout_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::{Command, Stdio};
use std::io::Write;

fn benchmark_stdin_processing(c: &mut Criterion) {
    c.bench_function("stdin processing", |b| {
        b.iter(|| {
            let mut child = Command::new("target/release/libdplyr")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start libdplyr");
            
            let stdin = child.stdin.as_mut().expect("Failed to open stdin");
            stdin.write_all(black_box(b"select(name, age) %>% filter(age > 18) %>% arrange(desc(age))"))
                .expect("Failed to write to stdin");
            stdin.flush().expect("Failed to flush stdin");
            drop(stdin);
            
            let _output = child.wait_with_output().expect("Failed to read stdout");
        });
    });
}

criterion_group!(benches, benchmark_stdin_processing);
criterion_main!(benches);
```

## Implementation Considerations

### Memory Management

- stdin 읽기 시 버퍼 크기 최적화 (8KB 기본값)
- 대용량 입력에 대한 스트리밍 처리 준비
- 메모리 사용량 모니터링 및 제한

### Performance Optimization

- 파이프 감지를 통한 최적화된 처리 경로
- JSON 출력 시 지연 직렬화
- 불필요한 문자열 복사 최소화

### Cross-platform Compatibility

- Unix 시스템의 파이프 감지 (`isatty` 사용)
- Windows 호환성을 위한 조건부 컴파일
- 시그널 처리의 플랫폼별 구현

## Cross-Platform Release and Distribution

### GitHub Actions CI/CD Pipeline

크로스 플랫폼 바이너리 빌드와 자동 릴리즈를 위한 CI/CD 파이프라인을 구성합니다:

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    name: Build for ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact_name: libdplyr
            asset_name: libdplyr-linux-x86_64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact_name: libdplyr
            asset_name: libdplyr-linux-aarch64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact_name: libdplyr
            asset_name: libdplyr-macos-x86_64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact_name: libdplyr
            asset_name: libdplyr-macos-aarch64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact_name: libdplyr.exe
            asset_name: libdplyr-windows-x86_64.exe

    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: ${{ matrix.target }}
          override: true
      
      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu
      
      - name: Build binary
        uses: actions-rs/cargo@v1
        with:
          command: build
          args: --release --target ${{ matrix.target }}
      
      - name: Strip binary (Unix)
        if: matrix.os != 'windows-latest'
        run: strip target/${{ matrix.target }}/release/${{ matrix.artifact_name }}
      
      - name: Upload binary
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.asset_name }}
          path: target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Download all artifacts
        uses: actions/download-artifact@v3
      
      - name: Generate release notes
        id: release_notes
        run: |
          echo "RELEASE_NOTES<<EOF" >> $GITHUB_OUTPUT
          echo "## libdplyr ${GITHUB_REF#refs/tags/}" >> $GITHUB_OUTPUT
          echo "" >> $GITHUB_OUTPUT
          echo "### 새로운 기능" >> $GITHUB_OUTPUT
          echo "- stdin/stdout 파이프라인 지원" >> $GITHUB_OUTPUT
          echo "- 다양한 출력 형식 (JSON, Pretty, Compact)" >> $GITHUB_OUTPUT
          echo "- 문법 검증 전용 모드" >> $GITHUB_OUTPUT
          echo "- 향상된 오류 처리 및 디버깅" >> $GITHUB_OUTPUT
          echo "" >> $GITHUB_OUTPUT
          echo "### 지원 플랫폼" >> $GITHUB_OUTPUT
          echo "- Linux (x86_64, ARM64)" >> $GITHUB_OUTPUT
          echo "- macOS (Intel, Apple Silicon)" >> $GITHUB_OUTPUT
          echo "- Windows (x86_64)" >> $GITHUB_OUTPUT
          echo "" >> $GITHUB_OUTPUT
          echo "### 설치 방법" >> $GITHUB_OUTPUT
          echo "\`\`\`bash" >> $GITHUB_OUTPUT
          echo "# 최신 버전 설치" >> $GITHUB_OUTPUT
          echo "curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh" >> $GITHUB_OUTPUT
          echo "" >> $GITHUB_OUTPUT
          echo "# 특정 버전 설치" >> $GITHUB_OUTPUT
          echo "LIBDPLYR_VERSION=${GITHUB_REF#refs/tags/} curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh" >> $GITHUB_OUTPUT
          echo "\`\`\`" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT
      
      - name: Create Release
        id: create_release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: libdplyr ${{ github.ref }}
          body: ${{ steps.release_notes.outputs.RELEASE_NOTES }}
          draft: false
          prerelease: false
      
      - name: Upload Release Assets
        run: |
          for asset in libdplyr-*; do
            if [ -f "$asset/${{ matrix.artifact_name }}" ]; then
              mv "$asset/${{ matrix.artifact_name }}" "$asset"
            fi
            gh release upload ${{ github.ref }} "$asset" --clobber
          done
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Installation Script Design

설치 스크립트는 다음과 같은 구조로 설계됩니다:

```bash
#!/bin/bash
# install.sh - libdplyr 설치 스크립트

set -e

# 설정 변수
REPO="libdplyr/libdplyr"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
FALLBACK_INSTALL_DIR="$HOME/.local/bin"
VERSION="${LIBDPLYR_VERSION:-latest}"

# 플랫폼 감지
detect_platform() {
    local os arch
    
    case "$(uname -s)" in
        Linux*)     os="linux" ;;
        Darwin*)    os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)          echo "지원되지 않는 운영체제: $(uname -s)" >&2; exit 1 ;;
    esac
    
    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              echo "지원되지 않는 아키텍처: $(uname -m)" >&2; exit 1 ;;
    esac
    
    echo "${os}-${arch}"
}

# 최신 버전 가져오기
get_latest_version() {
    curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# 바이너리 다운로드
download_binary() {
    local version="$1"
    local platform="$2"
    local binary_name="libdplyr-${platform}"
    
    if [ "$platform" = "windows-x86_64" ]; then
        binary_name="${binary_name}.exe"
    fi
    
    local download_url="https://github.com/${REPO}/releases/download/${version}/${binary_name}"
    
    echo "다운로드 중: ${download_url}"
    curl -L -o "libdplyr" "$download_url"
    chmod +x "libdplyr"
}

# 설치 실행
install_binary() {
    local install_path="$1"
    
    if [ ! -d "$(dirname "$install_path")" ]; then
        mkdir -p "$(dirname "$install_path")"
    fi
    
    if [ -w "$(dirname "$install_path")" ]; then
        mv "libdplyr" "$install_path"
        echo "libdplyr이 $install_path에 설치되었습니다."
    else
        echo "권한이 부족합니다. sudo를 사용하여 설치합니다..."
        sudo mv "libdplyr" "$install_path"
        echo "libdplyr이 $install_path에 설치되었습니다."
    fi
}

main() {
    echo "libdplyr 설치를 시작합니다..."
    
    # 플랫폼 감지
    local platform
    platform=$(detect_platform)
    echo "감지된 플랫폼: $platform"
    
    # 버전 결정
    if [ "$VERSION" = "latest" ]; then
        VERSION=$(get_latest_version)
        echo "최신 버전: $VERSION"
    else
        echo "지정된 버전: $VERSION"
    fi
    
    # 임시 디렉토리 생성
    local temp_dir
    temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    # 바이너리 다운로드
    download_binary "$VERSION" "$platform"
    
    # 설치 시도
    if install_binary "$INSTALL_DIR/libdplyr" 2>/dev/null; then
        :  # 성공
    elif install_binary "$FALLBACK_INSTALL_DIR/libdplyr"; then
        echo "경고: $INSTALL_DIR에 설치할 수 없어 $FALLBACK_INSTALL_DIR에 설치했습니다."
        echo "PATH에 $FALLBACK_INSTALL_DIR이 포함되어 있는지 확인하세요."
    else
        echo "설치에 실패했습니다." >&2
        exit 1
    fi
    
    # 정리
    cd /
    rm -rf "$temp_dir"
    
    # 설치 확인
    if command -v libdplyr >/dev/null 2>&1; then
        echo "설치가 완료되었습니다!"
        echo "사용법: libdplyr --help"
        libdplyr --version
    else
        echo "설치는 완료되었지만 PATH에서 libdplyr을 찾을 수 없습니다."
        echo "다음 명령을 실행하여 PATH를 업데이트하세요:"
        echo "export PATH=\"\$PATH:$FALLBACK_INSTALL_DIR\""
    fi
}

main "$@"
```

### Documentation Updates

README.md에 추가할 설치 및 사용법 섹션:

```markdown
## 설치

### 자동 설치 (권장)

최신 버전을 자동으로 설치:

```bash
curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh
```

특정 버전 설치:

```bash
LIBDPLYR_VERSION=v1.0.0 curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh
```

### 수동 설치

1. [Releases 페이지](https://github.com/libdplyr/libdplyr/releases)에서 플랫폼에 맞는 바이너리 다운로드
2. 바이너리를 PATH에 포함된 디렉토리로 이동
3. 실행 권한 부여: `chmod +x libdplyr`

### 지원 플랫폼

- **Linux**: x86_64, ARM64
- **macOS**: Intel (x86_64), Apple Silicon (ARM64)
- **Windows**: x86_64

## 사용법

### 기본 사용법

```bash
# stdin에서 dplyr 코드 읽기
echo "select(name, age) %>% filter(age > 18)" | libdplyr

# 파일에서 읽기
libdplyr -i query.R -o result.sql

# 직접 코드 입력
libdplyr -t "data %>% select(name, age)"
```

### 고급 옵션

```bash
# SQL 방언 지정
echo "select(name)" | libdplyr -d mysql

# 출력 형식 지정
echo "select(name)" | libdplyr --pretty
echo "select(name)" | libdplyr --json
echo "select(name)" | libdplyr --compact

# 문법 검증만 수행
echo "select(name, age)" | libdplyr --validate-only

# 디버그 정보 출력
echo "select(name)" | libdplyr --debug --verbose
```

### 파이프라인 사용 예시

```bash
# 여러 도구와 연계
cat data_analysis.R | libdplyr -d postgresql | psql -d mydb

# 조건부 처리
if echo "select(invalid)" | libdplyr --validate-only; then
    echo "유효한 문법입니다"
else
    echo "문법 오류가 있습니다"
fi

# 배치 처리
find . -name "*.R" -exec sh -c 'libdplyr -i "$1" -o "${1%.R}.sql"' _ {} \;
```
```

### Future Extensions

- 스트리밍 모드: 여러 dplyr 쿼리를 연속 처리
- 배치 모드: 여러 입력을 한 번에 처리
- 인터랙티브 모드: REPL 스타일 인터페이스
- 플러그인 시스템: 사용자 정의 출력 형식
- 패키지 매니저 지원: Homebrew, Chocolatey, APT 등