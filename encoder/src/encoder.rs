//! Core encoding pipeline with shared segment architecture
//!
//! Output structure:
//! ```text
//! output/
//! ├── segments/
//! │   ├── video/
//! │   │   ├── av1/{1080p, 720p, 480p, 360p}/
//! │   │   ├── vp9/{1080p, 720p, 480p, 360p}/
//! │   │   └── h264/{1080p, 720p, 480p, 360p}/
//! │   └── audio/
//! │       ├── opus/128k/
//! │       └── aac/128k/
//! ├── hls/
//! │   ├── master.m3u8
//! │   ├── av1_opus.m3u8
//! │   ├── vp9_opus.m3u8
//! │   ├── vp9_aac.m3u8
//! │   └── h264_aac.m3u8
//! └── dash/manifest.mpd
//! ```

use crate::config::{AudioCodec, QvbrParams, RateControl, Rendition, Tier, VideoCodec};
use crate::error::{EncoderError, Result};
use crate::job::{JobConfig, JobResult, TierStats};
use crate::upscaler;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tokio::fs;
use tracing::info;

/// Probe video duration in seconds using `FFprobe`
fn probe_duration(input: &Path) -> Result<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            input.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| EncoderError::FfmpegError(format!("ffprobe failed: {e}")))?;

    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str
        .trim()
        .parse::<f64>()
        .map_err(|e| EncoderError::FfmpegError(format!("Failed to parse duration: {e}")))
}

/// Probe duration from output directory
#[allow(clippy::unnecessary_wraps)] // Result kept for error handling consistency
fn probe_duration_from_output(output_base: &Path) -> Result<f64> {
    // Check segments/video directory for any playlist
    let video_base = output_base.join("segments").join("video");
    for codec in ["av1", "vp9", "h264"] {
        for rendition in ["1080p", "720p", "480p", "360p"] {
            let playlist = video_base.join(codec).join(rendition).join("playlist.m3u8");
            if playlist.exists()
                && let Ok(duration) = parse_hls_duration(&playlist) {
                    return Ok(duration);
                }
        }
    }

    // Fallback to legacy structure
    for tier_name in ["h264_aac", "vp9_aac", "vp9_opus", "av1_opus"] {
        let tier_dir = output_base.join(tier_name);
        for rendition in ["1080p", "720p", "480p", "360p"] {
            let video_playlist = tier_dir.join(rendition).join("video").join("playlist.m3u8");
            if video_playlist.exists()
                && let Ok(duration) = parse_hls_duration(&video_playlist) {
                    return Ok(duration);
                }
        }
    }

    Ok(30.0) // Default
}

/// Parse total duration from HLS playlist
fn parse_hls_duration(playlist: &Path) -> Result<f64> {
    let content = std::fs::read_to_string(playlist)
        .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

    let mut total_duration = 0.0;
    for line in content.lines() {
        if line.starts_with("#EXTINF:")
            && let Some(duration_str) = line.strip_prefix("#EXTINF:")
                && let Some(duration_part) = duration_str.split(',').next()
                    && let Ok(duration) = duration_part.trim().parse::<f64>() {
                        total_duration += duration;
                    }
    }

    if total_duration > 0.0 {
        Ok(total_duration)
    } else {
        Err(EncoderError::ManifestError("No duration found in playlist".to_string()))
    }
}

/// Format duration as ISO 8601 duration string (PT1H2M3.456S)
fn format_iso_duration(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor() as u32;
    let minutes = ((seconds % 3600.0) / 60.0).floor() as u32;
    let secs = seconds % 60.0;

    if hours > 0 {
        format!("PT{hours}H{minutes}M{secs:.3}S")
    } else if minutes > 0 {
        format!("PT{minutes}M{secs:.3}S")
    } else {
        format!("PT{secs:.3}S")
    }
}

/// Run the complete encoding pipeline with shared segments
pub async fn run_pipeline(config: &JobConfig) -> Result<JobResult> {
    let start = Instant::now();
    let mut output_files = Vec::new();
    let mut tier_stats = Vec::new();

    // Create working directory
    fs::create_dir_all(&config.work_dir).await?;

    // Download from S3 if needed
    let input_path = if config.is_s3_input() {
        download_from_s3(&config.input, &config.work_dir).await?
    } else {
        PathBuf::from(&config.input)
    };

    // Check if upscaling is needed
    let source_path = if config.upscale {
        let (width, height) = upscaler::probe_resolution(&input_path)?;
        if upscaler::needs_upscale(width, height, config.resolution) {
            let upscaled_path = config.work_dir.join("upscaled.mp4");
            upscaler::upscale_video(
                &input_path,
                &upscaled_path,
                config.resolution,
                config.upscaler,
            )
            .await?;
            upscaled_path
        } else {
            info!("Input resolution sufficient, skipping upscale");
            input_path.clone()
        }
    } else {
        input_path.clone()
    };

    // Create output directory structure
    let output_base = if config.is_s3_output() {
        config.work_dir.join("output")
    } else {
        PathBuf::from(&config.output)
    };
    fs::create_dir_all(&output_base).await?;

    // Determine which video and audio codecs are needed
    let (video_codecs, audio_codecs) = get_required_codecs(&config.tiers);

    // Create shared segment directories
    let segments_dir = output_base.join("segments");
    let video_segments_dir = segments_dir.join("video");
    let audio_segments_dir = segments_dir.join("audio");
    fs::create_dir_all(&video_segments_dir).await?;
    fs::create_dir_all(&audio_segments_dir).await?;

    // Encode each unique video codec once
    for video_codec in &video_codecs {
        let codec_start = Instant::now();
        let codec_dir = video_segments_dir.join(video_codec_dir_name(*video_codec));
        fs::create_dir_all(&codec_dir).await?;

        info!(video_codec = ?video_codec, "Encoding video codec");

        if config.abr_enabled {
            encode_video_abr(&source_path, &codec_dir, *video_codec, config).await?;
        } else {
            encode_video_single(&source_path, &codec_dir, *video_codec, config).await?;
        }

        let codec_duration = codec_start.elapsed();
        let output_size = calculate_dir_size(&codec_dir).await?;

        // Find a tier that uses this video codec for stats
        if let Some(tier) = config.tiers.iter().find(|t| t.video_codec() == *video_codec) {
            tier_stats.push(TierStats {
                tier: *tier,
                encoding_time: codec_duration,
                output_size_bytes: output_size,
                bitrate_kbps: 0,
            });
        }
    }

    // Encode each unique audio codec once
    for audio_codec in &audio_codecs {
        let codec_dir = audio_segments_dir.join(audio_codec_dir_name(*audio_codec));
        fs::create_dir_all(&codec_dir).await?;

        info!(audio_codec = ?audio_codec, "Encoding audio codec");

        encode_audio(&source_path, &codec_dir, *audio_codec, config).await?;
    }

    output_files.push(segments_dir.to_string_lossy().to_string());

    // Create manifest directories
    let hls_dir = output_base.join("hls");
    let dash_dir = output_base.join("dash");
    fs::create_dir_all(&hls_dir).await?;
    fs::create_dir_all(&dash_dir).await?;

    // Generate manifests
    if config.generate_dash {
        let dash_path = dash_dir.join("manifest.mpd");
        if config.encryption.dash_clearkey {
            generate_dash_manifest_clearkey(&output_base, &config.tiers, &dash_path, config)?;
        } else {
            generate_dash_manifest(&output_base, &config.tiers, &dash_path, config)?;
        }
        output_files.push(dash_path.to_string_lossy().to_string());
    }

    if config.generate_hls {
        let hls_path = hls_dir.join("master.m3u8");
        if config.encryption.hls_aes128 {
            generate_hls_key_files(&output_base, config)?;
            generate_hls_manifest_aes128(&output_base, &config.tiers, &hls_path, config)?;
        } else {
            generate_hls_manifest(&output_base, &config.tiers, &hls_path, config)?;
        }
        output_files.push(hls_path.to_string_lossy().to_string());
    }

    // Upload to S3 if needed
    if config.is_s3_output() {
        upload_to_s3(&output_base, &config.output).await?;
    }

    // Cleanup
    if config.is_s3_input() || config.is_s3_output() {
        let _ = fs::remove_dir_all(&config.work_dir).await;
    }

    Ok(JobResult {
        output_files,
        duration: start.elapsed(),
        tier_stats,
    })
}

