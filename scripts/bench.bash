#!/usr/bin/env bash

set -x

BIN=./target/release/conc-map-bench
OUT=./results

cargo build --release
mkdir -p "$OUT"

function bench {
    ARGS="${@:2}"

    date

    file="$OUT/$1.csv"

    if [ -s "$file" ]; then
        ARGS+=" --csv-no-headers"
    fi

    skip=$(cat "$file" | cut -d, -f1 | uniq | paste -sd ' ' -)

    if ! "$BIN" bench -w $1 $ARGS --skip $skip --csv 2>>"$file"; then
        bench "$1" "$2" "$3"
    fi
}

bench ReadHeavy "$@"
bench Exchange "$@"
bench RapidGrow "$@"
