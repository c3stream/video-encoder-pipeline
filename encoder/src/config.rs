//! Encoding configuration and codec definitions

use serde::{Deserialize, Serialize};

/// Encoding tier definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    /// AV1 + Opus - Fully royalty-free, best compression
    Tier1,
    /// VP9 + Opus - Fully royalty-free, wide support
    Tier2,
    /// VP9 + AAC - Video royalty-free, iOS 14+ compatible
    Tier3,
    /// H.264 + AAC - Universal fallback
    Tier4,
}

impl Tier {
    #[must_use]
    pub fn video_codec(&self) -> VideoCodec {
        match self {
            Self::Tier1 => VideoCodec::AV1,
            Self::Tier2 | Self::Tier3 => VideoCodec::VP9,
            Self::Tier4 => VideoCodec::H264,
        }
    }

    #[must_use]
    pub fn audio_codec(&self) -> AudioCodec {
        match self {
            Self::Tier1 | Self::Tier2 => AudioCodec::Opus,
            Self::Tier3 | Self::Tier4 => AudioCodec::AAC,
        }
    }

    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![Self::Tier1, Self::Tier2, Self::Tier3, Self::Tier4]
    }

    #[must_use]
    pub fn is_royalty_free(&self) -> bool {
        matches!(self, Self::Tier1 | Self::Tier2)
    }

    #[must_use]
    pub fn directory_name(&self) -> &'static str {
        match self {
            Self::Tier1 => "av1_opus",
            Self::Tier2 => "vp9_opus",
            Self::Tier3 => "vp9_aac",
            Self::Tier4 => "h264_aac",
        }
    }
}

/// Video codec options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoCodec {
    AV1,
    VP9,
    H264,
}

impl VideoCodec {
    #[must_use]
    pub fn ffmpeg_encoder(&self) -> &'static str {
        match self {
            Self::AV1 => "libsvtav1",
            Self::VP9 => "libvpx-vp9",
            Self::H264 => "libx264",
        }
    }

    #[must_use]
    pub fn codec_string(&self, profile: &str) -> String {
        match self {
            Self::AV1 => format!("av01.0.{profile}.08"),
            Self::VP9 => format!("vp09.00.{profile}.08"),
            Self::H264 => format!("avc1.{profile}"),
        }
    }
}

/// Audio codec options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum AudioCodec {
    Opus,
    AAC,
}

impl AudioCodec {
    #[must_use]
    pub fn ffmpeg_encoder(&self) -> &'static str {
        match self {
            Self::Opus => "libopus",
            Self::AAC => "aac",
        }
    }

    #[must_use]
    pub fn codec_string(&self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::AAC => "mp4a.40.2",
        }
    }

    #[must_use]
    pub fn bitrate(&self) -> &'static str {
        match self {
            Self::Opus | Self::AAC => "128k",
        }
    }
}

/// Audio quality/bitrate ladder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioLadder {
    pub bitrates: Vec<AudioBitrate>,
}

impl Default for AudioLadder {
    fn default() -> Self {
        Self {
            bitrates: vec![
                AudioBitrate::new(256, "High"),
                AudioBitrate::new(128, "Standard"),
                AudioBitrate::new(64, "Low"),
            ],
        }
    }
}

/// Single audio bitrate rendition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBitrate {
    pub kbps: u32,
    pub name: String,
}

impl AudioBitrate {
    #[must_use]
    pub fn new(kbps: u32, name: &str) -> Self {
        Self {
            kbps,
            name: name.to_string(),
        }
    }

    /// Directory name for this audio bitrate
    #[must_use]
    pub fn dir_name(&self) -> String {
        format!("{}k", self.kbps)
    }
}

