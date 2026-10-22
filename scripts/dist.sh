#!/bin/sh
# Builds release archives into dist/: one per target this host can build, named the way
# install.sh and install.ps1 look them up, plus SHA256SUMS.
#
#   scripts/dist.sh                 # every target this host builds (macOS: arm64, x86_64, universal)
#   scripts/dist.sh x86_64-unknown-linux-gnu
#
# Archive names: decision-loop-<target>.tar.gz (zip on Windows), holding the decision-loop binary, README.md and
# LICENSE. CI (.github/workflows/release.yml) runs this once per platform.
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
dist=dist
mkdir -p "$dist"

host_targets() {
  case "$(uname -s)" in
    Darwin) echo "aarch64-apple-darwin x86_64-apple-darwin" ;;
    Linux) echo "$(uname -m)-unknown-linux-gnu" ;;
    *) echo "x86_64-pc-windows-msvc" ;;
  esac
}

package() {
  target=$1
  binary=$2
  stage=$(mktemp -d)
  cp "$binary" "$stage/decision-loop$( [ "${target#*windows}" != "$target" ] && echo .exe || true)"
  cp README.md LICENSE "$stage/"
  case "$target" in
    *windows*) (cd "$stage" && zip -q -r - .) > "$dist/decision-loop-$target.zip" ;;
    *) tar -C "$stage" -czf "$dist/decision-loop-$target.tar.gz" . ;;
  esac
  rm -rf "$stage"
  echo "packaged decision-loop-$target"
}

targets=${*:-$(host_targets)}
for target in $targets; do
  cargo build --release --locked --target "$target"
  ext=""
  case "$target" in *windows*) ext=".exe" ;; esac
  package "$target" "target/$target/release/decision-loop$ext"
done

# One macOS download for both architectures.
if [ -f "target/aarch64-apple-darwin/release/decision-loop" ] && [ -f "target/x86_64-apple-darwin/release/decision-loop" ]; then
  universal=$(mktemp -d)/decision-loop
  lipo -create -output "$universal" \
    target/aarch64-apple-darwin/release/decision-loop target/x86_64-apple-darwin/release/decision-loop
  package universal-apple-darwin "$universal"
fi

(cd "$dist" && shasum -a 256 decision-loop-* > SHA256SUMS 2>/dev/null || sha256sum decision-loop-* > SHA256SUMS)
echo "decision-loop $version → $dist/"
ls -la "$dist"
