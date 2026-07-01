#!/bin/bash
# Changing this version will change the version of solana-verify that is installed in the docker image and in our CI pipeline
SOLANA_VERIFY=v0.5.0

cargo install solana-verify --git https://github.com/solana-foundation/solana-verifiable-build --tag $SOLANA_VERIFY
