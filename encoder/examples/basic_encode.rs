//! Basic Video Encoding Example
//!
//! Demonstrates the fundamental concepts of the video encoder pipeline:
//! - 4-tier codec strategy (AV1, VP9, H.264)
//! - Quality presets
//! - Rendition configuration
//!
//! Run with: `cargo run --example basic_encode`

use video_encoder::{Preset, Rendition, Tier, VideoCodec};

fn main() {
    println!("=== Video Encoder Pipeline - Basic Example ===\n");

    // -------------------------------------------------------------------------
    // 4-Tier Codec Strategy
    // -------------------------------------------------------------------------
    println!("## 4-Tier Codec Strategy\n");

    for tier in Tier::all() {
        let video_codec = tier.video_codec();
        let audio_codec = tier.audio_codec();
        let royalty_free = tier.is_royalty_free();
        let dir_name = tier.directory_name();

        println!(
            "  {tier:?}: {video_codec:?} + {audio_codec:?} (royalty-free: {royalty_free}) -> {dir_name}"
        );
    }

    // -------------------------------------------------------------------------
    // Quality Presets
    // -------------------------------------------------------------------------
    println!("\n## Quality Presets\n");

    let presets = [Preset::Fast, Preset::Balanced, Preset::Quality];

    for preset in presets {
        println!("  {preset:?}:");
        println!("    - AV1 preset: {}", preset.av1_preset());
        println!("    - VP9 cpu-used: {}", preset.vp9_cpu_used());
        println!("    - H.264 preset: {}", preset.h264_preset());
    }

    // -------------------------------------------------------------------------
    // Rendition Configuration
    // -------------------------------------------------------------------------
    println!("\n## Rendition Configuration\n");

    // Create renditions for different quality levels
    let renditions = [
        ("1080p", Rendition::new(1920, 1080, 8000, 128)),
        ("720p", Rendition::new(1280, 720, 4000, 96)),
        ("480p", Rendition::new(854, 480, 2000, 64)),
        ("360p", Rendition::new(640, 360, 1000, 48)),
    ];

    for (name, rendition) in &renditions {
        println!("  {} - {}:", name, rendition.dir_name());
        println!(
            "    Resolution: {}x{}, Video: {}kbps, Audio: {}kbps",
            rendition.width, rendition.height, rendition.video_bitrate_kbps, rendition.audio_bitrate_kbps
        );

        // Show QVBR parameters for each codec
        for codec in [VideoCodec::AV1, VideoCodec::VP9, VideoCodec::H264] {
            let params = rendition.qvbr_params(codec);
            println!("    {codec:?} QVBR: {params:?}");
        }
        println!();
    }

    // -------------------------------------------------------------------------
    // Codec-Specific Information
    // -------------------------------------------------------------------------
    println!("## Codec Information\n");

    let codecs = [VideoCodec::AV1, VideoCodec::VP9, VideoCodec::H264];

    for codec in codecs {
        println!("  {codec:?}:");
        println!("    - FFmpeg encoder: {}", codec.ffmpeg_encoder());
        println!("    - Codec string: {}", codec.codec_string("08M"));
    }

    println!("\n=== Example Complete ===");
}
