// 回调示例 - 使用自定义回调来跟踪下载进度
use async_trait::async_trait;
use modelscope::{ModelScope, ProgressCallback};

/// 自定义回调实现 - 将进度信息保存到结构体中
#[derive(Clone)]
struct CustomCallback;

#[async_trait]
impl ProgressCallback for CustomCallback {
    async fn on_file_start(&self, file_name: &str, file_size: u64) {
        println!("[开始] 文件: {} (大小: {} bytes)", file_name, file_size);
    }

    async fn on_file_progress(&self, file_name: &str, downloaded: u64, total: u64) {
        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0) as u32
        } else {
            0
        };
        // 只在进度变化较大时打印，避免输出过多
        if percent % 10 == 0 || downloaded == total {
            println!("[进度] {} - {}% ({} / {} bytes)", file_name, percent, downloaded, total);
        }
    }

    async fn on_file_complete(&self, file_name: &str) {
        println!("[完成] {}", file_name);
    }

    async fn on_file_error(&self, file_name: &str, error: &str) {
        eprintln!("[错误] {} - {}", file_name, error);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 示例 1: 使用内置的 SimpleCallback（简单打印进度）
    println!("=== 示例 1: 使用 SimpleCallback ===");
    ModelScope::download_with_callback(
        "damo/nlp_structbert_backbone_base_std",
        "./models",
        modelscope::SimpleCallback,
    )
    .await?;

    // 示例 2: 使用自定义回调
    println!("\n=== 示例 2: 使用自定义回调 ===");
    let callback = CustomCallback;
    ModelScope::download_with_callback(
        "damo/nlp_structbert_backbone_base_std",
        "./models_custom",
        callback,
    )
    .await?;

    // 示例 3: 使用进度条回调（默认行为）
    println!("\n=== 示例 3: 使用进度条回调（默认） ===");
    ModelScope::download(
        "damo/nlp_structbert_backbone_base_std",
        "./models_progress",
    )
    .await?;

    // 示例 4: 下载单个文件
    println!("\n=== 示例 4: 下载单个文件 ===");
    ModelScope::download_single_file_with_callback(
        "damo/nlp_structbert_backbone_base_std",
        "config.json",
        "./single_file",
        modelscope::SimpleCallback,
    )
    .await?;

    Ok(())
}
