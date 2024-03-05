#!/usr/bin/env node
import { Construct } from 'constructs';
import { BucketDeployment } from 'aws-cdk-lib/aws-s3-deployment';
import * as cdk from 'aws-cdk-lib'
import { S3Origin } from 'aws-cdk-lib/aws-cloudfront-origins';
import * as logs from 'aws-cdk-lib/aws-logs'
import { Config, NetbenchStackProps } from './config';
import path from 'path';
import { IBucket } from 'aws-cdk-lib/aws-s3';
import { readFileSync } from 'fs';
import { PolicyStatement } from 'aws-cdk-lib/aws-iam';
import { scheduler } from 'timers/promises';
import { Schedule } from 'aws-cdk-lib/aws-events';

export class NetbenchInfra extends cdk.Stack {
    private config: Config = new Config;

    constructor(scope: Construct, id: string, props?: NetbenchStackProps) {
        super(scope, id, props);
        this.createVPC();
        this.createCloudwatchGroup();
        this.createRole();
        this.createMonitorLambda();

        const GHAUser = this.createGHAIamUser();
        // We're over-riding CF's naming scheme so this name 
        // must be globally unique. By default, AWSStage will be username.
        if (props?.reportStack) {
            let bucketName: string = "";
            if (props && props.bucketSuffix) {
                bucketName = `netbenchrunnerlogs-public-${props.bucketSuffix}`;
            } else {
                throw new Error('Unable to determine reporting bucket suffix');
            }
            // Create the public logs bucket
            const distBucket = this.createS3Bucket(bucketName, true);
            new cdk.CfnOutput(this, "output:NetbenchRunnerPublicLogsBucket", { value: distBucket.bucketName })
            this.createCloudFront('CFdistribution', distBucket);

            // Create the private source code bucket, without any distribution.
            const srcCodeBucket = this.createS3Bucket(`netbenchrunner-private-source-${props.bucketSuffix}`, false);
            new cdk.CfnOutput(this, "output:NetbenchRunnerPrivateSrcBucket", { value: srcCodeBucket.bucketName })

            // Stitch together the buckets, a policy, and the GHA user
            distBucket.grantReadWrite(GHAUser);
            srcCodeBucket.grantReadWrite(GHAUser);
        }
    }

    private createCloudwatchGroup() {
        //SSM logs
        //TODO: add a retention policy
        const logGroup = new logs.LogGroup(this, 'NetbenchRunnerLogGroup');
        new cdk.CfnOutput(this, "output:NetbenchRunnerLogGroup", { value: logGroup.logGroupName })
    }

    private createVPC() {
        // Creating VPC for clients and servers
        const vpc = new cdk.aws_ec2.Vpc(this, 'vpc', {
            ipAddresses: cdk.aws_ec2.IpAddresses.cidr(this.config.VpcCidr),
            maxAzs: this.config.VpcMaxAzs,
            subnetConfiguration: [
                {
                    cidrMask: this.config.VpcCidrMask,
                    name: 'NetbenchRunnerSubnet',
                    subnetType: cdk.aws_ec2.SubnetType.PUBLIC,
                }
            ],

        });

        //Tag all available subnets the same. This behavior might need to change when MultiRegion is added.
        const subnetTagKey = "aws-cdk:netbench-subnet-name";
        const subnetTagValue = "public-subnet-for-netbench-runners";
        vpc.publicSubnets.forEach(element => {
            cdk.Tags.of(element).add(subnetTagKey, subnetTagValue);
        });
        new cdk.CfnOutput(this, "output:NetbenchSubnetTagKey", { value: subnetTagKey });
        new cdk.CfnOutput(this, "output:NetbenchSubnetTagValue", { value: subnetTagValue });
        new cdk.CfnOutput(this, "output:" + this.stackName + "Region", { value: this.region });
    };
    private createCloudFront(id: string, bucket: IBucket) {
        const cfDistribution = new cdk.aws_cloudfront.Distribution(this, id, {
            defaultBehavior: {
                origin: new S3Origin(bucket),
                viewerProtocolPolicy: cdk.aws_cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
            },
            defaultRootObject: "index.html"
        });
        new cdk.CfnOutput(this, 'output:NetbenchCloudfrontDistribution', { value: "https://" + cfDistribution.distributionDomainName });
    };

