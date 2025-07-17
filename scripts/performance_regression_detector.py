#!/usr/bin/env python3
"""
Performance Regression Detection System
Analyzes benchmark results and detects performance regressions with detailed reporting.
"""

import json
import sys
import os
import argparse
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass
from pathlib import Path
import statistics


@dataclass
class BenchmarkResult:
    """Represents a single benchmark result."""
    name: str
    time_ns: float
    confidence_interval: Optional[Tuple[float, float]] = None
    throughput: Optional[float] = None
    
    @property
    def time_ms(self) -> float:
        """Convert nanoseconds to milliseconds."""
        return self.time_ns / 1_000_000
    
    @property
    def time_us(self) -> float:
        """Convert nanoseconds to microseconds."""
        return self.time_ns / 1_000


@dataclass
class RegressionResult:
    """Represents a performance regression analysis result."""
    benchmark_name: str
    current_time_ms: float
    baseline_time_ms: float
    change_percent: float
    is_regression: bool
    is_improvement: bool
    severity: str  # 'minor', 'moderate', 'severe'
    
    @property
    def change_description(self) -> str:
        """Get human-readable change description."""
        if self.is_regression:
            return f"{abs(self.change_percent):.1f}% slower"
        elif self.is_improvement:
            return f"{abs(self.change_percent):.1f}% faster"
        else:
            return f"{self.change_percent:+.1f}% change"


