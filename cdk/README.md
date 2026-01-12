# Video Encoder CDK

AWS CDK（Go）によるVideo Encoderパイプラインのインフラストラクチャ定義。

## アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────────┐
│                        AWS Infrastructure                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐        │
│  │   S3 Input   │────▶│    Lambda    │────▶│  SQS Queue   │        │
│  │   Bucket     │     │  (Trigger)   │     │  (Jobs)      │        │
│  └──────────────┘     └──────────────┘     └──────┬───────┘        │
│                                                    │                │
│                                                    ▼                │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐        │
│  │  S3 Output   │◀────│  AWS Batch   │◀────│Step Functions│        │
│  │   Bucket     │     │  (Fargate)   │     │ (Workflow)   │        │
│  └──────────────┘     └──────────────┘     └──────────────┘        │
│                              │                                      │
│                              ▼                                      │
│                       ┌──────────────┐                              │
│                       │     ECR      │                              │
│                       │ (Container)  │                              │
│                       └──────────────┘                              │
└─────────────────────────────────────────────────────────────────────┘
```

## リソース構成

| リソース | 名前 | 説明 |
|----------|------|------|
| **ECR** | video-encoder | Dockerイメージリポジトリ |
| **S3** | video-encoder-input-{account} | 入力動画バケット |
| **S3** | video-encoder-output-{account} | 出力動画バケット |
| **SQS** | video-encoder-jobs | ジョブキュー |
| **SQS** | video-encoder-dlq | デッドレターキュー |
| **Batch** | video-encoder-compute | Fargate計算環境 |
| **Batch** | video-encoder-queue | ジョブキュー |
| **Step Functions** | VideoEncoderWorkflow | ワークフロー |
| **Lambda** | S3TriggerFunction | S3イベントトリガー |

## デプロイ

### 前提条件

- AWS CLI設定済み
- Go 1.21+
- AWS CDK CLI

```bash
# AWS CDK CLIインストール
npm install -g aws-cdk
```

### デプロイ手順

```bash
cd cdk

# 依存関係インストール
go mod download

# CDKブートストラップ（初回のみ）
cdk bootstrap

# スタック確認
cdk diff

# デプロイ
cdk deploy
```

### 削除

```bash
cdk destroy
```

## Dockerイメージのビルド・プッシュ

```bash
# ECRログイン
aws ecr get-login-password --region ap-northeast-1 | \
  docker login --username AWS --password-stdin {account}.dkr.ecr.ap-northeast-1.amazonaws.com

# イメージビルド
docker build -t video-encoder ../encoder

# タグ付け
docker tag video-encoder:latest \
  {account}.dkr.ecr.ap-northeast-1.amazonaws.com/video-encoder:latest

# プッシュ
docker push {account}.dkr.ecr.ap-northeast-1.amazonaws.com/video-encoder:latest
```

## ジョブ実行

### S3イベントトリガー（自動）

`s3://video-encoder-input-{account}/` に動画をアップロードすると自動でエンコードジョブが開始されます。

### 手動実行

```bash
# Step Functions実行
aws stepfunctions start-execution \
  --state-machine-arn arn:aws:states:ap-northeast-1:{account}:stateMachine:VideoEncoderWorkflow \
  --input '{
    "input": "s3://video-encoder-input-{account}/video.mp4",
    "output": "s3://video-encoder-output-{account}/encoded",
    "preset": "balanced",
    "tiers": "all",
    "abr": true
  }'
```

### Batch直接実行

```bash
aws batch submit-job \
  --job-name video-encode-job \
  --job-queue video-encoder-queue \
  --job-definition video-encoder-job-def \
  --container-overrides '{
    "command": [
      "-i", "s3://bucket/input.mp4",
      "-o", "s3://bucket/output",
      "--abr", "--hls", "--dash"
    ]
  }'
```

## 計算環境設定

### Fargate設定

| パラメータ | 値 | 説明 |
|-----------|-----|------|
| maxVcpus | 256 | 最大vCPU数 |
| Type | FARGATE | サーバーレスコンテナ |

### ジョブ定義

| パラメータ | 値 | 説明 |
|-----------|-----|------|
| vCPU | 4 | ジョブあたりのvCPU |
| Memory | 8192 MB | ジョブあたりのメモリ |
| Timeout | 3600秒 | タイムアウト |

## 環境変数

ジョブに渡される環境変数：

| 変数名 | 説明 |
|--------|------|
| `AWS_DEFAULT_REGION` | AWSリージョン |
| `INPUT_BUCKET` | 入力バケット名 |
| `OUTPUT_BUCKET` | 出力バケット名 |

## コスト最適化

### Fargateスポット

```go
// main.go でスポットインスタンス有効化
computeEnv := awsbatch.NewFargateComputeEnvironment(stack, jsii.String("ComputeEnv"), &awsbatch.FargateComputeEnvironmentProps{
    Spot: jsii.Bool(true),  // スポット有効化で最大70%コスト削減
})
```

### ライフサイクルルール

入力バケットの`temp/`プレフィックスは1日で自動削除されます。

## モニタリング

### CloudWatch Logs

- `/aws/batch/job` - Batchジョブログ
- `/aws/lambda/S3TriggerFunction` - Lambdaログ

### メトリクス

- `AWS/Batch` - ジョブ成功/失敗率
- `AWS/SQS` - キュー深度
- `AWS/States` - ワークフロー実行状況

## セキュリティ

### IAMポリシー

- **最小権限の原則** - 必要なS3バケットへのアクセスのみ許可
- **VPC内実行** - パブリックIPなしでの実行オプション
- **暗号化** - S3バケットはSSE-S3で暗号化

### ネットワーク

デフォルトVPCを使用。本番環境では専用VPCの使用を推奨。

## トラブルシューティング

### ジョブが開始しない

```bash
# Batch計算環境の状態確認
aws batch describe-compute-environments --compute-environments video-encoder-compute

# ジョブキューの状態確認
aws batch describe-job-queues --job-queues video-encoder-queue
```

### コンテナエラー

```bash
# ジョブログ確認
aws logs get-log-events \
  --log-group-name /aws/batch/job \
  --log-stream-name {job-id}
```

### ECRプルエラー

```bash
# ECRリポジトリ確認
aws ecr describe-repositories --repository-names video-encoder

# イメージ確認
aws ecr describe-images --repository-name video-encoder
```
