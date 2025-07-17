#!/usr/bin/env python3
"""
GitHub Actions Workflow Validation Script
Validates workflow files for syntax errors, best practices, and common issues.
"""

import os
import sys
import yaml
import json
import re
from pathlib import Path
from typing import Dict, List, Any, Optional
from dataclasses import dataclass


@dataclass
class ValidationResult:
    """Result of workflow validation."""
    file_path: str
    is_valid: bool
    errors: List[str]
    warnings: List[str]
    suggestions: List[str]


class WorkflowValidator:
    """Validates GitHub Actions workflow files."""
    
    def __init__(self):
        self.required_fields = ['name', 'on', 'jobs']
        self.recommended_fields = ['env']
        self.security_patterns = [
            r'\$\{\{\s*secrets\.',  # Secret usage
            r'\$\{\{\s*github\.token',  # GitHub token usage
        ]
    
    def validate_yaml_syntax(self, file_path: Path) -> tuple[bool, Optional[str]]:
        """Validate YAML syntax."""
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                yaml.safe_load(f)
            return True, None
        except yaml.YAMLError as e:
            return False, str(e)
        except Exception as e:
            return False, f"Failed to read file: {e}"
    
    def validate_workflow_structure(self, workflow: Dict[str, Any]) -> List[str]:
        """Validate basic workflow structure."""
        errors = []
        
        # Check required fields
        for field in self.required_fields:
            if field not in workflow:
                errors.append(f"Missing required field: {field}")
        
        # Validate jobs structure
        if 'jobs' in workflow:
            jobs = workflow['jobs']
            if not isinstance(jobs, dict):
                errors.append("'jobs' must be a dictionary")
            elif not jobs:
                errors.append("At least one job must be defined")
            else:
                for job_name, job_config in jobs.items():
                    if not isinstance(job_config, dict):
                        errors.append(f"Job '{job_name}' must be a dictionary")
                        continue
                    
                    # Check required job fields
                    if 'runs-on' not in job_config:
                        errors.append(f"Job '{job_name}' missing 'runs-on' field")
                    
                    # Validate steps
                    if 'steps' in job_config:
                        steps = job_config['steps']
                        if not isinstance(steps, list):
                            errors.append(f"Job '{job_name}' steps must be a list")
                        else:
                            for i, step in enumerate(steps):
                                if not isinstance(step, dict):
                                    errors.append(f"Job '{job_name}' step {i} must be a dictionary")
                                elif 'uses' not in step and 'run' not in step:
                                    errors.append(f"Job '{job_name}' step {i} must have either 'uses' or 'run'")
        
        return errors
    
    def validate_triggers(self, workflow: Dict[str, Any]) -> List[str]:
        """Validate workflow triggers."""
        warnings = []
        
        if 'on' not in workflow:
            return warnings
        
        triggers = workflow['on']
        
        # Check for common trigger issues
        if isinstance(triggers, dict):
            # Check for potentially expensive triggers
            if 'schedule' in triggers:
                warnings.append("Scheduled workflows consume GitHub Actions minutes")
            
            # Check push/PR configuration
            if 'push' in triggers and 'pull_request' in triggers:
                push_config = triggers['push']
                pr_config = triggers['pull_request']
                
                if isinstance(push_config, dict) and isinstance(pr_config, dict):
                    push_branches = push_config.get('branches', [])
                    pr_branches = pr_config.get('branches', [])
                    
                    if push_branches and pr_branches and set(push_branches) == set(pr_branches):
                        warnings.append("Push and PR triggers on same branches may cause duplicate runs")
        
        return warnings
    
    def validate_security(self, workflow: Dict[str, Any], file_content: str) -> List[str]:
        """Validate security aspects."""
        warnings = []
        
        # Check for hardcoded secrets
        if re.search(r'password\s*[:=]\s*["\'][^"\']+["\']', file_content, re.IGNORECASE):
            warnings.append("Potential hardcoded password detected")
        
        if re.search(r'token\s*[:=]\s*["\'][^"\']+["\']', file_content, re.IGNORECASE):
            warnings.append("Potential hardcoded token detected")
        
        # Check for proper secret usage
        secret_usage = re.findall(r'\$\{\{\s*secrets\.(\w+)\s*\}\}', file_content)
        if secret_usage:
            for secret in secret_usage:
                if secret.lower() in ['password', 'token', 'key']:
                    warnings.append(f"Generic secret name '{secret}' - consider more specific naming")
        
        # Check permissions
        if 'permissions' in workflow:
            permissions = workflow['permissions']
            if isinstance(permissions, dict):
                if permissions.get('contents') == 'write' and permissions.get('pull-requests') == 'write':
                    warnings.append("Broad write permissions - consider limiting scope")
        
        return warnings
    
    def validate_performance(self, workflow: Dict[str, Any]) -> List[str]:
        """Validate performance aspects."""
        suggestions = []
        
        if 'jobs' not in workflow:
            return suggestions
        
        jobs = workflow['jobs']
        
        for job_name, job_config in jobs.items():
            if not isinstance(job_config, dict):
                continue
            
            # Check for caching
            if 'steps' in job_config:
                steps = job_config['steps']
                has_cache = any(
                    isinstance(step, dict) and 
                    step.get('uses', '').startswith('actions/cache')
                    for step in steps
                )
                
                has_rust_setup = any(
                    isinstance(step, dict) and (
                        'rust' in step.get('uses', '').lower() or
                        'cargo' in str(step.get('run', '')).lower()
                    )
                    for step in steps
                )
                
                if has_rust_setup and not has_cache:
                    suggestions.append(f"Job '{job_name}' uses Rust but doesn't use caching")
            
            # Check for matrix strategy
            if 'strategy' in job_config:
                strategy = job_config['strategy']
                if isinstance(strategy, dict) and 'matrix' in strategy:
                    matrix = strategy['matrix']
                    if isinstance(matrix, dict):
                        # Count matrix combinations
                        combinations = 1
                        for key, values in matrix.items():
                            if isinstance(values, list):
                                combinations *= len(values)
                        
                        if combinations > 20:
                            suggestions.append(f"Job '{job_name}' has {combinations} matrix combinations - consider reducing")
    
    def validate_best_practices(self, workflow: Dict[str, Any]) -> List[str]:
        """Validate best practices."""
        suggestions = []
        
        # Check for workflow name
        if 'name' not in workflow:
            suggestions.append("Consider adding a descriptive workflow name")
        
        # Check for environment variables
        if 'env' not in workflow:
            suggestions.append("Consider defining common environment variables at workflow level")
        
        # Check for timeout settings
        if 'jobs' in workflow:
            jobs = workflow['jobs']
            for job_name, job_config in jobs.items():
                if isinstance(job_config, dict):
                    if 'timeout-minutes' not in job_config:
                        suggestions.append(f"Job '{job_name}' doesn't have timeout - consider adding one")
                    
                    # Check for continue-on-error usage
                    if job_config.get('continue-on-error') is True:
                        suggestions.append(f"Job '{job_name}' uses continue-on-error - ensure this is intentional")
        
        return suggestions
    
    def validate_action_versions(self, workflow: Dict[str, Any]) -> List[str]:
        """Validate action versions."""
        warnings = []
        
        if 'jobs' not in workflow:
            return warnings
        
        jobs = workflow['jobs']
        
        for job_name, job_config in jobs.items():
            if not isinstance(job_config, dict) or 'steps' not in job_config:
                continue
            
            steps = job_config['steps']
            for i, step in enumerate(steps):
                if not isinstance(step, dict) or 'uses' not in step:
                    continue
                
                uses = step['uses']
                
                # Check for latest tag usage
                if uses.endswith('@main') or uses.endswith('@master'):
                    warnings.append(f"Job '{job_name}' step {i} uses unstable branch reference: {uses}")
                
                # Check for missing version
                if '@' not in uses:
                    warnings.append(f"Job '{job_name}' step {i} missing version reference: {uses}")
                
                # Check for deprecated actions
                deprecated_actions = [
                    'actions/setup-node@v1',
                    'actions/setup-python@v1',
                    'actions/cache@v1',
                    'actions/cache@v2'
                ]
                
                if uses in deprecated_actions:
                    warnings.append(f"Job '{job_name}' step {i} uses deprecated action: {uses}")
        
        return warnings
    
    def validate_file(self, file_path: Path) -> ValidationResult:
        """Validate a single workflow file."""
        errors = []
        warnings = []
        suggestions = []
        
        # Check YAML syntax
        is_valid_yaml, yaml_error = self.validate_yaml_syntax(file_path)
        if not is_valid_yaml:
            return ValidationResult(
                file_path=str(file_path),
                is_valid=False,
                errors=[f"YAML syntax error: {yaml_error}"],
                warnings=[],
                suggestions=[]
            )
        
        # Load workflow
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                file_content = f.read()
                workflow = yaml.safe_load(file_content)
        except Exception as e:
            return ValidationResult(
                file_path=str(file_path),
                is_valid=False,
                errors=[f"Failed to load workflow: {e}"],
                warnings=[],
                suggestions=[]
            )
        
        # Run validations
        errors.extend(self.validate_workflow_structure(workflow))
        warnings.extend(self.validate_triggers(workflow))
        warnings.extend(self.validate_security(workflow, file_content))
        warnings.extend(self.validate_action_versions(workflow))
        suggestions.extend(self.validate_performance(workflow))
        suggestions.extend(self.validate_best_practices(workflow))
        
        return ValidationResult(
            file_path=str(file_path),
            is_valid=len(errors) == 0,
            errors=errors,
            warnings=warnings,
            suggestions=suggestions
        )
    
    def validate_directory(self, workflows_dir: Path) -> List[ValidationResult]:
        """Validate all workflow files in a directory."""
        results = []
        
        if not workflows_dir.exists():
            return results
        
        for file_path in workflows_dir.glob('*.yml'):
            results.append(self.validate_file(file_path))
        
        for file_path in workflows_dir.glob('*.yaml'):
            results.append(self.validate_file(file_path))
        
        return results


