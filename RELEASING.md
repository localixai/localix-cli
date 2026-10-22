# Releasing

A release is a pushed tag `vX.Y.Z`; CI builds and publishes the rest.

## Steps

1. On an up-to-date `main` with green CI, bump the workspace version in `Cargo.toml`
   (`[workspace.package] version = "X.Y.Z"`) and run `cargo check --workspace` so `Cargo.lock`
   follows.
2. In `CHANGELOG.md`, rename `## Unreleased` to `## X.Y.Z` and start a new, empty
   `## Unreleased` above it. `scripts/release-notes.sh X.Y.Z` prints the section that becomes the
   release notes; it must not be empty.
3. Commit (`Release X.Y.Z`) and push to `main`.
4. Tag and push the tag:

   ```bash
   git tag -a vX.Y.Z -m "llx X.Y.Z"
   git push origin vX.Y.Z
   ```

A version with a suffix (`v1.0.0-rc.1`) is published as a prerelease, which `install.sh`'s
"latest" skips.

## What CI does

`.github/workflows/release.yml`, on the tag:

1. **check**: the tag must equal `v` + the workspace version, and `CHANGELOG.md` must have its
   section; otherwise nothing is built.
2. **build**, one job per platform, each running `scripts/dist.sh` from a clean state (no cache):
   - macOS (`macos-14`): `aarch64-apple-darwin`, `x86_64-apple-darwin`, and
     `universal-apple-darwin` joined with `lipo`;
   - Linux (`ubuntu-22.04`, for an older glibc): `x86_64-unknown-linux-gnu`, linked against
     WebKitGTK 4.1;
   - Windows (`windows-latest`): `x86_64-pc-windows-msvc`.

   Each archive (`llx-<target>.tar.gz`, `.zip` on Windows) holds the binary, `README.md` and
   `LICENSE`. The binary is run with `--version` before upload.
3. **publish**: joins every platform's `SHA256SUMS`, checks them against the archives, and runs
   `gh release create vX.Y.Z` with the archives, `SHA256SUMS`, `install.sh` and `install.ps1`,
   and the CHANGELOG section as the notes.

`scripts/install.sh` and `scripts/install.ps1` read `releases/latest` (or `LLX_VERSION`) and
verify the archive against `SHA256SUMS`, so a release is installable as soon as it is published.

## Locally

`scripts/dist.sh` builds the same archives for the host (on a Mac: both architectures and the
universal one) into `dist/`. If a release went wrong, delete the GitHub release and the tag
(`git push --delete origin vX.Y.Z`), fix, and tag again; never reuse a version that was
installable.
