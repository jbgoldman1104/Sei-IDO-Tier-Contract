#!/bin/bash

#Build Flag

NETWORK=testnet

export GOPATH=$HOME/go
export PATH=$PATH:$GOROOT/bin:$GOPATH/bin

OWNER="lucas"
RETURN=""



TOKEN="HEART"

ADDR_OWNER=$(seid keys show $OWNER -a --keyring-backend test)
VALIDATOR="seivaloper183xtf2wmcah9fh5kpdr47wlspv3w47c0p8aqde"

echo "OWNER = $ADDR_OWNER" 
WALLET="--from $OWNER"
GAS=0.001

echo "seid keys show $OWNER -a --keyring-backend test"


# OUTPUT="/root/Sei-IDO-Tier-Contract/command.txt"
echo "" > /root/Sei-IDO-Tier-Contract/command.txt

case $NETWORK in
    localnet)
        NODE="http://localhost:26657"
        DENOM="usei"
        CHAIN_ID="testchain-1"
        ;;
    testnet)
        NODE="https://rpc.atlantic-2.seinetwork.io:443"
        DENOM="usei"
        CHAIN_ID="atlantic-2"
        ;;
    mainnet)
        NODE="https://terra-classic-rpc.publicnode.com:443"
        DENOM=uluna
        CHAIN_ID=columbus-5
        ;; 
esac

NODECHAIN="--node $NODE --chain-id $CHAIN_ID"
#TXFLAG="$NODECHAIN --gas=250000 --fees=250000usei --broadcast-mode block --keyring-backend test -y"
TXFLAG="$NODECHAIN --gas=250000 --fees=250000usei --broadcast-mode block --keyring-backend test -y"

CreateEnv() {
    apt update
    apt upgrade
    apt install curl
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

    sudo apt-get update && sudo apt upgrade -y
    sudo apt-get install make build-essential gcc git jq chrony -y
    wget https://golang.org/dl/go1.18.1.linux-amd64.tar.gz
    sudo tar -C /usr/local -xzf go1.18.1.linux-amd64.tar.gz
    rm -rf go1.18.1.linux-amd64.tar.gz

    export GOROOT=/usr/local/go
    export GOPATH=$HOME/go
    export GO111MODULE=on
    export PATH=$PATH:/usr/local/go/bin:$HOME/go/bin
        
    rustup default stable
    rustup target add wasm32-unknown-unknown

    git clone https://github.com/classic-terra/core/
    cd core
    git fetch
    git checkout main
    make install
    cd ../
    rm -rf core
}

Execute() {
    CMD=$1
    echo "**************BEGIN**********************" >> /root/Sei-IDO-Tier-Contract/command.txt
    echo $CMD >> /root/Sei-IDO-Tier-Contract/command.txt
    echo "------------------------------------------" >> /root/Sei-IDO-Tier-Contract/command.txt
    
    if  [[ $CMD == cd* ]] ; then
        $CMD > ~/out.log    
        RETURN=$(cat ~/out.log)
    else
        RETURN=$(eval $CMD)
    fi

    

    echo $RETURN >> /root/Sei-IDO-Tier-Contract/command.txt
    echo "*************END*************************" >> /root/Sei-IDO-Tier-Contract/command.txt
}

RustBuild() {
    CATEGORY=$1

    echo "================================================="
    echo "Rust Optimize Build Start for $CATEGORY"
    
    Execute "cd $CATEGORY"
    Execute "pwd"
    rm -rf target
    
    Execute "RUSTFLAGS='-C link-arg=-s' cargo wasm"
    Execute "cp ./target/wasm32-unknown-unknown/release/$CATEGORY.wasm ../release/"
    Execute "cd .."
}


Upload() {
    CATEGORY=$1
    echo "================================================="
    echo "Upload Wasm for $CATEGORY"
    Execute "seid tx wasm store release/$CATEGORY".wasm" $WALLET $NODECHAIN --gas=2500000 --fees=2500000usei --broadcast-mode block --keyring-backend test -y --output json | jq -r '.txhash'"
    UPLOADTX=$RETURN

    echo "Upload txHash: "$UPLOADTX
    echo "================================================="
    echo "GetCode"

    CODE_ID=""
    while [[ $CODE_ID == "" ]]
    do 
        sleep 3
        Execute "seid query tx $UPLOADTX $NODECHAIN --output json | jq -r '.logs[0].events[-1].attributes[1].value'"
        CODE_ID=$RETURN
    done

    echo "$CATEGORY Contract Code_id: "$CODE_ID
    echo $CODE_ID > data/code_$CATEGORY
}

