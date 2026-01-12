# Video Encoder Docker Image
# Multi-stage build with FFmpeg (AV1/VP9/H.264) + Real-ESRGAN support

# ============================================
# Stage 1: Build Real-ESRGAN
# ============================================
FROM debian:bookworm-slim AS realesrgan-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    wget \
    unzip \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Download pre-built Real-ESRGAN (ncnn version, no GPU required but slower)
RUN wget -q https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-ubuntu.zip \
    -O /tmp/realesrgan.zip \
    && unzip /tmp/realesrgan.zip -d /opt/realesrgan \
    && chmod +x /opt/realesrgan/realesrgan-ncnn-vulkan \
    && rm /tmp/realesrgan.zip

# ============================================
# Stage 2: Build Rust application
# ============================================
FROM rust:1.92-bookworm AS rust-builder

WORKDIR /build

# Copy source
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release binary
RUN cargo build --release

# ============================================
# Stage 3: Build FFmpeg with all codecs
# ============================================
FROM debian:bookworm-slim AS ffmpeg-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    git \
    wget \
    nasm \
    yasm \
    cmake \
    meson \
    ninja-build \
    autoconf \
    automake \
    libtool \
    libssl-dev \
    zlib1g-dev \
    # AV1 dependencies
    libdav1d-dev \
    # VP9 dependencies
    libvpx-dev \
    # H.264 dependencies
    libx264-dev \
    # Audio codecs
    libopus-dev \
    libfdk-aac-dev \
    # Other useful libs
    libfreetype6-dev \
    libfontconfig1-dev \
    && rm -rf /var/lib/apt/lists/*

# Build SVT-AV1 (for encoding, more efficient than libaom)
WORKDIR /tmp
RUN git clone --depth 1 --branch v2.3.0 https://gitlab.com/AOMediaCodec/SVT-AV1.git \
    && cd SVT-AV1/Build \
    && cmake .. -G"Unix Makefiles" -DCMAKE_BUILD_TYPE=Release \
    && make -j$(nproc) \
    && make install \
    && ldconfig

# Build FFmpeg
WORKDIR /tmp
RUN git clone --depth 1 --branch n7.1 https://github.com/FFmpeg/FFmpeg.git ffmpeg \
    && cd ffmpeg \
    && ./configure \
        --prefix=/usr/local \
        --enable-gpl \
        --enable-nonfree \
        --enable-libx264 \
        --enable-libvpx \
        --enable-libsvtav1 \
        --enable-libdav1d \
        --enable-libopus \
        --enable-libfdk-aac \
        --enable-libfreetype \
        --enable-libfontconfig \
        --enable-openssl \
        --disable-debug \
        --disable-doc \
        --disable-ffplay \
    && make -j$(nproc) \
    && make install

# ============================================
# Stage 4: Final runtime image
# ============================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libx264-164 \
    libvpx7 \
    libopus0 \
    libfdk-aac2 \
    libdav1d6 \
    libfreetype6 \
    libfontconfig1 \
    # For Real-ESRGAN (CPU mode)
    libgomp1 \
    libvulkan1 \
    && rm -rf /var/lib/apt/lists/*

# Copy FFmpeg
COPY --from=ffmpeg-builder /usr/local/bin/ffmpeg /usr/local/bin/
COPY --from=ffmpeg-builder /usr/local/bin/ffprobe /usr/local/bin/
COPY --from=ffmpeg-builder /usr/local/lib/libsvtav1enc* /usr/local/lib/

# Copy Real-ESRGAN
COPY --from=realesrgan-builder /opt/realesrgan /opt/realesrgan
RUN ln -s /opt/realesrgan/realesrgan-ncnn-vulkan /usr/local/bin/

# Copy Rust binary
COPY --from=rust-builder /build/target/release/video-encoder /usr/local/bin/

# Update library cache
RUN ldconfig

# Set working directory
WORKDIR /work

# Verify installations
RUN ffmpeg -version && \
    ffprobe -version && \
    video-encoder --help

# Default entrypoint
ENTRYPOINT ["video-encoder"]
CMD ["--help"]
