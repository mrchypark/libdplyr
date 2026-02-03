//! Performance monitoring and optimization utilities
//!
//! This module provides tools for monitoring performance characteristics
//! and identifying optimization opportunities in the libdplyr transpiler.

use crate::{SqlDialect, Transpiler};
use std::time::{Duration, Instant};

/// Performance metrics for a single transpilation operation
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total time taken for the operation
    pub total_time: Duration,
    /// Time spent in lexical analysis
    pub lexing_time: Option<Duration>,
    /// Time spent in parsing
    pub parsing_time: Option<Duration>,
    /// Time spent in SQL generation
    pub generation_time: Option<Duration>,
    /// Input size in characters
    pub input_size: usize,
    /// Output size in characters
    pub output_size: usize,
    /// Whether the operation was successful
    pub success: bool,
    /// Error type if operation failed
    pub error_type: Option<String>,
}

impl PerformanceMetrics {
    /// Calculate throughput in characters per second
    pub fn throughput(&self) -> f64 {
        if self.total_time.as_secs_f64() > 0.0 {
            self.input_size as f64 / self.total_time.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Calculate efficiency ratio (output size / input size)
    pub fn efficiency_ratio(&self) -> f64 {
        if self.input_size > 0 {
            self.output_size as f64 / self.input_size as f64
        } else {
            0.0
        }
    }

    /// Get the bottleneck stage (stage that takes the most time)
    pub fn bottleneck_stage(&self) -> Option<&'static str> {
        let lexing = self.lexing_time.unwrap_or(Duration::ZERO);
        let parsing = self.parsing_time.unwrap_or(Duration::ZERO);
        let generation = self.generation_time.unwrap_or(Duration::ZERO);

        if lexing >= parsing && lexing >= generation {
            Some("lexing")
        } else if parsing >= generation {
            Some("parsing")
        } else if generation > Duration::ZERO {
            Some("generation")
        } else {
            None
        }
    }
}

/// Performance profiler for transpilation operations
pub struct PerformanceProfiler {
    transpiler: Transpiler,
    enable_detailed_timing: bool,
}

impl PerformanceProfiler {
    /// Create a new performance profiler
    pub fn new(dialect: Box<dyn SqlDialect>) -> Self {
        Self {
            transpiler: Transpiler::new(dialect),
            enable_detailed_timing: false,
        }
    }

    /// Enable detailed timing for individual stages
    pub const fn enable_detailed_timing(&mut self) {
        self.enable_detailed_timing = true;
    }

    /// Profile a single transpilation operation
    pub fn profile_transpile(&self, input: &str) -> PerformanceMetrics {
        let start_time = Instant::now();
        let input_size = input.len();

        let lexing_time = None;
        let mut parsing_time = None;
        let mut generation_time = None;

        let result = if self.enable_detailed_timing {
            // Detailed timing - measure each stage separately
            let lex_start = Instant::now();
            let parse_result = self.transpiler.parse_dplyr(input);
            let parse_end = Instant::now();
            parsing_time = Some(parse_end.duration_since(lex_start));

            match parse_result {
                Ok(ast) => {
                    let gen_start = Instant::now();
                    let gen_result = self.transpiler.generate_sql(&ast);
                    let gen_end = Instant::now();
                    generation_time = Some(gen_end.duration_since(gen_start));
                    gen_result.map_err(|e| e.into())
                }
                Err(e) => Err(e.into()),
            }
        } else {
            // Simple timing - measure total time only
            self.transpiler.transpile(input)
        };

        let total_time = start_time.elapsed();

        let (success, output_size, error_type) = match result {
            Ok(sql) => (true, sql.len(), None),
            Err(e) => (false, 0, Some(format!("{e:?}"))),
        };

        PerformanceMetrics {
            total_time,
            lexing_time,
            parsing_time,
            generation_time,
            input_size,
            output_size,
            success,
            error_type,
        }
    }

