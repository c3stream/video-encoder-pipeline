//! Source video analysis and processing status detection
//!
//! Analyzes input video files to determine:
//! - Whether they have already been processed/filtered
//! - What filters are safe to apply without quality degradation
//! - Metadata for tracking processing history

use crate::error::{EncoderError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Processing status of a source video
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingStatus {
    /// Original/raw source - safe for all filters
    Raw,
    /// Has been filtered but not re-encoded (intermediate)
    Filtered,
    /// Has been encoded/compressed - be careful with filters
    Encoded,
    /// Unknown status - conservative approach recommended
    Unknown,
}

impl ProcessingStatus {
    /// Human-readable description
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Raw => "Raw/original source (all filters safe)",
            Self::Filtered => "Already filtered (skip duplicate filters)",
            Self::Encoded => "Already encoded (minimal processing recommended)",
            Self::Unknown => "Unknown status (conservative approach)",
        }
    }

    /// Whether denoising is recommended
    #[must_use]
    pub fn should_denoise(&self) -> bool {
        matches!(self, Self::Raw | Self::Unknown)
    }

    /// Whether deflickering is recommended
    #[must_use]
    pub fn should_deflicker(&self) -> bool {
        matches!(self, Self::Raw)
    }

    /// Whether loudness normalization is safe
    #[must_use]
    #[allow(clippy::unused_self)] // self reserved for future implementation
    pub fn should_normalize_audio(&self) -> bool {
        // Always safe to normalize - just ensure not double-normalized
        true
    }
}

/// Detailed source video information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Video codec name (e.g., "h264", "hevc", "prores")
    pub video_codec: String,
    /// Audio codec name (e.g., "aac", "`pcm_s16le`")
    pub audio_codec: Option<String>,
    /// Video bitrate in kbps
    pub video_bitrate_kbps: Option<u32>,
    /// Audio bitrate in kbps
    pub audio_bitrate_kbps: Option<u32>,
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Frame rate (e.g., 29.97, 30, 60)
    pub framerate: f64,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Encoder/muxer that created this file
    pub encoder: Option<String>,
    /// Processing comment metadata
    pub processing_comment: Option<String>,
    /// Detected processing status
    pub status: ProcessingStatus,
    /// Recommended filter adjustments
    pub filter_recommendations: FilterRecommendations,
}

/// Filter application recommendations based on source analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterRecommendations {
    /// Skip video denoising
    pub skip_video_denoise: bool,
    /// Skip audio denoising
    pub skip_audio_denoise: bool,
    /// Skip deflicker filter
    pub skip_deflicker: bool,
    /// Skip deblock filter (already clean)
    pub skip_deblock: bool,
    /// Skip audio normalization (already normalized)
    pub skip_audio_normalize: bool,
    /// Reason for each skip recommendation
    pub reasons: Vec<String>,
}

impl SourceInfo {
    /// Analyze a video file and return source information
    pub fn analyze(path: &Path) -> Result<Self> {
        let probe_output = run_ffprobe(path)?;
        Self::from_ffprobe_output(&probe_output, path)
    }

    /// Parse ffprobe JSON output
    fn from_ffprobe_output(json: &str, path: &Path) -> Result<Self> {
        let data: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| EncoderError::FfmpegError(format!("Failed to parse ffprobe output: {e}")))?;

        // Extract stream information
        let streams = data["streams"].as_array()
            .ok_or_else(|| EncoderError::FfmpegError("No streams found".to_string()))?;

        let mut video_codec = String::new();
        let mut audio_codec: Option<String> = None;
        let mut video_bitrate_kbps: Option<u32> = None;
        let mut audio_bitrate_kbps: Option<u32> = None;
        let mut width = 0u32;
        let mut height = 0u32;
        let mut framerate = 30.0f64;

