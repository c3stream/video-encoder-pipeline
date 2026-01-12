# Contributing to Video Encoder Pipeline

Thank you for your interest in contributing! This document provides guidelines for contributing to the Video Encoder Pipeline project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Architecture Overview](#architecture-overview)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Benchmarking](#benchmarking)
- [CI/CD Pipeline](#cicd-pipeline)
- [AI-Assisted Development](#ai-assisted-development)
- [Security Guidelines](#security-guidelines)
- [Review Process](#review-process)

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## Architecture Overview

### Project Structure

```
video-encoder-pipeline/
├── encoder/              # Core Rust encoder library and CLI
│   ├── src/
│   │   ├── lib.rs        # Library entry point
│   │   ├── main.rs       # CLI entry point
│   │   ├── config.rs     # Configuration types (Tier, Codec, Rendition)
│   │   ├── encoder.rs    # FFmpeg encoding pipeline
│   │   ├── job.rs        # Job configuration (JobConfig, JobArgs)
│   │   ├── error.rs      # Error types
│   │   ├── source_analyzer.rs  # Source video analysis
│   │   └── upscaler.rs   # Upscaling support
│   └── benches/          # Criterion benchmarks
├── cdk/                  # AWS CDK infrastructure (TypeScript)
├── player/               # HLS/DASH test player
├── scripts/              # Development and build scripts
└── sources/              # Sample video sources (gitignored)
```

### 4-Tier Encoding Strategy

| Tier   | Video Codec | Audio Codec | Use Case                           |
| ------ | ----------- | ----------- | ---------------------------------- |
| Tier 1 | AV1         | Opus        | Best compression, royalty-free     |
| Tier 2 | VP9         | Opus        | Wide browser support, royalty-free |
| Tier 3 | VP9         | AAC         | iOS 14+ compatibility              |
| Tier 4 | H.264       | AAC         | Universal fallback                 |

### Key Abstractions

- **JobConfig**: Complete encoding job configuration
- **JobArgs**: CLI argument abstraction for library usage
- **Tier**: Codec tier selection with codec/audio mapping
- **Rendition**: Resolution-specific encoding parameters
- **AbrLadder**: Adaptive bitrate ladder configuration

## Development Setup

### Prerequisites

- **Rust**: 1.75+ (Edition 2021)
- **FFmpeg**: 5.0+ with AV1/VP9/H.264 support
- **Node.js**: 18+ (for CDK infrastructure)

### Installation

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/video-encoder-pipeline.git
cd video-encoder-pipeline

# Install FFmpeg
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg libavcodec-dev libavformat-dev libavutil-dev

# Verify FFmpeg codecs
ffmpeg -encoders | grep -E "av1|vp9|264"

# Build the project
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check formatting and lints
cargo fmt --check
cargo clippy -- -D warnings
```

### Workspace Configuration

The project uses a Cargo workspace with:

- **Clippy pedantic lints**: High code quality standards
- **Edition 2021**: Modern Rust features
- **Rust 1.75+**: MSRV for stable async features

## How to Contribute

### Reporting Bugs

1. Check existing [Issues](https://github.com/kazuhirokondo/video-encoder-pipeline/issues) to avoid duplicates
2. Use the bug report template
3. Include:
   - Rust version (`rustc --version`)
   - FFmpeg version (`ffmpeg -version`)
   - Operating system
   - Steps to reproduce
   - Expected vs actual behavior
   - Relevant logs or error messages

### Suggesting Features

1. Open a new Issue with the feature request template
2. Describe the problem you're trying to solve
3. Explain your proposed solution
4. Consider potential drawbacks or alternatives

### Pull Requests

#### Before Starting

1. Check if an issue exists for your planned work
2. Comment on the issue to indicate you're working on it
3. Fork the repository

#### Making Changes

1. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Follow coding standards (see below)

3. Write/update tests as needed

4. Commit with clear messages:
   ```bash
   git commit -m "feat: add support for HEVC encoding"
   ```

5. Push and open a Pull Request

#### Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, semicolons, etc.)
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding or correcting tests
- `chore`: Maintenance tasks

## Coding Standards

### Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- All public items must have documentation

```rust
/// Encodes a video file using the specified codec and settings.
///
/// # Arguments
///
/// * `input` - Path to the input video file
/// * `output` - Path for the encoded output
/// * `codec` - The video codec to use
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `EncoderError` on failure.
///
/// # Examples
///
/// ```ignore
/// use video_encoder::encode_video;
///
/// encode_video("input.mp4", "output.mp4", VideoCodec::AV1)?;
/// ```
pub fn encode_video(input: &str, output: &str, codec: VideoCodec) -> Result<()> {
    // implementation
}
```

### Error Handling

- Use `thiserror` for error types
- Provide context with error messages
- Avoid panics in library code

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncoderError {
    #[error("FFmpeg execution failed: {0}")]
    FfmpegError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}
```

## Testing Guidelines

### Test Organization

- **Unit tests**: Same file as the code (`#[cfg(test)]` module)
- **Integration tests**: `tests/` directory
- **Doc tests**: In documentation comments

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_video_codec_mapping() {
        assert_eq!(Tier::Tier1.video_codec(), VideoCodec::AV1);
        assert_eq!(Tier::Tier2.video_codec(), VideoCodec::VP9);
        assert_eq!(Tier::Tier4.video_codec(), VideoCodec::H264);
    }

    #[test]
    fn invalid_input_returns_error() {
        let result = encode_video("nonexistent.mp4", "output.mp4", VideoCodec::AV1);
        assert!(result.is_err());
    }
}
```

### Test Coverage

- Aim for meaningful coverage, not 100%
- Focus on edge cases and error conditions
- Test all public API functions

Run tests with coverage:

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --html
```

## Benchmarking

The project uses [Criterion](https://bheisler.github.io/criterion.rs/book/) for benchmarking.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench --bench config_benchmarks

# Quick benchmark run
cargo bench -- --quick
```

### Benchmark Groups

- **Tier**: Codec lookup, royalty-free check, directory names
- **VideoCodec**: FFmpeg encoder names, codec strings
- **Rendition**: QVBR params generation, directory names
- **AbrLadder**: Default creation, height filtering
- **PreprocessConfig**: Configuration creation, filter checks
- **EncryptionConfig**: Key generation, enablement checks
- **Preset**: Codec-specific preset lookups

### Adding Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_new_operation(c: &mut Criterion) {
    c.benchmark_group("NewGroup")
        .bench_function("operation_name", |b| {
            b.iter(|| black_box(your_function()))
        });
}

criterion_group!(benches, bench_new_operation);
criterion_main!(benches);
```

## CI/CD Pipeline

### GitHub Actions Workflows

#### CI (ci.yml)

Runs on every push and PR:

- **check**: Cargo check across workspace
- **fmt**: Format verification
- **clippy**: Lint checks with warnings as errors
- **test**: Cross-platform tests (Ubuntu/macOS, stable/beta)
- **build**: Release build verification
- **docs**: Documentation build
- **audit**: Security vulnerability scanning
- **coverage**: Code coverage with Codecov

#### Release (release.yml)

Runs on version tags (`v*`):

- Multi-platform builds (Linux amd64/arm64, macOS amd64/arm64)
- Automatic GitHub release creation

### Required Checks

All PRs must pass:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

## AI-Assisted Development

This project welcomes AI-assisted contributions (Claude, GPT, etc.).

### Guidelines for AI-Assisted PRs

1. **Disclosure**: Mention AI assistance in PR description
2. **Review**: Human review of all AI-generated code
3. **Testing**: Ensure AI-generated code has tests
4. **Documentation**: Keep docs updated with changes

### Recommended AI Tools

- **Claude Code**: Excellent for Rust code generation and refactoring
- **Serena MCP**: Semantic code understanding and symbol operations
- **Context7**: Library documentation lookup

### AI-Friendly Code Patterns

- Clear, descriptive function and variable names
- Comprehensive doc comments
- Type-safe APIs
- Small, focused modules

## Security Guidelines

### Sensitive Data

- **Never commit**: API keys, credentials, encryption keys
- **Use environment variables**: For runtime secrets
- **Check .gitignore**: Ensure secrets are excluded

### FFmpeg Security

- Validate input file paths
- Sanitize FFmpeg command arguments
- Avoid shell injection vulnerabilities

```rust
// Good: Use Command builder
let output = Command::new("ffmpeg")
    .arg("-i")
    .arg(&input_path)  // Safe: passed as argument
    .output()?;

// Bad: String interpolation (shell injection risk)
// let cmd = format!("ffmpeg -i {}", input_path);
```

### Dependency Auditing

```bash
# Install cargo-audit
cargo install cargo-audit

# Run audit
cargo audit
```

## Review Process

1. All PRs require at least one review
2. CI must pass (tests, linting, formatting)
3. Documentation must be updated if applicable
4. Breaking changes require discussion in the issue first
5. Security-sensitive changes require extra scrutiny

## Questions?

Feel free to:
- Open an Issue for questions
- Start a Discussion for broader topics

Thank you for contributing!
