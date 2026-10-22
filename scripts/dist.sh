#!/bin/sh
# Builds release archives into dist/: one per target this host can build, named the way
# install.sh and install.ps1 look them up, plus SHA256SUMS.
#
#   scripts/dist.sh                 # every target this host builds (macOS: arm64, x86_64, universal)
#   scripts/dist.sh x86_64-unknown-linux-gnu
#
# Archive names: lxsurf-<target>.tar.gz (zip on Windows), holding the lxsurf binary, README.md and
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
  cp "$binary" "$stage/lxsurf$( [ "${target#*windows}" != "$target" ] && echo .exe || true)"
  cp README.md LICENSE "$stage/"
  case "$target" in
    *windows*) (cd "$stage" && zip -q -r - .) > "$dist/lxsurf-$target.zip" ;;
    *) tar -C "$stage" -czf "$dist/lxsurf-$target.tar.gz" . ;;
  esac
  rm -rf "$stage"
  echo "packaged lxsurf-$target"
}

targets=${*:-$(host_targets)}
for target in $targets; do
  cargo build --release --locked --target "$target"
  ext=""
  case "$target" in *windows*) ext=".exe" ;; esac
  package "$target" "target/$target/release/lxsurf$ext"
done

# One macOS download for both architectures.
if [ -f "target/aarch64-apple-darwin/release/lxsurf" ] && [ -f "target/x86_64-apple-darwin/release/lxsurf" ]; then
  universal=$(mktemp -d)/lxsurf
  lipo -create -output "$universal" \
    target/aarch64-apple-darwin/release/lxsurf target/x86_64-apple-darwin/release/lxsurf
  package universal-apple-darwin "$universal"
fi

(cd "$dist" && shasum -a 256 lxsurf-* > SHA256SUMS 2>/dev/null || sha256sum lxsurf-* > SHA256SUMS)
echo "lxsurf $version → $dist/"
ls -la "$dist"
