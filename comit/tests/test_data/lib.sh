container="" # Set by docker_run

docker_run() {
    if [ $# -eq 0 ]; then
        container=$(docker run -d coblox/bitcoin-core -regtest)
    else
        local temp_dir="$1"
        container=$(docker run -d -v $temp_dir:/root/.bitcoin coblox/bitcoin-core -regtest)
    fi

    echo "Create container $container ... sleeping"
    sleep 2
}

docker_stop() {
    docker stop $container > /dev/null
    echo "Stopped container $container"
}

docker_rm() {
    docker rm $container > /dev/null
    echo "Removed container $container"
}

docker_start() {
    docker start $container > /dev/null
    echo "Started container $container ... sleeping"
    sleep 2
}


generate_101_blocks() {
    docker exec $container bitcoin-cli -regtest generate 101 > /dev/null
    echo "Generated 101 blocks"
}

generate_block() {
    if [ $# -ne 1 ]; then
        echo "Incorrect number of parameters on generate_block"
        exit 2
    fi

    local path="$1"

    docker exec $container bitcoin-cli -regtest generate 1 > /dev/null
    blockhash=$(docker exec $container bitcoin-cli -regtest getbestblockhash)
    docker exec $container bitcoin-cli -regtest getblock $blockhash 0 > "$path"
    echo "Generated block $blockhash"
}

create_transaction() {
    if [ $# -ne 2 ]; then
        echo "Incorrect number of parameters on create_transaction"
        exit 2
    fi

    local address_path="$1"
    local transaction_path="$2"

    address=$(docker exec $container bitcoin-cli -regtest getnewaddress)
    echo ${address} > "$address_path"
    txid=$(docker exec $container bitcoin-cli -regtest sendtoaddress ${address} 1)
    docker exec $container bitcoin-cli -regtest getrawtransaction ${txid} 0 > "$transaction_path"

    echo "Created transaction $txid"
}
