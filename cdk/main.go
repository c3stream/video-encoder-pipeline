package main

import (
	"os"

	"github.com/aws/aws-cdk-go/awscdk/v2"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsbatch"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsec2"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsecr"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsecs"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsiam"
	"github.com/aws/aws-cdk-go/awscdk/v2/awslambda"
	"github.com/aws/aws-cdk-go/awscdk/v2/awslogs"
	"github.com/aws/aws-cdk-go/awscdk/v2/awss3"
	"github.com/aws/aws-cdk-go/awscdk/v2/awssqs"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsstepfunctions"
	"github.com/aws/aws-cdk-go/awscdk/v2/awsstepfunctionstasks"
	"github.com/aws/constructs-go/constructs/v10"
	"github.com/aws/jsii-runtime-go"
)

type VideoEncoderStackProps struct {
	awscdk.StackProps
}

func NewVideoEncoderStack(scope constructs.Construct, id string, props *VideoEncoderStackProps) awscdk.Stack {
	var sprops awscdk.StackProps
	if props != nil {
		sprops = props.StackProps
	}
	stack := awscdk.NewStack(scope, &id, &sprops)

	// ===========================================
	// VPC (use default or create new)
	// ===========================================
	vpc := awsec2.Vpc_FromLookup(stack, jsii.String("Vpc"), &awsec2.VpcLookupOptions{
		IsDefault: jsii.Bool(true),
	})

	// ===========================================
	// ECR Repository
	// ===========================================
	repository := awsecr.NewRepository(stack, jsii.String("EncoderRepository"), &awsecr.RepositoryProps{
		RepositoryName:     jsii.String("video-encoder"),
		RemovalPolicy:      awscdk.RemovalPolicy_DESTROY,
		ImageScanOnPush:    jsii.Bool(true),
		ImageTagMutability: awsecr.TagMutability_MUTABLE,
		LifecycleRules: &[]*awsecr.LifecycleRule{
			{
				Description:   jsii.String("Keep last 10 images"),
				MaxImageCount: jsii.Number(10),
				RulePriority:  jsii.Number(1),
				TagStatus:     awsecr.TagStatus_ANY,
			},
		},
	})

	// ===========================================
	// S3 Buckets
	// ===========================================
	inputBucket := awss3.NewBucket(stack, jsii.String("InputBucket"), &awss3.BucketProps{
		BucketName:        jsii.String("video-encoder-input-" + *awscdk.Aws_ACCOUNT_ID()),
		RemovalPolicy:     awscdk.RemovalPolicy_DESTROY,
		AutoDeleteObjects: jsii.Bool(true),
		Encryption:        awss3.BucketEncryption_S3_MANAGED,
		BlockPublicAccess: awss3.BlockPublicAccess_BLOCK_ALL(),
		LifecycleRules: &[]*awss3.LifecycleRule{
			{
				Id:         jsii.String("CleanupTemp"),
				Prefix:     jsii.String("temp/"),
				Expiration: awscdk.Duration_Days(jsii.Number(1)),
			},
		},
	})

	outputBucket := awss3.NewBucket(stack, jsii.String("OutputBucket"), &awss3.BucketProps{
		BucketName:        jsii.String("video-encoder-output-" + *awscdk.Aws_ACCOUNT_ID()),
		RemovalPolicy:     awscdk.RemovalPolicy_DESTROY,
		AutoDeleteObjects: jsii.Bool(true),
		Encryption:        awss3.BucketEncryption_S3_MANAGED,
		BlockPublicAccess: awss3.BlockPublicAccess_BLOCK_ALL(),
	})

	// ===========================================
	// SQS Queues
	// ===========================================
	deadLetterQueue := awssqs.NewQueue(stack, jsii.String("DeadLetterQueue"), &awssqs.QueueProps{
		QueueName:       jsii.String("video-encoder-dlq"),
		RetentionPeriod: awscdk.Duration_Days(jsii.Number(14)),
	})

	jobQueue := awssqs.NewQueue(stack, jsii.String("JobQueue"), &awssqs.QueueProps{
		QueueName:         jsii.String("video-encoder-jobs"),
		VisibilityTimeout: awscdk.Duration_Hours(jsii.Number(1)),
		RetentionPeriod:   awscdk.Duration_Days(jsii.Number(14)),
		DeadLetterQueue: &awssqs.DeadLetterQueue{
			Queue:           deadLetterQueue,
			MaxReceiveCount: jsii.Number(3),
		},
	})

	// ===========================================
	// CloudWatch Log Group
	// ===========================================
	logGroup := awslogs.NewLogGroup(stack, jsii.String("LogGroup"), &awslogs.LogGroupProps{
		LogGroupName:  jsii.String("/aws/batch/video-encoder"),
		Retention:     awslogs.RetentionDays_ONE_MONTH,
		RemovalPolicy: awscdk.RemovalPolicy_DESTROY,
	})

	// ===========================================
	// IAM Roles
	// ===========================================
	executionRole := awsiam.NewRole(stack, jsii.String("ExecutionRole"), &awsiam.RoleProps{
		RoleName:  jsii.String("video-encoder-execution-role"),
		AssumedBy: awsiam.NewServicePrincipal(jsii.String("ecs-tasks.amazonaws.com"), nil),
		ManagedPolicies: &[]awsiam.IManagedPolicy{
			awsiam.ManagedPolicy_FromAwsManagedPolicyName(jsii.String("service-role/AmazonECSTaskExecutionRolePolicy")),
		},
	})

	taskRole := awsiam.NewRole(stack, jsii.String("TaskRole"), &awsiam.RoleProps{
		RoleName:  jsii.String("video-encoder-task-role"),
		AssumedBy: awsiam.NewServicePrincipal(jsii.String("ecs-tasks.amazonaws.com"), nil),
	})

	inputBucket.GrantRead(taskRole, nil)
	outputBucket.GrantReadWrite(taskRole, nil)
	jobQueue.GrantConsumeMessages(taskRole)
	logGroup.GrantWrite(taskRole)

	// ===========================================
	// Security Group
	// ===========================================
	securityGroup := awsec2.NewSecurityGroup(stack, jsii.String("BatchSecurityGroup"), &awsec2.SecurityGroupProps{
		Vpc:               vpc,
		SecurityGroupName: jsii.String("video-encoder-batch-sg"),
		Description:       jsii.String("Security group for Batch Fargate tasks"),
		AllowAllOutbound:  jsii.Bool(true),
	})

	// ===========================================
	// AWS Batch - Fargate Spot Compute Environment
	// ===========================================
	computeEnv := awsbatch.NewFargateComputeEnvironment(stack, jsii.String("ComputeEnvironment"), &awsbatch.FargateComputeEnvironmentProps{
		ComputeEnvironmentName: jsii.String("video-encoder-fargate-spot"),
		Spot:                   jsii.Bool(true),
		MaxvCpus:               jsii.Number(256),
		Vpc:                    vpc,
		VpcSubnets: &awsec2.SubnetSelection{
			SubnetType: awsec2.SubnetType_PUBLIC,
		},
		SecurityGroups: &[]awsec2.ISecurityGroup{securityGroup},
	})

	// ===========================================
	// AWS Batch - Job Queue
	// ===========================================
	batchJobQueue := awsbatch.NewJobQueue(stack, jsii.String("BatchJobQueue"), &awsbatch.JobQueueProps{
		JobQueueName: jsii.String("video-encoder-job-queue"),
		Priority:     jsii.Number(1),
		ComputeEnvironments: &[]*awsbatch.OrderedComputeEnvironment{
			{
				ComputeEnvironment: computeEnv,
				Order:              jsii.Number(1),
			},
		},
	})

	// ===========================================
	// AWS Batch - Job Definition
	// ===========================================
	containerDef := awsbatch.NewEcsFargateContainerDefinition(stack, jsii.String("ContainerDef"), &awsbatch.EcsFargateContainerDefinitionProps{
		Image:          awsecs.ContainerImage_FromEcrRepository(repository, jsii.String("latest")),
		Cpu:            jsii.Number(4),
		Memory:         awscdk.Size_Mebibytes(jsii.Number(8192)),
		ExecutionRole:  executionRole,
		JobRole:        taskRole,
		AssignPublicIp: jsii.Bool(true),
		Logging: awsecs.NewAwsLogDriver(&awsecs.AwsLogDriverProps{
			LogGroup:     logGroup,
			StreamPrefix: jsii.String("encoder"),
		}),
		Environment: &map[string]*string{
			"INPUT_BUCKET":  inputBucket.BucketName(),
			"OUTPUT_BUCKET": outputBucket.BucketName(),
			"JOB_QUEUE_URL": jobQueue.QueueUrl(),
		},
	})

	jobDefinition := awsbatch.NewEcsJobDefinition(stack, jsii.String("JobDefinition"), &awsbatch.EcsJobDefinitionProps{
		JobDefinitionName: jsii.String("video-encoder"),
		Container:         containerDef,
		RetryAttempts:     jsii.Number(2),
		Timeout:           awscdk.Duration_Hours(jsii.Number(2)),
	})

	// ===========================================
	// Lambda - List S3 Files
	// ===========================================
	listFilesLambda := awslambda.NewFunction(stack, jsii.String("ListFilesLambda"), &awslambda.FunctionProps{
		FunctionName: jsii.String("video-encoder-list-files"),
		Runtime:      awslambda.Runtime_PYTHON_3_12(),
		Handler:      jsii.String("index.handler"),
		Code: awslambda.Code_FromInline(jsii.String(`
import boto3
import json

def handler(event, context):
    s3 = boto3.client('s3')

    bucket = event.get('bucket')
    prefix = event.get('prefix', '')
    extensions = event.get('extensions', ['.mp4', '.mov', '.avi', '.mkv', '.webm'])
    batch_size = event.get('batchSize', 10)

    # List all video files
    files = []
    paginator = s3.get_paginator('list_objects_v2')

    for page in paginator.paginate(Bucket=bucket, Prefix=prefix):
        for obj in page.get('Contents', []):
            key = obj['Key']
            if any(key.lower().endswith(ext) for ext in extensions):
                files.append({
                    'bucket': bucket,
                    'key': key,
                    'size': obj['Size']
                })

    # Split into batches
    batches = []
    for i in range(0, len(files), batch_size):
        batches.append({
            'batchIndex': len(batches),
            'files': files[i:i + batch_size],
            'totalFiles': len(files),
            'totalBatches': (len(files) + batch_size - 1) // batch_size
        })

    return {
        'batches': batches,
        'totalFiles': len(files),
        'totalBatches': len(batches)
    }
`)),
		Timeout:    awscdk.Duration_Minutes(jsii.Number(5)),
		MemorySize: jsii.Number(256),
		Environment: &map[string]*string{
			"INPUT_BUCKET": inputBucket.BucketName(),
		},
	})

	inputBucket.GrantRead(listFilesLambda, nil)

	// ===========================================
	// Lambda - Aggregate Results
	// ===========================================
	aggregateLambda := awslambda.NewFunction(stack, jsii.String("AggregateLambda"), &awslambda.FunctionProps{
		FunctionName: jsii.String("video-encoder-aggregate"),
		Runtime:      awslambda.Runtime_PYTHON_3_12(),
		Handler:      jsii.String("index.handler"),
		Code: awslambda.Code_FromInline(jsii.String(`
import json
from datetime import datetime

def handler(event, context):
    results = event.get('results', [])

    total_processed = 0
    total_failed = 0
    processed_files = []
    failed_files = []

    for batch_result in results:
        if isinstance(batch_result, dict):
            total_processed += batch_result.get('processed', 0)
            total_failed += batch_result.get('failed', 0)
            processed_files.extend(batch_result.get('processedFiles', []))
            failed_files.extend(batch_result.get('failedFiles', []))

    return {
        'summary': {
            'totalProcessed': total_processed,
            'totalFailed': total_failed,
            'completedAt': datetime.utcnow().isoformat(),
        },
        'processedFiles': processed_files,
        'failedFiles': failed_files
    }
`)),
		Timeout:    awscdk.Duration_Minutes(jsii.Number(1)),
		MemorySize: jsii.Number(256),
	})

	// ===========================================
	// Step Functions - State Machine
	// ===========================================

	// Task: List files from S3
	listFilesTask := awsstepfunctionstasks.NewLambdaInvoke(stack, jsii.String("ListFiles"), &awsstepfunctionstasks.LambdaInvokeProps{
		LambdaFunction: listFilesLambda,
		OutputPath:     jsii.String("$.Payload"),
		Comment:        jsii.String("List video files from S3 and split into batches"),
	})

	// Task: Submit Batch job for each file
	submitBatchJob := awsstepfunctionstasks.NewBatchSubmitJob(stack, jsii.String("SubmitEncodingJob"), &awsstepfunctionstasks.BatchSubmitJobProps{
		JobDefinitionArn: jobDefinition.JobDefinitionArn(),
		JobQueueArn:      batchJobQueue.JobQueueArn(),
		JobName:          awsstepfunctions.JsonPath_StringAt(jsii.String("States.Format('encode-{}', $.key)")),
		Payload:          awsstepfunctions.TaskInput_FromObject(&map[string]interface{}{
			"bucket":       awsstepfunctions.JsonPath_StringAt(jsii.String("$.bucket")),
			"key":          awsstepfunctions.JsonPath_StringAt(jsii.String("$.key")),
			"outputBucket": awsstepfunctions.JsonPath_StringAt(jsii.String("$.outputBucket")),
		}),
		IntegrationPattern: awsstepfunctions.IntegrationPattern_RUN_JOB,
		ResultPath:         jsii.String("$.batchResult"),
		Comment:            jsii.String("Submit encoding job to AWS Batch"),
	})

	// Catch errors for individual file processing
	handleFileError := awsstepfunctions.NewPass(stack, jsii.String("HandleFileError"), &awsstepfunctions.PassProps{
		Result: awsstepfunctions.Result_FromObject(&map[string]interface{}{
			"status": "FAILED",
			"error":  awsstepfunctions.JsonPath_StringAt(jsii.String("$.Error")),
			"cause":  awsstepfunctions.JsonPath_StringAt(jsii.String("$.Cause")),
		}),
		ResultPath: jsii.String("$.errorInfo"),
		Comment:    jsii.String("Handle file processing error"),
	})

	submitBatchJob.AddCatch(handleFileError, &awsstepfunctions.CatchProps{
		ResultPath: jsii.String("$.error"),
	})

	// Map state for processing files within a batch
	processFilesMap := awsstepfunctions.NewMap(stack, jsii.String("ProcessFilesInBatch"), &awsstepfunctions.MapProps{
		InputPath:      jsii.String("$.files"),
		MaxConcurrency: jsii.Number(10),
		ResultPath:     jsii.String("$.fileResults"),
		Comment:        jsii.String("Process each file in the batch concurrently"),
	})

	// Add output bucket to each file item
	addOutputBucket := awsstepfunctions.NewPass(stack, jsii.String("AddOutputBucket"), &awsstepfunctions.PassProps{
		Parameters: &map[string]interface{}{
			"bucket":       awsstepfunctions.JsonPath_StringAt(jsii.String("$.bucket")),
			"key":          awsstepfunctions.JsonPath_StringAt(jsii.String("$.key")),
			"size":         awsstepfunctions.JsonPath_NumberAt(jsii.String("$.size")),
			"outputBucket": outputBucket.BucketName(),
		},
		Comment: jsii.String("Add output bucket info to file item"),
	})

	processFilesMap.ItemProcessor(addOutputBucket.Next(submitBatchJob), &awsstepfunctions.ProcessorConfig{
		Mode: awsstepfunctions.ProcessorMode_INLINE,
	})

	// Collect batch results
	collectBatchResults := awsstepfunctions.NewPass(stack, jsii.String("CollectBatchResults"), &awsstepfunctions.PassProps{
		Parameters: &map[string]interface{}{
			"batchIndex":  awsstepfunctions.JsonPath_NumberAt(jsii.String("$.batchIndex")),
			"processed":   awsstepfunctions.JsonPath_NumberAt(jsii.String("States.ArrayLength($.fileResults)")),
			"fileResults": awsstepfunctions.JsonPath_ListAt(jsii.String("$.fileResults")),
		},
		Comment: jsii.String("Collect results from batch processing"),
	})

	// Map state for processing batches
	processBatchesMap := awsstepfunctions.NewMap(stack, jsii.String("ProcessBatches"), &awsstepfunctions.MapProps{
		InputPath:      jsii.String("$.batches"),
		MaxConcurrency: jsii.Number(5),
		ResultPath:     jsii.String("$.batchResults"),
		Comment:        jsii.String("Process batches in parallel (max 5 concurrent batches)"),
	})

	processBatchesMap.ItemProcessor(processFilesMap.Next(collectBatchResults), &awsstepfunctions.ProcessorConfig{
		Mode: awsstepfunctions.ProcessorMode_INLINE,
	})

	// Check if there are files to process
	checkFilesExist := awsstepfunctions.NewChoice(stack, jsii.String("CheckFilesExist"), &awsstepfunctions.ChoiceProps{
		Comment: jsii.String("Check if there are files to process"),
	})

	noFilesFound := awsstepfunctions.NewPass(stack, jsii.String("NoFilesFound"), &awsstepfunctions.PassProps{
		Result: awsstepfunctions.Result_FromObject(&map[string]interface{}{
			"message":    "No video files found in the specified location",
			"totalFiles": 0,
		}),
		Comment: jsii.String("No files found - return early"),
	})

	// Aggregate results
	aggregateTask := awsstepfunctionstasks.NewLambdaInvoke(stack, jsii.String("AggregateResults"), &awsstepfunctionstasks.LambdaInvokeProps{
		LambdaFunction: aggregateLambda,
		Payload: awsstepfunctions.TaskInput_FromObject(&map[string]interface{}{
			"results":    awsstepfunctions.JsonPath_ListAt(jsii.String("$.batchResults")),
			"totalFiles": awsstepfunctions.JsonPath_NumberAt(jsii.String("$.totalFiles")),
		}),
		OutputPath: jsii.String("$.Payload"),
		Comment:    jsii.String("Aggregate all batch results"),
	})

	// Build state machine definition
	definition := listFilesTask.Next(
		checkFilesExist.
			When(
				awsstepfunctions.Condition_NumberEquals(jsii.String("$.totalFiles"), jsii.Number(0)),
				noFilesFound,
				nil,
			).
			Otherwise(processBatchesMap.Next(aggregateTask)),
	)

	// Create state machine
	stateMachine := awsstepfunctions.NewStateMachine(stack, jsii.String("VideoEncoderStateMachine"), &awsstepfunctions.StateMachineProps{
		StateMachineName: jsii.String("video-encoder-pipeline"),
		DefinitionBody:   awsstepfunctions.DefinitionBody_FromChainable(definition),
		Timeout:          awscdk.Duration_Hours(jsii.Number(24)),
		TracingEnabled:   jsii.Bool(true),
		Logs: &awsstepfunctions.LogOptions{
			Destination:          logGroup,
			Level:                awsstepfunctions.LogLevel_ALL,
			IncludeExecutionData: jsii.Bool(true),
		},
		Comment: jsii.String("Video encoding pipeline with parallel batch processing"),
	})

	// Grant Step Functions permission to submit Batch jobs
	stateMachine.AddToRolePolicy(awsiam.NewPolicyStatement(&awsiam.PolicyStatementProps{
		Actions: &[]*string{
			jsii.String("batch:SubmitJob"),
			jsii.String("batch:DescribeJobs"),
			jsii.String("batch:TerminateJob"),
		},
		Resources: &[]*string{
			jsii.String("*"),
		},
	}))

	// ===========================================
	// Outputs
	// ===========================================
	awscdk.NewCfnOutput(stack, jsii.String("RepositoryUri"), &awscdk.CfnOutputProps{
		Value:       repository.RepositoryUri(),
		Description: jsii.String("ECR repository URI"),
		ExportName:  jsii.String("VideoEncoderRepositoryUri"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("InputBucketName"), &awscdk.CfnOutputProps{
		Value:       inputBucket.BucketName(),
		Description: jsii.String("Input S3 bucket name"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("OutputBucketName"), &awscdk.CfnOutputProps{
		Value:       outputBucket.BucketName(),
		Description: jsii.String("Output S3 bucket name"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("JobQueueArn"), &awscdk.CfnOutputProps{
		Value:       batchJobQueue.JobQueueArn(),
		Description: jsii.String("Batch Job Queue ARN"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("JobDefinitionArn"), &awscdk.CfnOutputProps{
		Value:       jobDefinition.JobDefinitionArn(),
		Description: jsii.String("Batch Job Definition ARN"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("StateMachineArn"), &awscdk.CfnOutputProps{
		Value:       stateMachine.StateMachineArn(),
		Description: jsii.String("Step Functions State Machine ARN"),
	})

	awscdk.NewCfnOutput(stack, jsii.String("SQSJobQueueUrl"), &awscdk.CfnOutputProps{
		Value:       jobQueue.QueueUrl(),
		Description: jsii.String("SQS Job Queue URL"),
	})

	return stack
}

func main() {
	defer jsii.Close()

	app := awscdk.NewApp(nil)

	NewVideoEncoderStack(app, "VideoEncoderStack", &VideoEncoderStackProps{
		awscdk.StackProps{
			Env: env(),
		},
	})

	app.Synth(nil)
}

func env() *awscdk.Environment {
	account := os.Getenv("CDK_DEFAULT_ACCOUNT")
	region := os.Getenv("CDK_DEFAULT_REGION")

	if account == "" {
		account = os.Getenv("AWS_ACCOUNT_ID")
	}
	if region == "" {
		region = "ap-northeast-1"
	}

	return &awscdk.Environment{
		Account: jsii.String(account),
		Region:  jsii.String(region),
	}
}