    /// Profile multiple operations and return aggregate statistics
    pub fn profile_batch(&self, inputs: &[&str]) -> BatchPerformanceStats {
        let mut metrics = Vec::new();

        for input in inputs {
            metrics.push(self.profile_transpile(input));
        }

        BatchPerformanceStats::new(metrics)
    }
}

/// Aggregate performance statistics for a batch of operations
#[derive(Debug)]
pub struct BatchPerformanceStats {
    pub metrics: Vec<PerformanceMetrics>,
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub avg_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
    pub avg_throughput: f64,
    pub total_input_size: usize,
    pub total_output_size: usize,
}

impl BatchPerformanceStats {
    fn new(metrics: Vec<PerformanceMetrics>) -> Self {
        let total_operations = metrics.len();
        let successful_operations = metrics.iter().filter(|m| m.success).count();
        let failed_operations = total_operations - successful_operations;

        let times: Vec<Duration> = metrics.iter().map(|m| m.total_time).collect();
        let avg_time = if !times.is_empty() {
            times.iter().sum::<Duration>() / times.len() as u32
        } else {
            Duration::ZERO
        };

        let min_time = times.iter().min().copied().unwrap_or(Duration::ZERO);
        let max_time = times.iter().max().copied().unwrap_or(Duration::ZERO);

        let throughputs: Vec<f64> = metrics.iter().map(|m| m.throughput()).collect();
        let avg_throughput = if !throughputs.is_empty() {
            throughputs.iter().sum::<f64>() / throughputs.len() as f64
        } else {
            0.0
        };

        let total_input_size = metrics.iter().map(|m| m.input_size).sum();
        let total_output_size = metrics.iter().map(|m| m.output_size).sum();

        Self {
            metrics,
            total_operations,
            successful_operations,
            failed_operations,
            avg_time,
            min_time,
            max_time,
            avg_throughput,
            total_input_size,
            total_output_size,
        }
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_operations > 0 {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Get the most common bottleneck stage
    pub fn common_bottleneck(&self) -> Option<&'static str> {
        let mut bottleneck_counts = std::collections::HashMap::new();

        for metric in &self.metrics {
            if let Some(bottleneck) = metric.bottleneck_stage() {
                *bottleneck_counts.entry(bottleneck).or_insert(0) += 1;
            }
        }

        bottleneck_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(stage, _)| stage)
    }

    /// Generate performance optimization recommendations
    pub fn optimization_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check success rate
        if self.success_rate() < 95.0 {
            recommendations.push(format!(
                "Success rate is {:.1}%. Consider improving error handling and input validation.",
                self.success_rate()
            ));
        }

        // Check average throughput
        if self.avg_throughput < 1000.0 {
            recommendations.push(format!(
                "Average throughput is {:.0} chars/sec. Consider optimizing parsing algorithms.",
                self.avg_throughput
            ));
        }

        // Check for bottlenecks
        if let Some(bottleneck) = self.common_bottleneck() {
            match bottleneck {
                "lexing" => recommendations.push(
                    "Lexing is the main bottleneck. Consider optimizing tokenization algorithms or using lookup tables.".to_string()
                ),
                "parsing" => recommendations.push(
                    "Parsing is the main bottleneck. Consider optimizing AST construction or reducing recursive calls.".to_string()
                ),
                "generation" => recommendations.push(
                    "SQL generation is the main bottleneck. Consider optimizing string building or caching dialect-specific patterns.".to_string()
                ),
                _ => {}
            }
        }

        // Check time variance
        let time_variance = self.max_time.as_secs_f64() / self.min_time.as_secs_f64().max(0.001);
        if time_variance > 10.0 {
            recommendations.push(format!(
                "High time variance detected ({}x difference). Consider investigating input-dependent performance issues.",
                time_variance as u32
            ));
        }

        // Check memory efficiency
        let avg_efficiency = self.total_output_size as f64 / self.total_input_size.max(1) as f64;
        if avg_efficiency > 5.0 {
            recommendations.push(format!(
                "Output is {avg_efficiency:.1}x larger than input on average. Consider optimizing SQL generation for conciseness."
            ));
        }

        if recommendations.is_empty() {
            recommendations
                .push("Performance looks good! No specific optimizations recommended.".to_string());
        }

        recommendations
    }
}

/// Performance regression detector
pub struct RegressionDetector {
    baseline_stats: Option<BatchPerformanceStats>,
}

impl RegressionDetector {
    /// Create a new regression detector
    pub const fn new() -> Self {
        Self {
            baseline_stats: None,
        }
    }
}

impl Default for RegressionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegressionDetector {
    /// Set baseline performance statistics
    pub fn set_baseline(&mut self, stats: BatchPerformanceStats) {
        self.baseline_stats = Some(stats);
    }