/// Pre-processing filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessConfig {
    /// Enable audio normalization (loudnorm EBU R128)
    pub audio_normalize: bool,
    /// Target loudness in LUFS (default: -14 for streaming)
    pub target_lufs: f32,
    /// Enable video deflicker filter (legacy, consider using `fluorescent_deflicker`)
    pub video_deflicker: bool,
    /// Deflicker mode: 0=none, 1=slight, 2=medium, 3=strong, 4=extreme (legacy)
    pub deflicker_mode: u8,
    /// Fluorescent light deflicker (power frequency aware)
    /// More accurate than legacy deflicker for fluorescent/LED light flicker
    pub fluorescent_deflicker: FluorescentDeflicker,
    /// Enable video deblock filter
    pub video_deblock: bool,
    /// Deblock strength (0-100)
    pub deblock_strength: u8,
    /// Enable audio noise reduction
    pub audio_denoise: bool,
    /// Audio noise reduction strength (0.0-1.0)
    pub audio_denoise_strength: f32,
    /// Enable video noise reduction (hqdn3d)
    pub video_denoise: bool,
    /// Video noise reduction strength: light, medium, strong
    pub video_denoise_strength: DenoiseStrength,
    /// Enable photosensitivity filter (anti-flash/strobe protection)
    /// Prevents rapid brightness changes that could trigger seizures (Polygon Shock prevention)
    pub photosensitivity_filter: bool,
    /// Photosensitivity protection level
    pub photosensitivity_level: PhotosensitivityLevel,

    // === Ofcom/ITU Broadcast Compliance Filters ===

    /// Enable red flash saturation filter (Ofcom Harding test compliance)
    /// Reduces saturated red content that is most dangerous for photosensitive viewers
    pub red_flash_filter: bool,
    /// Red saturation reduction level
    pub red_flash_level: RedFlashLevel,

    /// Enable saturated color limiter (ITU-R BT.2020 compliance)
    /// Limits overly saturated colors that may cause discomfort
    pub color_saturation_limiter: bool,
    /// Maximum saturation level (0.0-1.0, default 0.9)
    pub max_saturation: f32,

    /// Enable spatial pattern filter (Ofcom guidelines)
    /// Reduces high-contrast striped patterns that can trigger seizures
    pub spatial_pattern_filter: bool,
    /// Spatial pattern filter strength
    pub spatial_pattern_strength: SpatialPatternStrength,

    /// Enable audio loudness range control (EBU R128 LRA compliance)
    /// Limits dynamic range to prevent sudden loud sounds
    pub audio_loudness_range: bool,
    /// Target loudness range in LU (default: 7 for broadcast)
    pub target_lra: f32,

    /// Enable audio peak limiter (sudden sound protection)
    /// Prevents sudden loud sounds that could startle viewers
    pub audio_peak_limiter: bool,
    /// Maximum peak level in dBTP (default: -1.0)
    pub max_peak_dbtp: f32,
    /// Attack time in ms for peak limiter
    pub peak_attack_ms: f32,
    /// Release time in ms for peak limiter
    pub peak_release_ms: f32,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            audio_normalize: false,
            target_lufs: -14.0,
            video_deflicker: false,
            deflicker_mode: 2,
            fluorescent_deflicker: FluorescentDeflicker::default(),
            video_deblock: false,
            deblock_strength: 15,
            audio_denoise: false,
            audio_denoise_strength: 0.21,
            video_denoise: false,
            video_denoise_strength: DenoiseStrength::Medium,
            photosensitivity_filter: false,
            photosensitivity_level: PhotosensitivityLevel::Standard,
            // Broadcast compliance filters (Ofcom/ITU)
            red_flash_filter: false,
            red_flash_level: RedFlashLevel::Standard,
            color_saturation_limiter: false,
            max_saturation: 1.0,
            spatial_pattern_filter: false,
            spatial_pattern_strength: SpatialPatternStrength::Standard,
            audio_loudness_range: false,
            target_lra: 7.0,
            audio_peak_limiter: false,
            max_peak_dbtp: -1.0,
            peak_attack_ms: 5.0,
            peak_release_ms: 50.0,
        }
    }
}

impl PreprocessConfig {
    /// Create config with all preprocessing enabled
    #[must_use]
    pub fn all_enabled() -> Self {
        Self {
            audio_normalize: true,
            target_lufs: -14.0,
            video_deflicker: false, // Disabled in favor of fluorescent_deflicker
            deflicker_mode: 2,
            fluorescent_deflicker: FluorescentDeflicker {
                enabled: true,
                power_frequency: PowerFrequency::Auto,
                strength: DeflickerStrength::Medium,
                source_framerate: None,
            },
            video_deblock: true,
            deblock_strength: 15,
            audio_denoise: true,
            audio_denoise_strength: 0.21,
            video_denoise: true,
            video_denoise_strength: DenoiseStrength::Medium,
            photosensitivity_filter: true,
            photosensitivity_level: PhotosensitivityLevel::Standard,
            // Broadcast compliance filters (Ofcom/ITU) - all enabled
            red_flash_filter: true,
            red_flash_level: RedFlashLevel::Standard,
            color_saturation_limiter: true,
            max_saturation: 1.0,
            spatial_pattern_filter: true,
            spatial_pattern_strength: SpatialPatternStrength::Standard,
            audio_loudness_range: true,
            target_lra: 7.0,
            audio_peak_limiter: true,
            max_peak_dbtp: -1.0,
            peak_attack_ms: 5.0,
            peak_release_ms: 50.0,
        }
    }

    /// Create config with broadcast compliance filters only (Ofcom/ITU standards)
    #[must_use]
    pub fn broadcast_compliance() -> Self {
        Self {
            audio_normalize: true,
            target_lufs: -23.0, // EBU R128 broadcast standard
            video_deflicker: false,
            deflicker_mode: 2,
            fluorescent_deflicker: FluorescentDeflicker::default(),
            video_deblock: false,
            deblock_strength: 15,
            audio_denoise: false,
            audio_denoise_strength: 0.21,
            video_denoise: false,
            video_denoise_strength: DenoiseStrength::Medium,
            photosensitivity_filter: true,
            photosensitivity_level: PhotosensitivityLevel::Standard,
            // Broadcast compliance filters - all enabled for broadcast
            red_flash_filter: true,
            red_flash_level: RedFlashLevel::Standard,
            color_saturation_limiter: true,
            max_saturation: 0.85, // Stricter for broadcast
            spatial_pattern_filter: true,
            spatial_pattern_strength: SpatialPatternStrength::Standard,
            audio_loudness_range: true,
            target_lra: 7.0, // EBU R128 broadcast LRA
            audio_peak_limiter: true,
            max_peak_dbtp: -1.0,
            peak_attack_ms: 5.0,
            peak_release_ms: 50.0,
        }
    }

    /// Create config optimized for fluorescent light environments
    /// Use this for videos recorded under fluorescent/LED lighting
    #[must_use]
    pub fn fluorescent_light(power_freq: PowerFrequency) -> Self {
        Self {
            audio_normalize: true,
            target_lufs: -14.0,
            video_deflicker: false,
            deflicker_mode: 2,
            fluorescent_deflicker: FluorescentDeflicker {
                enabled: true,
                power_frequency: power_freq,
                strength: DeflickerStrength::Medium,
                source_framerate: None,
            },
            video_deblock: false,
            deblock_strength: 15,
            audio_denoise: false,
            audio_denoise_strength: 0.21,
            video_denoise: false,
            video_denoise_strength: DenoiseStrength::Medium,
            photosensitivity_filter: false,
            photosensitivity_level: PhotosensitivityLevel::Standard,
            red_flash_filter: false,
            red_flash_level: RedFlashLevel::Standard,
            color_saturation_limiter: false,
            max_saturation: 1.0,
            spatial_pattern_filter: false,
            spatial_pattern_strength: SpatialPatternStrength::Standard,
            audio_loudness_range: false,
            target_lra: 7.0,
            audio_peak_limiter: false,
            max_peak_dbtp: -1.0,
            peak_attack_ms: 5.0,
            peak_release_ms: 50.0,
        }
    }

