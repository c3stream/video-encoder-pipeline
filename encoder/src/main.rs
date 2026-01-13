//! Video Encoder Pipeline - AWS Batch Worker
//!
//! Multi-codec video encoding with 4-tier output:
//! - Tier 1: AV1 + Opus (royalty-free, best compression)
//! - Tier 2: VP9 + Opus (royalty-free, wide support)
//! - Tier 3: VP9 + AAC (video royalty-free, iOS 14+)
//! - Tier 4: H.264 + AAC (fallback, universal)

// Binary-level lint configuration (same as lib.rs)
#![allow(clippy::format_push_string)]
#![allow(clippy::struct_excessive_bools)]
#![allow(dead_code)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod encoder;
mod error;
mod job;
mod source_analyzer;
mod upscaler;

use anyhow::Result;
use clap::Parser;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "video-encoder")]
#[command(about = "Multi-codec video encoder for AWS Batch")]
struct Args {
    /// Input video file path or S3 URI
    #[arg(short, long)]
    input: String,

    /// Output directory path or S3 URI
    #[arg(short, long)]
    output: String,

    /// Encoding preset (fast, balanced, quality)
    #[arg(short, long, default_value = "balanced")]
    preset: String,

    /// Enable upscaling
    #[arg(long, default_value = "false")]
    upscale: bool,

    /// Upscaler to use (ffmpeg, realesrgan)
    #[arg(long, default_value = "ffmpeg")]
    upscaler: String,

    /// Target resolution (720, 1080)
    #[arg(long, default_value = "1080")]
    resolution: u32,

    /// Tiers to generate (1,2,3,4 or all)
    #[arg(long, default_value = "all")]
    tiers: String,

    /// Generate DASH manifest
    #[arg(long, default_value = "true")]
    dash: bool,

    /// Generate HLS manifest
    #[arg(long, default_value = "true")]
    hls: bool,

    /// Enable ABR (Adaptive Bitrate) multi-resolution encoding
    #[arg(long, default_value = "false")]
    abr: bool,

    /// Use QVBR (Quality-defined Variable Bitrate) rate control
    #[arg(long, default_value = "false")]
    qvbr: bool,

    /// Enable encryption (HLS `AES-128` + DASH `ClearKey`)
    #[arg(long, default_value = "false")]
    encrypt: bool,

    /// Enable multi-bitrate audio (64k, 128k, 256k)
    #[arg(long, default_value = "false")]
    audio_abr: bool,

    /// Enable preprocessing (normalization, denoising, deflicker)
    #[arg(long, default_value = "false")]
    preprocess: bool,

    /// Enable broadcast compliance mode (Ofcom/ITU filters)
    /// Includes: photosensitivity filter, red flash filter, color limiter,
    /// spatial pattern filter, audio loudness range, peak limiter
    #[arg(long, default_value = "false")]
    broadcast: bool,

    /// Analyze source file only (no encoding)
    /// Outputs processing status and filter recommendations
    #[arg(long, default_value = "false")]
    analyze: bool,

    /// Auto-adjust filters based on source analysis
    /// Skips filters that may degrade already-processed content
    #[arg(long, default_value = "false")]
    auto_filter: bool,
}

impl From<&Args> for job::JobArgs {
    fn from(args: &Args) -> Self {
        Self {
            input: args.input.clone(),
            output: args.output.clone(),
            preset: args.preset.clone(),
            tiers: args.tiers.clone(),
            resolution: args.resolution,
            qvbr: args.qvbr,
            encrypt: args.encrypt,
            hls: args.hls,
            dash: args.dash,
            preprocess: args.preprocess,
            broadcast: args.broadcast,
            auto_filter: args.auto_filter,
            upscale: args.upscale,
            upscaler: args.upscaler.clone(),
            abr: args.abr,
            audio_abr: args.audio_abr,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging (RAII pattern - must stay in scope)
    #[allow(clippy::let_unit_value)]
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();

    let args = Args::parse();

    // Handle analyze-only mode
    if args.analyze {
        return run_analyze_mode(&args.input).await;
    }

    info!(
        input = %args.input,
        output = %args.output,
        preset = %args.preset,
        upscale = args.upscale,
        "Starting video encoding job"
    );

    // Parse job configuration with optional source analysis
    let job_args: job::JobArgs = (&args).into();
    let job_config = job::JobConfig::from_args(&job_args)?;

    // Execute encoding pipeline
    let result = encoder::run_pipeline(&job_config).await?;

    info!(
        output_files = ?result.output_files,
        duration_secs = result.duration.as_secs(),
        "Encoding completed successfully"
    );

    Ok(())
}

/// Run source analysis only (no encoding)
#[allow(clippy::unused_async)] // async kept for API consistency
async fn run_analyze_mode(input: &str) -> Result<()> {
    use std::path::Path;

    println!("Analyzing source file: {input}");

    let path = Path::new(input);
    if !path.exists() {
        anyhow::bail!("Input file not found: {input}");
    }

    let source_info = source_analyzer::SourceInfo::analyze(path)?;
    source_info.print_report();

    // Print JSON for programmatic use
    println!("\nJSON Output:");
    println!("{}", serde_json::to_string_pretty(&source_info)?);

    Ok(())
}
