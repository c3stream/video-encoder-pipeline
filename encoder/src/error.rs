//! Error types for the encoder

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncoderError {
    #[error("FFmpeg execution failed: {0}")]
    FfmpegError(String),

    #[error("Upscaling failed: {0}")]
    UpscaleError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("S3 operation failed: {0}")]
    S3Error(String),

    #[error("Manifest generation failed: {0}")]
    ManifestError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, EncoderError>;
