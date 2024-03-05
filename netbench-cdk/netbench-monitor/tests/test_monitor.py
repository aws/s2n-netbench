# Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0

import pytest
import handler
import json
from datetime import datetime, timedelta, timezone

def test_MAX_LIFETIME():
    assert handler.MAX_LIFETIME

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
    assert response_json['overall_alarm'] == True
    assert len(response_json['instances_above_max']) == 6
    assert response_json['alarm_threshold'] ==  handler.MAX_LIFETIME

def test_handler_bad(file="tests/bad_response.json"):
    """
    bad_response.json has an invalid response;
    the block device mapping is missing.
    """
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    with pytest.raises(ValueError):
        handler.process_describe_instances(raw_json)