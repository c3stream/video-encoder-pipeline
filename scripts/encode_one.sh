#!/bin/bash

# Encode a single video file
# Usage: ./scripts/encode_one.sh <source_name> [preset]
# Example: ./scripts/encode_one.sh big_buck_bunny_720p fast

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENCODER="$PROJECT_DIR/target/release/video-encoder"
SOURCES_DIR="$PROJECT_DIR/sources"
OUTPUTS_DIR="$PROJECT_DIR/outputs"

SOURCE_NAME="${1:-}"
PRESET="${2:-balanced}"

if [ -z "$SOURCE_NAME" ]; then
    echo "Usage: $0 <source_name> [preset]"
    echo ""
    echo "Available sources:"
    ls -1 "$SOURCES_DIR"/*.mp4 2>/dev/null | xargs -I {} basename {} .mp4
    exit 1
fi

# Check if encoder exists
if [ ! -f "$ENCODER" ]; then
    echo "Error: Encoder not found. Run 'cargo build --release' first."
    exit 1
fi

# Find source file
INPUT_PATH=""
for ext in mp4 mov mkv avi; do
    if [ -f "$SOURCES_DIR/${SOURCE_NAME}.${ext}" ]; then
        INPUT_PATH="$SOURCES_DIR/${SOURCE_NAME}.${ext}"
        break
    fi
done

if [ -z "$INPUT_PATH" ]; then
    echo "Error: Source file not found: $SOURCE_NAME"
    echo ""
    echo "Available sources:"
    ls -1 "$SOURCES_DIR"/*.mp4 2>/dev/null | xargs -I {} basename {} .mp4
    exit 1
fi

OUTPUT_PATH="$OUTPUTS_DIR/$SOURCE_NAME"

echo "========================================"
echo "Video Encoding"
echo "========================================"
echo "Source:  $INPUT_PATH"
echo "Output:  $OUTPUT_PATH"
echo "Preset:  $PRESET"
echo ""

START_TIME=$(date +%s)

"$ENCODER" \
    --input "$INPUT_PATH" \
    --output "$OUTPUT_PATH" \
    --preset "$PRESET" \
    --abr \
    --tiers "all" \
    --dash \
    --hls

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo "========================================"
echo "Completed in ${DURATION}s"
echo "========================================"

# Update player symlink
echo ""
echo "Updating player symlink..."
cd "$PROJECT_DIR/player"
ln -sf "$OUTPUT_PATH" "./$SOURCE_NAME"
echo "  Linked: $SOURCE_NAME -> $OUTPUT_PATH"

echo ""
echo "Done! Start the player with:"
echo "  cd player && ./serve.sh"
