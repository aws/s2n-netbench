from boto3 import resource,client
from botocore.exceptions import ClientError
from enum import Enum


class CloudWatchNamespaceStr:
    """
    Namespace object to check for disallowed strings.
    """
    def __init__(self, namespace: str):
        if 'AWS/' in namespace:
            raise ValueError('AWS/ is not allowed in namespace')
        else:
            self.namespace = namespace

    def __str__(self):
        return self.namespace


class CWUnit(Enum):
    """
    Possible units for CloudWatch Metric Data.
    """
    Seconds = 'Seconds'
    Microseconds = 'Microseconds'
    Milliseconds = 'Milliseconds'
    Bytes = 'Bytes'
    Kilobytes = 'Kilobytes'
    Megabytes = 'Megabytes'
    Gigabytes = 'Gigabytes'
    Terabytes = 'Terabytes'
    Bits = 'Bits'
    Kilobits = 'Kilobits'
    Megabits = 'Megabits'
    Gigabits = 'Gigabits'
    Terabits = 'Terabits'
    Percent = 'Percent'
    Count = 'Count'
    Bytes_Second = 'Bytes/Second'
    Kilobytes_Second = 'Kilobytes/Second'
    Megabytes_Second = 'Megabytes/Second'
    Gigabytes_Second = 'Gigabytes/Second'
    Terabytes_Second = 'Terabytes/Second'
    Bits_Second = 'Bits/Second'
    Kilobits_Second = 'Kilobits/Second'
    Megabits_Second = 'Megabits/Second'
    Gigabits_Second = 'Gigabits/Second'
    Terabits_Second = 'Terabits/Second'
    Count_Second = 'Count/Second'
    NONE = 'None'


class CloudWatchMetricData:
    """
    Single CloudWatch Metric Data Object.
    """
    metricname: str
    value: float
    dimensions: list[dict]
    unit: CWUnit

    def __init__(self, metricname: str, value: float, unit: CWUnit, dimension:list):
        self.metricname = metricname
        self.value = value
        self.unit = unit
        self.dimensions = dimension

    def __call__(self):
        return {"MetricName": self.metricname,
                "Dimensions": self.dimensions,
                "Value": self.value,
                "Unit": self.unit.value}


class CloudWatchMetricDataRequest:
    """
    List of CloudWatch Metric Data Objects.
    Also knows how to send data to CloudWatch via a boto client.
    """
    namespace: str
    metriclist: list[CloudWatchMetricData] = []

    def __init__(self, namespace: str):
        self.namespace = namespace

    def append(self, value: CloudWatchMetricData):
        self.metriclist.append(value)

    def __len__(self):
        return len(self.metriclist)

    def _expand(self):
        response = []
        for metric in self.metriclist:
            response.append(metric())
        return response

    def __call__(self, ):
        return {'Namespace': self.namespace, 'MetricData': self._expand()}

    def put_data(self, cwClient: client):
        try:
            cwClient.put_metric_data(Namespace=self.namespace, MetricData=self._expand())
        except ClientError:
            raise ClientError("Couldn't put data for metric %s.%s", self.namespace, self.name)