/// Get unique video and audio codecs required by the tiers
fn get_required_codecs(tiers: &[Tier]) -> (Vec<VideoCodec>, Vec<AudioCodec>) {
    let mut video_set = HashSet::new();
    let mut audio_set = HashSet::new();

    for tier in tiers {
        video_set.insert(tier.video_codec());
        audio_set.insert(tier.audio_codec());
    }

    // Convert to sorted vectors for consistent ordering
    let mut video_codecs: Vec<_> = video_set.into_iter().collect();
    let mut audio_codecs: Vec<_> = audio_set.into_iter().collect();

    // Sort by codec preference (AV1 first, then VP9, then H264)
    video_codecs.sort_by_key(|c| match c {
        VideoCodec::AV1 => 0,
        VideoCodec::VP9 => 1,
        VideoCodec::H264 => 2,
    });

    audio_codecs.sort_by_key(|c| match c {
        AudioCodec::Opus => 0,
        AudioCodec::AAC => 1,
    });

    (video_codecs, audio_codecs)
}

/// Get directory name for video codec
fn video_codec_dir_name(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::AV1 => "av1",
        VideoCodec::VP9 => "vp9",
        VideoCodec::H264 => "h264",
    }
}

/// Get directory name for audio codec
fn audio_codec_dir_name(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Opus => "opus",
        AudioCodec::AAC => "aac",
    }
}

/// Encode video for all ABR renditions
async fn encode_video_abr(
    input: &Path,
    codec_dir: &Path,
    video_codec: VideoCodec,
    config: &JobConfig,
) -> Result<()> {
    for rendition in &config.abr_ladder.renditions {
        let rendition_dir = codec_dir.join(rendition.dir_name());
        fs::create_dir_all(&rendition_dir).await?;

        info!(
            video_codec = ?video_codec,
            rendition = %rendition.dir_name(),
            width = rendition.width,
            height = rendition.height,
            "Encoding video rendition"
        );

        encode_video_rendition(input, &rendition_dir, video_codec, rendition, config).await?;
    }

    Ok(())
}

/// Encode single video rendition
async fn encode_video_single(
    input: &Path,
    codec_dir: &Path,
    video_codec: VideoCodec,
    config: &JobConfig,
) -> Result<()> {
    // Use 1080p as default single rendition
    let rendition = Rendition::new(1920, 1080, 6000, 128);
    encode_video_rendition(input, codec_dir, video_codec, &rendition, config).await
}

/// Encode a single video rendition with CMAF segmentation
#[allow(clippy::unused_async)] // async kept for future parallelization
async fn encode_video_rendition(
    input: &Path,
    output_dir: &Path,
    video_codec: VideoCodec,
    rendition: &Rendition,
    config: &JobConfig,
) -> Result<()> {
    let segment_duration = config.segment_config.duration_secs;

    let mut args = vec![
        "-i".to_string(),
        input.to_str().unwrap_or_default().to_string(),
        "-an".to_string(), // No audio
    ];

    // Build video filter chain with preprocessing
    let video_filter = config.preprocess.video_filter_chain(Some((rendition.width, rendition.height)));
    if video_filter.is_empty() {
        // No preprocessing, just scale
        args.extend([
            "-vf".to_string(),
            format!("scale={}:{}", rendition.width, rendition.height),
        ]);
    } else {
        args.extend([
            "-vf".to_string(),
            video_filter,
        ]);
    }

    // Video encoding settings with QVBR if enabled
    args.extend(video_encoder_args_qvbr(video_codec, rendition, config));

    // Output using HLS muxer for CMAF/fMP4 video-only segments
    let playlist = output_dir.join("playlist.m3u8");
    args.extend([
        "-force_key_frames".to_string(),
        format!("expr:gte(t,n_forced*{segment_duration})"),
        "-f".to_string(),
        "hls".to_string(),
        "-hls_time".to_string(),
        segment_duration.to_string(),
        "-hls_playlist_type".to_string(),
        "vod".to_string(),
        "-hls_segment_type".to_string(),
        "fmp4".to_string(),
        "-hls_fmp4_init_filename".to_string(),
        "init.mp4".to_string(),
        "-hls_segment_filename".to_string(),
        output_dir.join("segment_%05d.m4s").to_str().unwrap_or_default().to_string(),
        "-y".to_string(),
        playlist.to_str().unwrap_or_default().to_string(),
    ]);

    info!(
        video_codec = ?video_codec,
        rendition = %rendition.dir_name(),
        rate_control = ?config.rate_control,
        "Encoding video-only track"
    );

    let status = Command::new("ffmpeg")
        .args(&args)
        .status()
        .map_err(|e| EncoderError::FfmpegError(e.to_string()))?;

    if !status.success() {
        return Err(EncoderError::FfmpegError(format!(
            "FFmpeg video encoding failed for {:?} {}",
            video_codec,
            rendition.dir_name()
        )));
    }

    Ok(())
}