    /// Preset for Eastern Japan (50Hz power grid)
    #[must_use]
    pub fn eastern_japan() -> Self {
        Self::fluorescent_light(PowerFrequency::Hz50)
    }

    /// Preset for Western Japan (60Hz power grid)
    #[must_use]
    pub fn western_japan() -> Self {
        Self::fluorescent_light(PowerFrequency::Hz60)
    }

    /// Build `FFmpeg` video filter chain
    ///
    /// # Arguments
    /// * `scale` - Optional target resolution (width, height)
    /// * `framerate` - Source video framerate (used for fluorescent deflicker calculation)
    #[must_use]
    pub fn video_filter_chain_with_framerate(
        &self,
        scale: Option<(u32, u32)>,
        framerate: f64,
    ) -> String {
        let mut filters = Vec::new();

        // Video denoising (hqdn3d) - apply first
        if self.video_denoise {
            let params = match self.video_denoise_strength {
                DenoiseStrength::Light => "1.5:1.5:3:3",
                DenoiseStrength::Medium => "3:3:4:4",
                DenoiseStrength::Strong => "6:6:6:6",
            };
            filters.push(format!("hqdn3d={params}"));
        }

        // Fluorescent light deflicker (new, power-frequency aware)
        // Takes priority over legacy deflicker if enabled
        if let Some(deflicker_filter) = self.fluorescent_deflicker.filter_string(framerate) {
            filters.push(deflicker_filter);
        } else if self.video_deflicker {
            // Legacy deflicker - helps with flicker from different lighting/frame rates
            // deflicker filter: size=frames, mode=arithmetic mean
            let size = match self.deflicker_mode {
                0 => return filters.join(","),
                1 => 3,
                2 => 5,
                3 => 7,
                _ => 9,
            };
            filters.push(format!("deflicker=size={size}:mode=am"));
        }

        self.build_remaining_video_filters(&mut filters, scale);
        filters.join(",")
    }

    /// Build `FFmpeg` video filter chain (legacy, assumes 30fps for fluorescent deflicker)
    #[must_use]
    pub fn video_filter_chain(&self, scale: Option<(u32, u32)>) -> String {
        // Default to 30fps for backward compatibility
        self.video_filter_chain_with_framerate(scale, 30.0)
    }

    /// Internal helper to build remaining video filters after deflicker
    fn build_remaining_video_filters(&self, filters: &mut Vec<String>, scale: Option<(u32, u32)>) {

        // Photosensitivity filter - prevents rapid brightness changes (Polygon Shock prevention)
        // Uses FFmpeg's built-in photosensitivity filter which is designed for this purpose
        // Based on ITU-R BT.1702 and Ofcom guidelines (max 3 flashes/second, 25% screen area)
        //
        // IMPORTANT: High threshold values = less intervention (only extreme flashes detected)
        // Low threshold = more intervention (normal scene changes may be affected)
        // For web streaming, use very high thresholds to avoid affecting normal content
        if self.photosensitivity_filter {
            // Parameters based on protection level:
            // - frames: number of frames to analyze (lower = faster response, less smoothing)
            // - threshold: sensitivity (HIGHER = less intervention, only extreme flashes)
            // Web streaming should use high thresholds to avoid scene change artifacts
            let (frames, threshold) = match self.photosensitivity_level {
                PhotosensitivityLevel::Light => (15, 2.0),     // Very minimal - only extreme strobe
                PhotosensitivityLevel::Standard => (20, 1.5), // Moderate - dangerous flashes only
                PhotosensitivityLevel::Strict => (30, 1.0),   // Stricter - more flash detection
            };

            // FFmpeg photosensitivity filter - only applies correction when extreme flash detected
            // High threshold ensures normal scene changes are NOT affected
            filters.push(format!("photosensitivity=frames={frames}:threshold={threshold:.1}"));

            // Note: Removed limiter as it can affect scene brightness transitions
        }

        // Deblock - reduce blocking artifacts
        if self.video_deblock {
            let strength = self.deblock_strength.min(100);
            let filter_str = f32::from(strength) / 100.0;
            filters.push(format!("deblock=filter={filter_str:.2}:block=8"));
        }

        // === Ofcom/ITU Broadcast Compliance Video Filters ===

        // Red flash saturation filter (Ofcom Harding test / ITU-R BT.1702)
        // Saturated red is the most dangerous color for photosensitive epilepsy
        // Definition of "saturated red": R/(R+G+B) >= 0.8 per industry standard
        //
        // NOTE: For web streaming of normal content (not strobe-heavy music videos etc.),
        // this filter is usually NOT needed and can desaturate colors unnecessarily.
        // Only apply for content known to contain rapid red flashing.
        if self.red_flash_filter {
            // Very gentle reduction - only targets extremely saturated reds
            // Most normal content will be unaffected
            let red_reduction = match self.red_flash_level {
                RedFlashLevel::Light => 5,      // 5% - barely noticeable
                RedFlashLevel::Standard => 10,  // 10% - subtle
                RedFlashLevel::Strict => 20,    // 20% - noticeable for dangerous content
            };
            // selectivecolor: only affects pixels in the pure red hue range
            filters.push(format!(
                "selectivecolor=reds='cyan=0 magenta=0 yellow=0 black=-{red_reduction}'"
            ));
        }

        // Saturated color limiter (ITU-R BT.709/BT.2020 compliance)
        // NOTE: For web streaming, this is usually NOT needed.
        // Only useful when converting HDR content to SDR or for strict broadcast compliance.
        // A value of 1.0 = no change, lower values reduce saturation.
        if self.color_saturation_limiter {
            // Clamp to 0.9-1.0 range to prevent over-desaturation
            // Default 0.98 is essentially invisible
            let max_sat = self.max_saturation.clamp(0.9, 1.0);
            if max_sat < 1.0 {
                filters.push(format!("eq=saturation={max_sat:.2}"));
            }
            // If max_sat >= 1.0, skip the filter entirely
        }

        // Spatial pattern filter (Ofcom guidelines)
        // High-contrast striped/regular patterns can trigger seizures
        // Uses a slight blur to reduce sharp pattern edges
        if self.spatial_pattern_filter {
            let (blur_strength, unsharp_strength) = match self.spatial_pattern_strength {
                SpatialPatternStrength::Light => (0.5, 0.3),
                SpatialPatternStrength::Standard => (1.0, 0.5),
                SpatialPatternStrength::Strong => (1.5, 0.7),
            };
            // Apply slight gaussian blur to break up regular patterns
            // Then apply mild unsharp to restore some detail
            filters.push(format!("gblur=sigma={blur_strength:.1}"));
            filters.push(format!("unsharp=5:5:{unsharp_strength:.1}:5:5:0"));
        }

        // Scale filter - add at the end
        if let Some((width, height)) = scale {
            filters.push(format!("scale={width}:{height}"));
        }
    }

