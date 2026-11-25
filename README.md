# Frameo Linux

Frameo Linux is an open-source Linux application for sending photos to Frameo digital photo frames. The project is intended to provide both a command-line interface (CLI) and an optional GTK desktop application.

The aim is to make Frameo uploads available from **any Linux desktop**, whether you prefer a terminal, a graphical application, a script, or a file manager's context menu.

> **Project status: early development.** This repository currently contains the project starting point. The CLI, GTK application, Frameo pairing and transfer workflow, file-manager integrations and Flatpak package are planned; they are not yet implemented or ready for everyday use.

## Interfaces

### Command-line interface

The CLI is a first-class interface and should work independently of any desktop environment or GTK installation. It will be suitable for terminal use, shell scripts, automation and integration with file managers.

The intended command style is:

```bash
# Upload one or more photos
frameo upload photo.jpg another-photo.jpg

# Upload photos with a caption
frameo upload --caption "Holiday 2026" ~/Pictures/*.jpg

# Pair with a Frameo device
frameo pair
```

These examples illustrate the planned interface; exact command syntax will be documented when the CLI is implemented.

### GTK desktop application

The graphical application will use GTK4, with libadwaita where appropriate. It is intended for Linux distributions and desktop environments that provide a compatible GTK4 runtime. It is not intended to be limited to Linux Mint or Cinnamon.

The GUI and CLI will use the same shared upload and pairing core, so protocol behavior and configuration remain consistent.

### File-manager integration

File-manager context-menu integration is optional and separate from the core application.

- **Nemo:** a Nemo action will provide an "Upload to Frameo" right-click entry.
- **Other file managers:** users should be able to configure a custom action that invokes the CLI with the selected file paths. This can be supported without writing a dedicated plugin for every file manager.
- **Terminal and scripts:** invoke the CLI directly.

The Nemo action is specific to Nemo; it is not a limitation of the GTK application or CLI.

## Planned architecture

```text
       GTK desktop application       Terminal / scripts
                    |                       |
                    +-----------+-----------+
                                |
                         Shared Rust core
                                |
                      Frameo pairing / upload
                                |
                           Frameo device

       Optional integrations:
       Nemo action | Other file-manager custom actions
```

The shared core will handle pairing, configuration, upload requests and transfer behavior. The CLI should remain usable on headless systems and should not require GTK libraries.

For Flatpak installations, host file-manager actions will need a host-side integration that passes selected files to the sandbox using the document portal/file-forwarding mechanism. The CLI may also be installed natively for users who prefer direct shell integration.

## Frameo protocol

The project will evaluate existing open-source Frameo implementations, including [unframeo](https://github.com/borje/unframeo), to understand pairing, device discovery and transfer behavior. The initial implementation may use the existing command-line tool as a backend. A native Rust implementation can be considered after the protocol and its edge cases are understood.

No compatibility with every Frameo device or transfer method is claimed at this stage.

## Goals

- Rust implementation with a reusable, desktop-independent core.
- A CLI that does not depend on GTK or a running desktop session.
- GTK4 graphical application for GTK-capable Linux desktops.
- Shared pairing, configuration and upload behavior across interfaces.
- Multiple image selection, optional captions and upload progress.
- Secure storage of pairing credentials.
- Optional Nemo action and documentation for custom actions in other file managers.
- Flatpak packaging for the graphical application.
- Automated builds and tests through GitHub Actions.

## Development status

| Component | Status |
| --- | --- |
| Rust project foundation | Planned |
| Shared upload core | Planned |
| Command-line interface | Planned |
| GTK4 application | Planned |
| Frame pairing | Under investigation |
| Photo upload | Planned |
| Video upload | To be evaluated |
| Captions and upload queue | Planned |
| Nemo context-menu action | Planned |
| Generic file-manager CLI integration | Planned |
| Flatpak packaging | Planned |
| Flathub publication | Future goal |

## Intended platform

The project targets Linux. The CLI should work across Linux distributions without requiring a graphical environment. The GTK application targets desktops with GTK4 support, including GNOME and Cinnamon; compatibility with other GTK4-capable environments will be tested as development progresses.

Nemo integration is specifically for the Nemo file manager. Other file managers can use the CLI through their own custom-action mechanisms.

## Development

The build dependencies and exact Rust toolchain requirements will be documented as the project takes shape.

The intended development workflow will use Cargo:

```bash
cargo check
cargo test
cargo run
```

These commands will become usable once the initial application source and Cargo manifest have been added.

## Contributing

Issues, bug reports and contributions will be welcome as the project becomes functional. Please include your Linux distribution/version, desktop environment (if applicable), Frameo device model and relevant application logs when reporting a problem. Do not include pairing secrets or private credentials in bug reports.

## Disclaimer

Frameo is a trademark of its respective owner. This is an independent community project and is not affiliated with, endorsed by or supported by Frameo.

## License

The intended project license is **GNU General Public License v3.0 or later (GPL-3.0-or-later)**. A license file will be added before the first distributable release.
