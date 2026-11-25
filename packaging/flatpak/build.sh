#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
pkg="$root/packaging/flatpak"
stage="$pkg/stage"
upstream="$pkg/.unframeo"
revision=149f60b5ac7702f2cbb0761185b0d49a7431bf3f

if [[ ! -d "$upstream/.git" ]]; then
    git clone https://github.com/borje/unframeo.git "$upstream"
fi
git -C "$upstream" fetch origin "$revision"
git -C "$upstream" checkout --detach "$revision"

mkdir -p "$stage"
cargo build --release --manifest-path "$root/Cargo.toml"
install -m755 "$root/target/release/frameo" "$stage/frameo"
(
    cd "$upstream"
    CGO_ENABLED=0 go build -trimpath -o "$stage/unframeo" ./cmd/unframeo
)
install -m644 "$root/LICENSE" "$stage/LICENSE"
install -m644 "$root/NOTICE" "$stage/NOTICE"
install -m644 "$upstream/LICENSE" "$stage/unframeo-LICENSE"
install -m644 "$upstream/NOTICE" "$stage/unframeo-NOTICE"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive -y flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08

cd "$pkg"
flatpak-builder --user --force-clean --disable-rofiles-fuse --repo=repo --install build-dir io.github.stiffes.Framebridge.yml
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo repo "Framebridge-$(flatpak --default-arch).flatpak" io.github.stiffes.Framebridge

# Check the installed application, including its bundled backend.
flatpak run io.github.stiffes.Framebridge --version
flatpak run io.github.stiffes.Framebridge --help
flatpak run --command=sh io.github.stiffes.Framebridge -ec 'test -x /app/bin/unframeo; /app/bin/unframeo -h'