    /// Build `FFmpeg` audio filter chain
    #[must_use]
    pub fn audio_filter_chain(&self) -> String {
        let mut filters = Vec::new();

        // Audio denoising using afftdn (FFT-based denoiser)
        if self.audio_denoise {
            let nr = (self.audio_denoise_strength * 40.0) as i32;
            filters.push(format!("afftdn=nr={}:nf=-25", nr.clamp(1, 40)));
        }

        // === Ofcom/ITU Broadcast Compliance Audio Filters ===

        // Audio peak limiter (sudden sound protection)
        // Prevents sudden loud sounds that could startle viewers
        // Uses compand filter for soft-knee limiting
        if self.audio_peak_limiter {
            // compand: attack|decay  transfer-function-points
            // Fast attack to catch transients, moderate release for natural sound
            let attack_s = self.peak_attack_ms / 1000.0;
            let release_s = self.peak_release_ms / 1000.0;
            // Convert dBTP to linear threshold for the limiter
            let threshold_db = self.max_peak_dbtp;
            filters.push(format!(
                "compand=attacks={}:decays={}:points=-80/-80|{}/-80|0/{}:soft-knee=6",
                attack_s, release_s, threshold_db - 6.0, threshold_db
            ));
        }

        // Loudness normalization with LRA control (EBU R128)
        // audio_loudness_range controls dynamic range compression
        // NOTE: Without measured_* parameters, loudnorm will analyze the input automatically
        // This is slower but more accurate than assuming fixed input measurements
        if self.audio_normalize || self.audio_loudness_range {
            let target_i = if self.audio_normalize { self.target_lufs } else { -23.0 };
            let target_lra = if self.audio_loudness_range { self.target_lra } else { 11.0 };
            // Use print_format=summary for single-pass mode without pre-measured values
            // This automatically analyzes input and normalizes to target
            filters.push(format!(
                "loudnorm=I={target_i}:LRA={target_lra}:TP=-1.0:print_format=summary"
            ));
        }

        filters.join(",")
    }

    /// Check if any broadcast compliance filters are enabled
    #[must_use]
    pub fn has_broadcast_filters(&self) -> bool {
        self.red_flash_filter
            || self.color_saturation_limiter
            || self.spatial_pattern_filter
            || self.audio_loudness_range
            || self.audio_peak_limiter
            || self.photosensitivity_filter
    }
}

/// Video denoising strength
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenoiseStrength {
    Light,
    Medium,
    Strong,
}

/// Power line frequency for fluorescent light flicker removal
///
/// Fluorescent lights flicker at twice the power line frequency:
/// - 50Hz regions (Eastern Japan, Europe, etc.): 100Hz flicker
/// - 60Hz regions (Western Japan, Americas, etc.): 120Hz flicker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum PowerFrequency {
    /// 50Hz power line (Eastern Japan, Europe, Australia, most of Asia/Africa)
    /// Results in 100Hz light flicker
    Hz50,
    /// 60Hz power line (Western Japan, Americas, Taiwan, Korea, Philippines)
    /// Results in 120Hz light flicker
    Hz60,
    /// Auto-detect based on content analysis (uses deflicker's built-in detection)
    #[default]
    Auto,
}


impl PowerFrequency {
    /// Calculate optimal deflicker frame window size based on framerate
    ///
    /// The goal is to average over at least one complete flicker cycle.
    /// Flicker frequency = 2 × power frequency (100Hz or 120Hz)
    /// Frames per cycle = framerate / `flicker_frequency`
    /// Window size should cover 1-2 complete cycles for smooth results
    #[must_use]
    pub fn optimal_window_size(&self, framerate: f64) -> u32 {
        let flicker_freq = match self {
            Self::Hz50 => 100.0,  // 50Hz × 2
            Self::Hz60 => 120.0,  // 60Hz × 2
            Self::Auto => 110.0,  // Middle ground
        };

        // Frames per flicker cycle
        let frames_per_cycle = framerate / flicker_freq;

        // Window should cover ~2 cycles, minimum 3 frames, maximum 15
        let window = (frames_per_cycle * 2.0).ceil() as u32;
        window.clamp(3, 15)
    }

