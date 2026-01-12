//! Job configuration and management

use crate::config::{AbrLadder, AudioLadder, EncryptionConfig, Preset, PreprocessConfig, RateControl, Resolution, SegmentConfig, Tier};
use crate::error::{EncoderError, Result};
use crate::source_analyzer::{FilterRecommendations, SourceInfo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Upscaler selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Upscaler {
    /// FFmpeg built-in scalers (lanczos, bicubic)
    Ffmpeg,
    /// Real-ESRGAN AI upscaler
    RealEsrgan,
}

impl Upscaler {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "realesrgan" | "esrgan" | "ai" => Self::RealEsrgan,
            _ => Self::Ffmpeg,
        }
    }
}

/// Arguments for creating a job configuration
///
/// This struct abstracts the CLI arguments to allow `JobConfig::from_args`
/// to be used from both the binary and library contexts.
#[derive(Debug, Clone)]
pub struct JobArgs {
    /// Input file path (local or S3 URI)
    pub input: String,
    /// Output directory (local or S3 URI)
    pub output: String,
    /// Encoding preset name (fast, balanced, quality)
    pub preset: String,
    /// Tier specification (e.g., "1,2,3", "all")
    pub tiers: String,
    /// Target resolution height
    pub resolution: u32,
    /// Use QVBR rate control
    pub qvbr: bool,
    /// Enable encryption
    pub encrypt: bool,
    /// Generate HLS output
    pub hls: bool,
    /// Generate DASH output
    pub dash: bool,
    /// Enable all preprocessing
    pub preprocess: bool,
    /// Enable broadcast compliance preprocessing
    pub broadcast: bool,
    /// Enable auto-filter based on source analysis
    pub auto_filter: bool,
    /// Enable upscaling
    pub upscale: bool,
    /// Upscaler name
    pub upscaler: String,
    /// Enable ABR encoding
    pub abr: bool,
    /// Enable audio ABR
    pub audio_abr: bool,
}

impl Default for JobArgs {
    fn default() -> Self {
        Self {
            input: String::new(),
            output: String::from("output"),
            preset: String::from("balanced"),
            tiers: String::from("all"),
            resolution: 1080,
            qvbr: false,
            encrypt: false,
            hls: true,
            dash: true,
            preprocess: false,
            broadcast: false,
            auto_filter: false,
            upscale: false,
            upscaler: String::from("ffmpeg"),
            abr: false,
            audio_abr: false,
        }
    }
}

/// Complete job configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    /// Input file path (local or S3 URI)
    pub input: String,

    /// Output directory (local or S3 URI)
    pub output: String,

    /// Encoding preset
    pub preset: Preset,

    /// Whether to upscale
    pub upscale: bool,

    /// Upscaler to use
    pub upscaler: Upscaler,

    /// Target resolution
    pub resolution: Resolution,

    /// Tiers to generate
    pub tiers: Vec<Tier>,

    /// Generate DASH manifest
    pub generate_dash: bool,

    /// Generate HLS manifest
    pub generate_hls: bool,

    /// Segment configuration
    pub segment_config: SegmentConfig,

    /// Working directory for temporary files
    pub work_dir: PathBuf,

    /// Enable ABR (Adaptive Bitrate) encoding
    pub abr_enabled: bool,

    /// ABR ladder configuration
    pub abr_ladder: AbrLadder,

    /// Rate control mode
    pub rate_control: RateControl,

    /// Encryption configuration
    pub encryption: EncryptionConfig,

    /// Enable multi-bitrate audio
    pub audio_abr_enabled: bool,

    /// Audio bitrate ladder configuration
    pub audio_ladder: AudioLadder,

    /// Pre-processing configuration (normalization, denoising, etc.)
    pub preprocess: PreprocessConfig,

    /// Enable auto-filter adjustment based on source analysis
    pub auto_filter: bool,

    /// Source analysis info (populated when auto_filter is enabled)
    #[serde(skip)]
    pub source_info: Option<SourceInfo>,

    /// Filter recommendations from source analysis
    #[serde(skip)]
    pub filter_recommendations: Option<FilterRecommendations>,
}

