# fsw

[![Crates.io](https://img.shields.io/crates/v/fsw.svg?style=flat-square)](https://crates.io/crates/fsw)
[![Crates.io](https://img.shields.io/crates/d/fsw.svg?style=flat-square)](https://crates.io/crates/fsw)
[![Travis CI](https://img.shields.io/travis/longshorej/fsw.svg?style=flat-square)](https://travis-ci.org/longshorej/fsw)

fsw is a tool for recursively watching the current working directory and running a command when its contents change.

It's integrated with Git, so it won't rerun the command if an ignored file changes.

## Install

You can find static binaries for Linux and macOS on the [Github Releases](https://github.com/longshorej/fsw/releases) page.

Alternatively, you can use `cargo` to install the tool.

```bash
cargo install fsw
```

## Usage

```bash
fsw <command> [<arg>]...
```

## Changelog

### unreleased - YYYY-MM-DD

- Reduce binary file size (on Linux from ~5.3MB to ~625KB)
- Switch to Rust 2021 Edition

### 0.1.1 - 2019-03-26

* Rework design to not watch ignored directories, improving reliability and resource utilization
* Reduce debouncing period to 125ms
* Mark fsw output with "fsw:"
* Bump notify and transitive dependencies

### 0.1.0 - 2019-02-26

* Initial release.