class PerformanceRegressionDetector:
    """Detects performance regressions in benchmark results."""
    
    def __init__(self, 
                 regression_threshold: float = 15.0,
                 improvement_threshold: float = 10.0,
                 severe_threshold: float = 50.0):
        """
        Initialize the regression detector.
        
        Args:
            regression_threshold: Percentage threshold for detecting regressions
            improvement_threshold: Percentage threshold for detecting improvements
            severe_threshold: Percentage threshold for severe regressions
        """
        self.regression_threshold = regression_threshold
        self.improvement_threshold = improvement_threshold
        self.severe_threshold = severe_threshold
    
    def parse_benchmark_results(self, results_file: Path) -> Dict[str, BenchmarkResult]:
        """Parse benchmark results from JSON file."""
        benchmarks = {}
        
        if not results_file.exists():
            return benchmarks
        
        try:
            with open(results_file, 'r') as f:
                lines = f.readlines()
            
            for line in lines:
                try:
                    data = json.loads(line.strip())
                    if data.get('reason') == 'benchmark-complete':
                        name = data.get('id', 'Unknown')
                        
                        # Extract timing information
                        time_ns = None
                        confidence_interval = None
                        throughput = None
                        
                        if 'typical' in data:
                            time_ns = data['typical']['estimate']
                            if 'confidence_interval' in data['typical']:
                                ci = data['typical']['confidence_interval']
                                confidence_interval = (ci['lower_bound'], ci['upper_bound'])
                        elif 'mean' in data:
                            time_ns = data['mean']['estimate']
                            if 'confidence_interval' in data['mean']:
                                ci = data['mean']['confidence_interval']
                                confidence_interval = (ci['lower_bound'], ci['upper_bound'])
                        
                        if 'throughput' in data:
                            throughput = data['throughput'].get('per_iteration', 0)
                        
                        if time_ns is not None:
                            benchmarks[name] = BenchmarkResult(
                                name=name,
                                time_ns=time_ns,
                                confidence_interval=confidence_interval,
                                throughput=throughput
                            )
                
                except (json.JSONDecodeError, KeyError):
                    continue
        
        except Exception as e:
            print(f"Error parsing benchmark results: {e}", file=sys.stderr)
        
        return benchmarks
    
    def analyze_regressions(self, 
                          current_results: Dict[str, BenchmarkResult],
                          baseline_results: Dict[str, BenchmarkResult]) -> List[RegressionResult]:
        """Analyze performance regressions between current and baseline results."""
        regressions = []
        
        for name, current in current_results.items():
            if name not in baseline_results:
                continue
            
            baseline = baseline_results[name]
            
            if baseline.time_ns <= 0:
                continue
            
            change_percent = ((current.time_ns - baseline.time_ns) / baseline.time_ns) * 100
            
            is_regression = change_percent > self.regression_threshold
            is_improvement = change_percent < -self.improvement_threshold
            
            # Determine severity
            severity = 'minor'
            if abs(change_percent) > self.severe_threshold:
                severity = 'severe'
            elif abs(change_percent) > self.regression_threshold * 2:
                severity = 'moderate'
            
            if is_regression or is_improvement or abs(change_percent) > 5:  # Include notable changes
                regressions.append(RegressionResult(
                    benchmark_name=name,
                    current_time_ms=current.time_ms,
                    baseline_time_ms=baseline.time_ms,
                    change_percent=change_percent,
                    is_regression=is_regression,
                    is_improvement=is_improvement,
                    severity=severity
                ))
        
        return sorted(regressions, key=lambda x: abs(x.change_percent), reverse=True)
    
    def generate_report(self, 
                       current_results: Dict[str, BenchmarkResult],
                       baseline_results: Dict[str, BenchmarkResult],
                       regressions: List[RegressionResult]) -> str:
        """Generate a detailed performance regression report."""
        report = []
        report.append("# Performance Regression Analysis Report")
        report.append("")
        
        # Summary statistics
        total_benchmarks = len(current_results)
        regression_count = sum(1 for r in regressions if r.is_regression)
        improvement_count = sum(1 for r in regressions if r.is_improvement)
        severe_count = sum(1 for r in regressions if r.severity == 'severe')
        
        report.append("## Executive Summary")
        report.append("")
        report.append(f"- **Total Benchmarks**: {total_benchmarks}")
        report.append(f"- **Performance Regressions**: {regression_count}")
        report.append(f"- **Performance Improvements**: {improvement_count}")
        report.append(f"- **Severe Issues**: {severe_count}")
        report.append("")
        
        # Overall status
        if severe_count > 0:
            report.append("🚨 **Status**: CRITICAL - Severe performance regressions detected")
        elif regression_count > 0:
            report.append("⚠️ **Status**: WARNING - Performance regressions detected")
        elif improvement_count > 0:
            report.append("✅ **Status**: GOOD - Performance improvements detected")
        else:
            report.append("📊 **Status**: STABLE - No significant performance changes")
        report.append("")
        
        # Current performance metrics
        if current_results:
            report.append("## Current Performance Metrics")
            report.append("")
            sorted_current = sorted(current_results.items(), key=lambda x: x[1].time_ns)
            
            # Show top 10 fastest and slowest benchmarks
            report.append("### Fastest Benchmarks")
            for name, result in sorted_current[:10]:
                report.append(f"- **{name}**: {result.time_ms:.3f}ms")
            report.append("")
            
            if len(sorted_current) > 10:
                report.append("### Slowest Benchmarks")
                for name, result in sorted_current[-10:]:
                    report.append(f"- **{name}**: {result.time_ms:.3f}ms")
                report.append("")
        
        # Regression analysis
        if regressions:
            # Severe regressions
            severe_regressions = [r for r in regressions if r.severity == 'severe' and r.is_regression]
            if severe_regressions:
                report.append("## 🚨 Severe Performance Regressions")
                report.append("")
                for reg in severe_regressions:
                    report.append(f"- **{reg.benchmark_name}**: {reg.change_description}")
                    report.append(f"  - Current: {reg.current_time_ms:.3f}ms")
                    report.append(f"  - Baseline: {reg.baseline_time_ms:.3f}ms")
                report.append("")
            
            # Regular regressions
            regular_regressions = [r for r in regressions if r.is_regression and r.severity != 'severe']
            if regular_regressions:
                report.append("## ⚠️ Performance Regressions")
                report.append("")
                for reg in regular_regressions:
                    report.append(f"- **{reg.benchmark_name}**: {reg.change_description}")
                    report.append(f"  - Current: {reg.current_time_ms:.3f}ms")
                    report.append(f"  - Baseline: {reg.baseline_time_ms:.3f}ms")
                report.append("")
            
            # Improvements
            improvements = [r for r in regressions if r.is_improvement]
            if improvements:
                report.append("## ✅ Performance Improvements")
                report.append("")
                for reg in improvements:
                    report.append(f"- **{reg.benchmark_name}**: {reg.change_description}")
                    report.append(f"  - Current: {reg.current_time_ms:.3f}ms")
                    report.append(f"  - Baseline: {reg.baseline_time_ms:.3f}ms")
                report.append("")
        
        # Configuration
        report.append("## Analysis Configuration")
        report.append("")
        report.append(f"- **Regression Threshold**: {self.regression_threshold}%")
        report.append(f"- **Improvement Threshold**: {self.improvement_threshold}%")
        report.append(f"- **Severe Threshold**: {self.severe_threshold}%")
        report.append(f"- **Analysis Timestamp**: {os.popen('date').read().strip()}")
        
        return "\n".join(report)
    
    def set_github_outputs(self, regressions: List[RegressionResult]) -> None:
        """Set GitHub Actions outputs for CI integration."""
        regression_count = sum(1 for r in regressions if r.is_regression)
        severe_count = sum(1 for r in regressions if r.severity == 'severe')
        improvement_count = sum(1 for r in regressions if r.is_improvement)
        
        # Set outputs for GitHub Actions
        print(f"::set-output name=regression_count::{regression_count}")
        print(f"::set-output name=severe_count::{severe_count}")
        print(f"::set-output name=improvement_count::{improvement_count}")
        
        # Set warnings and errors
        if severe_count > 0:
            print(f"::error::Severe performance regressions detected in {severe_count} benchmarks")
            for reg in regressions:
                if reg.severity == 'severe' and reg.is_regression:
                    print(f"::error::{reg.benchmark_name} is {abs(reg.change_percent):.1f}% slower")
        elif regression_count > 0:
            print(f"::warning::Performance regressions detected in {regression_count} benchmarks")
            for reg in regressions:
                if reg.is_regression:
                    print(f"::warning::{reg.benchmark_name} is {abs(reg.change_percent):.1f}% slower")
        
        # Highlight improvements
        if improvement_count > 0:
            print(f"::notice::Performance improvements detected in {improvement_count} benchmarks")


