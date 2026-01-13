//! Benchmarks for configuration and codec operations
//!
//! These benchmarks measure the performance of core configuration
//! operations that are frequently called during encoding.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use video_encoder::config::{
    AbrLadder, EncryptionConfig, PreprocessConfig, Preset, Rendition, Tier, VideoCodec,
};

/// Benchmark Tier operations
fn bench_tier_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tier");

    group.bench_function("video_codec_lookup", |b| {
        let tiers = Tier::all();
        b.iter(|| {
            for tier in &tiers {
                black_box(tier.video_codec());
            }
        });
    });

    group.bench_function("audio_codec_lookup", |b| {
        let tiers = Tier::all();
        b.iter(|| {
            for tier in &tiers {
                black_box(tier.audio_codec());
            }
        });
    });

    group.bench_function("is_royalty_free", |b| {
        let tiers = Tier::all();
        b.iter(|| {
            for tier in &tiers {
                black_box(tier.is_royalty_free());
            }
        });
    });

    group.bench_function("directory_name", |b| {
        let tiers = Tier::all();
        b.iter(|| {
            for tier in &tiers {
                black_box(tier.directory_name());
            }
        });
    });

    group.finish();
}

/// Benchmark `VideoCodec` operations
fn bench_video_codec_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("VideoCodec");

    group.bench_function("ffmpeg_encoder", |b| {
        let codecs = [VideoCodec::AV1, VideoCodec::VP9, VideoCodec::H264];
        b.iter(|| {
            for codec in &codecs {
                black_box(codec.ffmpeg_encoder());
            }
        });
    });

    group.bench_function("codec_string", |b| {
        let codecs = [VideoCodec::AV1, VideoCodec::VP9, VideoCodec::H264];
        let profile = "08M";
        b.iter(|| {
            for codec in &codecs {
                black_box(codec.codec_string(profile));
            }
        });
    });

    group.finish();
}

/// Benchmark Rendition operations
fn bench_rendition_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Rendition");

    group.bench_function("qvbr_params_av1", |b| {
        let rendition = Rendition::new(1920, 1080, 8000, 128);
        b.iter(|| black_box(rendition.qvbr_params(VideoCodec::AV1)));
    });

    group.bench_function("qvbr_params_vp9", |b| {
        let rendition = Rendition::new(1920, 1080, 8000, 128);
        b.iter(|| black_box(rendition.qvbr_params(VideoCodec::VP9)));
    });

    group.bench_function("qvbr_params_h264", |b| {
        let rendition = Rendition::new(1920, 1080, 8000, 128);
        b.iter(|| black_box(rendition.qvbr_params(VideoCodec::H264)));
    });

    group.bench_function("dir_name", |b| {
        let rendition = Rendition::new(1920, 1080, 8000, 128);
        b.iter(|| black_box(rendition.dir_name()));
    });

    group.finish();
}

/// Benchmark ABR ladder operations
fn bench_abr_ladder_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("AbrLadder");

    group.bench_function("default_creation", |b| {
        b.iter(|| black_box(AbrLadder::default()));
    });

    group.bench_function("with_max_height_720", |b| {
        b.iter(|| black_box(AbrLadder::with_max_height(720)));
    });

    group.bench_function("with_max_height_480", |b| {
        b.iter(|| black_box(AbrLadder::with_max_height(480)));
    });

    group.finish();
}

/// Benchmark `PreprocessConfig` operations
fn bench_preprocess_config_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("PreprocessConfig");

    group.bench_function("default_creation", |b| {
        b.iter(|| black_box(PreprocessConfig::default()));
    });

    group.bench_function("all_enabled_creation", |b| {
        b.iter(|| black_box(PreprocessConfig::all_enabled()));
    });

    group.bench_function("broadcast_compliance_creation", |b| {
        b.iter(|| black_box(PreprocessConfig::broadcast_compliance()));
    });

    group.bench_function("has_broadcast_filters_check", |b| {
        let config = PreprocessConfig::broadcast_compliance();
        b.iter(|| black_box(config.has_broadcast_filters()));
    });

    group.finish();
}

/// Benchmark `EncryptionConfig` operations
fn bench_encryption_config_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("EncryptionConfig");

    group.bench_function("default_creation", |b| {
        b.iter(|| black_box(EncryptionConfig::default()));
    });

    group.bench_function("generated_keys_creation", |b| {
        b.iter(|| black_box(EncryptionConfig::new_with_generated_keys(true, true)));
    });

    group.bench_function("is_enabled_check", |b| {
        let config = EncryptionConfig::new_with_generated_keys(true, true);
        b.iter(|| black_box(config.is_enabled()));
    });

    group.finish();
}

/// Benchmark Preset operations
fn bench_preset_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Preset");

    let presets = [Preset::Fast, Preset::Balanced, Preset::Quality];

    group.bench_function("av1_preset_lookup", |b| {
        b.iter(|| {
            for preset in &presets {
                black_box(preset.av1_preset());
            }
        });
    });

    group.bench_function("vp9_cpu_used_lookup", |b| {
        b.iter(|| {
            for preset in &presets {
                black_box(preset.vp9_cpu_used());
            }
        });
    });

    group.bench_function("h264_preset_lookup", |b| {
        b.iter(|| {
            for preset in &presets {
                black_box(preset.h264_preset());
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tier_operations,
    bench_video_codec_operations,
    bench_rendition_operations,
    bench_abr_ladder_operations,
    bench_preprocess_config_operations,
    bench_encryption_config_operations,
    bench_preset_operations,
);

criterion_main!(benches);
