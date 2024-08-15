# Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0

import pytest
import handler
import json
from datetime import datetime, timedelta, timezone


def test_get_ebs_age_happy():
    """
    Validate the time math is correct.
    """
    yesterday = datetime.now(tz=timezone.utc) - timedelta(days=1)
    assert handler.get_ebs_age(yesterday) == 86400


def test_get_ebs_age_future():
    """
    Validate that times in the future raise an exception.
    """
    today = datetime.now(tz=timezone.utc) + timedelta(days=1)
    with pytest.raises(ValueError):
        handler.get_ebs_age(today)


def test_get_ebs_age_badformat():
    """
    Validate that an invalid date format raises an exception.
    """
    with pytest.raises(ValueError):
        handler.get_ebs_age("2024-03-05T15:59")


def test_handler_happy(file="tests/response.json"):
    """
    response.json is an actual valid ec2 describe instance response.
    """
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    response_json = handler.process_describe_instances(raw_json)
    assert response_json['i-0e1c4f0a1d8b96602'] > 13302048
    assert len(response_json) == 6


def test_handler_bad(file="tests/bad_response.json"):
    """
    bad_response.json has an invalid response;
    the block device mapping is missing.
    """
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    with pytest.raises(ValueError):
        handler.process_describe_instances(raw_json)


def test_create_cw_metric_happy(file="tests/response.json"):
    """
    response.json is an actual valid ec2 describe instance response.
    check the cw metric object creation
    """
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    response_json = handler.process_describe_instances(raw_json)
    result = handler.create_cw_metric_data(response_json)
    assert len(result) == 6
    assert result()['Namespace']== 'netbench'
    assert result()['MetricData'][5]['Unit'] == 'Seconds'
    assert result()['MetricData'][0]['MetricName'] == 'InstanceAge'
    assert result()['MetricData'][0]['Dimensions'][0]['Value'] == 'i-0da8d68c057b87a55'
