//! Video upscaling implementations

use crate::config::Resolution;
use crate::error::{EncoderError, Result};
use crate::job::Upscaler;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

/// Upscale a video file
pub async fn upscale_video(
    input: &Path,
    output: &Path,
    target: Resolution,
    upscaler: Upscaler,
) -> Result<()> {
    match upscaler {
        Upscaler::Ffmpeg => upscale_ffmpeg(input, output, target).await,
        Upscaler::RealEsrgan => upscale_realesrgan(input, output, target).await,
    }
}

/// FFmpeg-based upscaling using lanczos
#[allow(clippy::unused_async)] // async kept for future parallelization
async fn upscale_ffmpeg(input: &Path, output: &Path, target: Resolution) -> Result<()> {
    info!(
        input = %input.display(),
        target_width = target.width,
        target_height = target.height,
        "Upscaling with FFmpeg (lanczos)"
    );

    let status = Command::new("ffmpeg")
        .args([
            "-i",
            input.to_str().unwrap_or_default(),
            "-vf",
            &format!("scale={}:{}:flags=lanczos", target.width, target.height),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "18",
            "-c:a",
            "copy",
            "-y",
            output.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|e| EncoderError::UpscaleError(e.to_string()))?;

    if !status.success() {
        return Err(EncoderError::UpscaleError(
            "FFmpeg upscaling failed".to_string(),
        ));
    }

    Ok(())
}

/// Real-ESRGAN AI upscaling
async fn upscale_realesrgan(input: &Path, output: &Path, target: Resolution) -> Result<()> {
    info!(
        input = %input.display(),
        target_width = target.width,
        target_height = target.height,
        "Upscaling with Real-ESRGAN"
    );

    // Check if realesrgan-ncnn-vulkan is available
    let which_result = Command::new("which").arg("realesrgan-ncnn-vulkan").output();

    if which_result.is_err() || !which_result.unwrap().status.success() {
        warn!("Real-ESRGAN not found, falling back to FFmpeg");
        return upscale_ffmpeg(input, output, target).await;
    }

    // Real-ESRGAN works on images, so we need to:
    // 1. Extract frames
    // 2. Upscale each frame
    // 3. Reassemble with audio

    let work_dir = output.parent().unwrap_or(Path::new("/tmp"));
    let frames_dir = work_dir.join("frames_in");
    let upscaled_dir = work_dir.join("frames_out");

    std::fs::create_dir_all(&frames_dir)?;
    std::fs::create_dir_all(&upscaled_dir)?;

    // Extract frames
    info!("Extracting frames...");
    let extract_status = Command::new("ffmpeg")
        .args([
            "-i",
            input.to_str().unwrap_or_default(),
            "-qscale:v",
            "2",
            frames_dir
                .join("frame_%06d.png")
                .to_str()
                .unwrap_or_default(),
        ])
        .status()
        .map_err(|e| EncoderError::UpscaleError(e.to_string()))?;

    if !extract_status.success() {
        return Err(EncoderError::UpscaleError(
            "Frame extraction failed".to_string(),
        ));
    }

    // Run Real-ESRGAN
    info!("Running Real-ESRGAN upscaling...");
    let esrgan_status = Command::new("realesrgan-ncnn-vulkan")
        .args([
            "-i",
            frames_dir.to_str().unwrap_or_default(),
            "-o",
            upscaled_dir.to_str().unwrap_or_default(),
            "-n",
            "realesrgan-x4plus",
            "-s",
            "4",
            "-f",
            "png",
        ])
        .status()
        .map_err(|e| EncoderError::UpscaleError(e.to_string()))?;

    if !esrgan_status.success() {
        warn!("Real-ESRGAN failed, falling back to FFmpeg");
        // Cleanup
        let _ = std::fs::remove_dir_all(&frames_dir);
        let _ = std::fs::remove_dir_all(&upscaled_dir);
        return upscale_ffmpeg(input, output, target).await;
    }

    // Get framerate from input
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            input.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| EncoderError::UpscaleError(e.to_string()))?;

    let fps = String::from_utf8_lossy(&probe.stdout).trim().to_string();

    // Reassemble video with audio
    info!("Reassembling video...");
    let reassemble_status = Command::new("ffmpeg")
        .args([
            "-framerate",
            &fps,
            "-i",
            upscaled_dir
                .join("frame_%06d.png")
                .to_str()
                .unwrap_or_default(),
            "-i",
            input.to_str().unwrap_or_default(),
            "-map",
            "0:v",
            "-map",
            "1:a?",
            "-vf",
            &format!("scale={}:{}:flags=lanczos", target.width, target.height),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "18",
            "-c:a",
            "copy",
            "-y",
            output.to_str().unwrap_or_default(),
        ])
        .status()
        .map_err(|e| EncoderError::UpscaleError(e.to_string()))?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&frames_dir);
    let _ = std::fs::remove_dir_all(&upscaled_dir);

    if !reassemble_status.success() {
        return Err(EncoderError::UpscaleError(
            "Video reassembly failed".to_string(),
        ));
    }

    Ok(())
}

/// Check if upscaling is needed based on input resolution
#[must_use]
pub fn needs_upscale(input_width: u32, input_height: u32, target: Resolution) -> bool {
    input_width < target.width || input_height < target.height
}

/// Probe input video resolution
pub fn probe_resolution(input: &Path) -> Result<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=s=x:p=0",
            input.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| EncoderError::FfmpegError(e.to_string()))?;

    let resolution_str = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = resolution_str.trim().split('x').collect();

    if parts.len() != 2 {
        return Err(EncoderError::InvalidInput(
            "Could not determine input resolution".to_string(),
        ));
    }

    let width = parts[0]
        .parse()
        .map_err(|_| EncoderError::InvalidInput("Invalid width".to_string()))?;
    let height = parts[1]
        .parse()
        .map_err(|_| EncoderError::InvalidInput("Invalid height".to_string()))?;

    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_upscale_smaller_input() {
        let target = Resolution::P1080;
        // Both dimensions smaller
        assert!(needs_upscale(1280, 720, target));
        // Only width smaller
        assert!(needs_upscale(1280, 1080, target));
        // Only height smaller
        assert!(needs_upscale(1920, 720, target));
    }

    #[test]
    fn needs_upscale_equal_or_larger() {
        let target = Resolution::P1080;
        // Exact match
        assert!(!needs_upscale(1920, 1080, target));
        // Larger input
        assert!(!needs_upscale(3840, 2160, target));
    }

    #[test]
    fn needs_upscale_common_resolutions() {
        // 480p to 1080p
        assert!(needs_upscale(854, 480, Resolution::P1080));
        // 720p to 1080p
        assert!(needs_upscale(1280, 720, Resolution::P1080));
        // 720p to 720p
        assert!(!needs_upscale(1280, 720, Resolution::P720));
    }

    #[test]
    fn resolution_from_target_values() {
        // 720p returns P720
        let r720 = Resolution::from_target(720);
        assert_eq!(r720.width, 1280);
        assert_eq!(r720.height, 720);

        // 1080p returns P1080
        let r1080 = Resolution::from_target(1080);
        assert_eq!(r1080.width, 1920);
        assert_eq!(r1080.height, 1080);

        // Unknown defaults to 1080p
        let r_default = Resolution::from_target(480);
        assert_eq!(r_default.width, 1920);
        assert_eq!(r_default.height, 1080);
    }
}
