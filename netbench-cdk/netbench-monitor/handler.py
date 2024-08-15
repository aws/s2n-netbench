#!/usr/bin/env python3
# Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0
import boto3
import logging
import metrics
from datetime import datetime, timezone
from os import getenv

logger = logging.getLogger()
logger.setLevel(logging.INFO)

def get_ebs_age(date_obj: datetime|str) -> int:
    """
    Convert a date object or str to age in seconds
    """

    # String to Date object conversion is done automajically with boto3
    # but not with a standard json.load() from tests
    if type(date_obj) == str:
      date_format = '%Y-%m-%dT%H:%M:%S%z'
      try:
        date_obj = datetime.strptime(date_obj, date_format)
      except ValueError as e:
        print(f"The date format was unexpected: {e} ")
        raise

    now = datetime.now(tz=timezone.utc)
    delta = now - date_obj
    if delta.total_seconds() < 1:
        raise ValueError(f"Date is in the future:{delta}")
    else:
      return int(delta.total_seconds())

def lambda_handler(event, context):
    """
    Use the ec2 describe-instances call to determine the age
    of all running instances. Emit weather an alarm is true,
    and a list of instances above and below threshold.
    """
    ec2_client = boto3.client('ec2')
    print("Running ec2 describe-instances")
    response = ec2_client.describe_instances(
      Filters=[
        {'Name': 'instance-state-name',
         'Values': [ "running"]
        }
      ])
    print("Processing ec2 response...")
    cwobj = create_cw_metric_data(process_describe_instances(response))
    client = boto3.client('cloudwatch')
    print(cwobj())
    cwobj.put_data(client)
    print("Done")

def process_describe_instances(response: dict) -> dict:
    # Walk the running instance list, gather checking the age of the disk mount
    # returns [{InstanceId,VolumeAge} ]
    instances:set[str, int] = {}
    for group in response['Reservations']:
        instance = group['Instances'][0]
        # If this is missing, skip this instance
        # this should never happen in the running state.
        if 'BlockDeviceMappings' not in instance:
            raise ValueError("Missing expected field BlockDeviceMapping; "
                             + "running instances should have at least one "
                             + "block device attached.")

        if len(instance['BlockDeviceMappings']) > 0:
            instances[instance['InstanceId']] = get_ebs_age(instance['BlockDeviceMappings'][0]['Ebs']['AttachTime'])
        else:
            continue
    return instances

def create_cw_metric_data(instances: set[str, int]) -> metrics.CloudWatchMetricDataRequest:
   """
   Convert the list of instanceIds/ages to a list of CloudWatchMetricData objects.
   {'i-01b672902ce93a9de': 13305681, 'i-04629ccbb2da1de4b': 13305689}
   """
   CWmetrics = metrics.CloudWatchMetricDataRequest("netbench")
   for k,v in instances.items():
      dimensions = [{"Name":"InstanceId", "Value":k}]
      CWmetrics.append(metrics.CloudWatchMetricData(metricname="InstanceAge",value=v,unit=metrics.CWUnit.Seconds, dimension=dimensions))
   return CWmetrics


if __name__ == "__main__":
    lambda_handler(None, None)