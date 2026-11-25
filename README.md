# Frameo Linux

A native Linux desktop application for sending photos (and, where supported by the Frameo device and transfer backend, videos) to Frameo digital photo frames.

The goal is to make sending pictures from Linux Mint Cinnamon straightforward in two ways:

- **Standalone GTK application:** open Frameo Linux from the desktop menu, select images, add a caption and upload them.
- **Nemo integration:** select one or more files in the Nemo file manager, right-click and choose **Upload to Frameo**.

> **Project status: early development.** This repository currently contains the project starting point. The GTK application, Frameo pairing and transfer workflow, Nemo action and Flatpak package are planned; they should not yet be considered implemented or ready for everyday use.

## Goals

- Native Linux desktop experience, written in Rust.
- GTK4 user interface, with libadwaita where appropriate.
- One shared upload implementation used by both the GUI and Nemo entry point.
- Multiple image selection, optional captions and visible upload progress.
- Frame pairing and secure storage of pairing credentials.
- Nemo right-click integration for Linux Mint Cinnamon.
- Flatpak packaging, with a path towards Flathub distribution.
- Automated builds and tests through GitHub Actions.

## Planned architecture

```text
GTK desktop application          Nemo right-click action
          |                                |
          +---------------+----------------+
                          |
                   Shared Rust core
                          |
              Frameo pairing and transfer
                          |
                 Local network / relay
                          |
                    Frameo frame
```

The Nemo integration will be installed on the host because Flatpak applications cannot directly install files into the host Nemo configuration. The Flatpak will receive selected files through the document portal/file-forwarding mechanism.

## Frameo protocol

The project will first evaluate existing open-source Frameo implementations, including [unframeo](https://github.com/borje/unframeo), to understand pairing, device discovery and transfer behavior. The initial implementation may call the existing command-line tool from the Rust application. A native Rust protocol implementation can be considered after the protocol and its edge cases are understood.

No compatibility with every Frameo device or transfer method is claimed at this stage.

## Development status

| Component | Status |
| --- | --- |
| Rust project foundation | Planned |
| GTK4 / libadwaita interface | Planned |
| Frame pairing | Under investigation |
| Photo upload | Planned |
| Video upload | To be evaluated |
| Captions and upload queue | Planned |
| Nemo context-menu action | Planned |
| Flatpak packaging | Planned |
| Flathub publication | Future goal |

## Intended platform

The initial target is **Linux Mint Cinnamon**, using GTK4 and the Nemo file manager. Other GTK-based Linux desktops may be supported later.

## Development

The build dependencies and exact Rust toolchain requirements will be documented here as the project takes shape.

The intended development workflow will use Cargo:

```bash
cargo check
cargo test
cargo run
```

These commands will become usable once the initial application source and Cargo manifest have been added.

## Contributing

Issues, bug reports and contributions will be welcome as the project becomes functional. Please include your Linux distribution/version, desktop environment, Frameo device model and relevant application logs when reporting a problem. Do not include pairing secrets or private credentials in bug reports.

## Disclaimer

Frameo is a trademark of its respective owner. This is an independent community project and is not affiliated with, endorsed by or supported by Frameo.

## License

The intended project license is **GNU General Public License v3.0 or later (GPL-3.0-or-later)**. A license file will be added before the first distributable release.
