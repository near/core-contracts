FROM rust:1.51.0

LABEL description="Container for builds"

RUN rustup default 1.51.0
RUN rustup target add wasm32-unknown-unknown

RUN apt-get update && apt-get install -y git less vim
RUN apt-get install -y binutils-dev libcurl4-openssl-dev zlib1g-dev libdw-dev libiberty-dev cmake gcc g++ python protobuf-compiler libssl-dev pkg-config clang llvm