/// Encode audio with CMAF segmentation (supports multi-bitrate)
async fn encode_audio(
    input: &Path,
    codec_dir: &Path,
    audio_codec: AudioCodec,
    config: &JobConfig,
) -> Result<()> {
    if config.audio_abr_enabled {
        // Encode multiple audio bitrates
        for bitrate in &config.audio_ladder.bitrates {
            encode_audio_bitrate(input, codec_dir, audio_codec, bitrate.kbps, config).await?;
        }
    } else {
        // Single bitrate (128k default)
        encode_audio_bitrate(input, codec_dir, audio_codec, 128, config).await?;
    }

    Ok(())
}

/// Encode audio at a specific bitrate
async fn encode_audio_bitrate(
    input: &Path,
    codec_dir: &Path,
    audio_codec: AudioCodec,
    bitrate_kbps: u32,
    config: &JobConfig,
) -> Result<()> {
    let bitrate_dir = codec_dir.join(format!("{bitrate_kbps}k"));
    fs::create_dir_all(&bitrate_dir).await?;

    let segment_duration = config.segment_config.duration_secs;

    let mut args = vec![
        "-i".to_string(),
        input.to_str().unwrap_or_default().to_string(),
        "-vn".to_string(), // No video
    ];

    // Apply audio preprocessing filters if any
    let audio_filter = config.preprocess.audio_filter_chain();
    if !audio_filter.is_empty() {
        args.extend([
            "-af".to_string(),
            audio_filter,
        ]);
    }

    // Audio encoding settings with specified bitrate
    args.extend(audio_encoder_args_with_bitrate(audio_codec, bitrate_kbps));

    // Output using HLS muxer for CMAF/fMP4 audio-only segments
    let playlist = bitrate_dir.join("playlist.m3u8");
    args.extend([
        "-f".to_string(),
        "hls".to_string(),
        "-hls_time".to_string(),
        segment_duration.to_string(),
        "-hls_playlist_type".to_string(),
        "vod".to_string(),
        "-hls_segment_type".to_string(),
        "fmp4".to_string(),
        "-hls_fmp4_init_filename".to_string(),
        "init.mp4".to_string(),
        "-hls_segment_filename".to_string(),
        bitrate_dir.join("segment_%05d.m4s").to_str().unwrap_or_default().to_string(),
        "-y".to_string(),
        playlist.to_str().unwrap_or_default().to_string(),
    ]);

    info!(
        audio_codec = ?audio_codec,
        bitrate_kbps = bitrate_kbps,
        preprocessing = !config.preprocess.audio_filter_chain().is_empty(),
        "Encoding audio-only track"
    );

    let status = Command::new("ffmpeg")
        .args(&args)
        .status()
        .map_err(|e| EncoderError::FfmpegError(e.to_string()))?;

    if !status.success() {
        return Err(EncoderError::FfmpegError(format!(
            "FFmpeg audio encoding failed for {audio_codec:?} at {bitrate_kbps}kbps"
        )));
    }

    Ok(())
}

/// Get video encoder arguments with QVBR support
fn video_encoder_args_qvbr(codec: VideoCodec, rendition: &Rendition, config: &JobConfig) -> Vec<String> {
    let qvbr = rendition.qvbr_params(codec);

    match config.rate_control {
        RateControl::Qvbr => video_encoder_args_with_qvbr(codec, &qvbr, config),
        RateControl::Crf => video_encoder_args_crf_only(codec, config),
        RateControl::Cbr => video_encoder_args_cbr(codec, rendition, config),
    }
}

/// Video encoder args with QVBR (CRF + maxrate)
fn video_encoder_args_with_qvbr(codec: VideoCodec, qvbr: &QvbrParams, config: &JobConfig) -> Vec<String> {
    match codec {
        VideoCodec::AV1 => vec![
            "-c:v".to_string(),
            "libsvtav1".to_string(),
            "-preset".to_string(),
            config.preset.av1_preset().to_string(),
            "-crf".to_string(),
            qvbr.crf.to_string(),
            "-pix_fmt".to_string(),
            "yuv420p10le".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-svtav1-params".to_string(),
            format!("tune=0:mbr={}k", qvbr.maxrate_kbps),
        ],
        VideoCodec::VP9 => vec![
            "-c:v".to_string(),
            "libvpx-vp9".to_string(),
            "-cpu-used".to_string(),
            config.preset.vp9_cpu_used().to_string(),
            "-crf".to_string(),
            qvbr.crf.to_string(),
            "-b:v".to_string(),
            format!("{}k", qvbr.maxrate_kbps),
            "-maxrate".to_string(),
            format!("{}k", qvbr.maxrate_kbps),
            "-bufsize".to_string(),
            format!("{}k", qvbr.bufsize_kbps),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-row-mt".to_string(),
            "1".to_string(),
        ],
        VideoCodec::H264 => vec![
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            config.preset.h264_preset().to_string(),
            "-crf".to_string(),
            qvbr.crf.to_string(),
            "-maxrate".to_string(),
            format!("{}k", qvbr.maxrate_kbps),
            "-bufsize".to_string(),
            format!("{}k", qvbr.bufsize_kbps),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-level".to_string(),
            "4.1".to_string(),
        ],
    }
}

/// Video encoder args with CRF only (no maxrate)
fn video_encoder_args_crf_only(codec: VideoCodec, config: &JobConfig) -> Vec<String> {
    match codec {
        VideoCodec::AV1 => vec![
            "-c:v".to_string(),
            "libsvtav1".to_string(),
            "-preset".to_string(),
            config.preset.av1_preset().to_string(),
            "-crf".to_string(),
            "30".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p10le".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-svtav1-params".to_string(),
            "tune=0".to_string(),
        ],
        VideoCodec::VP9 => vec![
            "-c:v".to_string(),
            "libvpx-vp9".to_string(),
            "-cpu-used".to_string(),
            config.preset.vp9_cpu_used().to_string(),
            "-crf".to_string(),
            "30".to_string(),
            "-b:v".to_string(),
            "0".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-row-mt".to_string(),
            "1".to_string(),
        ],
        VideoCodec::H264 => vec![
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            config.preset.h264_preset().to_string(),
            "-crf".to_string(),
            "23".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-level".to_string(),
            "4.1".to_string(),
        ],
    }
}

