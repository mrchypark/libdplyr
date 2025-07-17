#!/usr/bin/env python3
"""
CI Monitoring and Metrics Collection System
Collects and reports CI execution metrics, performance data, and resource usage.
"""

import json
import os
import sys
import time
import argparse
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, asdict
import subprocess


@dataclass
class JobMetrics:
    """Metrics for a single CI job."""
    job_name: str
    start_time: str
    end_time: Optional[str] = None
    duration_seconds: Optional[float] = None
    status: str = "running"
    steps: List[Dict[str, Any]] = None
    resource_usage: Dict[str, Any] = None
    
    def __post_init__(self):
        if self.steps is None:
            self.steps = []
        if self.resource_usage is None:
            self.resource_usage = {}


@dataclass
class WorkflowMetrics:
    """Metrics for an entire workflow run."""
    workflow_name: str
    run_id: str
    run_number: int
    trigger_event: str
    branch: str
    commit_sha: str
    start_time: str
    end_time: Optional[str] = None
    total_duration_seconds: Optional[float] = None
    status: str = "running"
    jobs: List[JobMetrics] = None
    
    def __post_init__(self):
        if self.jobs is None:
            self.jobs = []


class CIMonitor:
    """CI monitoring and metrics collection system."""
    
    def __init__(self, output_dir: Path = Path("ci-metrics")):
        """Initialize CI monitor."""
        self.output_dir = output_dir
        self.output_dir.mkdir(exist_ok=True)
        self.start_time = datetime.now(timezone.utc)
    
    def get_github_context(self) -> Dict[str, str]:
        """Extract GitHub Actions context information."""
        return {
            "workflow": os.getenv("GITHUB_WORKFLOW", "unknown"),
            "run_id": os.getenv("GITHUB_RUN_ID", "unknown"),
            "run_number": os.getenv("GITHUB_RUN_NUMBER", "0"),
            "event": os.getenv("GITHUB_EVENT_NAME", "unknown"),
            "branch": os.getenv("GITHUB_REF_NAME", "unknown"),
            "commit": os.getenv("GITHUB_SHA", "unknown")[:8],
            "actor": os.getenv("GITHUB_ACTOR", "unknown"),
            "repository": os.getenv("GITHUB_REPOSITORY", "unknown")
        }
    
    def collect_system_metrics(self) -> Dict[str, Any]:
        """Collect system resource usage metrics."""
        metrics = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "cpu_info": {},
            "memory_info": {},
            "disk_info": {},
            "network_info": {}
        }
        
        try:
            # CPU information
            if os.path.exists("/proc/cpuinfo"):
                with open("/proc/cpuinfo", "r") as f:
                    cpu_lines = f.readlines()
                    cpu_count = len([line for line in cpu_lines if line.startswith("processor")])
                    metrics["cpu_info"]["cores"] = cpu_count
            
            # Memory information
            if os.path.exists("/proc/meminfo"):
                with open("/proc/meminfo", "r") as f:
                    mem_lines = f.readlines()
                    for line in mem_lines:
                        if line.startswith("MemTotal:"):
                            metrics["memory_info"]["total_kb"] = int(line.split()[1])
                        elif line.startswith("MemAvailable:"):
                            metrics["memory_info"]["available_kb"] = int(line.split()[1])
            
            # Disk usage
            result = subprocess.run(["df", "-h", "."], capture_output=True, text=True)
            if result.returncode == 0:
                lines = result.stdout.strip().split("\n")
                if len(lines) > 1:
                    parts = lines[1].split()
                    if len(parts) >= 4:
                        metrics["disk_info"]["total"] = parts[1]
                        metrics["disk_info"]["used"] = parts[2]
                        metrics["disk_info"]["available"] = parts[3]
                        metrics["disk_info"]["use_percent"] = parts[4]
        
        except Exception as e:
            metrics["error"] = f"Failed to collect system metrics: {e}"
        
        return metrics
    
    def measure_step_performance(self, step_name: str, command: List[str]) -> Dict[str, Any]:
        """Measure performance of a CI step."""
        start_time = time.time()
        start_timestamp = datetime.now(timezone.utc).isoformat()
        
        try:
            # Run the command and measure execution time
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=3600  # 1 hour timeout
            )
            
            end_time = time.time()
            duration = end_time - start_time
            
            return {
                "step_name": step_name,
                "command": " ".join(command),
                "start_time": start_timestamp,
                "end_time": datetime.now(timezone.utc).isoformat(),
                "duration_seconds": duration,
                "exit_code": result.returncode,
                "success": result.returncode == 0,
                "stdout_lines": len(result.stdout.splitlines()) if result.stdout else 0,
                "stderr_lines": len(result.stderr.splitlines()) if result.stderr else 0,
                "system_metrics": self.collect_system_metrics()
            }
        
        except subprocess.TimeoutExpired:
            return {
                "step_name": step_name,
                "command": " ".join(command),
                "start_time": start_timestamp,
                "end_time": datetime.now(timezone.utc).isoformat(),
                "duration_seconds": time.time() - start_time,
                "exit_code": -1,
                "success": False,
                "error": "Command timed out after 1 hour",
                "system_metrics": self.collect_system_metrics()
            }
        
        except Exception as e:
            return {
                "step_name": step_name,
                "command": " ".join(command),
                "start_time": start_timestamp,
                "end_time": datetime.now(timezone.utc).isoformat(),
                "duration_seconds": time.time() - start_time,
                "exit_code": -1,
                "success": False,
                "error": str(e),
                "system_metrics": self.collect_system_metrics()
            }
    
    def generate_workflow_summary(self, metrics_file: Path) -> str:
        """Generate a workflow execution summary."""
        if not metrics_file.exists():
            return "No metrics data available."
        
        try:
            with open(metrics_file, 'r') as f:
                data = json.load(f)
            
            github_context = data.get("github_context", {})
            steps = data.get("steps", [])
            
            # Calculate summary statistics
            total_duration = sum(step.get("duration_seconds", 0) for step in steps)
            successful_steps = sum(1 for step in steps if step.get("success", False))
            failed_steps = len(steps) - successful_steps
            
            # Find slowest steps
            slowest_steps = sorted(steps, key=lambda x: x.get("duration_seconds", 0), reverse=True)[:5]
            
            summary = []
            summary.append("# CI Workflow Execution Summary")
            summary.append("")
            summary.append("## Workflow Information")
            summary.append(f"- **Workflow**: {github_context.get('workflow', 'Unknown')}")
            summary.append(f"- **Run ID**: {github_context.get('run_id', 'Unknown')}")
            summary.append(f"- **Run Number**: {github_context.get('run_number', 'Unknown')}")
            summary.append(f"- **Trigger**: {github_context.get('event', 'Unknown')}")
            summary.append(f"- **Branch**: {github_context.get('branch', 'Unknown')}")
            summary.append(f"- **Commit**: {github_context.get('commit', 'Unknown')}")
            summary.append(f"- **Actor**: {github_context.get('actor', 'Unknown')}")
            summary.append("")
            
            summary.append("## Execution Summary")
            summary.append(f"- **Total Steps**: {len(steps)}")
            summary.append(f"- **Successful Steps**: {successful_steps}")
            summary.append(f"- **Failed Steps**: {failed_steps}")
            summary.append(f"- **Total Duration**: {total_duration:.2f} seconds ({total_duration/60:.1f} minutes)")
            summary.append(f"- **Average Step Duration**: {total_duration/len(steps):.2f} seconds" if steps else "- **Average Step Duration**: N/A")
            summary.append("")
            
            if slowest_steps:
                summary.append("## Slowest Steps")
                for i, step in enumerate(slowest_steps, 1):
                    duration = step.get("duration_seconds", 0)
                    status = "✅" if step.get("success", False) else "❌"
                    summary.append(f"{i}. {status} **{step.get('step_name', 'Unknown')}**: {duration:.2f}s")
                summary.append("")
            
            if failed_steps > 0:
                summary.append("## Failed Steps")
                for step in steps:
                    if not step.get("success", False):
                        error = step.get("error", "Unknown error")
                        summary.append(f"- ❌ **{step.get('step_name', 'Unknown')}**: {error}")
                summary.append("")
            
            # Resource usage summary
            if steps and steps[0].get("system_metrics"):
                latest_metrics = steps[-1]["system_metrics"]
                summary.append("## Resource Usage")
                
                if "memory_info" in latest_metrics:
                    mem_info = latest_metrics["memory_info"]
                    if "total_kb" in mem_info and "available_kb" in mem_info:
                        total_mb = mem_info["total_kb"] / 1024
                        available_mb = mem_info["available_kb"] / 1024
                        used_mb = total_mb - available_mb
                        usage_percent = (used_mb / total_mb) * 100
                        summary.append(f"- **Memory**: {used_mb:.0f}MB / {total_mb:.0f}MB ({usage_percent:.1f}% used)")
                
                if "disk_info" in latest_metrics:
                    disk_info = latest_metrics["disk_info"]
                    if "use_percent" in disk_info:
                        summary.append(f"- **Disk Usage**: {disk_info.get('used', 'Unknown')} / {disk_info.get('total', 'Unknown')} ({disk_info['use_percent']})")
                
                if "cpu_info" in latest_metrics:
                    cpu_info = latest_metrics["cpu_info"]
                    if "cores" in cpu_info:
                        summary.append(f"- **CPU Cores**: {cpu_info['cores']}")
                
                summary.append("")
            
            summary.append("## Recommendations")
            if total_duration > 1800:  # 30 minutes
                summary.append("- ⚠️ Workflow duration is quite long. Consider optimizing slow steps or using parallel execution.")
            if failed_steps > 0:
                summary.append("- ❌ Some steps failed. Review the error messages and fix the issues.")
            if successful_steps == len(steps):
                summary.append("- ✅ All steps completed successfully!")
            
            summary.append("")
            summary.append(f"## Generated at")
            summary.append(f"{datetime.now(timezone.utc).isoformat()}")
            
            return "\n".join(summary)
        
        except Exception as e:
            return f"Error generating summary: {e}"
    
    def save_metrics(self, data: Dict[str, Any], filename: str = None) -> Path:
        """Save metrics data to JSON file."""
        if filename is None:
            timestamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
            filename = f"ci_metrics_{timestamp}.json"
        
        output_file = self.output_dir / filename
        
        with open(output_file, 'w') as f:
            json.dump(data, f, indent=2, default=str)
        
        return output_file
    
    def create_github_summary(self, metrics_file: Path) -> None:
        """Create GitHub Actions job summary."""
        summary = self.generate_workflow_summary(metrics_file)
        
        # Write to GitHub Actions summary if available
        summary_file = os.getenv("GITHUB_STEP_SUMMARY")
        if summary_file:
            with open(summary_file, "a") as f:
                f.write(summary)
                f.write("\n")
        
        # Also save as separate file
        summary_path = self.output_dir / "workflow_summary.md"
        with open(summary_path, "w") as f:
            f.write(summary)
        
        print("::notice::CI monitoring summary generated")


