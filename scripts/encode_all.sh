#!/bin/bash

# Batch encode all source videos
# Usage: ./scripts/encode_all.sh [preset]
# preset: fast, balanced, quality (default: balanced)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENCODER="$PROJECT_DIR/target/release/video-encoder"
SOURCES_DIR="$PROJECT_DIR/sources"
OUTPUTS_DIR="$PROJECT_DIR/outputs"

PRESET="${1:-balanced}"

# Check if encoder exists
if [ ! -f "$ENCODER" ]; then
    echo "Error: Encoder not found. Run 'cargo build --release' first."
    exit 1
fi

echo "========================================"
echo "Batch Video Encoding"
echo "========================================"
echo "Preset: $PRESET"
echo "Sources: $SOURCES_DIR"
echo "Outputs: $OUTPUTS_DIR"
echo ""

# Source files and their output directories
declare -A VIDEOS=(
    ["bbb_1080p.mp4"]="bbb_1080p"
    ["bbb_720p.mp4"]="bbb_720p"
    ["big_buck_bunny_720p.mp4"]="big_buck_bunny_720p"
    ["elephants_dream.mp4"]="elephants_dream"
    ["flash_test.mp4"]="flash_test"
    ["noisy_test.mp4"]="noisy_test"
    ["noisy_test_2min.mp4"]="noisy_test_2min"
    ["sintel_trailer.mp4"]="sintel_trailer"
    ["tears_of_steel_720p.mp4"]="tears_of_steel_720p"
    ["test_30s.mp4"]="test_30s"
    ["test_video.mp4"]="test_video"
    ["test_video_.mp4"]="test_video_"
)

# Count total videos
TOTAL=${#VIDEOS[@]}
CURRENT=0
FAILED=0

for SOURCE in "${!VIDEOS[@]}"; do
    OUTPUT_NAME="${VIDEOS[$SOURCE]}"
    INPUT_PATH="$SOURCES_DIR/$SOURCE"
    OUTPUT_PATH="$OUTPUTS_DIR/$OUTPUT_NAME"

    CURRENT=$((CURRENT + 1))

    if [ ! -f "$INPUT_PATH" ]; then
        echo "[$CURRENT/$TOTAL] SKIP: $SOURCE (not found)"
        continue
    fi

    # Skip if already encoded (check for manifest)
    if [ -f "$OUTPUT_PATH/manifest.mpd" ] || [ -f "$OUTPUT_PATH/manifest.m3u8" ]; then
        echo "[$CURRENT/$TOTAL] SKIP: $SOURCE (already encoded)"
        continue
    fi

    echo ""
    echo "========================================"
    echo "[$CURRENT/$TOTAL] Encoding: $SOURCE"
    echo "========================================"
    echo "Input:  $INPUT_PATH"
    echo "Output: $OUTPUT_PATH"
    echo ""

    START_TIME=$(date +%s)

    if "$ENCODER" \
        --input "$INPUT_PATH" \
        --output "$OUTPUT_PATH" \
        --preset "$PRESET" \
        --abr \
        --tiers "all" \
        --dash \
        --hls; then

        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        echo ""
        echo "Completed in ${DURATION}s"
    else
        FAILED=$((FAILED + 1))
        echo ""
        echo "FAILED: $SOURCE"
    fi
done

echo ""
echo "========================================"
echo "Batch Encoding Complete"
echo "========================================"
echo "Total: $TOTAL"
echo "Failed: $FAILED"
echo ""

# Update player symlinks
echo "Updating player symlinks..."
cd "$PROJECT_DIR/player"

# Remove old symlinks
find . -maxdepth 1 -type l -delete

# Create new symlinks for each output
for OUTPUT_NAME in "${VIDEOS[@]}"; do
    OUTPUT_PATH="$OUTPUTS_DIR/$OUTPUT_NAME"
    if [ -d "$OUTPUT_PATH" ] && [ "$(ls -A "$OUTPUT_PATH" 2>/dev/null)" ]; then
        ln -sf "$OUTPUT_PATH" "./$OUTPUT_NAME"
        echo "  Linked: $OUTPUT_NAME"
    fi
done

echo ""
echo "Done! Start the player with:"
echo "  cd player && ./serve.sh"