/// Video encoder args with CBR
fn video_encoder_args_cbr(codec: VideoCodec, rendition: &Rendition, config: &JobConfig) -> Vec<String> {
    let bitrate = format!("{}k", rendition.video_bitrate_kbps);
    let bufsize = format!("{}k", rendition.video_bitrate_kbps * 2);

    match codec {
        VideoCodec::AV1 => vec![
            "-c:v".to_string(),
            "libsvtav1".to_string(),
            "-preset".to_string(),
            config.preset.av1_preset().to_string(),
            "-rc".to_string(),
            "1".to_string(),
            "-b:v".to_string(),
            bitrate.clone(),
            "-maxrate".to_string(),
            bitrate,
            "-bufsize".to_string(),
            bufsize,
            "-pix_fmt".to_string(),
            "yuv420p10le".to_string(),
            "-g".to_string(),
            "120".to_string(),
        ],
        VideoCodec::VP9 => vec![
            "-c:v".to_string(),
            "libvpx-vp9".to_string(),
            "-cpu-used".to_string(),
            config.preset.vp9_cpu_used().to_string(),
            "-b:v".to_string(),
            bitrate.clone(),
            "-maxrate".to_string(),
            bitrate,
            "-bufsize".to_string(),
            bufsize,
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-row-mt".to_string(),
            "1".to_string(),
        ],
        VideoCodec::H264 => vec![
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            config.preset.h264_preset().to_string(),
            "-b:v".to_string(),
            bitrate.clone(),
            "-maxrate".to_string(),
            bitrate,
            "-bufsize".to_string(),
            bufsize,
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-g".to_string(),
            "120".to_string(),
            "-profile:v".to_string(),
            "high".to_string(),
            "-level".to_string(),
            "4.1".to_string(),
        ],
    }
}

/// Get audio encoder arguments for `FFmpeg`
fn audio_encoder_args(codec: AudioCodec) -> Vec<String> {
    audio_encoder_args_with_bitrate(codec, 128)
}

/// Get audio encoder arguments with specific bitrate
fn audio_encoder_args_with_bitrate(codec: AudioCodec, bitrate_kbps: u32) -> Vec<String> {
    let bitrate_str = format!("{bitrate_kbps}k");
    match codec {
        AudioCodec::Opus => vec![
            "-c:a".to_string(),
            "libopus".to_string(),
            "-b:a".to_string(),
            bitrate_str,
            "-ar".to_string(),
            "48000".to_string(),
            "-application".to_string(),
            "audio".to_string(),
        ],
        AudioCodec::AAC => vec![
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            bitrate_str,
            "-ar".to_string(),
            "48000".to_string(),
            "-aac_coder".to_string(),
            "twoloop".to_string(),
        ],
    }
}

/// Generate DASH manifest with shared segment structure
fn generate_dash_manifest(
    output_base: &Path,
    tiers: &[Tier],
    output_path: &Path,
    config: &JobConfig,
) -> Result<()> {
    let duration = probe_duration_from_output(output_base)?;
    let duration_str = format_iso_duration(duration);

    let mut mpd = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011"
     profiles="urn:mpeg:dash:profile:isoff-live:2011"
     type="static"
     mediaPresentationDuration="{duration_str}"
     minBufferTime="PT2S">
  <Period>
"#);

    // Get unique video and audio codecs
    let (video_codecs, audio_codecs) = get_required_codecs(tiers);

    // Add Video AdaptationSets (one per video codec with all renditions)
    for video_codec in &video_codecs {
        let video_codec_str = match video_codec {
            VideoCodec::AV1 => "av01.0.08M.08",
            VideoCodec::VP9 => "vp09.00.31.08",
            VideoCodec::H264 => "avc1.640028",
        };
        let codec_dir = video_codec_dir_name(*video_codec);

        mpd.push_str(&format!(
            r#"    <!-- {codec_dir} Video -->
    <AdaptationSet mimeType="video/mp4" codecs="{video_codec_str}" segmentAlignment="true" startWithSAP="1">
"#
        ));

        if config.abr_enabled {
            let renditions = [
                ("1080p", 6_000_000_u32, 1920_u32, 1080_u32),
                ("720p", 3_000_000, 1280, 720),
                ("480p", 1_500_000, 854, 480),
                ("360p", 800_000, 640, 360),
            ];

            for (rendition, bandwidth, width, height) in &renditions {
                let rendition_path = output_base
                    .join("segments")
                    .join("video")
                    .join(codec_dir)
                    .join(rendition);
                if rendition_path.exists() {
                    mpd.push_str(&format!(
                        r#"      <Representation id="{codec_dir}_{rendition}" bandwidth="{bandwidth}" width="{width}" height="{height}">
        <SegmentTemplate media="../segments/video/{codec_dir}/{rendition}/segment_$Number%05d$.m4s" initialization="../segments/video/{codec_dir}/{rendition}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
                    ));
                }
            }
        } else {
            mpd.push_str(&format!(
                r#"      <Representation id="{codec_dir}" bandwidth="6000000" width="1920" height="1080">
        <SegmentTemplate media="../segments/video/{codec_dir}/segment_$Number%05d$.m4s" initialization="../segments/video/{codec_dir}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
            ));
        }

        mpd.push_str("    </AdaptationSet>\n");
    }

    // Add Audio AdaptationSets (one per audio codec with all bitrates)
    for audio_codec in &audio_codecs {
        let audio_codec_str = audio_codec.codec_string();
        let codec_dir = audio_codec_dir_name(*audio_codec);

        mpd.push_str(&format!(
            r#"    <!-- {codec_dir} Audio -->
    <AdaptationSet mimeType="audio/mp4" codecs="{audio_codec_str}" segmentAlignment="true" startWithSAP="1" lang="und">
"#
        ));

        // Check for multiple bitrates
        let bitrates = if config.audio_abr_enabled {
            vec![("256k", 256_000), ("128k", 128_000), ("64k", 64_000)]
        } else {
            vec![("128k", 128_000)]
        };

        for (bitrate_dir, bandwidth) in &bitrates {
            let bitrate_path = output_base
                .join("segments")
                .join("audio")
                .join(codec_dir)
                .join(bitrate_dir);
            if bitrate_path.exists() {
                mpd.push_str(&format!(
                    r#"      <Representation id="{codec_dir}_audio_{bitrate_dir}" bandwidth="{bandwidth}" audioSamplingRate="48000">
        <AudioChannelConfiguration schemeIdUri="urn:mpeg:dash:23003:3:audio_channel_configuration:2011" value="2"/>
        <SegmentTemplate media="../segments/audio/{codec_dir}/{bitrate_dir}/segment_$Number%05d$.m4s" initialization="../segments/audio/{codec_dir}/{bitrate_dir}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
                ));
            }
        }

        mpd.push_str("    </AdaptationSet>\n");
    }

    mpd.push_str("  </Period>\n</MPD>\n");

    std::fs::write(output_path, mpd).map_err(|e| EncoderError::ManifestError(e.to_string()))?;

    info!(path = %output_path.display(), "Generated DASH manifest");
    Ok(())
}

