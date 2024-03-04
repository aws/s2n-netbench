#!/usr/bin/env python3
import boto3
import json
import logging
import os

from datetime import datetime, timezone
from base64 import b64decode
from urllib.request import Request, urlopen
from urllib.error import URLError, HTTPError

logger = logging.getLogger()
logger.setLevel(logging.INFO)

# Max instance lifetime in seconds
MAX_LIFETIME: int = 86400

def get_ebs_age(date_obj: datetime) -> int:
    # Date object conversion is unneeded because boto3 is doing this automatically.
    #date_format = '%Y-%m-%dT%H:%M:%S%z'
    #date_obj = datetime.strptime(ec2_ebs, date_format)
    now = datetime.now(tz=timezone.utc)
    delta = now-date_obj
    return int(delta.total_seconds())
    
def lambda_handler(event, context):
    """
    - call the ec2 describe-instances 
    - filter on ! terminated
    - compare the EBS root volume attach time to now()
    - alarm if it's over the limit
    """
    print("Event: " + str(event))
    instance_above_max: dict[str, int] = {}
    instance_below_max: dict[str, int] = {}
    alarm: bool = False
    ec2_client = boto3.client('ec2')
    response = ec2_client.describe_instances(
      Filters=[
        {'Name': 'instance-state-name',
         'Values': [ "running"]
        }
      ])
    '''
    aws api `describe-instance` returns an Ebs structure, with a timestamp when 
    the EBS Volume was attached, using this as a proxy for age of the instance.
    aws ec2 desribe-instances | jq -r '.Reservations[].Instances[]|select(.State.Name=="running")|.BlockDeviceMappings[0].Ebs.AttachTime' ec2-desc-instances.json
    2024-01-22T16:37:56+00:00
    
    Risks:
      BlockDevice mapping might not be ordered reliably.
      Large instances lists might get paginated, but the running filter helps reduce response size.
    '''
    print(f"Response: {response['Reservations']}")
    for group in response['Reservations']:
        instance = group['Instances'][0]
        if len(instance['BlockDeviceMappings']) > 0: 
            #instance_ages[instance['InstanceId']] = get_ebs_age(instance['BlockDeviceMappings'][0]['Ebs']['AttachTime'])
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

'''
    cw_metric_data = [{
            'MetricName': name_mapping[event['detail']['build-status']],
            'Dimensions':event_dimension(event),
            'Timestamp': event['time'],
            'Value': 1,
            'Unit': 'Count',
            'StorageResolution': 60
        }]

    logging.info(f"New metric: {cw_metric_data}")

    response = client.put_metric_data(Namespace='CustomCodeBuild',MetricData=cw_metric_data)
    '''
