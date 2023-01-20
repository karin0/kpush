#!/bin/bash
set -e
remote="${1:-cloud}"
cargo build --release
exec rsync -avz --progress target/release/kpush "$remote:~/bin/"
