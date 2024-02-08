#!/usr/bin/env node
import 'source-map-support/register';
import { App } from 'aws-cdk-lib';
import { NetbenchInfra } from "../lib/netbench"
import { ProdStackPrimaryProps, ProdStackSecondaryProps } from '../lib/config';

const AWS_DEFAULT_REGION = process.env["AWS_DEFAULT_REGION"] || "us-west-2";
const app = new App();

if (process.env["DEV_ACCOUNT_ID"]) {
  // Development stack only exists if DEV_ACCOUNT_ID is set.
  let user = process.env["USER"];
  if (user == null) {
    throw new Error('Unable to determine username');
  } else {
    user = user.toLowerCase();
  }
  new NetbenchInfra(app, `NetbenchInfraDev-${user}`, {
    env: { account: `${process.env.DEV_ACCOUNT_ID}`, region: AWS_DEFAULT_REGION },
    terminationProtection: false,
    bucketSuffix: `${user}`,
    reportStack: true
  });
} else {
  // Production stack only exists if DEV_ACCOUNT_ID is NOT set.
  new NetbenchInfra(app, 'NetbenchInfraPrimaryProd', ProdStackPrimaryProps);
  /* TODO: Second Region
  new NetbenchInfra(app, 'NetbenchInfraSecondaryProd', ProdStackSecondaryProps);
  */
}