def print_results(results: List[ValidationResult]) -> None:
    """Print validation results."""
    total_files = len(results)
    valid_files = sum(1 for r in results if r.is_valid)
    total_errors = sum(len(r.errors) for r in results)
    total_warnings = sum(len(r.warnings) for r in results)
    total_suggestions = sum(len(r.suggestions) for r in results)
    
    print("=" * 60)
    print("GitHub Actions Workflow Validation Results")
    print("=" * 60)
    print(f"Files validated: {total_files}")
    print(f"Valid files: {valid_files}")
    print(f"Files with errors: {total_files - valid_files}")
    print(f"Total errors: {total_errors}")
    print(f"Total warnings: {total_warnings}")
    print(f"Total suggestions: {total_suggestions}")
    print()
    
    for result in results:
        print(f"📄 {result.file_path}")
        print(f"   Status: {'✅ Valid' if result.is_valid else '❌ Invalid'}")
        
        if result.errors:
            print("   🚨 Errors:")
            for error in result.errors:
                print(f"      - {error}")
        
        if result.warnings:
            print("   ⚠️  Warnings:")
            for warning in result.warnings:
                print(f"      - {warning}")
        
        if result.suggestions:
            print("   💡 Suggestions:")
            for suggestion in result.suggestions:
                print(f"      - {suggestion}")
        
        print()


