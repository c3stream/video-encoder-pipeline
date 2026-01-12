# Video Encoder Pipeline

[![Rust](https://img.shields.io/badge/rust-1.92%2B-blue.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests](https://img.shields.io/badge/tests-68%20passing-brightgreen.svg)](./encoder/src/)
[![Clippy](https://img.shields.io/badge/clippy-pedantic-green.svg)](./encoder/src/lib.rs)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](./CHANGELOG.md)

AWS Batch対応のマルチコーデック動画エンコーディングパイプライン。HLS/DASH配信に対応した4層コーデック戦略で、あらゆるデバイスへの動画配信を実現します。

*Multi-codec video encoding pipeline for AWS Batch. Delivers video to any device with a 4-tier codec strategy supporting HLS/DASH streaming.*

## 目次 / Table of Contents

- [特徴 / Features](#特徴)
- [アーキテクチャ / Architecture](#アーキテクチャ)
- [4層コーデック戦略 / 4-Tier Codec Strategy](#4層コーデック戦略)
- [クイックスタート / Quick Start](#クイックスタート)
- [プロジェクト構成 / Project Structure](#プロジェクト構成)
- [主要機能 / Key Features](#主要機能)
- [CLIオプション / CLI Options](#cliオプション一覧)
- [開発 / Development](#開発)
- [AWS Batch デプロイ](#aws-batch-デプロイ)
- [ライセンス / License](#ライセンス)

## 特徴

- **4層コーデック戦略** - デバイス互換性と圧縮効率を両立
- **ABR（適応的ビットレート）** - ネットワーク状況に応じた品質切り替え
- **HLS/DASH対応** - 主要なストリーミングプロトコルをサポート
- **放送準拠フィルター** - Ofcom/ITU基準の安全性フィルター
- **ソース自動解析** - 処理済みファイルの二重処理を防止
- **AWS Batch統合** - スケーラブルなクラウドエンコーディング

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────┐
│                    Video Encoder Pipeline                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │  Input   │───▶│ Analyze  │───▶│  Filter  │───▶│  Encode  │  │
│  │  Video   │    │  Source  │    │ Pipeline │    │  Multi   │  │
│  └──────────┘    └──────────┘    └──────────┘    │  Codec   │  │
│                                                   └────┬─────┘  │
│                                                        │        │
│  ┌──────────────────────────────────────────────────────┘       │
│  │                                                               │
│  ▼                                                               │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    Output Structure                         │ │
│  │  segments/                                                  │ │
│  │  ├── video/{av1,vp9,h264}/{1080p,720p,480p,360p}/          │ │
│  │  └── audio/{opus,aac}/{256k,128k,64k}/                     │ │
│  │  hls/master.m3u8                                           │ │
│  │  dash/manifest.mpd                                         │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 4層コーデック戦略

| Tier       | Video | Audio | 用途                                 |
| ---------- | ----- | ----- | ------------------------------------ |
| **Tier 1** | AV1   | Opus  | 最高圧縮効率（ロイヤリティフリー）   |
| **Tier 2** | VP9   | Opus  | 広範なブラウザサポート               |
| **Tier 3** | VP9   | AAC   | iOS 14+ 互換                         |
| **Tier 4** | H.264 | AAC   | ユニバーサルフォールバック           |

## クイックスタート

### 必要環境

- Rust 1.75+
- FFmpeg 6.0+ (libsvtav1, libvpx, libx264, libopus 含む)
- AWS CLI (S3連携使用時)

### インストール

```bash
git clone https://github.com/yourusername/video-encoder-pipeline.git
cd video-encoder-pipeline

# ビルド
cargo build --release

# インストール（オプション）
cargo install --path encoder
```

### 基本的な使い方

```bash
# シンプルなエンコード
video-encoder -i input.mp4 -o ./output --tiers all --hls --dash

# ABR（適応的ビットレート）有効
video-encoder -i input.mp4 -o ./output --abr --hls --dash

# プリプロセス + 自動フィルター調整
video-encoder -i input.mp4 -o ./output --preprocess --auto-filter --abr

# ソース解析のみ
video-encoder -i input.mp4 -o /tmp --analyze
```

## プロジェクト構成

```
.
├── encoder/          # Rustエンコーダー本体
│   ├── src/
│   │   ├── lib.rs            # ライブラリエントリーポイント
│   │   ├── main.rs           # CLIエントリーポイント
│   │   ├── encoder.rs        # エンコードパイプライン
│   │   ├── config.rs         # 設定・フィルター定義
│   │   ├── source_analyzer.rs # ソース解析
│   │   ├── job.rs            # ジョブ管理
│   │   ├── upscaler.rs       # アップスケーラー
│   │   └── error.rs          # エラー定義
│   └── benches/              # Criterionベンチマーク
├── cdk/              # AWS CDKインフラ（Go）
├── player/           # HTML5プレーヤー
├── scripts/          # 開発・ビルドスクリプト
└── sources/          # テスト用ソース動画
```

## 主要機能

### ソース自動解析 (`--analyze`, `--auto-filter`)

処理済みファイルを自動検出し、二重フィルターによる品質劣化を防止します。

```bash
# 解析のみ実行
video-encoder -i video.mp4 -o /tmp --analyze

# 出力例:
# Processing Status: Already encoded (minimal processing recommended)
# Filter Recommendations:
#   - Already encoded: skipping video denoise to avoid quality loss
#   - Low bitrate (702kbps): deflicker may introduce artifacts
```

### 放送準拠フィルター (`--broadcast`)

Ofcom/ITU基準に準拠した安全性フィルターを適用します。

- **光過敏性フィルター** - 急激な明度変化を抑制（ポリゴンショック対策）
- **赤フラッシュフィルター** - 危険な赤色点滅を軽減
- **空間パターンフィルター** - 縞模様パターンを緩和
- **ラウドネス正規化** - EBU R128準拠の音量調整

```bash
video-encoder -i input.mp4 -o ./output --broadcast
```

### 暗号化 (`--encrypt`)

HLS AES-128およびDASH ClearKey暗号化に対応。

```bash
video-encoder -i input.mp4 -o ./output --encrypt --hls --dash
```

## CLIオプション一覧

| オプション      | 説明                                         | デフォルト |
| --------------- | -------------------------------------------- | ---------- |
| `-i, --input`   | 入力ファイル（ローカルまたはS3 URI）         | 必須       |
| `-o, --output`  | 出力ディレクトリ                             | 必須       |
| `-p, --preset`  | エンコードプリセット（fast/balanced/quality）| balanced   |
| `--tiers`       | 生成するTier（1,2,3,4 または all）           | all        |
| `--abr`         | ABRマルチ解像度エンコード有効                | false      |
| `--qvbr`        | QVBR（品質重視VBR）レート制御                | false      |
| `--hls`         | HLSマニフェスト生成                          | true       |
| `--dash`        | DASHマニフェスト生成                         | true       |
| `--encrypt`     | 暗号化有効（AES-128 + ClearKey）             | false      |
| `--preprocess`  | 前処理フィルター全有効                       | false      |
| `--broadcast`   | 放送準拠フィルター有効                       | false      |
| `--analyze`     | ソース解析のみ（エンコードなし）             | false      |
| `--auto-filter` | ソース解析に基づくフィルター自動調整         | false      |
| `--upscale`     | アップスケール有効                           | false      |
| `--audio-abr`   | マルチビットレートオーディオ                 | false      |

## 開発

### テスト

```bash
# 全テスト実行
cargo test

# 詳細出力
cargo test -- --nocapture
```

### ベンチマーク

```bash
# 全ベンチマーク実行
cargo bench

# 特定グループのみ
cargo bench --bench config_benchmarks
```

### コード品質

```bash
# フォーマット
cargo fmt

# リント
cargo clippy -- -D warnings

# ドキュメント生成
cargo doc --open
```

### ライブラリとして使用

```rust
use video_encoder::{JobArgs, JobConfig, Tier, VideoCodec};

// JobArgsを作成
let args = JobArgs {
    input: "input.mp4".to_string(),
    output: "output".to_string(),
    tiers: "1,2".to_string(),
    ..Default::default()
};

// JobConfigを生成
let config = JobConfig::from_args(&args)?;
```

詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

## AWS Batch デプロイ

```bash
cd cdk

# 依存関係インストール
go mod download

# デプロイ
cdk deploy
```

詳細は [cdk/README.md](cdk/README.md) を参照してください。

## HTML5プレーヤー

```bash
cd player
./serve.sh
# http://localhost:8080 でアクセス
```

詳細は [player/README.md](player/README.md) を参照してください。

## 出力構造

```
output/
├── segments/
│   ├── video/
│   │   ├── av1/{1080p,720p,480p,360p}/
│   │   ├── vp9/{1080p,720p,480p,360p}/
│   │   └── h264/{1080p,720p,480p,360p}/
│   └── audio/
│       ├── opus/{256k,128k,64k}/
│       └── aac/{256k,128k,64k}/
├── hls/
│   ├── master.m3u8
│   ├── av1_opus_1080p.m3u8
│   └── ...
└── dash/
    └── manifest.mpd
```

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照

## 貢献

コントリビューションを歓迎します！詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