    /// Get human-readable description
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Hz50 => "50Hz (Eastern Japan, Europe)",
            Self::Hz60 => "60Hz (Western Japan, Americas)",
            Self::Auto => "Auto-detect",
        }
    }
}

/// Fluorescent light deflicker configuration
///
/// Specialized settings for removing flicker caused by fluorescent/LED lights
/// that are powered by AC current and flicker at the power line frequency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluorescentDeflicker {
    /// Enable fluorescent-specific deflicker processing
    pub enabled: bool,
    /// Power line frequency of the region where the video was recorded
    pub power_frequency: PowerFrequency,
    /// Strength of the deflicker effect
    pub strength: DeflickerStrength,
    /// Source video framerate (used to calculate optimal window)
    /// If None, will be auto-detected from input
    pub source_framerate: Option<f64>,
}

impl Default for FluorescentDeflicker {
    fn default() -> Self {
        Self {
            enabled: false,
            power_frequency: PowerFrequency::Auto,
            strength: DeflickerStrength::Medium,
            source_framerate: None,
        }
    }
}

impl FluorescentDeflicker {
    /// Create config for Eastern Japan / Europe (50Hz)
    #[must_use]
    pub fn hz50() -> Self {
        Self {
            enabled: true,
            power_frequency: PowerFrequency::Hz50,
            strength: DeflickerStrength::Medium,
            source_framerate: None,
        }
    }

    /// Create config for Western Japan / Americas (60Hz)
    #[must_use]
    pub fn hz60() -> Self {
        Self {
            enabled: true,
            power_frequency: PowerFrequency::Hz60,
            strength: DeflickerStrength::Medium,
            source_framerate: None,
        }
    }

    /// Build `FFmpeg` filter string for fluorescent deflicker
    #[must_use]
    pub fn filter_string(&self, detected_framerate: f64) -> Option<String> {
        if !self.enabled {
            return None;
        }

        let framerate = self.source_framerate.unwrap_or(detected_framerate);
        let window_size = self.power_frequency.optimal_window_size(framerate);

        // Apply strength modifier to window size
        let adjusted_window = match self.strength {
            DeflickerStrength::Light => window_size.saturating_sub(1).max(3),
            DeflickerStrength::Medium => window_size,
            DeflickerStrength::Strong => window_size + 2,
            DeflickerStrength::Extreme => window_size + 4,
        };

        // Use arithmetic mean mode for smoother results
        Some(format!("deflicker=size={}:mode=am", adjusted_window.min(15)))
    }
}

/// Deflicker strength level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum DeflickerStrength {
    /// Light - minimal smoothing, preserves more detail
    Light,
    /// Medium - balanced (recommended for most cases)
    #[default]
    Medium,
    /// Strong - aggressive smoothing for severe flicker
    Strong,
    /// Extreme - maximum smoothing (may cause motion blur)
    Extreme,
}


/// Photosensitivity protection level (Polygon Shock prevention)
/// Based on guidelines similar to Ofcom/ITU recommendations for broadcast
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhotosensitivityLevel {
    /// Light protection - minimal impact on video quality
    /// Suitable for content with occasional bright scenes
    Light,
    /// Standard protection - balanced approach
    /// Recommended for general use, prevents most flash patterns
    Standard,
    /// Strict protection - maximum safety
    /// For content with known strobing/flashing, or for sensitive audiences
    /// May noticeably affect fast action scenes
    Strict,
}

/// Red flash filter level (Ofcom Harding test compliance)
/// The Ofcom Harding test specifically identifies saturated red as most dangerous
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum RedFlashLevel {
    /// Light - reduces red saturation by 15%
    Light,
    /// Standard - reduces red saturation by 25% (broadcast recommended)
    #[default]
    Standard,
    /// Strict - reduces red saturation by 40%
    Strict,
}


/// Spatial pattern filter strength (Ofcom guidelines)
/// High-contrast regular patterns (especially stripes) can trigger seizures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum SpatialPatternStrength {
    /// Light - subtle pattern smoothing
    Light,
    /// Standard - moderate pattern reduction
    #[default]
    Standard,
    /// Strong - aggressive pattern suppression
    Strong,
}


/// Encoding preset (speed vs quality tradeoff)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    Fast,
    Balanced,
    Quality,
}

impl Preset {
    #[must_use]
    pub fn av1_preset(&self) -> u8 {
        match self {
            Self::Fast => 10,
            Self::Balanced => 6,
            Self::Quality => 3,
        }
    }

    #[must_use]
    pub fn vp9_cpu_used(&self) -> i8 {
        match self {
            Self::Fast => 4,
            Self::Balanced => 2,
            Self::Quality => 0,
        }
    }

    #[must_use]
    pub fn h264_preset(&self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "medium",
            Self::Quality => "slow",
        }
    }
}

/// Resolution configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const P720: Self = Self {
        width: 1280,
        height: 720,
    };
    pub const P1080: Self = Self {
        width: 1920,
        height: 1080,
    };

    #[must_use]
    pub fn from_target(target: u32) -> Self {
        match target {
            720 => Self::P720,
            _ => Self::P1080,
        }
    }

    /// Calculate target bitrate based on resolution and codec
    #[must_use]
    pub fn target_bitrate(&self, codec: VideoCodec) -> u32 {
        let base = match self.height {
            720 => 2_500_000,
            _ => 5_000_000, // 1080p and other resolutions default to 5Mbps
        };

        // AV1 and VP9 are more efficient
        match codec {
            VideoCodec::AV1 => base * 60 / 100,
            VideoCodec::VP9 => base * 70 / 100,
            VideoCodec::H264 => base,
        }
    }
}

