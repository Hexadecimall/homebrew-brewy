# Brewy

Brewy is a search-first Homebrew TUI for macOS. Its sparse full-screen interface combines an OpenCode-style command surface with the fast multi-select package flow of Omarchy's package installer.

## Features

- Browse formulae and casks from the active Homebrew installation
- Installed, outdated, cask, and tap views
- Type-anywhere ranked fuzzy search and sortable package lists
- Inline package metadata preview with preview scrolling
- Stage several installs, removals, upgrades, and pin changes
- Run the current item or multi-selection directly with `Enter`
- Stream command output inline without interrupting the package list
- TokyoNight-inspired color palette and mouse-wheel navigation

## Install with Homebrew

The tap repository must be published as `Hexadecimall/homebrew-brewy`. Install the formula directly:

```sh
brew install Hexadecimall/brewy/brewy
```

Or add the tap first:

```sh
brew trust --formula Hexadecimall/brewy/brewy
brew tap Hexadecimall/brewy
brew install brewy
```

Homebrew 6.0 and newer requires explicit trust for non-official taps. The command above trusts only the Brewy formula, not every future item in the tap. Homebrew expands `Hexadecimall/brewy` to the GitHub repository named `Hexadecimall/homebrew-brewy`.

## Install from source

Rust 1.88 or newer and Homebrew are required:

```sh
curl -fsSL https://raw.githubusercontent.com/Hexadecimall/homebrew-brewy/main/install.sh | sh
```

The installer uses the Homebrew prefix by default. Set `BREWY_INSTALL_DIR` to install somewhere else or `BREWY_VERSION` to select another release tag.

## Build and run locally

```sh
cargo build --release
./target/release/brewy
```

Use `scripts/build-release.sh` for distributable binaries. It applies compiler path remapping and rejects binaries containing local build paths.

The `brew` executable must be available in `PATH`. Brewy never invokes `sudo`. `Enter` immediately runs the contextual action shown in the package information pane.

## Keyboard

Press `?` inside Brewy for the complete reference. Start typing to search, use `Tab` to multi-select packages, and press `Enter` to run the selected actions. Arrow keys navigate results, `Alt-P` toggles the package preview, and `Ctrl-Q` exits.
