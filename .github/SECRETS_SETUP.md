# GitHub Secrets Setup Guide

This document explains how to set up the required secrets for the libdplyr release workflow.

## Required Secrets

### 1. GITHUB_TOKEN (Automatic)
- **Description**: Used for creating releases and uploading assets
- **Setup**: This is automatically provided by GitHub Actions
- **Permissions**: Ensure the repository has "Read and write permissions" for Actions

### 2. CRATES_TOKEN (Optional)
- **Description**: Used for publishing to crates.io
- **Setup**: 
  1. Go to [crates.io](https://crates.io/)
  2. Log in with your GitHub account
  3. Go to Account Settings → API Tokens
  4. Create a new token with "Publish" scope
  5. Add it as a repository secret named `CRATES_TOKEN`

## Setting Up Repository Secrets

1. Go to your repository on GitHub
2. Click on "Settings" tab
3. In the left sidebar, click "Secrets and variables" → "Actions"
4. Click "New repository secret"
5. Add the secret name and value

## Repository Permissions

Ensure your repository has the following permissions:

### Actions Permissions
1. Go to Settings → Actions → General
2. Under "Actions permissions", select "Allow all actions and reusable workflows"
3. Under "Workflow permissions", select "Read and write permissions"
4. Check "Allow GitHub Actions to create and approve pull requests"

### Token Permissions
The GITHUB_TOKEN needs the following permissions:
- `contents: write` (for creating releases)
- `actions: read` (for workflow access)

## Environment Setup

### Repository Variables (Optional)
You can set up repository variables for configuration:

1. Go to Settings → Secrets and variables → Actions
2. Click on "Variables" tab
3. Add the following variables if needed:

- `RELEASE_DRAFT`: Set to `true` to create draft releases
- `RELEASE_PRERELEASE`: Set to `true` to mark releases as pre-release
- `CRATES_PUBLISH`: Set to `false` to disable crates.io publishing

## Testing the Setup

### 1. Test Release Workflow
Create a test tag to verify the workflow:

```bash
# Create a test tag
git tag -a v0.0.1-test -m "Test release"
git push origin v0.0.1-test

# Monitor the workflow in GitHub Actions
# Delete the test tag and release after verification
git tag -d v0.0.1-test
git push origin :refs/tags/v0.0.1-test
```

### 2. Verify Permissions
Check that the workflow can:
- Create releases
- Upload assets
- Update repository files
- (Optional) Publish to crates.io

## Troubleshooting

### Common Issues

#### 1. Permission Denied Errors
- Check that "Read and write permissions" are enabled for Actions
- Verify that the GITHUB_TOKEN has sufficient permissions

#### 2. Asset Upload Failures
- Ensure the release was created successfully
- Check that asset names don't contain invalid characters
- Verify file paths are correct

#### 3. Crates.io Publishing Failures
- Verify the CRATES_TOKEN is valid and has publish permissions
- Check that the package name is available on crates.io
- Ensure Cargo.toml has all required fields for publishing

#### 4. Cross-compilation Failures
- Some targets may require additional setup
- Check the workflow logs for specific error messages
- Consider using cross-compilation tools like `cross`

### Debug Steps

1. **Check Workflow Logs**: Go to Actions tab and examine the failed job logs
2. **Verify Secrets**: Ensure all required secrets are set correctly
3. **Test Locally**: Use the test scripts to verify builds work locally
4. **Check Dependencies**: Ensure all required tools are available in the workflow

## Security Considerations

### Secret Management
- Never commit secrets to the repository
- Use repository secrets for sensitive data
- Regularly rotate API tokens
- Use environment-specific secrets when possible

### Workflow Security
- Review all workflow files for security issues
- Use pinned action versions (e.g., `actions/checkout@v4`)
- Limit workflow permissions to minimum required
- Monitor workflow runs for suspicious activity

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [GitHub Secrets Documentation](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Crates.io Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Cross-compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)