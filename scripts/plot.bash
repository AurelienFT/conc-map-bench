#!/usr/bin/env bash

set -e

BIN=./target/release/conc-map-bench
DATA_DIR=results

cargo build --release

function plot {
    cat "$DATA_DIR/$1.csv" | "$BIN" plot "$DATA_DIR" "$1"
}

plot ReadHeavy
plot Exchange
plot RapidGrow