def main():
    """Main entry point."""
    import argparse
    
    parser = argparse.ArgumentParser(description="Validate GitHub Actions workflows")
    parser.add_argument("--workflows-dir", type=Path, default=Path(".github/workflows"),
                       help="Directory containing workflow files")
    parser.add_argument("--fail-on-error", action="store_true",
                       help="Exit with non-zero code if validation errors found")
    parser.add_argument("--fail-on-warning", action="store_true",
                       help="Exit with non-zero code if warnings found")
    parser.add_argument("--json", action="store_true",
                       help="Output results in JSON format")
    
    args = parser.parse_args()
    
    validator = WorkflowValidator()
    results = validator.validate_directory(args.workflows_dir)
    
    if args.json:
        # Output JSON format
        json_results = []
        for result in results:
            json_results.append({
                "file_path": result.file_path,
                "is_valid": result.is_valid,
                "errors": result.errors,
                "warnings": result.warnings,
                "suggestions": result.suggestions
            })
        print(json.dumps(json_results, indent=2))
    else:
        # Output human-readable format
        print_results(results)
    
    # Determine exit code
    exit_code = 0
    
    if args.fail_on_error and any(not r.is_valid for r in results):
        exit_code = 1
    
    if args.fail_on_warning and any(r.warnings for r in results):
        exit_code = 2
    
    if exit_code > 0:
        print(f"Validation failed with exit code {exit_code}")
    
    sys.exit(exit_code)


if __name__ == "__main__":
    main()