#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
out="$repo/artifacts/agent/0.1.0-preview6"
mkdir -p "$out"
for arch in amd64 arm64; do
 (cd "$repo/manager" && CGO_ENABLED=0 GOOS=linux GOARCH="$arch" go build -trimpath -o "$out/dwdesktop-agent-$arch" ./cmd/device)
done
install -m 0755 "$repo/packaging/agent/install.sh" "$out/install.sh"
install -m 0644 "$repo/packaging/agent/dwdesktop-agent.service" "$out/dwdesktop-agent.service"
(cd "$out" && sha256sum dwdesktop-agent-amd64 dwdesktop-agent-arm64 install.sh dwdesktop-agent.service > SHA256SUMS)
tar -C "$out" -czf "$repo/artifacts/agent/dwdesktop-agent-0.1.0-preview6.tar.gz" dwdesktop-agent-amd64 dwdesktop-agent-arm64 install.sh dwdesktop-agent.service SHA256SUMS
echo "Built $repo/artifacts/agent/dwdesktop-agent-0.1.0-preview6.tar.gz"
