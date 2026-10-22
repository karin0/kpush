#!/bin/bash
set -e
cd "$(dirname "$0")"
if [ $# -eq 0 ]; then
	echo "usage: $0 HOST..." >&2
	exit 2
fi
set -a
. ./.env
set +a
cargo build --release
for host in "$@"; do
	rsync -pz --timeout=60 --rsync-path='sudo rsync' target/release/kpush "$host:/usr/local/bin/kpush"
done
