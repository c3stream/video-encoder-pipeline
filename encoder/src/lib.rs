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
//! ```
//! use video_encoder::{Tier, VideoCodec, Rendition};
//!
//! // Get codec for a tier
//! let codec = Tier::Tier1.video_codec();
//! assert_eq!(codec, VideoCodec::AV1);
//!
//! // Create a rendition
//! let r = Rendition::new(1920, 1080, 8000, 128);
//! let params = r.qvbr_params(VideoCodec::AV1);
//! ```

// Crate-level lint configuration for pedantic clippy
// These are intentional design decisions for this video encoding library:

// Manifest generation uses extensive string building with format! for readability
#![allow(clippy::format_push_string)]
// Config structs legitimately have many boolean flags for encoding options
#![allow(clippy::struct_excessive_bools)]
// API functions may not all be used by the CLI but are part of the public interface
#![allow(dead_code)]
// Manifest generation functions are inherently complex due to HLS/DASH specs
#![allow(clippy::too_many_lines)]
// API design uses references for consistency even for small Copy types
#![allow(clippy::trivially_copy_pass_by_ref)]
// Cast operations are intentional in video processing (dimensions, bitrates)
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
// Will add comprehensive error documentation in future iteration
#![allow(clippy::missing_errors_doc)]

pub mod config;
pub mod encoder;
pub mod error;
pub mod job;
pub mod source_analyzer;
pub mod upscaler;

// Re-export commonly used types
pub use config::{
    AbrLadder, AudioCodec, EncryptionConfig, PreprocessConfig, Preset, Rendition, Tier, VideoCodec,
};
pub use error::{EncoderError, Result};
pub use job::{JobArgs, JobConfig};
