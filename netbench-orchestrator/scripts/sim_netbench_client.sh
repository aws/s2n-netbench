[[ -z "$1" ]] && { echo "Please specify an 'id'" ; exit 1; }

id=$1


ctr=1
cd target
mkdir -p test_output
cd test_output

touch $id
echo "--------" >> $id

    while [ $ctr -le 4 ]
    do
        echo "c $ctr" >> "$id"
        sleep 1
        ctr=$((ctr+1))
    done

