#!/usr/bin/env node
import { Construct } from 'constructs';
import * as cdk from 'aws-cdk-lib';
import { BucketDeployment } from 'aws-cdk-lib/aws-s3-deployment';
import { aws_cloudfront as cloudfront } from 'aws-cdk-lib';
import { aws_ec2 as ec2, aws_iam as iam, aws_s3 as s3, CfnResource } from 'aws-cdk-lib';
import { S3Origin } from 'aws-cdk-lib/aws-cloudfront-origins';
import * as logs from 'aws-cdk-lib/aws-logs'
import { Config, NetbenchStackProps } from './config';
import path from 'path';
import { IBucket } from 'aws-cdk-lib/aws-s3';

export class NetbenchInfra extends cdk.Stack {
    private config: Config = new Config;

    constructor(scope: Construct, id: string, props?: NetbenchStackProps) {
        super(scope, id, props);
        this.createPlacementGroups();
        this.createVPC();
        this.createCloudwatchGroup();
        this.createRole();
        // We're over-riding CF's naming scheme so this name 
        // must be globally unique. By default, AWSStage will be username.
        if (props?.reportStack) {
            let bucketName: string = "";
            if (props && props.bucketSuffix) {
                bucketName = `netbenchrunnerlogs-public-${props.bucketSuffix}`;
            } else {
                throw new Error('Unable to determine reporting bucket suffix');
            }
            const distBucket = this.createS3Bucket(bucketName, true);
            this.createCloudFront('CFdistribution', distBucket);
            // Create the private source code bucket, without any distribution.
            const srcCodeBucket = this.createS3Bucket(`netbenchrunner-private-source-${props.bucketSuffix}`, false);
        }
    }

    private createCloudwatchGroup() {
        //SSM logs
        //TODO: add a retention policy
        const logGroup = new logs.LogGroup(this, 'NetbenchRunnerLogGroup');
        new cdk.CfnOutput(this, "output:NetbenchRunnerLogGroup", { value: logGroup.logGroupName })
    }

    private createPlacementGroups() {
        const cluster = new ec2.PlacementGroup(this, 'Cluster', {
            placementGroupName: 'NetbenchRunnerPlacementGroupCluster',
            strategy: ec2.PlacementGroupStrategy.CLUSTER
        });
        // Max 7 partitions per AZ: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/placement-groups.html
        const partition = new ec2.PlacementGroup(this, 'Partition', {
            placementGroupName: 'NetbenchRunnerPlacementGroupPartition',
            partitions: 7,
            strategy: ec2.PlacementGroupStrategy.PARTITION
        });
        const spread = new ec2.PlacementGroup(this, 'Spread', {
            placementGroupName: 'NetbenchRunnerPlacementGroupSpread',
            spreadLevel: ec2.PlacementGroupSpreadLevel.RACK,
            strategy: ec2.PlacementGroupStrategy.SPREAD
        })
    }
    private createVPC() {
        // Creating VPC for clients and servers
        const vpc = new ec2.Vpc(this, 'vpc', {
            ipAddresses: ec2.IpAddresses.cidr(this.config.VpcCidr),
            maxAzs: this.config.VpcMaxAzs,
            subnetConfiguration: [
                {
                    cidrMask: this.config.VpcCidrMask,
                    name: 'NetbenchRunnerSubnet',
                    subnetType: ec2.SubnetType.PUBLIC,
                }
            ],

        });
        //Netbench Orchistrator is expecting only one tagged subnet.
        cdk.Tags.of(vpc.publicSubnets[0]).add('aws-cdk:subnet-name', 'public-subnet-for-runners');
    };
    private createCloudFront(id: string, bucket: IBucket) {
        const cfDistribution = new cloudfront.Distribution(this, id, {
            defaultBehavior: {
                origin: new S3Origin(bucket),
                viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
            },
            defaultRootObject: "index.html"
        });
        new cdk.CfnOutput(this, 'NetbenchCloudfrontDistribution', { value: "https://" + cfDistribution.distributionDomainName });
    };

    private createRole() {
        // Create IAM role for the EC2 instances
        const instanceRole = new iam.Role(this, 'NetbenchRunnerInstanceRole', {
            assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
        });

        // Create an instance profile to allow ec2 to use the role.
        // https://docs.aws.amazon.com/IAM/latest/UserGuide/id_roles_use_switch-role-ec2_instance-profiles.html
        const instanceProfile = new iam.InstanceProfile(this, 'instanceProfile', { role: instanceRole })
        new cdk.CfnOutput(this, "NetbenchRunnerInstanceProfile", { value: instanceProfile.instanceProfileName })

        // Attach managed policies to the IAM role
        instanceRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMFullAccess'));
        // TODO: This is too permissive- scope this down to just the netbench bucket.
        instanceRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonS3FullAccess'));
    };

    private createS3Bucket(id: string, reportBucket: boolean): cdk.aws_s3.Bucket {
        // NOTE: putting the bucketName in the bucketProperties
        // over-rides CloudFormation's unique naming scheme
        let bucketProperties = {
            bucketName: id,
            blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
            encryption: s3.BucketEncryption.S3_MANAGED,
            enforceSSL: true,
            // On stack destroy, keep the bucket and it's contents, leaving an orphan.
            // This will require manual cleanup if you'd like to recreate the stack.
            removalPolicy: cdk.RemovalPolicy.RETAIN,
        }
        const netbenchBucket = new s3.Bucket(this, id, bucketProperties)

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

}