/// CMAF segment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub duration_secs: u32,
    pub fragment_duration_ms: u32,
}

impl Default for SegmentConfig {
    fn default() -> Self {
        Self {
            duration_secs: 4,
            fragment_duration_ms: 1000,
        }
    }
}

/// ABR (Adaptive Bitrate) ladder configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbrLadder {
    pub renditions: Vec<Rendition>,
}

impl Default for AbrLadder {
    fn default() -> Self {
        Self {
            renditions: vec![
                // 1080p
                Rendition::new(1920, 1080, 6000, 128),
                // 720p
                Rendition::new(1280, 720, 3000, 128),
                // 480p
                Rendition::new(854, 480, 1500, 96),
                // 360p
                Rendition::new(640, 360, 800, 64),
            ],
        }
    }
}

impl AbrLadder {
    /// Create ABR ladder with max resolution constraint
    #[must_use]
    pub fn with_max_height(max_height: u32) -> Self {
        let all = Self::default();
        Self {
            renditions: all
                .renditions
                .into_iter()
                .filter(|r| r.height <= max_height)
                .collect(),
        }
    }
}

/// Single rendition in ABR ladder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rendition {
    pub width: u32,
    pub height: u32,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
}

impl Rendition {
    #[must_use]
    pub const fn new(width: u32, height: u32, video_bitrate_kbps: u32, audio_bitrate_kbps: u32) -> Self {
        Self {
            width,
            height,
            video_bitrate_kbps,
            audio_bitrate_kbps,
        }
    }

    /// Directory name for this rendition
    #[must_use]
    pub fn dir_name(&self) -> String {
        format!("{}p", self.height)
    }

    /// Calculate QVBR parameters (CRF with maxrate constraint)
    #[must_use]
    pub fn qvbr_params(&self, codec: VideoCodec) -> QvbrParams {
        // QVBR uses CRF with maxrate to maintain quality while respecting bitrate ceiling
        let crf = match codec {
            VideoCodec::AV1 => 30,
            VideoCodec::VP9 => 31,
            VideoCodec::H264 => 23,
        };

        // Codec efficiency factors
        let efficiency = match codec {
            VideoCodec::AV1 => 0.6,
            VideoCodec::VP9 => 0.75,
            VideoCodec::H264 => 1.0,
        };

        let target_kbps = (f64::from(self.video_bitrate_kbps) * efficiency) as u32;
        let maxrate_kbps = target_kbps * 150 / 100;
        let bufsize_kbps = target_kbps * 200 / 100;

        QvbrParams {
            crf,
            maxrate_kbps,
            bufsize_kbps,
        }
    }
}

/// QVBR (Quality-defined Variable Bitrate) parameters
#[derive(Debug, Clone)]
pub struct QvbrParams {
    pub crf: u8,
    pub maxrate_kbps: u32,
    pub bufsize_kbps: u32,
}

/// Rate control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum RateControl {
    /// Constant Rate Factor (quality-based, variable bitrate)
    Crf,
    /// Quality-defined Variable Bitrate (CRF with maxrate constraint)
    #[default]
    Qvbr,
    /// Constant Bitrate
    Cbr,
}


/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct EncryptionConfig {
    /// Enable HLS AES-128 encryption
    pub hls_aes128: bool,
    /// Enable DASH `ClearKey` encryption
    pub dash_clearkey: bool,
    /// Key ID (16 bytes hex)
    pub key_id: Option<String>,
    /// Content encryption key (16 bytes hex)
    pub key: Option<String>,
    /// Key server URL for HLS
    pub key_url: Option<String>,
}


impl EncryptionConfig {
    /// Create encryption config with auto-generated keys
    #[must_use]
    pub fn new_with_generated_keys(hls: bool, dash: bool) -> Self {
        use std::fmt::Write;

        // Generate random 16-byte key and key_id
        let key_bytes: [u8; 16] = rand_bytes();
        let key_id_bytes: [u8; 16] = rand_bytes();

        let mut key = String::with_capacity(32);
        let mut key_id = String::with_capacity(32);

        for b in key_bytes {
            write!(key, "{b:02x}").unwrap();
        }
        for b in key_id_bytes {
            write!(key_id, "{b:02x}").unwrap();
        }

        Self {
            hls_aes128: hls,
            dash_clearkey: dash,
            key_id: Some(key_id),
            key: Some(key),
            key_url: None,
        }
    }

    /// Check if any encryption is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.hls_aes128 || self.dash_clearkey
    }
}

