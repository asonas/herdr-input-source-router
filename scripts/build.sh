#!/bin/sh

set -eu

cargo build --release --locked
mkdir -p bin
cp target/release/herdr-input-source-router bin/herdr-input-source-router
