#!/bin/bash
# Create 10-minute test video with noise, flicker, and audio issues for filter testing
# This video intentionally has quality issues that the preprocessor should fix

set -e

OUTPUT_DIR="${1:-/Users/kazuhirokondo/work/my/rust/rust_plactice_1/test_input}"
OUTPUT_FILE="$OUTPUT_DIR/noisy_test_10min.mp4"
DURATION=600  # 10 minutes

mkdir -p "$OUTPUT_DIR"

echo "Creating 10-minute test video with intentional quality issues..."
echo "Output: $OUTPUT_FILE"

# Create test video with:
# 1. Video noise (using noise filter)
# 2. Flicker effect (using random brightness changes)
# 3. Blocking artifacts (simulated via low quality encode)
# 4. Audio noise (using anoisesrc)
# 5. Uneven audio levels

ffmpeg -y \
    -f lavfi -i "testsrc2=duration=${DURATION}:size=1920x1080:rate=30" \
    -f lavfi -i "sine=frequency=440:duration=${DURATION}" \
    -f lavfi -i "anoisesrc=duration=${DURATION}:amplitude=0.1:color=pink" \
    -filter_complex "
        [0:v]
        noise=alls=20:allf=t+u,
        eq=brightness='0.05*sin(2*PI*t*0.5)',
        scale=1920:1080:flags=neighbor
        [noisy_video];

        [1:a][2:a]amix=inputs=2:weights='1 0.3'[mixed_audio];
        [mixed_audio]volume='0.5+0.5*sin(2*PI*t*0.1)'[uneven_audio]
    " \
    -map "[noisy_video]" \
    -map "[uneven_audio]" \
    -c:v libx264 \
    -preset ultrafast \
    -crf 35 \
    -c:a aac \
    -b:a 64k \
    -t "$DURATION" \
    "$OUTPUT_FILE"

echo ""
echo "Test video created: $OUTPUT_FILE"
echo ""
echo "Quality issues intentionally added:"
echo "  - Video noise (temporal + uniform)"
echo "  - Brightness flicker (0.5Hz sine wave)"
echo "  - Blocking artifacts (CRF 35, ultrafast preset)"
echo "  - Audio noise (pink noise mixed in)"
echo "  - Uneven audio levels (volume oscillation)"
echo ""
echo "Now test encoding with preprocessing:"
echo "  cd encoder && cargo run -- -i '$OUTPUT_FILE' -o output_filtered --abr --audio-abr --preprocess"
