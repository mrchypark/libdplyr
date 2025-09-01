# Pull Request

## Description
<!-- Provide a clear and concise description of what this PR does -->

## Type of Change
<!-- Mark the relevant option with an "x" -->
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring
- [ ] Test improvements
- [ ] CI/CD improvements

## Related Issues
<!-- Link to related issues using "Fixes #123" or "Closes #123" -->
- Fixes #
- Related to #

## Requirements Addressed
<!-- List the requirements from the spec that this PR addresses -->
- [ ] R1-AC1: (description)
- [ ] R2-AC2: (description)
- [ ] Other: (description)

## Changes Made
<!-- Detailed list of changes made -->
- 
- 
- 

## Testing
<!-- Describe the tests you ran and how to reproduce them -->

### Test Environment
- [ ] Linux x86_64
- [ ] macOS x86_64
- [ ] macOS ARM64
- [ ] Windows x86_64

### Tests Added/Modified
- [ ] Unit tests
- [ ] Integration tests
- [ ] Smoke tests
- [ ] Performance tests
- [ ] Documentation tests

### Test Results
```
# Paste relevant test output here
```

## Performance Impact
<!-- Describe any performance implications -->
- [ ] No performance impact
- [ ] Performance improvement (describe)
- [ ] Performance regression (justify)
- [ ] Performance impact unknown/needs testing

### Benchmark Results
<!-- If applicable, include benchmark results -->
```
# Paste benchmark results here
```

## Breaking Changes
<!-- List any breaking changes and migration steps -->
- [ ] No breaking changes
- [ ] Breaking changes (list below):
  - 
  - 

## Documentation
<!-- Documentation changes -->
- [ ] Code comments updated
- [ ] README updated
- [ ] API documentation updated
- [ ] User guide updated
- [ ] No documentation changes needed

## Security Considerations
<!-- Security implications of this change -->
- [ ] No security implications
- [ ] Security improvement
- [ ] Potential security impact (describe)

## Checklist
<!-- Mark completed items with an "x" -->

### Code Quality
- [ ] Code follows the project's style guidelines
- [ ] Self-review of code completed
- [ ] Code is well-commented, particularly in hard-to-understand areas
- [ ] No unnecessary debug prints or commented code

### Testing
- [ ] Tests pass locally
- [ ] New tests added for new functionality
- [ ] Existing tests updated if needed
- [ ] Edge cases considered and tested

### Rust Specific (if applicable)
- [ ] `cargo fmt` passes
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo test` passes
- [ ] No unsafe code added (or justified if necessary)

### C++ Specific (if applicable)
- [ ] Code follows C++17 standards
- [ ] Memory management is correct
- [ ] No memory leaks introduced
- [ ] Exception safety considered

### Build System
- [ ] CMake configuration updated if needed
- [ ] All platforms build successfully
- [ ] Dependencies updated in relevant files

### CI/CD
- [ ] All CI checks pass
- [ ] No new warnings introduced
- [ ] Performance benchmarks pass (if applicable)

## Additional Notes
<!-- Any additional information that reviewers should know -->

## Screenshots/Examples
<!-- If applicable, add screenshots or code examples -->

```sql
-- Example usage
DPLYR 'mtcars %>% select(mpg, cyl) %>% filter(mpg > 20)';
```

## Reviewer Notes
<!-- Specific areas you'd like reviewers to focus on -->
- Please pay special attention to:
- Questions for reviewers:
- Known limitations:

---

<!-- 
For maintainers:
- Ensure all requirements are properly addressed
- Verify test coverage is adequate
- Check for potential security implications
- Validate performance impact
- Confirm documentation is updated
-->