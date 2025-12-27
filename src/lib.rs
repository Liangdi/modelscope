use anyhow::{Context, bail};
use async_trait::async_trait;
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env::home_dir;
use std::fs;
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 进度回调 trait
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

/// 默认的进度回调实现（使用进度条）
pub struct ProgressBarCallback {
    bars: Arc<MultiProgress>,
    progress_bars: Arc<Mutex<HashMap<String, ProgressBar>>>,
}

impl ProgressBarCallback {
    pub fn new() -> Self {
        Self {
            bars: Arc::new(MultiProgress::new()),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for ProgressBarCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProgressBarCallback {
    fn clone(&self) -> Self {
        Self {
            bars: self.bars.clone(),
            progress_bars: self.progress_bars.clone(),
        }
    }
}

#[async_trait]
impl ProgressCallback for ProgressBarCallback {
    async fn on_file_start(&self, file_name: &str, file_size: u64) {
        // 检查是否已经存在相同名称的进度条
        {
            let bars = self.progress_bars.lock().unwrap();
            if bars.contains_key(file_name) {
                return; // 如果已存在，不再创建新的进度条
            }
        }
        
        let bar = ProgressBar::new(file_size);
        let style = ProgressStyle::default_bar().template(BAR_STYLE).unwrap();
        bar.set_style(style);
        bar.set_message(file_name.to_string());
        self.bars.add(bar.clone());
        
        let mut bars = self.progress_bars.lock().unwrap();
        bars.insert(file_name.to_string(), bar);
    }
    
    async fn on_file_progress(&self, file_name: &str, downloaded: u64, _total: u64) {
        let bars = self.progress_bars.lock().unwrap();
        if let Some(bar) = bars.get(file_name) {
            bar.set_position(downloaded);
        }
    }
    
    async fn on_file_complete(&self, file_name: &str) {
        let mut bars = self.progress_bars.lock().unwrap();
        if let Some(bar) = bars.remove(file_name) {
            bar.finish();
        }
    }
    
    async fn on_file_error(&self, file_name: &str, _error: &str) {
        let mut bars = self.progress_bars.lock().unwrap();
        if let Some(bar) = bars.remove(file_name) {
            bar.abandon();
        }
    }
}

/// 简单的回调实现，只打印进度信息
#[derive(Clone)]
pub struct SimpleCallback;

#[async_trait]
impl ProgressCallback for SimpleCallback {
    async fn on_file_start(&self, file_name: &str, file_size: u64) {
        println!("开始下载: {} (大小: {} bytes)", file_name, file_size);
    }
    
    async fn on_file_progress(&self, file_name: &str, downloaded: u64, total: u64) {
        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0) as u32
        } else {
            0
        };
        println!("下载中: {} - {}% ({} / {} bytes)", file_name, percent, downloaded, total);
    }
    
    async fn on_file_complete(&self, file_name: &str) {
        println!("下载完成: {}", file_name);
    }
    
    async fn on_file_error(&self, file_name: &str, error: &str) {
        eprintln!("下载失败: {} - 错误: {}", file_name, error);
    }
}

const FILES_URL: &str = "https://modelscope.cn/api/v1/models/<model_id>/repo/files?Recursive=true";
const DOWNLOAD_URL: &str = "https://modelscope.cn/models/<model_id>/resolve/master/<path>";
const LOGIN_URL: &str = "https://modelscope.cn/api/v1/login";
const DIR: &str = ".modelscope";
const COOKIES_FILE: &str = "cookies";

const UA: (&str, &str) = (
    "User-Agent",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.90 Safari/537.36",
);
pub struct ModelScope;

#[derive(Debug, Deserialize)]
struct ModelScopeResponse {
    #[serde(rename = "Code")]
    #[allow(unused)]
    code: i64,
    #[serde(rename = "Success")]
    success: bool,
    #[serde(rename = "Message")]
    message: String,
    #[serde(rename = "Data")]
    data: Option<ModelScopeResponseData>,
}

#[derive(Debug, Deserialize)]
struct ModelScopeResponseData {
    #[serde(rename = "Files")]
    files: Vec<RepoFile>,
}
#[derive(Debug, Deserialize)]
struct RepoFile {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Path")]
    path: String,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "Sha256")]
    #[allow(unused)]
    sha256: String,
    #[serde(rename = "Type")]
    r#type: String,
}