    /// Check for performance regressions compared to baseline
    pub fn check_regression(&self, current_stats: &BatchPerformanceStats) -> Vec<String> {
        let mut regressions = Vec::new();

        if let Some(baseline) = &self.baseline_stats {
            // Check average time regression
            let time_ratio =
                current_stats.avg_time.as_secs_f64() / baseline.avg_time.as_secs_f64().max(0.001);
            if time_ratio > 1.1 {
                regressions.push(format!(
                    "Average time increased by {:.1}% ({:.3}ms -> {:.3}ms)",
                    (time_ratio - 1.0) * 100.0,
                    baseline.avg_time.as_secs_f64() * 1000.0,
                    current_stats.avg_time.as_secs_f64() * 1000.0
                ));
            }

            // Check throughput regression
            let throughput_ratio =
                current_stats.avg_throughput / baseline.avg_throughput.max(0.001);
            if throughput_ratio < 0.9 {
                regressions.push(format!(
                    "Average throughput decreased by {:.1}% ({:.0} -> {:.0} chars/sec)",
                    (1.0 - throughput_ratio) * 100.0,
                    baseline.avg_throughput,
                    current_stats.avg_throughput
                ));
            }

            // Check success rate regression
            let success_diff = current_stats.success_rate() - baseline.success_rate();
            if success_diff < -1.0 {
                regressions.push(format!(
                    "Success rate decreased by {:.1}% ({:.1}% -> {:.1}%)",
                    -success_diff,
                    baseline.success_rate(),
                    current_stats.success_rate()
                ));
            }
        }

        if regressions.is_empty() {
            regressions.push("No significant performance regressions detected.".to_string());
        }

        regressions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PostgreSqlDialect;

    #[test]
    fn test_performance_profiler_basic() {
        let profiler = PerformanceProfiler::new(Box::new(PostgreSqlDialect));
        let metrics = profiler.profile_transpile("select(name, age)");

        assert!(metrics.success);
        assert!(metrics.total_time > Duration::ZERO);
        assert_eq!(metrics.input_size, "select(name, age)".len());
        assert!(metrics.output_size > 0);
        assert!(metrics.throughput() > 0.0);
    }

    #[test]
    fn test_batch_performance_stats() {
        let profiler = PerformanceProfiler::new(Box::new(PostgreSqlDialect));
        let inputs = vec!["select(name)", "select(age)", "filter(age > 18)"];

        let stats = profiler.profile_batch(&inputs);

        assert_eq!(stats.total_operations, 3);
        assert_eq!(stats.successful_operations, 3);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.success_rate(), 100.0);
        assert!(stats.avg_throughput > 0.0);
    }

    #[test]
    fn test_performance_metrics_calculations() {
        let metrics = PerformanceMetrics {
            total_time: Duration::from_millis(10),
            lexing_time: Some(Duration::from_millis(3)),
            parsing_time: Some(Duration::from_millis(4)),
            generation_time: Some(Duration::from_millis(3)),
            input_size: 100,
            output_size: 200,
            success: true,
            error_type: None,
        };

        assert_eq!(metrics.throughput(), 10000.0); // 100 chars / 0.01 sec
        assert_eq!(metrics.efficiency_ratio(), 2.0); // 200 / 100
        assert_eq!(metrics.bottleneck_stage(), Some("parsing"));
    }

    #[test]
    fn test_regression_detector() {
        let mut detector = RegressionDetector::new();

        // Create baseline stats
        let baseline_metrics = vec![PerformanceMetrics {
            total_time: Duration::from_millis(5),
            lexing_time: None,
            parsing_time: None,
            generation_time: None,
            input_size: 50,
            output_size: 100,
            success: true,
            error_type: None,
        }];
        let baseline_stats = BatchPerformanceStats::new(baseline_metrics);
        detector.set_baseline(baseline_stats);

        // Create current stats (slower)
        let current_metrics = vec![PerformanceMetrics {
            total_time: Duration::from_millis(8),
            lexing_time: None,
            parsing_time: None,
            generation_time: None,
            input_size: 50,
            output_size: 100,
            success: true,
            error_type: None,
        }];
        let current_stats = BatchPerformanceStats::new(current_metrics);

        let regressions = detector.check_regression(&current_stats);
        assert!(!regressions.is_empty());
        assert!(regressions[0].contains("Average time increased"));
    }
}