/// Generate HLS master playlist with tier-specific playlists
fn generate_hls_manifest(
    output_base: &Path,
    tiers: &[Tier],
    output_path: &Path,
    config: &JobConfig,
) -> Result<()> {
    let hls_dir = output_base.join("hls");

    // Generate master playlist
    let mut master = String::from("#EXTM3U\n#EXT-X-VERSION:7\n\n");

    for tier in tiers {
        let dir_name = tier.directory_name();
        let video_codec = tier.video_codec();
        let audio_codec = tier.audio_codec();

        let video_codec_str = match video_codec {
            VideoCodec::AV1 => "av01.0.08M.08",
            VideoCodec::VP9 => "vp09.00.31.08",
            VideoCodec::H264 => "avc1.640028",
        };

        let audio_codec_str = audio_codec.codec_string();

        let score = match tier {
            Tier::Tier1 => 100,
            Tier::Tier2 => 80,
            Tier::Tier3 => 60,
            Tier::Tier4 => 40,
        };

        if config.abr_enabled {
            for (rendition, bandwidth, resolution) in [
                ("1080p", 6_000_000, "1920x1080"),
                ("720p", 3_000_000, "1280x720"),
                ("480p", 1_500_000, "854x480"),
                ("360p", 800_000, "640x360"),
            ] {
                let video_path = output_base
                    .join("segments")
                    .join("video")
                    .join(video_codec_dir_name(video_codec))
                    .join(rendition);
                if video_path.exists() {
                    // Define audio group for this tier
                    let audio_group = format!("audio_{}", audio_codec_dir_name(audio_codec));

                    master.push_str(&format!(
                        r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="{audio_group}",NAME="Audio",DEFAULT=YES,AUTOSELECT=YES,URI="{dir_name}_audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH={bandwidth},RESOLUTION={resolution},CODECS="{video_codec_str},{audio_codec_str}",SCORE={score},AUDIO="{audio_group}"
{dir_name}_{rendition}.m3u8
"#
                    ));
                }
            }
        } else {
            // Non-ABR: reference video playlist directly with audio group
            master.push_str(&format!(
                r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="Audio",DEFAULT=YES,AUTOSELECT=YES,URI="{dir_name}_audio.m3u8"

#EXT-X-STREAM-INF:BANDWIDTH=6000000,RESOLUTION=1920x1080,CODECS="{video_codec_str},{audio_codec_str}",SCORE={score},AUDIO="audio"
{dir_name}_video.m3u8
"#
            ));
        }

        // Generate tier-specific video playlists
        generate_tier_hls_playlists(output_base, &hls_dir, *tier, config)?;
    }

    std::fs::write(output_path, master).map_err(|e| EncoderError::ManifestError(e.to_string()))?;

    info!(path = %output_path.display(), "Generated HLS master playlist");
    Ok(())
}

/// Generate HLS playlists for a specific tier
fn generate_tier_hls_playlists(
    output_base: &Path,
    hls_dir: &Path,
    tier: Tier,
    config: &JobConfig,
) -> Result<()> {
    let dir_name = tier.directory_name();
    let video_codec = tier.video_codec();
    let audio_codec = tier.audio_codec();
    let video_dir = video_codec_dir_name(video_codec);
    let audio_dir = audio_codec_dir_name(audio_codec);

    if config.abr_enabled {
        // Generate per-rendition video playlists
        for rendition in ["1080p", "720p", "480p", "360p"] {
            let source_playlist = output_base
                .join("segments")
                .join("video")
                .join(video_dir)
                .join(rendition)
                .join("playlist.m3u8");

            if source_playlist.exists() {
                // Read source playlist and rewrite paths
                let content = std::fs::read_to_string(&source_playlist)
                    .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

                let rewritten = rewrite_hls_paths(&content, &format!("../segments/video/{video_dir}/{rendition}/"));
                let target = hls_dir.join(format!("{dir_name}_{rendition}.m3u8"));
                std::fs::write(&target, rewritten)
                    .map_err(|e| EncoderError::ManifestError(e.to_string()))?;
            }
        }

        // Generate audio playlists (multiple bitrates if audio ABR enabled)
        let audio_bitrates = if config.audio_abr_enabled {
            vec!["256k", "128k", "64k"]
        } else {
            vec!["128k"]
        };

        for bitrate in &audio_bitrates {
            let audio_source = output_base
                .join("segments")
                .join("audio")
                .join(audio_dir)
                .join(bitrate)
                .join("playlist.m3u8");

            if audio_source.exists() {
                let content = std::fs::read_to_string(&audio_source)
                    .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

                let rewritten = rewrite_hls_paths(&content, &format!("../segments/audio/{audio_dir}/{bitrate}/"));
                let target = if config.audio_abr_enabled {
                    hls_dir.join(format!("{dir_name}_audio_{bitrate}.m3u8"))
                } else {
                    hls_dir.join(format!("{dir_name}_audio.m3u8"))
                };
                std::fs::write(&target, rewritten)
                    .map_err(|e| EncoderError::ManifestError(e.to_string()))?;
            }
        }
    } else {
        // Single rendition mode
        let video_source = output_base
            .join("segments")
            .join("video")
            .join(video_dir)
            .join("playlist.m3u8");

        if video_source.exists() {
            let content = std::fs::read_to_string(&video_source)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

            // Create combined playlist
            let mut combined = String::from("#EXTM3U\n#EXT-X-VERSION:7\n\n");
            combined.push_str(&format!(
                "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Audio\",DEFAULT=YES,AUTOSELECT=YES,URI=\"{dir_name}_audio.m3u8\"\n\n"
            ));
            combined.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH=6000000,RESOLUTION=1920x1080,AUDIO=\"audio\"\n{dir_name}_video.m3u8\n"
            ));

            let target = hls_dir.join(format!("{dir_name}.m3u8"));
            std::fs::write(&target, combined)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

            // Write video playlist
            let rewritten = rewrite_hls_paths(&content, &format!("../segments/video/{video_dir}/"));
            let video_target = hls_dir.join(format!("{dir_name}_video.m3u8"));
            std::fs::write(&video_target, rewritten)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;
        }

        // Audio playlist
        let audio_source = output_base
            .join("segments")
            .join("audio")
            .join(audio_dir)
            .join("128k")
            .join("playlist.m3u8");

        if audio_source.exists() {
            let content = std::fs::read_to_string(&audio_source)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

            let rewritten = rewrite_hls_paths(&content, &format!("../segments/audio/{audio_dir}/128k/"));
            let target = hls_dir.join(format!("{dir_name}_audio.m3u8"));
            std::fs::write(&target, rewritten)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;
        }
    }

    Ok(())
}

