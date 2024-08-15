#!/bin/bash
while inotifywait -e modify *.py tests/*.py; do
    echo "Running the tests"
    make test
    echo "Done"
done