    private createRole() {
        // Create IAM role for the EC2 instances
        const instanceRole = new cdk.aws_iam.Role(this, 'NetbenchRunnerInstanceRole', {
            assumedBy: new cdk.aws_iam.ServicePrincipal('ec2.amazonaws.com'),
        });

        // Create an instance profile to allow ec2 to use the role.
        // https://docs.aws.amazon.com/IAM/latest/UserGuide/id_roles_use_switch-role-ec2_instance-profiles.html
        const instanceProfile = new cdk.aws_iam.InstanceProfile(this, 'instanceProfile', { role: instanceRole })
        new cdk.CfnOutput(this, "output:NetbenchRunnerInstanceProfile", { value: instanceProfile.instanceProfileName })

        // Attach managed policies to the IAM role
        instanceRole.addManagedPolicy(cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMFullAccess'));
        // TODO: This is too permissive- scope this down to just the netbench bucket.
        instanceRole.addManagedPolicy(cdk.aws_iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonS3FullAccess'));
    };

    private createGHAIamUser(): cdk.aws_iam.User {
        return new cdk.aws_iam.User(this, "s2n-netbench-githubactions", { userName: "s2n-netbench-githubactions" });
    }

    /* For now, let CDK create this policy with bucket.GrantReadWrite()
    private createGHAIamPolicy(s3Bucket: cdk.aws_s3.Bucket): cdk.aws_iam.Policy {
        return new cdk.aws_iam.Policy(this, "s2n-netbench-githubactions-policy", {
            statements: [new iam.PolicyStatement({
                effect: Effect.ALLOW,
                actions: ["s3:PutObject",
                    "s3:GetObject",
                    "s3:AbortMultipartUpload",
                    "s3:ListBucket",
                    "s3:GetObjectVersion"],
                resources: [s3Bucket.bucketArn, s3Bucket.bucketArn + "/*"],
            })]
        });
    }
    */

    private createS3Bucket(id: string, reportBucket: boolean): cdk.aws_s3.Bucket {
        // NOTE: putting the bucketName in the bucketProperties
        // over-rides CloudFormation's unique naming scheme
        let bucketProperties = {
            bucketName: id,
            blockPublicAccess: cdk.aws_s3.BlockPublicAccess.BLOCK_ALL,
            encryption: cdk.aws_s3.BucketEncryption.S3_MANAGED,
            enforceSSL: true,
            // On stack destroy, keep the bucket and it's contents, leaving an orphan.
            // This will require manual cleanup if you'd like to recreate the stack.
            removalPolicy: cdk.RemovalPolicy.RETAIN,
        }
        const netbenchBucket = new cdk.aws_s3.Bucket(this, id, bucketProperties)

        if (reportBucket) {
            // If this is a reporting bucket, populate it with the contents of ./staticfiles/.
            const deployment = new BucketDeployment(this, 'NetbenchReportBucketContents', {
                sources: [cdk.aws_s3_deployment.Source.asset(path.join(__dirname, "../staticfiles"))],
                destinationBucket: netbenchBucket,
                prune: false,  // Do NOT delete objects in s3 that don't exist locally.
            });
        }

        const bucketActions = ['s3:AbortMultipartUpload',
            's3:GetBucketLocation',
            's3:GetObject',
            's3:ListBucket',
            's3:ListBucketMultipartUploads',
            's3:ListMultipartUploadParts',
            's3:PutObject']
        netbenchBucket.addToResourcePolicy(new cdk.aws_iam.PolicyStatement({
            sid: 'netbenchec2',
            // Special CDK construct that implicitly adds a condition to the policy.
            principals: [new cdk.aws_iam.AnyPrincipal().inOrganization(`arn:aws:sts::${this.account}:assumed-role`)],
            effect: cdk.aws_iam.Effect.ALLOW,
            actions: bucketActions,
            resources: [`${netbenchBucket.bucketArn}/*`,
            netbenchBucket.bucketArn]
        }))

        return netbenchBucket;
    };

    private createMonitorLambda(): any {
        const monitorLambda = new cdk.aws_lambda.Function(this, "netbenchMonitor", {
            runtime: cdk.aws_lambda.Runtime.PYTHON_3_12,
            handler: "index.lambda_handler",
            code: cdk.aws_lambda.Code.fromInline(readFileSync('./netbench-monitor/handler.py', 'utf-8')),
            timeout: cdk.Duration.seconds(15)
        })
        //monitorLambda.addToRolePolicy(PolicyStatement())
        const describePolicy = cdk.aws_iam.PolicyStatement.fromJson({
            "Sid": "VisualEditor0",
            "Effect": "Allow",
            "Action": "ec2:DescribeInstances",
            "Resource": "*"
        }
        );
        monitorLambda.addToRolePolicy(describePolicy);

        new cdk.aws_events.Rule(this, 'ScheduledRun', {
            description: "Schedule; managed by cdk",
            schedule: Schedule.cron({
                year: "*",
                month: "*",
                day: "*",
                hour: "17",
                minute: "0"
            }),
            targets: [new cdk.aws_events_targets.LambdaFunction(monitorLambda)],
        });
        return monitorLambda;
    }
}
