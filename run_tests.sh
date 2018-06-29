#!/bin/sh
set -ev;

END(){
    if test "${bitcoin_docker_id}${ganache_docker_id}"; then
        printf '\e[34m%s\e[0m\n' "Logs for container ganache: $ganache_docker_id:"
        docker logs "${ganache_docker_id}"
        echo "KILLING docker containers $bitcoin_docker_id $ganache_docker_id";
        docker rm -f $bitcoin_docker_id $ganache_docker_id;
    fi
}

trap 'END' EXIT;


export RUST_TEST_THREADS=1;
export BITCOIN_RPC_URL="http://localhost:18443"
export BITCOIN_RPC_USERNAME="bitcoin"
export BITCOIN_RPC_PASSWORD="54pLR_f7-G6is32LP-7nbhzZSbJs_2zSATtZV_r05yg="
export GANACHE_ENDPOINT="http://localhost:8545"
export ETHEREUM_NETWORK_ID=42

bitcoin_docker_id="$(sh .blockchain_nodes/bitcoind-regtest)";
ganache_docker_id="$(sh .blockchain_nodes/ganache)";

sleep_for=10
echo "sleeping for $sleep_for while bitcoind and ganache start";
sleep $sleep_for;

cargo test --all
