#!/usr/bin/env bash
# Build a native Linux lab bundle from this checkout, including dirty integration code.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:/usr/local/go/bin:$PATH"
version=${1:-0.1.0-lab.$(date -u +%Y%m%d%H%M%S)}
[[ $version =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,79}$ ]] || { echo 'Unsafe version' >&2; exit 1; }
[[ $(uname -s) == Linux ]] || { echo 'Native Linux build required' >&2; exit 1; }
case $(uname -m) in x86_64) arch=amd64;; aarch64) arch=arm64;; *) echo 'Unsupported architecture' >&2; exit 1;; esac
for tool in cargo go npm python3 tar sha256sum readelf ldd; do command -v "$tool" >/dev/null; done
out="$repo/artifacts/releases"
name="donkeywork-desktop-$version-linux-$arch"
mkdir -p "$out"
[[ ! -e $out/$name.tar.gz ]] || { echo 'Release exists; use a new version' >&2; exit 1; }
stage=$(mktemp -d "$out/.build-$version.XXXXXX")
payload="$stage/$name"
mkdir -p "$payload"/{bin,libexec,share/web,share/systemd,share/doc,share/build,packaging}
printf 'Build staging (retained on failure): %s\n' "$stage"
cargo build --locked --release --manifest-path "$repo/device/console/Cargo.toml"
(cd "$repo/console-web" && go build -mod=readonly -trimpath -o "$payload/bin/console-web" .)
npm --prefix "$repo/console-ui" ci
npm --prefix "$repo/console-ui" run build
for binary in dwconsole-daemon dwconsole input-daemon input-cli web-launch socket-broker; do
  install -m 0755 "$repo/device/console/target/release/$binary" "$payload/bin/$binary"
done
for script in package-runner.py session-supervisor.py start-console-x11.sh x11-session-supervisor.py x11_support.py x11-target-guard.py; do
  install -m 0755 "$repo/deploy/package/$script" "$payload/libexec/$script"
done
for script in keep-console-output.py input-target-guard.py; do
  install -m 0755 "$repo/deploy/vkms/$script" "$payload/libexec/$script"
done
cp -a "$repo/console-ui/dist/." "$payload/share/web/"
cp "$repo"/deploy/package/systemd/* "$payload/share/systemd/"
cp "$repo"/packaging/{install.sh,doctor.sh,config.example,README.md} "$payload/packaging/"
chmod 0755 "$payload/packaging/"*.sh
cp "$repo/PROVENANCE.md" "$payload/share/doc/"
printf '%s\n' "$version" > "$payload/VERSION"
printf '%s\n' "$arch" > "$payload/ARCH"
git -C "$repo" rev-parse HEAD > "$payload/share/build/base-commit.txt"
git -C "$repo" status --porcelain > "$payload/share/build/checkout-status.txt"
cp /etc/os-release "$payload/share/build/build-os-release"
{ cargo --version; rustc --version; go version; node --version; npm --version; } > "$payload/share/build/toolchains.txt"
cp "$repo/device/console/Cargo.lock" "$payload/share/build/Cargo.lock"
cp "$repo/console-web/go.sum" "$payload/share/build/go.sum"
cp "$repo/console-ui/package-lock.json" "$payload/share/build/package-lock.json"
cargo metadata --locked --format-version 1 --manifest-path "$repo/device/console/Cargo.toml" > "$payload/share/build/cargo-metadata.json"
(cd "$repo/console-web" && go list -mod=readonly -m -json all) > "$payload/share/build/go-modules.json"
# Preserve exact original integration sources, not merely the last Git commit.
tar -czf "$payload/share/build/project-source.tar.gz" --exclude=node_modules --exclude=target --exclude=dist --exclude=__pycache__ \
  -C "$repo" device/console console-web console-ui deploy/package deploy/vkms packaging PROVENANCE.md
for binary in "$payload"/bin/*; do
  readelf -h "$binary" > "$payload/share/build/${binary##*/}.elf.txt"
  ldd "$binary" > "$payload/share/build/${binary##*/}.dependencies.txt" 2>&1 || {
    grep -Eq 'not a dynamic executable|statically linked' "$payload/share/build/${binary##*/}.dependencies.txt"
  }
  if grep -q 'not found' "$payload/share/build/${binary##*/}.dependencies.txt"; then echo "Missing library: $binary" >&2; exit 1; fi
done
printf '%s\n' 'Internal lab alpha; no project outbound licence selected. Not a public redistribution release.' \
  'Native build, not a universal Linux binary: inspect share/build dependency and OS records.' \
  'FFmpeg/x264 are NOT bundled. Install host-provided compatible FFmpeg with libx264.' \
  'Dependency inventories are build provenance, not a completed third-party licence/source compliance bundle.' \
  'No authentication: restrict HTTP 8090 and WebRTC UDP 8091 to a trusted lab network or VPN.' > "$payload/share/doc/LAB-RELEASE.txt"
(cd "$payload" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)
tar -czf "$out/$name.tar.gz" -C "$stage" "$name"
(cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
printf 'Bundle: %s\nExtracted payload: %s\n' "$out/$name.tar.gz" "$payload"