        for stream in streams {
            let codec_type = stream["codec_type"].as_str().unwrap_or("");

            match codec_type {
                "video" => {
                    video_codec = stream["codec_name"].as_str().unwrap_or("unknown").to_string();
                    width = stream["width"].as_u64().unwrap_or(0) as u32;
                    height = stream["height"].as_u64().unwrap_or(0) as u32;

                    // Parse bitrate
                    if let Some(br) = stream["bit_rate"].as_str() {
                        video_bitrate_kbps = br.parse::<u64>().ok().map(|b| (b / 1000) as u32);
                    }

                    // Parse framerate
                    if let Some(fr) = stream["r_frame_rate"].as_str() {
                        framerate = parse_framerate(fr);
                    } else if let Some(fr) = stream["avg_frame_rate"].as_str() {
                        framerate = parse_framerate(fr);
                    }
                }
                "audio" => {
                    audio_codec = Some(stream["codec_name"].as_str().unwrap_or("unknown").to_string());
                    if let Some(br) = stream["bit_rate"].as_str() {
                        audio_bitrate_kbps = br.parse::<u64>().ok().map(|b| (b / 1000) as u32);
                    }
                }
                _ => {}
            }
        }

        // Extract format information
        let format = &data["format"];
        let duration_secs = format["duration"].as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0);

        // Extract encoder from tags
        let encoder = format["tags"]["encoder"].as_str()
            .or_else(|| format["tags"]["writing_library"].as_str())
            .or_else(|| format["tags"]["handler_name"].as_str())
            .map(String::from);

        // Extract processing comment
        let processing_comment = format["tags"]["comment"].as_str()
            .or_else(|| format["tags"]["description"].as_str())
            .map(String::from);

        // Determine processing status
        let status = detect_processing_status(
            &video_codec,
            video_bitrate_kbps,
            encoder.as_deref(),
            processing_comment.as_deref(),
            path,
        );

        // Generate filter recommendations
        let filter_recommendations = generate_recommendations(
            &status,
            &video_codec,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            encoder.as_deref(),
        );

        Ok(Self {
            video_codec,
            audio_codec,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            width,
            height,
            framerate,
            duration_secs,
            encoder,
            processing_comment,
            status,
            filter_recommendations,
        })
    }

    /// Check if this appears to be a raw/master source
    #[must_use]
    pub fn is_likely_raw(&self) -> bool {
        matches!(self.status, ProcessingStatus::Raw)
    }

    /// Check if already processed by our encoder
    #[must_use]
    pub fn is_our_output(&self) -> bool {
        if let Some(ref comment) = self.processing_comment {
            comment.contains("rust-video-encoder") || comment.contains("video-encoder-pipeline")
        } else {
            false
        }
    }

    /// Get a summary string for logging
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{}x{} {} @ {:.2}fps, {}, status: {:?}",
            self.width,
            self.height,
            self.video_codec,
            self.framerate,
            self.video_bitrate_kbps.map_or("unknown bps".to_string(), |b| format!("{b}kbps")),
            self.status
        )
    }

    /// Print detailed analysis report
    pub fn print_report(&self) {
        println!("\n========== Source Analysis Report ==========");
        println!("Resolution:    {}x{}", self.width, self.height);
        println!("Video Codec:   {}", self.video_codec);
        println!("Audio Codec:   {}", self.audio_codec.as_deref().unwrap_or("none"));
        println!("Framerate:     {:.3} fps", self.framerate);
        println!("Duration:      {:.2} seconds", self.duration_secs);

        if let Some(kbps) = self.video_bitrate_kbps {
            println!("Video Bitrate: {kbps} kbps");
        }
        if let Some(kbps) = self.audio_bitrate_kbps {
            println!("Audio Bitrate: {kbps} kbps");
        }
        if let Some(ref enc) = self.encoder {
            println!("Encoder:       {enc}");
        }

        println!("\nProcessing Status: {}", self.status.description());

        if !self.filter_recommendations.reasons.is_empty() {
            println!("\nFilter Recommendations:");
            for reason in &self.filter_recommendations.reasons {
                println!("  - {reason}");
            }
        }
        println!("=============================================");
    }
}

