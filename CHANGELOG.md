# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-13

### Added
- **4-Tier Codec Strategy**: AV1+Opus, VP9+Opus, VP9+AAC, H.264+AAC
- **ABR (Adaptive Bitrate)**: Multi-resolution encoding (1080p, 720p, 480p, 360p)
- **HLS/DASH Support**: Streaming manifest generation for both protocols
- **Encryption**: HLS AES-128 and DASH ClearKey support
- **Source Analysis**: Automatic detection of codec, framerate, and processing status
- **Broadcast Compliance**: Ofcom/ITU-compliant safety filters (denoising, deflickering)
- **Upscaling**: FFmpeg lanczos and Real-ESRGAN AI upscaler support
- **AWS Batch Integration**: S3 input/output and SQS job queue support

### Quality
- 36 unit tests with comprehensive coverage
- 10 Criterion benchmarks for performance tracking
- Clippy pedantic compliance (0 warnings)
- CI/CD pipeline with GitHub Actions
- Documentation with executable doctests

### Security
- Encryption key generation for content protection
- Secure handling of sensitive test outputs
- Comprehensive .gitignore for security hygiene

---

## Release Notes Format

### Types of Changes
- **Added**: New features
- **Changed**: Changes in existing functionality
- **Deprecated**: Soon-to-be removed features
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Vulnerability fixes