const BAR_STYLE: &str = "{msg:<30} {bar} {decimal_bytes:<10} / {decimal_total_bytes:<10} {decimal_bytes_per_sec:<12} {percent:<3}%  {eta_precise}";

impl ModelScope {
    async fn get_client() -> anyhow::Result<reqwest::Client> {
        let client = reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(10));
        let mut default_headers = reqwest::header::HeaderMap::new();
        if let Some(cookies) = Self::get_cookies()? {
            default_headers.insert("Cookie", cookies.parse()?);
        }
        let client = client.default_headers(default_headers);
        Ok(client.build()?)
    }

    pub async fn download(model_id: &str, save_dir: impl Into<PathBuf>) -> anyhow::Result<()> {
        Self::download_with_callback(model_id, save_dir, ProgressBarCallback::default()).await
    }

    pub async fn download_with_callback<C: ProgressCallback + Clone + 'static>(
        model_id: &str,
        save_dir: impl Into<PathBuf>,
        callback: C,
    ) -> anyhow::Result<()> {
        // Model root dir
        let save_dir = save_dir.into();
        fs::create_dir_all(&save_dir)?;

        // Model save dir, like <save_dir>/<model_id>
        let model_dir = save_dir.join(model_id);

        println!();
        println!("Downloading model {} to: {}", model_id, model_dir.display());
        println!();

        fs::create_dir_all(&model_dir)?;

        let files_url = FILES_URL.replace("<model_id>", model_id);

        let client = Arc::new(Self::get_client().await?);

        let resp = client.get(files_url).send().await?;

        if !resp.status().is_success() {
            bail!(
                "Failed to get model files: {}\nTip: Maybe the model ID is incorrect or login is required",
                resp.text().await?
            );
        }

        let response = resp.json::<ModelScopeResponse>().await?;
        if !response.success {
            bail!("Failed to get model files: {}", response.message);
        }

        let data = response.data.unwrap();
        let repo_files = data.files;

        // Add the incoming model save path to the known model paths
        // This is used when using the list command
        Config::append_save_dir(&save_dir)?;

        let mut tasks = Vec::new();

        for repo_file in repo_files.into_iter().filter(|f| f.r#type == "blob") {
            let model_id = model_id.to_string();
            let client = client.clone();
            let save_dir = model_dir.clone();
            let callback = callback.clone();

            let task = tokio::spawn(async move {
                let res = Self::download_file_with_callback(client, model_id, repo_file, save_dir, callback).await;
                if let Err(e) = res {
                    bail!("Error downloading file: {}", e);
                }
                Ok::<(), anyhow::Error>(())
            });

            tasks.push(task);
        }
        for task in tasks {
            task.await??;
        }

        Ok(())
    }

    async fn download_file(
        client: Arc<reqwest::Client>,
        model_id: String,
        repo_file: RepoFile,
        save_dir: PathBuf,
        bar: ProgressBar,
    ) -> anyhow::Result<()> {
        let path = &repo_file.path;
        let name = &repo_file.name;

        bar.set_message(name.clone());

        let file_path = save_dir.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut existing_size = 0;
        let mut file_options = fs::OpenOptions::new();
        file_options.write(true).create(true);

        if file_path.exists() {
            if let Ok(metadata) = fs::metadata(&file_path) {
                existing_size = metadata.len();
                file_options.append(true);
            }
        } else {
            file_options.truncate(true);
        }

        let mut file = BufWriter::new(file_options.open(&file_path)?);

        // Set progress bar initial position
        bar.set_position(existing_size);
        bar.set_length(repo_file.size);

        let url = DOWNLOAD_URL
            .replace("<model_id>", &model_id)
            .replace("<path>", path);

        let mut rb = client.get(&url).header(UA.0, UA.1);

        // Already downloaded, just return ok.
        // If file size equal repo file size, maybe check sha256
        // But I think the probability of files having the same number of bytes is relatively low, so I won't check here. 🙊
        if existing_size == repo_file.size {
            bar.finish();
            return Ok(());
        }

        // Resume download
        if existing_size < repo_file.size {
            rb = rb.header("Range", format!("bytes={}-", existing_size));
        }

        let response = rb.send().await?;

        let status = response.status();

        // Server doesn't support resume download, re-downloading from beginning
        // Or existing file size is larger than repo size, re-downloading from beginning
        if status == reqwest::StatusCode::OK && existing_size > 0 || existing_size > repo_file.size
        {
            file.rewind()?;
            file.get_ref().set_len(0)?;
            bar.set_position(0);
        }

        // If status is not success or partial content, bail
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            bail!(
                "Failed to download file {}: HTTP {}",
                name,
                response.status()
            );
        }

        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            bar.inc(chunk.len() as u64);
        }

        file.flush()?;

        bar.finish();

        Ok(())
    }

    async fn download_file_with_callback<C: ProgressCallback + Clone + 'static>(
        client: Arc<reqwest::Client>,
        model_id: String,
        repo_file: RepoFile,
        save_dir: PathBuf,
        callback: C,
    ) -> anyhow::Result<()> {
        let path = &repo_file.path;
        let name = &repo_file.name;

        let file_path = save_dir.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut existing_size = 0;
        let mut file_options = fs::OpenOptions::new();
        file_options.write(true).create(true);

        if file_path.exists() {
            if let Ok(metadata) = fs::metadata(&file_path) {
                existing_size = metadata.len();
                file_options.append(true);
            }
        } else {
            file_options.truncate(true);
        }

        let mut file = BufWriter::new(file_options.open(&file_path)?);

        let url = DOWNLOAD_URL
            .replace("<model_id>", &model_id)
            .replace("<path>", path);

        // Now we call on_file_start after checking if file exists
        callback.on_file_start(name, repo_file.size).await;

        let mut rb = client.get(&url).header(UA.0, UA.1);

        // Already downloaded, just return ok.
        if existing_size == repo_file.size {
            callback.on_file_progress(name, repo_file.size, repo_file.size).await;
            callback.on_file_complete(name).await;
            return Ok(());
        }

        // Resume download
        if existing_size < repo_file.size {
            rb = rb.header("Range", format!("bytes={}-", existing_size));
        }

        let response = rb.send().await?;

        let status = response.status();

        // Server doesn't support resume download, re-downloading from beginning
        // Or existing file size is larger than repo size, re-downloading from beginning
        if status == reqwest::StatusCode::OK && existing_size > 0 || existing_size > repo_file.size
        {
            file.rewind()?;
            file.get_ref().set_len(0)?;
            existing_size = 0;
            callback.on_file_progress(name, 0, repo_file.size).await;
        }

        // If status is not success or partial content, bail
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            let error_msg = format!("HTTP {}", response.status());
            callback.on_file_error(name, &error_msg).await;
            bail!(
                "Failed to download file {}: HTTP {}",
                name,
                response.status()
            );
        }

        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk)?;
            existing_size += chunk.len() as u64;
            callback.on_file_progress(name, existing_size, repo_file.size).await;
        }

        file.flush()?;

        callback.on_file_complete(name).await;

        Ok(())
    }

    pub async fn login(token: &str) -> anyhow::Result<()> {
        println!("Logging in...");
        let client = Self::get_client().await?;
        let resp = client
            .post(LOGIN_URL)
            .json(&serde_json::json!({
                "AccessToken": token
            }))
            .send()
            .await?;

        let status = resp.status();

        if !status.is_success() {
            bail!("Failed to login: {}", resp.text().await?);
        }

        let cookies: serde_json::Value = resp
            .cookies()
            .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
            .collect();

        let dir = Dirs::config_dir()?;

        let cookies_file = dir.join(COOKIES_FILE);
        fs::write(cookies_file, cookies.to_string())?;

        println!("Login successful.");

        Ok(())
    }

    pub async fn download_single_file(
        model_id: &str,
        file_path: &str,
        save_dir: impl Into<PathBuf>,
    ) -> anyhow::Result<()> {
        Self::download_single_file_with_callback(model_id, file_path, save_dir, ProgressBarCallback::default()).await
    }

    pub async fn download_single_file_with_callback<C: ProgressCallback + Clone + 'static>(
        model_id: &str,
        file_path: &str,
        save_dir: impl Into<PathBuf>,
        callback: C,
    ) -> anyhow::Result<()> {
        let save_dir = save_dir.into();
        fs::create_dir_all(&save_dir)?;

        let model_dir = save_dir.join(model_id);
        fs::create_dir_all(&model_dir)?;

        println!();
        println!(
            "Downloading file {} from model {} to: {}",
            file_path,
            model_id,
            model_dir.display()
        );
        println!();

        let files_url = FILES_URL.replace("<model_id>", model_id);

        let client = Arc::new(Self::get_client().await?);

        // Get file list from API
        let resp = client.get(files_url).send().await?;

        if !resp.status().is_success() {
            bail!(
                "Failed to get model files: {}\nTip: Maybe the model ID is incorrect or login is required",
                resp.text().await?
            );
        }

        let response = resp.json::<ModelScopeResponse>().await?;
        if !response.success {
            bail!("Failed to get model files: {}", response.message);
        }

        let data = response.data.unwrap();
        let repo_files = data.files;

        // Find the target file
        let repo_file = repo_files
            .into_iter()
            .find(|f| f.path == file_path && f.r#type == "blob")
            .ok_or_else(|| anyhow::anyhow!("File not found in model: {}", file_path))?;

        Self::download_file_with_callback(client, model_id.to_string(), repo_file, model_dir, callback).await?;

        Ok(())
    }

    fn get_cookies() -> anyhow::Result<Option<String>> {
        let cookies_file = Dirs::config_dir()?.join(COOKIES_FILE);

        if cookies_file.exists() {
            let cookies = fs::read_to_string(cookies_file)?;
            let cookies: serde_json::Value = serde_json::from_str(&cookies)?;

            let cookies = cookies
                .as_object()
                .context("Failed to parse cookies")?
                .iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("; ");
            return Ok(Some(cookies));
        }

        Ok(None)
    }

    pub async fn logout() -> anyhow::Result<()> {
        // May just delete cookies file
        let cookies_file = Dirs::config_dir()?.join(COOKIES_FILE);
        if cookies_file.exists() {
            fs::remove_file(cookies_file)?;
        }
        println!("Logged out.");
        Ok(())
    }

    pub async fn list() -> anyhow::Result<Vec<(String, String)>> {
        // Known model save paths
        let model_paths = Config::get_known_save_dirs()?;

        let mut models = vec![];
        for model_path in model_paths {
            for dir in fs::read_dir(model_path)? {
                let dir = dir?;
                // This level is the model vendor, and the next level is the model name
                if dir.file_type()?.is_dir() {
                    for entry in fs::read_dir(dir.path())? {
                        let entry = entry?;
                        if entry.file_type()?.is_dir() {
                            models.push((
                                // Model ID
                                format!(
                                    "{}/{}",
                                    dir.file_name().display(),
                                    entry.file_name().display()
                                ),
                                // Model path
                                dir.path().display().to_string(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(models)
    }
}

struct Dirs {}
impl Dirs {
    fn base_dir() -> anyhow::Result<PathBuf> {
        let base_dir = home_dir()
            .context("Failed to get home directory")?
            .join(DIR);
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }
        Ok(base_dir)
    }

    fn config_dir() -> anyhow::Result<PathBuf> {
        let config_dir = Self::base_dir()?.join("config");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }
        Ok(config_dir)
    }

    #[allow(unused)]
    pub fn model_dir() -> anyhow::Result<PathBuf> {
        let model_dir = Self::base_dir()?.join("models");
        if !model_dir.exists() {
            fs::create_dir_all(&model_dir)?;
        }
        Ok(model_dir)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    known_save_dirs: Vec<PathBuf>,
}

impl Config {
    const KNOWN_SAVE_DIRS: &'static str = "known_save_dirs";
    fn append_save_dir(dir: &Path) -> anyhow::Result<()> {
        let f = Dirs::config_dir()?.join(Self::KNOWN_SAVE_DIRS);

        // Get existing known save dirs
        let mut known_save_dirs = Self::get_known_save_dirs()?;

        // Canonicalize the directory
        let dir = dir.canonicalize()?;

        if known_save_dirs.contains(&dir) {
            return Ok(());
        }

        known_save_dirs.push(dir);
        fs::write(
            f,
            known_save_dirs
                .iter()
                .filter(|p| p.exists())
                .map(|p| p.display().to_string())
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<String>>()
                .join("\n"),
        )?;

        Ok(())
    }

    fn get_known_save_dirs() -> anyhow::Result<Vec<PathBuf>> {
        let config_dir = Dirs::config_dir()?;
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
            return Ok(vec![]);
        }

        let f = config_dir.join(Self::KNOWN_SAVE_DIRS);
        if !f.exists() {
            return Ok(vec![]);
        }

        let paths = fs::read_to_string(f)?
            .lines()
            .map(PathBuf::from)
            // Filter out non-existent paths
            // These paths will be cleaned up when append_save_dir is called
            .filter(|p| p.exists())
            .collect::<Vec<_>>();

        Ok(paths)
    }
}