/// Rewrite HLS playlist paths to use relative paths from hls directory
fn rewrite_hls_paths(content: &str, base_path: &str) -> String {
    let mut result = String::new();

    for line in content.lines() {
        if line.starts_with('#') {
            // Handle EXT-X-MAP for init segment
            if line.starts_with("#EXT-X-MAP:URI=") {
                let new_line = line.replace("URI=\"", &format!("URI=\"{base_path}"));
                result.push_str(&new_line);
            } else {
                result.push_str(line);
            }
        } else if !line.is_empty() {
            // Segment file reference
            result.push_str(base_path);
            result.push_str(line);
        }
        result.push('\n');
    }

    result
}

/// Download file from S3
async fn download_from_s3(s3_uri: &str, work_dir: &Path) -> Result<PathBuf> {
    let filename = s3_uri.rsplit('/').next().unwrap_or("input.mp4");
    let local_path = work_dir.join(filename);

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&config);

    let uri = s3_uri.strip_prefix("s3://").unwrap_or(s3_uri);
    let mut parts = uri.splitn(2, '/');
    let bucket = parts.next().ok_or_else(|| {
        EncoderError::S3Error("Invalid S3 URI: missing bucket".to_string())
    })?;
    let key = parts.next().ok_or_else(|| {
        EncoderError::S3Error("Invalid S3 URI: missing key".to_string())
    })?;

    info!(bucket, key, "Downloading from S3");

    let response = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| EncoderError::S3Error(e.to_string()))?;

    let body = response
        .body
        .collect()
        .await
        .map_err(|e| EncoderError::S3Error(e.to_string()))?;

    fs::write(&local_path, body.into_bytes()).await?;

    Ok(local_path)
}

/// Upload directory to S3
async fn upload_to_s3(local_dir: &Path, s3_uri: &str) -> Result<()> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&config);

    let uri = s3_uri.strip_prefix("s3://").unwrap_or(s3_uri);
    let mut parts = uri.splitn(2, '/');
    let bucket = parts.next().ok_or_else(|| {
        EncoderError::S3Error("Invalid S3 URI: missing bucket".to_string())
    })?;
    let prefix = parts.next().unwrap_or("");

    info!(bucket, prefix, "Uploading to S3");

    upload_dir_recursive(&client, local_dir, bucket, prefix).await?;

    Ok(())
}

#[async_recursion::async_recursion]
async fn upload_dir_recursive(
    client: &aws_sdk_s3::Client,
    local_dir: &Path,
    bucket: &str,
    prefix: &str,
) -> Result<()> {
    let mut entries = fs::read_dir(local_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let key = if prefix.is_empty() {
            file_name.to_string()
        } else {
            format!("{prefix}/{file_name}")
        };

        if path.is_dir() {
            upload_dir_recursive(client, &path, bucket, &key).await?;
        } else {
            let body = fs::read(&path).await?;
            client
                .put_object()
                .bucket(bucket)
                .key(&key)
                .body(body.into())
                .send()
                .await
                .map_err(|e| EncoderError::S3Error(e.to_string()))?;
        }
    }

    Ok(())
}

/// Calculate total size of directory
async fn calculate_dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;
    calculate_dir_size_recursive(dir, &mut total).await?;
    Ok(total)
}

#[async_recursion::async_recursion]
async fn calculate_dir_size_recursive(dir: &Path, total: &mut u64) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            *total += metadata.len();
        } else if metadata.is_dir() {
            calculate_dir_size_recursive(&entry.path(), total).await?;
        }
    }

    Ok(())
}

// ============================================================================
// HLS AES-128 Encryption Functions
// ============================================================================

/// Generate HLS AES-128 key files
fn generate_hls_key_files(output_base: &Path, config: &JobConfig) -> Result<()> {
    let key_hex = config.encryption.key.as_ref().ok_or_else(|| {
        EncoderError::EncryptionError("Missing encryption key".to_string())
    })?;

    let key_bytes = hex_to_bytes(key_hex)?;

    let key_path = output_base.join("key.bin");
    std::fs::write(&key_path, &key_bytes)
        .map_err(|e| EncoderError::EncryptionError(e.to_string()))?;

    let iv = if let Some(key_id) = &config.encryption.key_id {
        format!("0x{}", &key_id[..32.min(key_id.len())])
    } else {
        "0x00000000000000000000000000000000".to_string()
    };

    let key_url = config.encryption.key_url.as_deref().unwrap_or("../key.bin");
    let key_info_path = output_base.join("key_info.txt");
    let key_info = format!("{}\n{}\n{}", key_url, key_path.display(), iv);
    std::fs::write(&key_info_path, &key_info)
        .map_err(|e| EncoderError::EncryptionError(e.to_string()))?;

    info!(
        key_path = %key_path.display(),
        key_info_path = %key_info_path.display(),
        "Generated HLS AES-128 key files"
    );

    Ok(())
}

