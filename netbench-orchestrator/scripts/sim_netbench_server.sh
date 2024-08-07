#/usr/bin/env bash

# Test program to simulate a netbench server run.
#
# Continue to echo an incrementing counter value until the
# process is killed.

[[ -z "$1" ]] && { echo "Please specify an 'id'" ; exit 1; }

id=$1


ctr=1
cd target
mkdir -p test_output
cd test_output

touch $id
echo "--------" >> $id

    while :
    do
        echo $ctr >> $id
        sleep 1
        ctr=$((ctr+1))
    done

