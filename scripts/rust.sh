#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
ACTION=${1:-build}
CONTAINER=${DAIW_BUILD_CONTAINER:-daiw-rust-build}
if command -v cargo >/dev/null 2>&1 && command -v cc >/dev/null 2>&1; then
  cargo "$@"
  exit
fi
if ! docker inspect "$CONTAINER" >/dev/null 2>&1; then
  docker run --name "$CONTAINER" -d -w /work rust:1.94-bookworm sleep infinity >/dev/null
fi
docker start "$CONTAINER" >/dev/null
docker cp Cargo.toml "$CONTAINER:/work/Cargo.toml"
docker cp crates "$CONTAINER:/work/"
if [ -f Cargo.lock ]; then docker cp Cargo.lock "$CONTAINER:/work/Cargo.lock"; fi
docker exec "$CONTAINER" cargo "$@"
docker cp "$CONTAINER:/work/Cargo.lock" ./Cargo.lock
if [ "$ACTION" = build ]; then
  mkdir -p target/debug
  docker cp "$CONTAINER:/work/target/debug/daiw-core" target/debug/daiw-core.next
  docker cp "$CONTAINER:/work/target/debug/daiw-render" target/debug/daiw-render.next
  mv target/debug/daiw-core.next target/debug/daiw-core
  mv target/debug/daiw-render.next target/debug/daiw-render
fi
