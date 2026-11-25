# Framebridge Flatpak (development)

This experimental CLI package bundles the current Rust executable and a pinned revision of the temporary GPL-licensed unframeo backend. It does not contain the planned GTK4 interface.

## Build

On Ubuntu 24.04 (or another compatible build host), install Git, Flatpak, flatpak-builder, Rust and Go 1.26 or newer. Run from the repository root:

```sh
bash packaging/flatpak/build.sh
```

The script builds the two executables on the host, packages them with the Freedesktop 25.08 runtime, installs the app for smoke testing and creates `packaging/flatpak/Framebridge-<architecture>.flatpak`. Use a build host whose glibc is no newer than the Flatpak runtime. CI uses Ubuntu 24.04.

## Install a CI bundle

Download the build artifact from GitHub Actions, extract the ZIP, and install the bundle:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub org.freedesktop.Platform//25.08
flatpak install --user ./Framebridge-x86_64.flatpak
flatpak run io.github.stiffes.Framebridge --help
```

## Use

```sh
flatpak run io.github.stiffes.Framebridge pair 12345678
flatpak run io.github.stiffes.Framebridge frames
flatpak run io.github.stiffes.Framebridge discover
flatpak run io.github.stiffes.Framebridge upload "$HOME/Pictures/photo.jpg"
```

The sandbox has network access and read-only access to the XDG Pictures directory. To upload from another directory, grant read-only access explicitly for that invocation:

```sh
flatpak run --filesystem="$HOME/Downloads:ro" io.github.stiffes.Framebridge upload "$HOME/Downloads/photo.jpg"
```

The pairing identity is stored separately from the host installation, under the Flatpak app's private config directory. Protect it and back it up. Do not share pairing keys in bug reports.

## Limits

This is a development bundle, not a Flathub submission or a GTK desktop application. The host-built Rust executable is tested inside the runtime by CI; a real Frameo device is required to validate pairing and uploads. The native Rust protocol remains incomplete.
