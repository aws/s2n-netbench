import pytest
import handler
import json
from datetime import datetime, timedelta, timezone

def test_MAX_LIFETIME():
    assert handler.MAX_LIFETIME

def test_get_ebs_age_happy():
    today = datetime.now(tz=timezone.utc) - timedelta(days=1)
    assert handler.get_ebs_age(today) == 86400

def test_get_ebs_age_future():
    today = datetime.now(tz=timezone.utc) + timedelta(days=1)
    with pytest.raises(TypeError):
        handler.get_ebs_age(today)

def test_handler_happy(file="tests/response.json"):
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    response_json = handler.process_describe_instances(raw_json)
    assert response_json['overall_alarm'] == True
    assert len(response_json['instances_above_max']) == 6
    assert response_json['alarm_threshold'] ==  handler.MAX_LIFETIME

def test_handler_bad(file="tests/bad_response.json"):
    with open(file, "rb") as fh:
        raw_json = json.load(fh)
    response_json = handler.process_describe_instances(raw_json)
    assert response_json['overall_alarm'] == True
    assert len(response_json['instances_above_max']) == 4
    assert response_json['alarm_threshold'] ==  handler.MAX_LIFETIME