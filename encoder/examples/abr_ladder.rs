//! Adaptive Bitrate (ABR) Ladder Example
//!
//! Demonstrates ABR ladder configuration for HLS/DASH streaming:
//! - Default ABR ladder (1080p to 360p)
//! - Max resolution constraints
//! - Per-tier rendition configuration
//!
//! Run with: `cargo run --example abr_ladder`

use video_encoder::{AbrLadder, Tier};

fn main() {
    println!("=== Video Encoder Pipeline - ABR Ladder Example ===\n");

    // -------------------------------------------------------------------------
    // Default ABR Ladder
    // -------------------------------------------------------------------------
    println!("## Default ABR Ladder (Full Resolution Range)\n");

    let default_ladder = AbrLadder::default();
    print_ladder("Default", &default_ladder);

    // -------------------------------------------------------------------------
    // Max Resolution Constraints
    // -------------------------------------------------------------------------
    println!("## ABR Ladder with Max Height Constraints\n");

    // 720p max - suitable for mobile or bandwidth-constrained delivery
    let ladder_720p = AbrLadder::with_max_height(720);
    print_ladder("Max 720p", &ladder_720p);

    // 480p max - suitable for very low bandwidth scenarios
    let ladder_480p = AbrLadder::with_max_height(480);
    print_ladder("Max 480p", &ladder_480p);

    // -------------------------------------------------------------------------
    // Per-Tier Output Structure
    // -------------------------------------------------------------------------
    println!("## Output Directory Structure by Tier\n");

    let ladder = AbrLadder::default();

    for tier in Tier::all() {
        let codec = tier.video_codec();
        let dir = tier.directory_name();

        println!("  {dir}/ ({codec:?}):");

        for rendition in &ladder.renditions {
            let params = rendition.qvbr_params(codec);
            println!(
                "    {}x{}/  ({}kbps video, {}kbps audio)",
                rendition.width,
                rendition.height,
                rendition.video_bitrate_kbps,
                rendition.audio_bitrate_kbps
            );
            println!("      QVBR params: {params:?}");
        }
        println!();
    }

    // -------------------------------------------------------------------------
    // Bandwidth Estimation
    // -------------------------------------------------------------------------
    println!("## Total Bandwidth Requirements\n");

    let ladder = AbrLadder::default();

    for tier in Tier::all() {
        let codec = tier.video_codec();
        let mut total_video_kbps = 0;
        let mut total_audio_kbps = 0;

        for rendition in &ladder.renditions {
            total_video_kbps += rendition.video_bitrate_kbps;
            total_audio_kbps += rendition.audio_bitrate_kbps;
        }

        let combined_kbps = total_video_kbps + total_audio_kbps;
        let bandwidth_mbps = f64::from(combined_kbps) / 1000.0;

        println!("  {} ({:?}):", tier.directory_name(), codec);
        println!("    Video: {total_video_kbps} kbps, Audio: {total_audio_kbps} kbps");
        println!("    Total: {bandwidth_mbps:.2} Mbps (for all renditions)\n");
    }

    println!("=== Example Complete ===");
}

fn print_ladder(name: &str, ladder: &AbrLadder) {
    println!(
        "  {} Ladder ({} renditions):",
        name,
        ladder.renditions.len()
    );

    for rendition in &ladder.renditions {
        let label = match rendition.height {
            1080 => "1080p",
            720 => "720p",
            480 => "480p",
            360 => "360p",
            _ => "custom",
        };

        println!(
            "    - {} ({}x{}): {}kbps video + {}kbps audio",
            label,
            rendition.width,
            rendition.height,
            rendition.video_bitrate_kbps,
            rendition.audio_bitrate_kbps
        );
    }
    println!();
}
