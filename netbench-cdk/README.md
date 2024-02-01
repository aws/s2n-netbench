# Netbench-cdk

This CDK routine sets up the long-lived infrastructure needed to run complex NetbenchOrchestration tests.

## Setup

You'll need nodejs (recommended 18), typescript and cdk.

For individual use, in a terminal: 
- export the environment variable `DEV_ACCOUNT_ID` with your AWS account, e.g. `export DEV_ACCOUNT_ID=857630911`
- Be sure your aws cli has authenticated; test with `aws s3 ls`, which should return a list of buckets.
- Next, bootstrap your environment: `cdk bootstrap aws://$DEV_ACCOUNT_ID/us-west-2`. This is region specific, so update this as needed.  This is only needed one time, as it sets up CDK metadata stores and other helper utilitis.
- Finally, deploy the infrastructure with `cdk deploy NetbenchInfraDev-$USER`
- You'll be prompted, everytime, to accept security permissions changes.  This can be disabled by adding the flag `--require-approval never`.

## State

Reminder that the cdk.out directory contains the state tracking for the CDK stacks.  
**The state directory is not checked into source control**


## Useful commands

* `npm run build`   compile typescript to js
* `npm run watch`   watch for changes and compile
* `npm run test`    perform the jest unit tests
* `npx cdk deploy`  deploy this stack to your default AWS account/region
* `npx cdk diff`    compare deployed stack with current state
* `npx cdk synth`   emits the synthesized CloudFormation template