def main():
    """Main entry point for CI monitoring."""
    parser = argparse.ArgumentParser(description="CI Monitoring and Metrics Collection")
    parser.add_argument("--mode", choices=["start", "step", "end", "summary"], 
                       required=True, help="Monitoring mode")
    parser.add_argument("--step-name", help="Name of the step being monitored")
    parser.add_argument("--command", nargs="+", help="Command to execute and monitor")
    parser.add_argument("--output-dir", type=Path, default=Path("ci-metrics"),
                       help="Output directory for metrics")
    parser.add_argument("--metrics-file", type=Path, help="Existing metrics file to update")
    
    args = parser.parse_args()
    
    monitor = CIMonitor(args.output_dir)
    
    if args.mode == "start":
        # Initialize workflow monitoring
        github_context = monitor.get_github_context()
        system_metrics = monitor.collect_system_metrics()
        
        data = {
            "workflow_start": datetime.now(timezone.utc).isoformat(),
            "github_context": github_context,
            "initial_system_metrics": system_metrics,
            "steps": []
        }
        
        metrics_file = monitor.save_metrics(data, "current_workflow_metrics.json")
        print(f"::set-output name=metrics_file::{metrics_file}")
        print("::notice::CI monitoring started")
    
    elif args.mode == "step":
        if not args.step_name or not args.command:
            print("::error::Step name and command are required for step monitoring")
            sys.exit(1)
        
        # Monitor a specific step
        step_metrics = monitor.measure_step_performance(args.step_name, args.command)
        
        # Load existing metrics and add this step
        metrics_file = args.metrics_file or (args.output_dir / "current_workflow_metrics.json")
        
        if metrics_file.exists():
            with open(metrics_file, 'r') as f:
                data = json.load(f)
        else:
            data = {"steps": []}
        
        data["steps"].append(step_metrics)
        
        # Save updated metrics
        monitor.save_metrics(data, metrics_file.name)
        
        if step_metrics["success"]:
            print(f"::notice::Step '{args.step_name}' completed in {step_metrics['duration_seconds']:.2f}s")
        else:
            print(f"::error::Step '{args.step_name}' failed: {step_metrics.get('error', 'Unknown error')}")
    
    elif args.mode == "end":
        # Finalize workflow monitoring
        metrics_file = args.metrics_file or (args.output_dir / "current_workflow_metrics.json")
        
        if metrics_file.exists():
            with open(metrics_file, 'r') as f:
                data = json.load(f)
            
            data["workflow_end"] = datetime.now(timezone.utc).isoformat()
            data["final_system_metrics"] = monitor.collect_system_metrics()
            
            # Calculate total workflow duration
            if "workflow_start" in data:
                start_time = datetime.fromisoformat(data["workflow_start"].replace('Z', '+00:00'))
                end_time = datetime.fromisoformat(data["workflow_end"].replace('Z', '+00:00'))
                data["total_duration_seconds"] = (end_time - start_time).total_seconds()
            
            # Save final metrics
            final_file = monitor.save_metrics(data, f"workflow_metrics_{data['github_context']['run_id']}.json")
            
            # Generate summary
            monitor.create_github_summary(final_file)
            
            print("::notice::CI monitoring completed")
        else:
            print("::warning::No metrics file found to finalize")
    
    elif args.mode == "summary":
        # Generate summary from existing metrics
        metrics_file = args.metrics_file or (args.output_dir / "current_workflow_metrics.json")
        
        if metrics_file.exists():
            monitor.create_github_summary(metrics_file)
        else:
            print("::error::No metrics file found for summary generation")
            sys.exit(1)


if __name__ == "__main__":
    main()