impl JobConfig {
    /// Create configuration from job arguments
    ///
    /// This method accepts `JobArgs` which can be constructed from CLI arguments
    /// or created programmatically for library usage.
    pub fn from_args(args: &JobArgs) -> Result<Self> {
        let preset = match args.preset.to_lowercase().as_str() {
            "fast" => Preset::Fast,
            "quality" => Preset::Quality,
            _ => Preset::Balanced,
        };

        let tiers = parse_tiers(&args.tiers)?;
        let resolution = Resolution::from_target(args.resolution);

        let work_dir = std::env::temp_dir().join(format!(
            "video-encoder-{}",
            std::process::id()
        ));

        let rate_control = if args.qvbr {
            RateControl::Qvbr
        } else {
            RateControl::Crf
        };

        let abr_ladder = AbrLadder::with_max_height(args.resolution);

        // Setup encryption if requested
        let encryption = if args.encrypt {
            EncryptionConfig::new_with_generated_keys(args.hls, args.dash)
        } else {
            EncryptionConfig::default()
        };

        // Setup preprocessing if requested
        // --broadcast enables Ofcom/ITU broadcast compliance filters
        // --preprocess enables all preprocessing including broadcast filters
        let mut preprocess = if args.preprocess {
            PreprocessConfig::all_enabled()
        } else if args.broadcast {
            PreprocessConfig::broadcast_compliance()
        } else {
            PreprocessConfig::default()
        };

        // Perform source analysis if auto_filter is enabled
        let (source_info, filter_recommendations) = if args.auto_filter {
            let input_path = Path::new(&args.input);
            if input_path.exists() && !args.input.starts_with("s3://") {
                match SourceInfo::analyze(input_path) {
                    Ok(info) => {
                        info!(
                            status = ?info.status,
                            video_codec = %info.video_codec,
                            "Source analysis complete"
                        );

                        // Apply filter adjustments
                        let recs = info.filter_recommendations.clone();
                        apply_filter_recommendations(&mut preprocess, &recs);

                        (Some(info), Some(recs))
                    }
                    Err(e) => {
                        info!(error = %e, "Source analysis failed, using default filters");
                        (None, None)
                    }
                }
            } else {
                info!("Auto-filter: S3 input or file not found, skipping analysis");
                (None, None)
            }
        } else {
            (None, None)
        };

        Ok(Self {
            input: args.input.clone(),
            output: args.output.clone(),
            preset,
            upscale: args.upscale,
            upscaler: Upscaler::from_str(&args.upscaler),
            resolution,
            tiers,
            generate_dash: args.dash,
            generate_hls: args.hls,
            segment_config: SegmentConfig::default(),
            work_dir,
            abr_enabled: args.abr,
            abr_ladder,
            rate_control,
            encryption,
            audio_abr_enabled: args.audio_abr,
            audio_ladder: AudioLadder::default(),
            preprocess,
            auto_filter: args.auto_filter,
            source_info,
            filter_recommendations,
        })
    }

    /// Check if input is an S3 URI
    #[must_use]
    pub fn is_s3_input(&self) -> bool {
        self.input.starts_with("s3://")
    }

    /// Check if output is an S3 URI
    #[must_use]
    pub fn is_s3_output(&self) -> bool {
        self.output.starts_with("s3://")
    }
}

/// Parse tier specification string
fn parse_tiers(spec: &str) -> Result<Vec<Tier>> {
    if spec == "all" {
        return Ok(Tier::all());
    }

    let mut tiers = Vec::new();
    for part in spec.split(',') {
        let tier = match part.trim() {
            "1" => Tier::Tier1,
            "2" => Tier::Tier2,
            "3" => Tier::Tier3,
            "4" => Tier::Tier4,
            other => {
                return Err(EncoderError::ConfigError(format!(
                    "Invalid tier: {other}. Use 1, 2, 3, 4, or all"
                )));
            }
        };
        if !tiers.contains(&tier) {
            tiers.push(tier);
        }
    }

    if tiers.is_empty() {
        return Err(EncoderError::ConfigError(
            "At least one tier must be specified".to_string(),
        ));
    }

    Ok(tiers)
}

/// Encoding job result
#[derive(Debug)]
pub struct JobResult {
    /// List of output files/URIs
    pub output_files: Vec<String>,

    /// Total encoding duration
    pub duration: std::time::Duration,

    /// Per-tier statistics
    pub tier_stats: Vec<TierStats>,
}

/// Statistics for a single tier encoding
#[derive(Debug)]
pub struct TierStats {
    pub tier: Tier,
    pub encoding_time: std::time::Duration,
    pub output_size_bytes: u64,
    pub bitrate_kbps: u32,
}

/// Apply filter recommendations to preprocess config
fn apply_filter_recommendations(preprocess: &mut PreprocessConfig, recs: &FilterRecommendations) {
    if recs.skip_video_denoise {
        preprocess.video_denoise = false;
        info!("Auto-filter: Disabled video denoise");
    }
    if recs.skip_audio_denoise {
        preprocess.audio_denoise = false;
        info!("Auto-filter: Disabled audio denoise");
    }
    if recs.skip_deflicker {
        preprocess.video_deflicker = false;
        preprocess.fluorescent_deflicker.enabled = false;
        info!("Auto-filter: Disabled deflicker");
    }
    if recs.skip_deblock {
        preprocess.video_deblock = false;
        info!("Auto-filter: Disabled deblock");
    }
    if recs.skip_audio_normalize {
        preprocess.audio_normalize = false;
        info!("Auto-filter: Disabled audio normalization");
    }

    // Log all reasons
    for reason in &recs.reasons {
        info!(reason = %reason, "Filter adjustment reason");
    }
}