def main():
    """Main entry point for the performance regression detector."""
    parser = argparse.ArgumentParser(description="Detect performance regressions in benchmark results")
    parser.add_argument("current_results", type=Path, help="Path to current benchmark results JSON file")
    parser.add_argument("--baseline", type=Path, help="Path to baseline benchmark results JSON file")
    parser.add_argument("--output", type=Path, help="Path to output report file")
    parser.add_argument("--regression-threshold", type=float, default=15.0, 
                       help="Percentage threshold for detecting regressions (default: 15.0)")
    parser.add_argument("--improvement-threshold", type=float, default=10.0,
                       help="Percentage threshold for detecting improvements (default: 10.0)")
    parser.add_argument("--severe-threshold", type=float, default=50.0,
                       help="Percentage threshold for severe regressions (default: 50.0)")
    parser.add_argument("--github-actions", action="store_true",
                       help="Enable GitHub Actions integration (set outputs and annotations)")
    
    args = parser.parse_args()
    
    # Initialize detector
    detector = PerformanceRegressionDetector(
        regression_threshold=args.regression_threshold,
        improvement_threshold=args.improvement_threshold,
        severe_threshold=args.severe_threshold
    )
    
    # Parse current results
    current_results = detector.parse_benchmark_results(args.current_results)
    if not current_results:
        print("No current benchmark results found", file=sys.stderr)
        sys.exit(1)
    
    # Parse baseline results if available
    baseline_results = {}
    if args.baseline and args.baseline.exists():
        baseline_results = detector.parse_benchmark_results(args.baseline)
    
    # Analyze regressions
    regressions = detector.analyze_regressions(current_results, baseline_results)
    
    # Generate report
    report = detector.generate_report(current_results, baseline_results, regressions)
    
    # Output report
    if args.output:
        with open(args.output, 'w') as f:
            f.write(report)
        print(f"Report written to {args.output}")
    else:
        print(report)
    
    # GitHub Actions integration
    if args.github_actions:
        detector.set_github_outputs(regressions)
    
    # Exit with appropriate code
    severe_count = sum(1 for r in regressions if r.severity == 'severe' and r.is_regression)
    if severe_count > 0:
        sys.exit(2)  # Severe regressions
    
    regression_count = sum(1 for r in regressions if r.is_regression)
    if regression_count > 0:
        sys.exit(1)  # Regular regressions
    
    sys.exit(0)  # No regressions


if __name__ == "__main__":
    main()