#!/bin/bash
# build one of the contract in docker
# Usage: ./build-in-docker.sh <contract-folder> 
#   e.g. ./build-in-docker.sh lockup

set -e

docker build . -t near-core-contracts-builder
srcdir=$(pwd)
workdir=$srcdir/$1
docker run -v $srcdir:$srcdir -w $workdir near-core-contracts-builder bash build.sh
