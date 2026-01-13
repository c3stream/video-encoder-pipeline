# Video Encoder Pipeline

[![CI](https://github.com/c3stream/video-encoder-pipeline/actions/workflows/ci.yml/badge.svg)](https://github.com/c3stream/video-encoder-pipeline/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-blue.svg)](https://www.rust-lang.org/)

> **Multi-codec video encoding pipeline with 4-tier output strategy for HLS/DASH streaming**

Rustで実装されたマルチコーデック動画エンコーダー。AWS Batchでのスケーラブルなエンコードジョブ実行に最適化されています。

## 4-Tier Codec Strategy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        4-Tier Output Strategy                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Tier 1 (Best Compression)      Tier 2 (Wide Support)                      │
│  ┌─────────────────────┐        ┌─────────────────────┐                    │
│  │   AV1 + Opus        │        │   VP9 + Opus        │                    │
│  │   Royalty-free      │        │   Royalty-free      │                    │
│  │   Chrome/Firefox    │        │   Android/Desktop   │                    │
│  └─────────────────────┘        └─────────────────────┘                    │
│                                                                             │
│  Tier 3 (iOS 14+)               Tier 4 (Universal)                         │
│  ┌─────────────────────┐        ┌─────────────────────┐                    │
│  │   VP9 + AAC         │        │   H.264 + AAC       │                    │
│  │   Video royalty-free│        │   Maximum compat    │                    │
│  │   Safari 14+        │        │   All devices       │                    │
│  └─────────────────────┘        └─────────────────────┘                    │
│                                                                             │
│  Output: HLS (.m3u8) + DASH (.mpd) with ABR ladder (1080p→360p)            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# Build
cargo build --release

# Basic encoding with all tiers
./target/release/video-encoder -i input.mp4 -o ./output --tiers all --hls --dash

# ABR ladder (multi-resolution)
./target/release/video-encoder -i input.mp4 -o ./output --abr --hls --dash

# H.264 only (universal compatibility)
./target/release/video-encoder -i input.mp4 -o ./output --tiers 4 --hls
```

## Examples

Run the included examples to understand the API:

```bash
# Basic concepts: tiers, presets, renditions
cargo run --example basic_encode

# ABR ladder configuration
cargo run --example abr_ladder
```

## 機能概要

### コーデックサポート

| コーデック | エンコーダー | 用途 |
|-----------|------------|------|
| AV1 | libsvtav1 | 次世代高圧縮（ロイヤリティフリー） |
| VP9 | libvpx-vp9 | Web標準（ロイヤリティフリー） |
| H.264 | libx264 | ユニバーサル互換 |
| Opus | libopus | 高品質オーディオ |
| AAC | aac | 広範な互換性 |

### 前処理フィルター

```
┌─────────────────────────────────────────────────────────────┐
│                    Filter Pipeline                          │
├─────────────────────────────────────────────────────────────┤
│  Video Filters:                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ Denoise  │─▶│Deflicker │─▶│Photosens │─▶│ Deblock  │    │
│  │ (hqdn3d) │  │(蛍光灯)  │  │(光過敏性)│  │          │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
│                                     │                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │RedFlash  │  │ColorLimit│  │ Spatial  │                  │
│  │ Filter   │  │(彩度制限)│  │ Pattern  │                  │
│  └──────────┘  └──────────┘  └──────────┘                  │
├─────────────────────────────────────────────────────────────┤
│  Audio Filters:                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │ Denoise  │─▶│PeakLimit │─▶│Loudnorm  │                  │
│  │(afftdn)  │  │(突発音)  │  │(EBU R128)│                  │
│  └──────────┘  └──────────┘  └──────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

## インストール

### 依存関係

```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg libavcodec-dev libavformat-dev

# FFmpegに必要なコーデック
# - libsvtav1 (AV1)
# - libvpx (VP9)
# - libx264 (H.264)
# - libopus (Opus)
# - libfdk-aac または aac (AAC)
```

### ビルド

```bash
cd encoder
cargo build --release

# バイナリは target/release/video-encoder に生成されます
```

## 使い方

### 基本コマンド

```bash
# 全Tier出力（HLS + DASH）
video-encoder -i input.mp4 -o ./output --tiers all --hls --dash

# ABR有効（マルチ解像度）
video-encoder -i input.mp4 -o ./output --abr --hls --dash

# 特定Tierのみ
video-encoder -i input.mp4 -o ./output --tiers 4 --hls  # H.264+AACのみ
```

### プリプロセス

```bash
# 全前処理フィルター有効
video-encoder -i input.mp4 -o ./output --preprocess

# 放送準拠フィルターのみ
video-encoder -i input.mp4 -o ./output --broadcast

# 自動フィルター調整（処理済みソース対応）
video-encoder -i input.mp4 -o ./output --preprocess --auto-filter
```

### ソース解析

```bash
# 解析のみ実行
video-encoder -i input.mp4 -o /tmp --analyze
```

**出力例:**
```
========== Source Analysis Report ==========
Resolution:    1280x720
Video Codec:   h264
Audio Codec:   aac
Framerate:     30.000 fps
Duration:      120.00 seconds
Video Bitrate: 54696 kbps
Encoder:       Lavf62.3.100

Processing Status: Already filtered (skip duplicate filters)

Filter Recommendations:
  - Already filtered: skipping denoise
  - Already filtered: skipping deflicker
=============================================
```

### 暗号化

```bash
# HLS AES-128 + DASH ClearKey
video-encoder -i input.mp4 -o ./output --encrypt --hls --dash
```

### S3連携

```bash
# S3から入力、S3へ出力
video-encoder \
  -i s3://bucket/input/video.mp4 \
  -o s3://bucket/output/encoded \
  --abr --hls --dash
```

## ソース解析システム

### 処理状態の判定

| ステータス | 判定条件 | 推奨アクション |
|-----------|----------|---------------|
| **Raw** | ProRes/DNxHD/高ビットレート(>50Mbps) | 全フィルター適用可 |
| **Filtered** | FFmpeg処理済み + 高ビットレート | denoise/deflickerスキップ |
| **Encoded** | 低〜中ビットレートで圧縮済み | 最小限の処理のみ |
| **Unknown** | 判定不能 | 保守的アプローチ |

### 自動フィルター調整

`--auto-filter` フラグを使用すると、ソース解析結果に基づいてフィルターが自動調整されます：

```bash
# ソースが既にエンコード済みの場合
video-encoder -i compressed.mp4 -o ./out --preprocess --auto-filter
# → denoise, deblock, deflicker が自動的にスキップされます
```

## 設定詳細

### エンコードプリセット

| プリセット | AV1 preset | VP9 cpu-used | H.264 preset | 用途 |
|-----------|-----------|--------------|--------------|------|
| fast | 10 | 4 | fast | 高速エンコード |
| balanced | 6 | 2 | medium | バランス型（デフォルト） |
| quality | 3 | 0 | slow | 高品質 |

### ABRラダー

| 解像度 | ビデオビットレート | オーディオビットレート |
|--------|-------------------|---------------------|
| 1080p | 6,000 kbps | 128 kbps |
| 720p | 3,000 kbps | 128 kbps |
| 480p | 1,500 kbps | 96 kbps |
| 360p | 800 kbps | 64 kbps |

### レート制御

| モード | 説明 | 用途 |
|--------|------|------|
| CRF | 品質固定（可変ビットレート） | 一般的な用途 |
| QVBR | CRF + maxrate制限 | ストリーミング推奨 |
| CBR | 固定ビットレート | ライブ配信 |

## 放送準拠フィルター

### 光過敏性フィルター（Photosensitivity）

ITU-R BT.1702およびOfcomガイドラインに基づく、急激な明度変化の抑制。

| レベル | 検出感度 | 用途 |
|--------|---------|------|
| Light | 低（極端なストロボのみ） | 通常コンテンツ |
| Standard | 中（危険なフラッシュ検出） | 一般推奨 |
| Strict | 高（より厳格な検出） | 高リスクコンテンツ |

### 蛍光灯フリッカー除去

電源周波数に基づく蛍光灯/LEDのフリッカー除去。

```bash
# 日本の場合
# 東日本: 50Hz → 100Hzフリッカー
# 西日本: 60Hz → 120Hzフリッカー
# 自動検出も可能
```

### EBU R128 ラウドネス正規化

| パラメータ | 放送基準 | ストリーミング基準 |
|-----------|---------|------------------|
| Target I | -23 LUFS | -14 LUFS |
| Target LRA | 7 LU | 11 LU |
| True Peak | -1 dBTP | -1 dBTP |

## 出力構造

```
output/
├── segments/
│   ├── video/
│   │   ├── av1/
│   │   │   ├── 1080p/
│   │   │   │   ├── init.mp4
│   │   │   │   ├── segment_00001.m4s
│   │   │   │   └── playlist.m3u8
│   │   │   └── ...
│   │   ├── vp9/
│   │   └── h264/
│   └── audio/
│       ├── opus/
│       │   └── 128k/
│       └── aac/
│           └── 128k/
├── hls/
│   ├── master.m3u8
│   ├── av1_opus_1080p.m3u8
│   ├── av1_opus_720p.m3u8
│   └── ...
├── dash/
│   └── manifest.mpd
├── key.bin              # 暗号化キー（--encrypt時）
└── clearkey_license.json # ClearKeyライセンス（--encrypt時）
```

## モジュール構成

### `main.rs`
CLIエントリーポイント。引数解析と実行モード分岐。

### `encoder.rs`
エンコードパイプラインのコア実装。FFmpegプロセス制御、セグメント生成、マニフェスト生成。

### `config.rs`
設定定義。コーデック、プリセット、フィルター、ABRラダー等。

### `source_analyzer.rs`
ソース動画の解析。処理状態検出、フィルター推奨生成。

### `job.rs`
ジョブ設定管理。CLI引数からの設定構築、ソース解析統合。

### `upscaler.rs`
アップスケーラー実装。FFmpeg bicubic/lanczos、Real-ESRGAN対応。

### `error.rs`
エラー型定義。

## テスト

```bash
# ユニットテスト
cargo test

# 統合テスト（テスト動画が必要）
cargo test --features integration
```

## パフォーマンス

### エンコード時間の目安（1080p 1分動画）

| プリセット | AV1 | VP9 | H.264 |
|-----------|-----|-----|-------|
| fast | ~2分 | ~1分 | ~30秒 |
| balanced | ~5分 | ~3分 | ~1分 |
| quality | ~15分 | ~8分 | ~3分 |

※ M1 Mac基準。ハードウェア・動画内容により変動。

## トラブルシューティング

### FFmpegコーデックが見つからない

```bash
# インストール済みコーデック確認
ffmpeg -encoders | grep -E "(av1|vp9|x264|opus)"
```

### メモリ不足

高解像度・長時間動画の場合、セグメント方式で処理されるため通常は問題ありません。それでも問題がある場合は `--preset fast` を使用してください。

### S3アクセスエラー

```bash
# AWS認証情報を確認
aws sts get-caller-identity
aws s3 ls s3://your-bucket/
```
