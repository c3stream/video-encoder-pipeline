# Video Player

HLS/DASH対応のHTML5動画プレーヤー。マルチコーデック自動選択、ABR品質切り替え、暗号化コンテンツ再生に対応。

## 特徴

- **マルチコーデック対応** - AV1, VP9, H.264を自動選択
- **ABR対応** - ネットワーク状況に応じた品質切り替え
- **HLS/DASH対応** - 主要ストリーミングプロトコルをサポート
- **暗号化対応** - HLS AES-128, DASH ClearKey
- **デバッグ情報** - リアルタイム再生ステータス表示

## 使い方

### ローカルサーバー起動

```bash
cd player
./serve.sh
```

ブラウザで http://localhost:8080 にアクセス。

### 動画の再生

1. エンコード済みの動画フォルダを `player/` 配下にシンボリックリンク
2. プレーヤーUIで動画を選択

```bash
# 例: outputs/my_video を再生可能にする
ln -s ../outputs/my_video player/my_video
```

## プレーヤー機能

### コーデック自動選択

ブラウザの対応状況に応じて最適なコーデックを自動選択：

| 優先度 | コーデック | 対応ブラウザ |
|--------|-----------|-------------|
| 1 | AV1 + Opus | Chrome 70+, Firefox 67+ |
| 2 | VP9 + Opus | Chrome 29+, Firefox 28+ |
| 3 | VP9 + AAC | Safari 14+ (iOS含む) |
| 4 | H.264 + AAC | 全ブラウザ |

### ABR品質切り替え

ネットワーク帯域に応じて自動的に解像度を切り替え：

- **1080p** - 6 Mbps以上
- **720p** - 3 Mbps以上
- **480p** - 1.5 Mbps以上
- **360p** - 800 kbps以上

### デバッグパネル

リアルタイムで以下の情報を表示：

- 現在のコーデック
- 解像度・ビットレート
- バッファ状況
- ネットワーク推定帯域

## 技術スタック

- **HLS.js** - HLS再生ライブラリ
- **dash.js** - DASH再生ライブラリ
- **Vanilla JS** - フレームワーク不使用

## ファイル構成

```
player/
├── index.html    # メインHTML
├── styles.css    # スタイルシート
├── app.js        # アプリケーションロジック
└── serve.sh      # ローカルサーバー起動スクリプト
```

## 暗号化コンテンツの再生

### HLS AES-128

鍵ファイル（`key.bin`）が出力ディレクトリに含まれていれば自動的に復号化。

### DASH ClearKey

`clearkey_license.json` を使用して復号化。プレーヤーは自動的にライセンスを取得。

## カスタマイズ

### 新しい動画ソースの追加

`app.js` の動画リストに追加：

```javascript
const videoSources = [
  {
    name: "My Video",
    hls: "./my_video/hls/master.m3u8",
    dash: "./my_video/dash/manifest.mpd"
  },
  // ...
];
```

### UIテーマ変更

`styles.css` でカスタマイズ可能。

## ブラウザ互換性

| ブラウザ | HLS | DASH | AV1 | VP9 | H.264 |
|---------|-----|------|-----|-----|-------|
| Chrome 90+ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Firefox 90+ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Safari 14+ | ✅* | ❌ | ❌ | ✅ | ✅ |
| Edge 90+ | ✅ | ✅ | ✅ | ✅ | ✅ |

*Safari はネイティブHLS対応

## トラブルシューティング

### CORSエラー

ローカルファイルを直接開くとCORSエラーが発生します。必ず `serve.sh` でサーバーを起動してください。

```bash
# NGの例（CORSエラー）
open index.html

# OKの例
./serve.sh
```

### コーデックが再生されない

ブラウザのコーデックサポートを確認：

```javascript
// ブラウザコンソールで実行
const video = document.createElement('video');
console.log('AV1:', video.canPlayType('video/mp4; codecs="av01.0.08M.08"'));
console.log('VP9:', video.canPlayType('video/webm; codecs="vp9"'));
console.log('H264:', video.canPlayType('video/mp4; codecs="avc1.640028"'));
```

### バッファリングが頻発する

- ネットワーク帯域を確認
- 低解像度のストリームを手動選択
- サーバーのスループットを確認
