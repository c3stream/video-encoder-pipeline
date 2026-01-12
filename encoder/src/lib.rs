//! Video Encoder Pipeline Library
//!
//! Multi-codec video encoding library with 4-tier output strategy.
//!
//! # Tier Strategy
//!
//! - **Tier 1**: AV1 + Opus (royalty-free, best compression)
//! - **Tier 2**: VP9 + Opus (royalty-free, wide support)
//! - **Tier 3**: VP9 + AAC (video royalty-free, iOS 14+)
//! - **Tier 4**: H.264 + AAC (fallback, universal)
//!
//! # Example
//!
//! ```ignore
//! use video_encoder::config::{Tier, VideoCodec, Rendition};
//!
//! // Get codec for a tier
//! let codec = Tier::Tier1.video_codec();
//! assert_eq!(codec, VideoCodec::AV1);
//!
//! // Create a rendition
//! let r = Rendition::new(1920, 1080, 8000, 128);
//! let params = r.qvbr_params(VideoCodec::AV1);
//! ```

pub mod config;
pub mod encoder;
pub mod error;
pub mod job;
pub mod source_analyzer;
pub mod upscaler;

// Re-export commonly used types
pub use config::{
    AbrLadder, AudioCodec, EncryptionConfig, PreprocessConfig, Preset, Rendition, Tier,
    VideoCodec,
};
pub use error::{EncoderError, Result};
pub use job::{JobArgs, JobConfig};
