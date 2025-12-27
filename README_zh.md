# modelscope-ng

[中文](README_zh.md) | [English](README.md)

![Release](https://github.com/Liangdi/modelscope/actions/workflows/publish.yml/badge.svg)
![Crates.io](https://img.shields.io/crates/d/modelscope-ng)

用于从Modelscope下载模型的CLI工具。

本项目是 [xgpxg/modelscope](https://github.com/xgpxg/modelscope) 的一个 fork。

功能：

- ⬇️ 从Modelscope下载模型
- 🫏 显示进度条
- ⚡ 多线程下载
- 🔗 断点续传

支持的操作系统：

- Windows
- macOS
- Linux

## 安装

可以通过以下方式之一安装：

- 使用Cargo安装

```shell
cargo install modelscope-ng
```

- 使用预编译的包
  从 [发布页面](https://github.com/Liangdi/modelscope/releases) 下载适合你的操作系统的包，然后解压。

## 使用方式：

```shell
modelscope-ng download -m <MODEL_ID> -s <SAVE_DIR>
```

![img.png](screenshot.png)

## 命令：

```shell
Usage: modelscope-ng <COMMAND>

Commands:
  download      Download model
  download-file Download a single file from a model
  login         Login to modelscope use your token
  logout        Logout
  list          List all local models
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### 下载单个文件

你可以使用 `download-file` 命令从模型中下载单个文件：

```shell
modelscope-ng download-file -m <MODEL_ID> -f <FILE_PATH> -s <SAVE_DIR>
```

示例：
```shell
modelscope-ng download-file -m Qwen/Qwen3-0.6B -f config.json -s ./data
```

## 在lib中使用

添加依赖：

```shell
cargo add modelscope-ng
```

示例：

```rust
use modelscope_ng::ModelScope;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let save_dir = "./data";
    ModelScope::download(model_id, save_dir).await?;

    Ok(())
}
```

## 使用回调函数

本库提供了回调机制来跟踪下载进度。你可以实现 `ProgressCallback` trait 来自定义进度报告方式。

### ProgressCallback Trait

```rust
#[async_trait]
pub trait ProgressCallback: Send + Sync {
    /// 当文件下载开始时调用
    async fn on_file_start(&self, file_name: &str, file_size: u64);
    
    /// 当文件下载进度更新时调用
    async fn on_file_progress(&self, file_name: &str, downloaded: u64, total: u64);
    
    /// 当文件下载完成时调用
    async fn on_file_complete(&self, file_name: &str);
    
    /// 当文件下载失败时调用
    async fn on_file_error(&self, file_name: &str, error: &str);
}
```

### 内置回调实现

#### 1. ProgressBarCallback（默认）

为每个正在下载的文件显示进度条：

```rust
use modelscope_ng::{ModelScope, ProgressBarCallback};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let save_dir = "./data";
    let callback = ProgressBarCallback::new();
    
    ModelScope::download_with_callback(model_id, save_dir, callback).await?;
    
    Ok(())
}
```

#### 2. SimpleCallback

将进度信息打印到控制台：

```rust
use modelscope_ng::{ModelScope, SimpleCallback};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let save_dir = "./data";
    
    ModelScope::download_with_callback(model_id, save_dir, SimpleCallback).await?;
    
    Ok(())
}
```

### 自定义回调实现

你可以创建自己的回调实现：

```rust
use async_trait::async_trait;
use modelscope_ng::{ModelScope, ProgressCallback};

#[derive(Clone)]
struct CustomCallback;

#[async_trait]
impl ProgressCallback for CustomCallback {
    async fn on_file_start(&self, file_name: &str, file_size: u64) {
        println!("开始下载: {} ({} bytes)", file_name, file_size);
    }

    async fn on_file_progress(&self, file_name: &str, downloaded: u64, total: u64) {
        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0) as u32
        } else {
            0
        };
        println!("进度: {} - {}%", file_name, percent);
    }

    async fn on_file_complete(&self, file_name: &str) {
        println!("完成: {}", file_name);
    }

    async fn on_file_error(&self, file_name: &str, error: &str) {
        eprintln!("下载错误 {}: {}", file_name, error);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let save_dir = "./data";
    let callback = CustomCallback;
    
    ModelScope::download_with_callback(model_id, save_dir, callback).await?;
    
    Ok(())
}
```

### 使用回调下载单个文件

你也可以在下载单个文件时使用回调：

```rust
use modelscope_ng::{ModelScope, SimpleCallback};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_id = "Qwen/Qwen3-0.6B";
    let file_path = "config.json";
    let save_dir = "./data";
    
    ModelScope::download_single_file_with_callback(
        model_id,
        file_path,
        save_dir,
        SimpleCallback
    ).await?;
    
    Ok(())
}
```