InstantiateTier() {
    CATEGORY='tier'
    echo "================================================="
    echo "Instantiate Contract "$CATEGORY
    #read from FILE_CODE_ID
    
    CODE_ID=$(cat data/code_$CATEGORY)

    echo "Code id: " $CODE_ID

    Execute "seid tx wasm instantiate $CODE_ID '{\"validator\":\"'$VALIDATOR'\", \"admin\":\"'$ADDR_OWNER'\", \"deposits\":[\"300\",\"100\",\"50\",\"10\",\"1\"]}' --admin $ADDR_OWNER $WALLET $TXFLAG --label \"TierContract\" --output json | jq -r '.txhash'"
    TXHASH=$RETURN

    echo "Transaction hash = $TXHASH"
    CONTRACT_ADDR=""
    while [[ $CONTRACT_ADDR == "" ]]
    do
        sleep 3
        Execute "seid query tx $TXHASH $NODECHAIN --output json | jq -r '.logs[0].events[0].attributes[0].value'"
        CONTRACT_ADDR=$RETURN
    done
    echo "Contract Address: " $CONTRACT_ADDR
    echo $CONTRACT_ADDR > data/contract_$CATEGORY
}


#################################################################################
PrintWalletBalance() {
    echo "native balance"
    echo "========================================="
    seid query bank balances $ADDR_OWNER $NODECHAIN
    echo "========================================="
    # echo "CW20 Token balance"
    # echo "========================================="
    # seid query wasm contract-state smart "$(cat data/contract_cw20_base)" '{"balance":{"address":"'$ADDR_OWNER'"}}' $NODECHAIN
    # echo "========================================="
}

TokenTransfer() {
    echo "================================================="
    echo "Token Transfer"
    PARAM_1='{"transfer": {"recipient": "terra1gwzndny4e4xf7evm5kjva73fqedux5gfwdr0ta", "amount": "'$1'000000"}}'
    printf "y\n" | seid tx wasm execute "$(cat data/contract_cw20_base)" "$PARAM_1" $WALLET $TXFLAG
    sleep 5
    echo "End"
}

CreateNewToken() {
    echo "================================================="
    echo "Create New Token"
    # CODE_ID=$(cat data/code_$CATEGORY)
    PAYLOAD='{"deposit": {"instantiate": {"decimals": 6,"initial_balances": [{"address": "terra1gwzndny4e4xf7evm5kjva73fqedux5gfwdr0ta","amount": "123000000"}],"marketing": {"logo": {"url": ""},"marketing": "terra1cajleyhmtf8lua47m8ctnduksr7qtd8at3aezppk89pt5qnwvcgsl9jrdc"},"name": "asd","symbol": "ads"}}}'
    printf "y\n" | seid tx wasm execute terra1vuundfe5ck4fsgqyttyt7098k0cafegjxj59yvdzehn0y7gsv28q3xa7pt "$PAYLOAD"  --amount 123000000uluna $WALLET $TXFLAG
    sleep 5
    echo "End"
}


DeployTier() {
    CATEGORY=tier
    RustBuild $CATEGORY
    Upload $CATEGORY
    InstantiateTier
} 

DeployCW20Stake() {
    CATEGORY=cw20_stake
    RustBuild $CATEGORY
    Upload $CATEGORY
    InstantiateStake
} 

DeployCW20TokenFactory() {
    CATEGORY=cw20_token_factory
    # RustBuild $CATEGORY
    Upload $CATEGORY
    InstantiateTokenFactory
    
} 

Optimize() {
    CATEGORY=$1
    
    Execute "docker run --rm -v ./$CATEGORY   --mount type=volume,source=/root/cache/$CATEGORY,target=/target/$CATEGORY   --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry   cosmwasm/rust-optimizer:0.12.11"

    
}

# InstantiateCW20

# DeployCW20Base
# DeployCW20Stake
# DeployCW20TokenFactory
# TokenTransfer $1

# PrintWalletBalance

# RustBuild tier
# Optimize tier
# Upload tier
InstantiateTier
# DeployTier

#CreateNewToken