/// Generate HLS master playlist with AES-128 encryption
fn generate_hls_manifest_aes128(
    output_base: &Path,
    tiers: &[Tier],
    output_path: &Path,
    config: &JobConfig,
) -> Result<()> {
    // Generate standard HLS first
    generate_hls_manifest(output_base, tiers, output_path, config)?;

    // Add encryption key info to tier playlists
    let key_url = config.encryption.key_url.as_deref().unwrap_or("../key.bin");
    let iv = if let Some(key_id) = &config.encryption.key_id {
        key_id[..32.min(key_id.len())].to_string()
    } else {
        "00000000000000000000000000000000".to_string()
    };

    let hls_dir = output_base.join("hls");

    // Update all tier playlists with encryption
    for entry in std::fs::read_dir(&hls_dir).map_err(|e| EncoderError::ManifestError(e.to_string()))? {
        let entry = entry.map_err(|e| EncoderError::ManifestError(e.to_string()))?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "m3u8") && path != *output_path {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;

            // Add encryption key after version
            let encrypted = content.replace(
                "#EXT-X-VERSION:7\n",
                &format!("#EXT-X-VERSION:7\n#EXT-X-KEY:METHOD=AES-128,URI=\"{key_url}\",IV=0x{iv}\n")
            );

            std::fs::write(&path, encrypted)
                .map_err(|e| EncoderError::ManifestError(e.to_string()))?;
        }
    }

    info!(path = %output_path.display(), "Generated HLS AES-128 encrypted manifest");
    Ok(())
}

// ============================================================================
// DASH ClearKey Encryption Functions
// ============================================================================

/// Generate DASH manifest with `ClearKey` encryption
fn generate_dash_manifest_clearkey(
    output_base: &Path,
    tiers: &[Tier],
    output_path: &Path,
    config: &JobConfig,
) -> Result<()> {
    let key_id = config.encryption.key_id.as_ref().ok_or_else(|| {
        EncoderError::EncryptionError("Missing key_id for ClearKey".to_string())
    })?;

    let key_id_uuid = format!(
        "{}-{}-{}-{}-{}",
        &key_id[0..8],
        &key_id[8..12],
        &key_id[12..16],
        &key_id[16..20],
        &key_id[20..32]
    );

    let duration = probe_duration_from_output(output_base)?;
    let duration_str = format_iso_duration(duration);
    let pssh = generate_clearkey_pssh(key_id);

    let mut mpd = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011"
     xmlns:cenc="urn:mpeg:cenc:2013"
     profiles="urn:mpeg:dash:profile:isoff-live:2011"
     type="static"
     mediaPresentationDuration="{duration_str}"
     minBufferTime="PT2S">
  <Period>
"#);

    let (video_codecs, audio_codecs) = get_required_codecs(tiers);

    // Add Video AdaptationSets with ClearKey
    for video_codec in &video_codecs {
        let video_codec_str = match video_codec {
            VideoCodec::AV1 => "av01.0.08M.08",
            VideoCodec::VP9 => "vp09.00.31.08",
            VideoCodec::H264 => "avc1.640028",
        };
        let codec_dir = video_codec_dir_name(*video_codec);

        mpd.push_str(&format!(
            r#"    <!-- {codec_dir} Video with ClearKey -->
    <AdaptationSet mimeType="video/mp4" codecs="{video_codec_str}" segmentAlignment="true" startWithSAP="1">
      <ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" value="cenc" cenc:default_KID="{key_id_uuid}"/>
      <ContentProtection schemeIdUri="urn:uuid:e2719d58-a985-b3c9-781a-b030af78d30e">
        <cenc:pssh>{pssh}</cenc:pssh>
      </ContentProtection>
"#
        ));

        if config.abr_enabled {
            let renditions = [
                ("1080p", 6_000_000_u32, 1920_u32, 1080_u32),
                ("720p", 3_000_000, 1280, 720),
                ("480p", 1_500_000, 854, 480),
                ("360p", 800_000, 640, 360),
            ];

            for (rendition, bandwidth, width, height) in &renditions {
                let rendition_path = output_base
                    .join("segments")
                    .join("video")
                    .join(codec_dir)
                    .join(rendition);
                if rendition_path.exists() {
                    mpd.push_str(&format!(
                        r#"      <Representation id="{codec_dir}_{rendition}" bandwidth="{bandwidth}" width="{width}" height="{height}">
        <SegmentTemplate media="../segments/video/{codec_dir}/{rendition}/segment_$Number%05d$.m4s" initialization="../segments/video/{codec_dir}/{rendition}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
                    ));
                }
            }
        } else {
            mpd.push_str(&format!(
                r#"      <Representation id="{codec_dir}" bandwidth="6000000" width="1920" height="1080">
        <SegmentTemplate media="../segments/video/{codec_dir}/segment_$Number%05d$.m4s" initialization="../segments/video/{codec_dir}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
            ));
        }

        mpd.push_str("    </AdaptationSet>\n");
    }

    // Add Audio AdaptationSets with ClearKey (supports multi-bitrate)
    for audio_codec in &audio_codecs {
        let audio_codec_str = audio_codec.codec_string();
        let codec_dir = audio_codec_dir_name(*audio_codec);

        mpd.push_str(&format!(
            r#"    <!-- {codec_dir} Audio with ClearKey -->
    <AdaptationSet mimeType="audio/mp4" codecs="{audio_codec_str}" segmentAlignment="true" startWithSAP="1" lang="und">
      <ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" value="cenc" cenc:default_KID="{key_id_uuid}"/>
      <ContentProtection schemeIdUri="urn:uuid:e2719d58-a985-b3c9-781a-b030af78d30e">
        <cenc:pssh>{pssh}</cenc:pssh>
      </ContentProtection>
"#
        ));

        // Check for multiple bitrates
        let bitrates = if config.audio_abr_enabled {
            vec![("256k", 256_000), ("128k", 128_000), ("64k", 64_000)]
        } else {
            vec![("128k", 128_000)]
        };

        for (bitrate_dir, bandwidth) in &bitrates {
            let bitrate_path = output_base
                .join("segments")
                .join("audio")
                .join(codec_dir)
                .join(bitrate_dir);
            if bitrate_path.exists() {
                mpd.push_str(&format!(
                    r#"      <Representation id="{codec_dir}_audio_{bitrate_dir}" bandwidth="{bandwidth}" audioSamplingRate="48000">
        <AudioChannelConfiguration schemeIdUri="urn:mpeg:dash:23003:3:audio_channel_configuration:2011" value="2"/>
        <SegmentTemplate media="../segments/audio/{codec_dir}/{bitrate_dir}/segment_$Number%05d$.m4s" initialization="../segments/audio/{codec_dir}/{bitrate_dir}/init.mp4" startNumber="1" timescale="1000" duration="4000"/>
      </Representation>
"#
                ));
            }
        }

        mpd.push_str("    </AdaptationSet>\n");
    }

    mpd.push_str("  </Period>\n</MPD>\n");

    std::fs::write(output_path, mpd).map_err(|e| EncoderError::ManifestError(e.to_string()))?;

    generate_clearkey_license_file(output_base, config)?;

    info!(path = %output_path.display(), "Generated DASH ClearKey encrypted manifest");
    Ok(())
}

