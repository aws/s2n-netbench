#!/usr/bin/env python3
import boto3
import logging
from datetime import datetime, timezone
from os import getenv

logger = logging.getLogger()
logger.setLevel(logging.INFO)

# Max instance lifetime in seconds defaulting to one day.
# Over-rideable via an environment variable.
MAX_LIFETIME: int = int(getenv("MAX_LIFETIME", 86400))

def get_ebs_age(date_obj: datetime|str) -> int:
    # Convert a date object or str to age in seconds

    # Date object conversion is done automajically with boto3
    # but not with a standard json.load() from tests
    if type(date_obj) == str:
      date_format = '%Y-%m-%dT%H:%M:%S%z'
      date_obj = datetime.strptime(date_obj, date_format)
    
    now = datetime.now(tz=timezone.utc)
    delta = now - date_obj
    if delta.total_seconds() < 1:
        raise TypeError(f"Date is in the future:{delta}")
    else:
      return int(delta.total_seconds())

def lambda_handler(event, context):
    """
    - call the ec2 describe-instances, filtering on running
    - compare the EBS root volume attach time to now()
    - emit an alarm if it's over the limit
    """
    ec2_client = boto3.client('ec2')
    response = ec2_client.describe_instances(
      Filters=[
        {'Name': 'instance-state-name',
         'Values': [ "running"]
        }
      ])
    return process_describe_instances(response)

def process_describe_instances(response: dict) -> dict:
    # Walk the running instance list checking the age of the disk mount
    # against MAX_LIFETIME
    instance_above_max: dict[str, int] = {}
    instance_below_max: dict[str, int] = {}
    alarm: bool = False
    for group in response['Reservations']:
        instance = group['Instances'][0]
        # If this is missing, skip this instance
        # this should never happen in the running state.
        if 'BlockDeviceMappings' not in instance:
            continue
        if len(instance['BlockDeviceMappings']) > 0:
            age = get_ebs_age(instance['BlockDeviceMappings'][0]['Ebs']['AttachTime'])
            if age > MAX_LIFETIME:
                alarm = True
                instance_above_max[instance['InstanceId']] = age
            else:
                instance_below_max[instance['InstanceId']] = age
        else:
            continue
    return {"alarm_threshold": MAX_LIFETIME, 
            "overall_alarm": alarm, 
            "instances_below_max": instance_below_max, 
            "instances_above_max": instance_above_max }