/// Generate random bytes for encryption keys
fn rand_bytes<const N: usize>() -> [u8; N] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let mut state = seed;
    let mut bytes = [0u8; N];

    for byte in &mut bytes {
        // Simple xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // Tier Tests
    // ==========================================

    #[test]
    fn tier_video_codec_mapping() {
        assert_eq!(Tier::Tier1.video_codec(), VideoCodec::AV1);
        assert_eq!(Tier::Tier2.video_codec(), VideoCodec::VP9);
        assert_eq!(Tier::Tier3.video_codec(), VideoCodec::VP9);
        assert_eq!(Tier::Tier4.video_codec(), VideoCodec::H264);
    }

    #[test]
    fn tier_audio_codec_mapping() {
        assert_eq!(Tier::Tier1.audio_codec(), AudioCodec::Opus);
        assert_eq!(Tier::Tier2.audio_codec(), AudioCodec::Opus);
        assert_eq!(Tier::Tier3.audio_codec(), AudioCodec::AAC);
        assert_eq!(Tier::Tier4.audio_codec(), AudioCodec::AAC);
    }

    #[test]
    fn tier_all_returns_four_tiers() {
        let tiers = Tier::all();
        assert_eq!(tiers.len(), 4);
        assert!(tiers.contains(&Tier::Tier1));
        assert!(tiers.contains(&Tier::Tier2));
        assert!(tiers.contains(&Tier::Tier3));
        assert!(tiers.contains(&Tier::Tier4));
    }

    #[test]
    fn tier_royalty_free_classification() {
        assert!(Tier::Tier1.is_royalty_free());
        assert!(Tier::Tier2.is_royalty_free());
        assert!(!Tier::Tier3.is_royalty_free());
        assert!(!Tier::Tier4.is_royalty_free());
    }

    #[test]
    fn tier_directory_names() {
        assert_eq!(Tier::Tier1.directory_name(), "av1_opus");
        assert_eq!(Tier::Tier2.directory_name(), "vp9_opus");
        assert_eq!(Tier::Tier3.directory_name(), "vp9_aac");
        assert_eq!(Tier::Tier4.directory_name(), "h264_aac");
    }

    // ==========================================
    // VideoCodec Tests
    // ==========================================

    #[test]
    fn video_codec_ffmpeg_encoder_names() {
        assert_eq!(VideoCodec::AV1.ffmpeg_encoder(), "libsvtav1");
        assert_eq!(VideoCodec::VP9.ffmpeg_encoder(), "libvpx-vp9");
        assert_eq!(VideoCodec::H264.ffmpeg_encoder(), "libx264");
    }

    #[test]
    fn video_codec_string_format() {
        assert_eq!(VideoCodec::AV1.codec_string("12M"), "av01.0.12M.08");
        assert_eq!(VideoCodec::VP9.codec_string("40"), "vp09.00.40.08");
        assert_eq!(VideoCodec::H264.codec_string("640028"), "avc1.640028");
    }

    // ==========================================
    // AudioCodec Tests
    // ==========================================

    #[test]
    fn audio_codec_ffmpeg_encoder_names() {
        assert_eq!(AudioCodec::Opus.ffmpeg_encoder(), "libopus");
        assert_eq!(AudioCodec::AAC.ffmpeg_encoder(), "aac");
    }

    #[test]
    fn audio_codec_string_format() {
        assert_eq!(AudioCodec::Opus.codec_string(), "opus");
        assert_eq!(AudioCodec::AAC.codec_string(), "mp4a.40.2");
    }

    #[test]
    fn audio_codec_bitrate_values() {
        assert_eq!(AudioCodec::Opus.bitrate(), "128k");
        assert_eq!(AudioCodec::AAC.bitrate(), "128k");
    }

    // ==========================================
    // Rendition Tests
    // ==========================================

    #[test]
    fn rendition_new_creation() {
        let r = Rendition::new(1920, 1080, 8000, 128);
        assert_eq!(r.width, 1920);
        assert_eq!(r.height, 1080);
        assert_eq!(r.video_bitrate_kbps, 8000);
        assert_eq!(r.audio_bitrate_kbps, 128);
    }

    #[test]
    fn rendition_dir_name() {
        let r1080 = Rendition::new(1920, 1080, 8000, 128);
        let r720 = Rendition::new(1280, 720, 4000, 128);
        let r480 = Rendition::new(854, 480, 2000, 96);
        let r360 = Rendition::new(640, 360, 1000, 64);

        assert_eq!(r1080.dir_name(), "1080p");
        assert_eq!(r720.dir_name(), "720p");
        assert_eq!(r480.dir_name(), "480p");
        assert_eq!(r360.dir_name(), "360p");
    }

    #[test]
    fn rendition_qvbr_params_av1() {
        let r = Rendition::new(1920, 1080, 8000, 128);
        let params = r.qvbr_params(VideoCodec::AV1);

        assert_eq!(params.crf, 30);
        // AV1 efficiency factor is 0.6, so target = 8000 * 0.6 = 4800
        // maxrate = 4800 * 1.5 = 7200
        assert_eq!(params.maxrate_kbps, 7200);
    }

    #[test]
    fn rendition_qvbr_params_vp9() {
        let r = Rendition::new(1920, 1080, 8000, 128);
        let params = r.qvbr_params(VideoCodec::VP9);

        assert_eq!(params.crf, 31);
        // VP9 efficiency factor is 0.75, so target = 8000 * 0.75 = 6000
        // maxrate = 6000 * 1.5 = 9000
        assert_eq!(params.maxrate_kbps, 9000);
    }

    #[test]
    fn rendition_qvbr_params_h264() {
        let r = Rendition::new(1920, 1080, 8000, 128);
        let params = r.qvbr_params(VideoCodec::H264);

        assert_eq!(params.crf, 23);
        // H264 efficiency factor is 1.0, so target = 8000
        // maxrate = 8000 * 1.5 = 12000
        assert_eq!(params.maxrate_kbps, 12000);
    }

    // ==========================================
    // EncryptionConfig Tests
    // ==========================================

    #[test]
    fn encryption_config_default_disabled() {
        let config = EncryptionConfig::default();
        assert!(!config.hls_aes128);
        assert!(!config.dash_clearkey);
        assert!(!config.is_enabled());
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn encryption_config_is_enabled() {
        let mut config = EncryptionConfig::default();

        config.hls_aes128 = true;
        assert!(config.is_enabled());

        config.hls_aes128 = false;
        config.dash_clearkey = true;
        assert!(config.is_enabled());

        config.hls_aes128 = true;
        assert!(config.is_enabled());
    }

    #[test]
    fn encryption_config_generated_keys() {
        let config = EncryptionConfig::new_with_generated_keys(true, true);

        assert!(config.hls_aes128);
        assert!(config.dash_clearkey);
        assert!(config.key.is_some());
        assert!(config.key_id.is_some());

        // Keys should be 32 hex characters (16 bytes)
        let key = config.key.unwrap();
        let key_id = config.key_id.unwrap();
        assert_eq!(key.len(), 32);
        assert_eq!(key_id.len(), 32);

        // Should be valid hex
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(key_id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ==========================================
    // AbrLadder Tests
    // ==========================================

    #[test]
    fn abr_ladder_default_has_four_renditions() {
        let ladder = AbrLadder::default();
        assert_eq!(ladder.renditions.len(), 4);
    }

    #[test]
    fn abr_ladder_with_max_height_filters() {
        let ladder_720 = AbrLadder::with_max_height(720);
        assert!(ladder_720.renditions.iter().all(|r| r.height <= 720));

        let ladder_480 = AbrLadder::with_max_height(480);
        assert!(ladder_480.renditions.iter().all(|r| r.height <= 480));
    }

    // ==========================================
    // PreprocessConfig Tests
    // ==========================================

    #[test]
    fn preprocess_config_default() {
        let config = PreprocessConfig::default();
        // Default has all processing disabled
        assert!(!config.audio_normalize);
        assert!(!config.video_denoise);
        assert!(!config.photosensitivity_filter);
        assert!(!config.video_deflicker);
    }

    #[test]
    fn preprocess_config_all_enabled() {
        let config = PreprocessConfig::all_enabled();
        assert!(config.audio_normalize);
        assert!(config.video_denoise);
        // video_deflicker is disabled in favor of fluorescent_deflicker
        assert!(config.fluorescent_deflicker.enabled);
        assert!(config.video_deblock);
        assert!(config.audio_denoise);
    }

    #[test]
    fn preprocess_config_broadcast_compliance() {
        let config = PreprocessConfig::broadcast_compliance();
        assert!(config.photosensitivity_filter);
        assert!(config.red_flash_filter);
        assert!(config.color_saturation_limiter);
        assert!(config.spatial_pattern_filter);
        assert!(config.audio_loudness_range);
        assert!(config.audio_peak_limiter);
    }

    #[test]
    fn preprocess_config_has_broadcast_filters() {
        let default = PreprocessConfig::default();
        assert!(!default.has_broadcast_filters());

        let broadcast = PreprocessConfig::broadcast_compliance();
        assert!(broadcast.has_broadcast_filters());
    }

    // ==========================================
    // Resolution Tests
    // ==========================================

    #[test]
    fn resolution_from_target() {
        let r1080 = Resolution::from_target(1080);
        assert_eq!(r1080.height, 1080);
        assert_eq!(r1080.width, 1920);

        let r720 = Resolution::from_target(720);
        assert_eq!(r720.height, 720);
        assert_eq!(r720.width, 1280);
    }

    #[test]
    fn resolution_target_bitrate() {
        let r1080 = Resolution::from_target(1080);
        let bitrate = r1080.target_bitrate(VideoCodec::H264);
        assert!(bitrate > 0);

        let r720 = Resolution::from_target(720);
        let bitrate_720 = r720.target_bitrate(VideoCodec::H264);

        // 1080p should have higher bitrate than 720p
        assert!(bitrate > bitrate_720);
    }

    #[test]
    fn resolution_target_bitrate_codec_efficiency() {
        let r1080 = Resolution::from_target(1080);

        let h264_bitrate = r1080.target_bitrate(VideoCodec::H264);
        let vp9_bitrate = r1080.target_bitrate(VideoCodec::VP9);
        let av1_bitrate = r1080.target_bitrate(VideoCodec::AV1);

        // AV1 < VP9 < H264 (more efficient codecs use lower bitrates)
        assert!(av1_bitrate < vp9_bitrate);
        assert!(vp9_bitrate < h264_bitrate);
    }

    // ==========================================
    // Preset Tests
    // ==========================================

    #[test]
    fn preset_av1_values() {
        assert_eq!(Preset::Fast.av1_preset(), 10);
        assert_eq!(Preset::Balanced.av1_preset(), 6);
        assert_eq!(Preset::Quality.av1_preset(), 3);
    }

    #[test]
    fn preset_vp9_values() {
        assert_eq!(Preset::Fast.vp9_cpu_used(), 4);
        assert_eq!(Preset::Balanced.vp9_cpu_used(), 2);
        assert_eq!(Preset::Quality.vp9_cpu_used(), 0);
    }

    #[test]
    fn preset_h264_values() {
        assert_eq!(Preset::Fast.h264_preset(), "fast");
        assert_eq!(Preset::Balanced.h264_preset(), "medium");
        assert_eq!(Preset::Quality.h264_preset(), "slow");
    }

    // ==========================================
    // RateControl Tests
    // ==========================================

    #[test]
    fn rate_control_default_is_qvbr() {
        let rc = RateControl::default();
        assert_eq!(rc, RateControl::Qvbr);
    }

    // ==========================================
    // SegmentConfig Tests
    // ==========================================

    #[test]
    fn segment_config_default() {
        let config = SegmentConfig::default();
        assert_eq!(config.duration_secs, 4);
        assert_eq!(config.fragment_duration_ms, 1000);
    }
}