/// Generate `ClearKey` PSSH box (base64 encoded)
fn generate_clearkey_pssh(key_id: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let system_id: [u8; 16] = [
        0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02,
        0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b
    ];

    let key_id_bytes = hex_to_bytes(key_id).unwrap_or_default();

    let mut pssh = Vec::new();
    pssh.extend_from_slice(&[0, 0, 0, 0]);
    pssh.extend_from_slice(b"pssh");
    pssh.extend_from_slice(&[1, 0, 0, 0]);
    pssh.extend_from_slice(&system_id);
    pssh.extend_from_slice(&(1u32.to_be_bytes()));
    pssh.extend_from_slice(&key_id_bytes);
    pssh.extend_from_slice(&(0u32.to_be_bytes()));

    let size = pssh.len() as u32;
    pssh[0..4].copy_from_slice(&size.to_be_bytes());

    STANDARD.encode(&pssh)
}

/// Generate `ClearKey` license file for testing
fn generate_clearkey_license_file(output_base: &Path, config: &JobConfig) -> Result<()> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    let key_id = config.encryption.key_id.as_ref().ok_or_else(|| {
        EncoderError::EncryptionError("Missing key_id".to_string())
    })?;
    let key = config.encryption.key.as_ref().ok_or_else(|| {
        EncoderError::EncryptionError("Missing key".to_string())
    })?;

    let key_id_bytes = hex_to_bytes(key_id)?;
    let key_bytes = hex_to_bytes(key)?;

    let key_id_b64 = URL_SAFE_NO_PAD.encode(&key_id_bytes);
    let key_b64 = URL_SAFE_NO_PAD.encode(&key_bytes);

    let license = format!(
        r#"{{
  "keys": [
    {{
      "kty": "oct",
      "k": "{key_b64}",
      "kid": "{key_id_b64}"
    }}
  ],
  "type": "temporary"
}}"#
    );

    let license_path = output_base.join("clearkey_license.json");
    std::fs::write(&license_path, &license)
        .map_err(|e| EncoderError::EncryptionError(e.to_string()))?;

    info!(license_path = %license_path.display(), "Generated ClearKey license file");

    Ok(())
}

/// Convert hex string to bytes
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return Err(EncoderError::EncryptionError("Invalid hex string length".to_string()));
    }

    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| EncoderError::EncryptionError(format!("Invalid hex: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_iso_duration_seconds_only() {
        assert_eq!(format_iso_duration(30.5), "PT30.500S");
        assert_eq!(format_iso_duration(0.0), "PT0.000S");
        assert_eq!(format_iso_duration(59.999), "PT59.999S");
    }

    #[test]
    fn format_iso_duration_with_minutes() {
        assert_eq!(format_iso_duration(90.0), "PT1M30.000S");
        assert_eq!(format_iso_duration(125.5), "PT2M5.500S");
    }

    #[test]
    fn format_iso_duration_with_hours() {
        assert_eq!(format_iso_duration(3661.5), "PT1H1M1.500S");
        assert_eq!(format_iso_duration(7200.0), "PT2H0M0.000S");
    }

    #[test]
    fn video_codec_dir_names() {
        assert_eq!(video_codec_dir_name(VideoCodec::AV1), "av1");
        assert_eq!(video_codec_dir_name(VideoCodec::VP9), "vp9");
        assert_eq!(video_codec_dir_name(VideoCodec::H264), "h264");
    }

    #[test]
    fn audio_codec_dir_names() {
        assert_eq!(audio_codec_dir_name(AudioCodec::Opus), "opus");
        assert_eq!(audio_codec_dir_name(AudioCodec::AAC), "aac");
    }

    #[test]
    fn get_required_codecs_single_tier() {
        let (video, audio) = get_required_codecs(&[Tier::Tier1]);
        assert_eq!(video, vec![VideoCodec::AV1]);
        assert_eq!(audio, vec![AudioCodec::Opus]);
    }

    #[test]
    fn get_required_codecs_all_tiers() {
        let (video, audio) = get_required_codecs(&Tier::all());
        assert_eq!(video.len(), 3); // AV1, VP9, H264
        assert_eq!(audio.len(), 2); // Opus, AAC
        assert!(video.contains(&VideoCodec::AV1));
        assert!(video.contains(&VideoCodec::VP9));
        assert!(video.contains(&VideoCodec::H264));
    }

    #[test]
    fn get_required_codecs_deduplicates() {
        // Tier2 and Tier3 both use VP9
        let (video, _) = get_required_codecs(&[Tier::Tier2, Tier::Tier3]);
        assert_eq!(video, vec![VideoCodec::VP9]);
    }

    #[test]
    fn hex_to_bytes_valid() {
        assert_eq!(hex_to_bytes("00").unwrap(), vec![0]);
        assert_eq!(hex_to_bytes("ff").unwrap(), vec![255]);
        assert_eq!(hex_to_bytes("0123456789abcdef").unwrap(),
                   vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    }

    #[test]
    fn hex_to_bytes_uppercase() {
        assert_eq!(hex_to_bytes("ABCDEF").unwrap(), vec![0xab, 0xcd, 0xef]);
    }

    #[test]
    fn hex_to_bytes_invalid_length() {
        assert!(hex_to_bytes("abc").is_err()); // Odd length
    }

    #[test]
    fn hex_to_bytes_invalid_chars() {
        assert!(hex_to_bytes("gg").is_err()); // Invalid hex chars
    }

    #[test]
    fn generate_clearkey_pssh_produces_valid_base64() {
        let key_id = "00000000000000000000000000000000";
        let pssh = generate_clearkey_pssh(key_id);

        // Should be valid base64
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        assert!(STANDARD.decode(&pssh).is_ok());

        // Decoded should start with size bytes and 'pssh'
        let decoded = STANDARD.decode(&pssh).unwrap();
        assert!(decoded.len() > 8);
        assert_eq!(&decoded[4..8], b"pssh");
    }
}
