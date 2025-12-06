
# modelscope

[中文](README_zh.md) | [English](README.md)

![Release](https://github.com/xgpxg/modelscope/actions/workflows/publish.yml/badge.svg)
![Crates.io](https://img.shields.io/crates/d/modelscope)

A CLI tool for downloading models from Modelscope.

Features:

- ⬇️ Download models from Modelscope
- 🫏 Show progress bar
- ⚡ Multi-threaded download
- 🔗 Resume interrupted downloads

Supported OS:

- Windows
- macOS
- Linux

## Installation

You can install it in one of the following ways:

- Install using Cargo

```shell
cargo install modelscope
```

- Use precompiled package
  Download the binary package for your operating system from
  the [release page](https://github.com/xgpxg/modelscope/releases) and extract it.

## Usage

```shell
modelscope download -m <MODEL_ID> -s <SAVE_DIR>
```

![img.png](screenshot.png)

## Commands

```shell
Usage: modelscope <COMMAND>

Commands:
  download  Download model
  login     Login to modelscope use your token
  logout    Logout
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print versio
```

## Library

Add crate:

```shell
cargo add modelscope
```

Example:

```rust
use modelscope::ModelScope;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let save_dir = "./data";
    ModelScope::download(model_id, save_dir).await?;

    Ok(())
}
```