# Netbench-cdk

This CDK routine sets up the long-lived infrastructure needed to run complex NetbenchOrchestration tests.
Note, after finishing the getting started steps, you'll still need to install and setup the NetbenchOrchestrator to actually run tests. 

## Pre-requisites

You'll need: 
- [nodejs](https://nodejs.org/en/learn/getting-started/how-to-install-nodejs) (recommended 18), 
- typescript 
- [AWS CDK](https://github.com/aws/aws-cdk?tab=readme-ov-file#at-a-glance)
- [AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html)
- Gnu Make (optional)


## Getting started

For individual use, in a terminal: 
- export the environment variable `DEV_ACCOUNT_ID` with your AWS account, e.g. `export DEV_ACCOUNT_ID=857630911`
- export the environment variable `AWS_DEFAULT_REGION` with your preferred AWS account region, e.g. `export AWS_DEFAULT_REGION=us-west-2`.  CDK will put some helper infrastructure in whatever region is specified.
- Be sure your AWS cli has authenticated; test with `aws s3 ls`, which should return a list of buckets.
- Build and deploy with: `make deploy`

Reminder that some state files are kept in the cdk.out directory for the CDK stacks and **is not checked into source control**

While we've tried to put helpful shortcuts into the Makefile, `cdk` commands can always be run directly.
