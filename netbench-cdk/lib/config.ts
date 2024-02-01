import { StackProps } from "aws-cdk-lib";
import { ConstructOrder } from "constructs";
import { Construct } from 'constructs';

interface ConfigInterface {
  AWSStage: string | null;
  VpcCidr: string;
  VpcCidrMask: number;
  VpcMaxAzs: number;
}

export class Config implements ConfigInterface {
  AWSStage: string = "";
  VpcCidr: string = "10.0.0.0/16";
  VpcCidrMask: number = 24;
  VpcMaxAzs: number = 2;

  constructor(bucketSuffix?: string) {
    //TODO: Look into emitting these as json or TOML
    //consider a 2nd json, one with user settings, one with cdk output
  }

};

export interface NetbenchStackProps extends StackProps {
  bucketSuffix?: string;
  reportStack: boolean
}

// Production stack properties

export const ProdStackPrimaryProps: NetbenchStackProps = {
  env: { region: "us-west-2" },
  /*
    There are two s3 buckets, one for public reports, and one
    for private source code. To keep their names globally unique, the
    bucketSuffix will be appended to the end,
    defaulting to `prod` or $user if DEV_ACCOUNT_ID is set.
  */
  bucketSuffix: "prod",
  reportStack: true, // Only one reporting stack per environment.
  terminationProtection: true,
}

export const ProdStackSecondaryProps: NetbenchStackProps = {
  env: { region: "us-east-2" },
  terminationProtection: true,
  reportStack: false
}