# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `narf` command-line tool that rebuilds a NixOS flake via `nh` and commits
  every new generation with host, generation, NixOS and kernel version.
- Updating all flake inputs (`-u`) or selected ones (`-U`) before a rebuild.
- Pulling before (`-p`) and pushing after (`-P`) a rebuild, squashing unpushed
  rebuild commits unless `-S` is given.
- Dry run (`-d`), build without switching (`-b`), skipping unchanged flakes
  (`-n`), commit comments (`-c`), trace output (`-t`) and non-interactive mode
  (`-y`).
- Saved flake path in `~/.config/narf/flake`; the host defaults to the current
  hostname.
- `narf-tui` terminal frontend with an options form, repository status
  (upstream, unpushed commits, changes, inputs) and narf running in an
  embedded terminal.
- Nix flake with `narf`, `narf-tui` and `default` packages and an overlay for
  `x86_64-linux` and `aarch64-linux`; `narf` and `narf-tui` bring `nh` and
  `git` on their `PATH`; `nix` and `nixos-rebuild` come from the NixOS
  system.
- MIT license, declared in the Cargo manifests and in the `meta.license` of
  the Nix packages.

[Unreleased]: https://github.com/farbzen/narf.nix/commits/main