/// Run ffprobe and get JSON output
fn run_ffprobe(path: &Path) -> Result<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| EncoderError::FfmpegError(format!("ffprobe failed: {e}")))?;

    if !output.status.success() {
        return Err(EncoderError::FfmpegError(
            String::from_utf8_lossy(&output.stderr).to_string()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse framerate from string like "30000/1001" or "30"
fn parse_framerate(fr_str: &str) -> f64 {
    if let Some((num, den)) = fr_str.split_once('/') {
        let n: f64 = num.parse().unwrap_or(30.0);
        let d: f64 = den.parse().unwrap_or(1.0);
        if d > 0.0 { n / d } else { 30.0 }
    } else {
        fr_str.parse().unwrap_or(30.0)
    }
}

/// Detect processing status based on video characteristics
fn detect_processing_status(
    video_codec: &str,
    video_bitrate_kbps: Option<u32>,
    encoder: Option<&str>,
    comment: Option<&str>,
    _path: &Path,
) -> ProcessingStatus {
    // Check for our own processing marker
    if let Some(c) = comment {
        if c.contains("rust-video-encoder") || c.contains("video-encoder-pipeline") {
            return ProcessingStatus::Encoded;
        }
        if c.contains("filtered") || c.contains("preprocessed") {
            return ProcessingStatus::Filtered;
        }
    }

    // Check encoder signature
    if let Some(enc) = encoder {
        let enc_lower = enc.to_lowercase();

        // FFmpeg/Lavf indicates processing
        if enc_lower.contains("lavf") || enc_lower.contains("ffmpeg") {
            // High bitrate FFmpeg output might be intermediate
            if let Some(kbps) = video_bitrate_kbps
                && kbps > 20000 {
                    return ProcessingStatus::Filtered;
                }
            return ProcessingStatus::Encoded;
        }

        // HandBrake, x264, x265 indicate encoding
        if enc_lower.contains("handbrake")
            || enc_lower.contains("x264")
            || enc_lower.contains("x265")
            || enc_lower.contains("svt-av1")
            || enc_lower.contains("libvpx")
        {
            return ProcessingStatus::Encoded;
        }
    }

    // Check codec for raw/professional formats
    let raw_codecs = [
        "prores", "dnxhd", "dnxhr", "cfhd", "v210",
        "rawvideo", "ffv1", "huffyuv", "magicyuv",
        "r210", "v410", "yuv4",
    ];

    if raw_codecs.iter().any(|c| video_codec.to_lowercase().contains(c)) {
        return ProcessingStatus::Raw;
    }

    // High bitrate H.264/HEVC might be camera original
    if let Some(kbps) = video_bitrate_kbps {
        match video_codec.to_lowercase().as_str() {
            "h264" | "avc" => {
                if kbps > 50000 {
                    return ProcessingStatus::Raw; // Likely camera original
                } else if kbps > 15000 {
                    return ProcessingStatus::Unknown; // Could be either
                }
            }
            "hevc" | "h265" => {
                if kbps > 30000 {
                    return ProcessingStatus::Raw;
                } else if kbps > 10000 {
                    return ProcessingStatus::Unknown;
                }
            }
            _ => {}
        }
    }

    // Default to Unknown for safety
    ProcessingStatus::Unknown
}

/// Generate filter recommendations based on analysis
fn generate_recommendations(
    status: &ProcessingStatus,
    video_codec: &str,
    video_bitrate_kbps: Option<u32>,
    audio_bitrate_kbps: Option<u32>,
    encoder: Option<&str>,
) -> FilterRecommendations {
    let mut rec = FilterRecommendations::default();

    match status {
        ProcessingStatus::Encoded => {
            // Already encoded - be very conservative
            rec.skip_video_denoise = true;
            rec.skip_deblock = true;
            rec.reasons.push("Already encoded: skipping video denoise to avoid quality loss".to_string());
            rec.reasons.push("Already encoded: skipping deblock (may already be applied)".to_string());

            // Check for low bitrate (likely heavily compressed)
            if let Some(kbps) = video_bitrate_kbps
                && kbps < 3000 {
                    rec.skip_deflicker = true;
                    rec.reasons.push(format!(
                        "Low bitrate ({kbps}kbps): deflicker may introduce artifacts"
                    ));
                }
        }
        ProcessingStatus::Filtered => {
            // Already filtered - skip duplicate processing
            rec.skip_video_denoise = true;
            rec.skip_deflicker = true;
            rec.reasons.push("Already filtered: skipping denoise".to_string());
            rec.reasons.push("Already filtered: skipping deflicker".to_string());
        }
        ProcessingStatus::Raw => {
            // Raw source - all filters are safe
            rec.reasons.push("Raw source detected: all filters recommended".to_string());
        }
        ProcessingStatus::Unknown => {
            // Unknown - use conservative defaults based on codec
            if (video_codec.to_lowercase().contains("h264") || video_codec.to_lowercase().contains("hevc"))
                && let Some(kbps) = video_bitrate_kbps
                    && kbps < 5000 {
                        rec.skip_video_denoise = true;
                        rec.reasons.push("Compressed source: denoise may degrade quality".to_string());
                    }
        }
    }

    // Check encoder for normalization status
    if let Some(enc) = encoder
        && enc.to_lowercase().contains("loudnorm") {
            rec.skip_audio_normalize = true;
            rec.reasons.push("Audio already normalized (loudnorm detected)".to_string());
        }

    // Check for low audio bitrate
    if let Some(kbps) = audio_bitrate_kbps
        && kbps < 64 {
            rec.skip_audio_denoise = true;
            rec.reasons.push(format!(
                "Low audio bitrate ({kbps}kbps): skipping denoise to preserve quality"
            ));
        }

    rec
}

/// Our encoder's metadata marker for tracking
pub const ENCODER_MARKER: &str = "rust-video-encoder-pipeline";

/// Generate metadata comment for processed files
#[must_use]
pub fn generate_processing_metadata(filters_applied: &[&str]) -> String {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    format!(
        "{}|v1.0|{}|filters:{}",
        ENCODER_MARKER,
        timestamp,
        filters_applied.join(",")
    )
}

/// `FFmpeg` metadata args for embedding processing info
#[must_use] 
pub fn metadata_args(comment: &str) -> Vec<String> {
    vec![
        "-metadata".to_string(),
        format!("comment={}", comment),
        "-metadata".to_string(),
        format!("encoded_by={}", ENCODER_MARKER),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_status_recommendations() {
        assert!(ProcessingStatus::Raw.should_denoise());
        assert!(ProcessingStatus::Raw.should_deflicker());

        assert!(!ProcessingStatus::Encoded.should_deflicker());

        assert!(ProcessingStatus::Unknown.should_denoise());
        assert!(!ProcessingStatus::Unknown.should_deflicker());
    }

    #[test]
    fn test_parse_framerate() {
        assert!((parse_framerate("30") - 30.0).abs() < 0.01);
        assert!((parse_framerate("30000/1001") - 29.97).abs() < 0.01);
        assert!((parse_framerate("24000/1001") - 23.976).abs() < 0.01);
    }

    #[test]
    fn test_detect_raw_codec() {
        let status = detect_processing_status("prores", Some(100000), None, None, Path::new("test.mov"));
        assert_eq!(status, ProcessingStatus::Raw);
    }

    #[test]
    fn test_detect_encoded() {
        let status = detect_processing_status("h264", Some(3000), Some("Lavf58.76.100"), None, Path::new("test.mp4"));
        assert_eq!(status, ProcessingStatus::Encoded);
    }
}
