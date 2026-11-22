//! 桌面更新适配。由 Tauri 解析更新清单、验证签名并安装更新包。
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Error, Update, Updater, UpdaterExt};
use tokio::sync::Mutex;

use desktop_app_core::update::{UpdateInfo, UpdateProgress, UpdateStatus};

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(125);

/// 同一个锁保护待安装版本和正在执行的操作，防止多个请求同时下载安装。
#[derive(Default)]
pub struct UpdateState {
    operation: Mutex<PendingUpdate>,
}

#[derive(Default)]
struct PendingUpdate {
    candidate: Option<Update>,
    restart_pending: bool,
}

/// 使用应用已配置的更新源。页面只能选择后端检查到的版本。
pub struct TauriUpdater<'a, R: Runtime> {
    app_handle: AppHandle<R>,
    state: &'a UpdateState,
}

impl<'a, R: Runtime> TauriUpdater<'a, R> {
    /// 使用应用句柄和进程内共享更新状态。
    pub fn new(app_handle: AppHandle<R>, state: &'a UpdateState) -> Self {
        Self { app_handle, state }
    }

    /// 检查更新并保留同一次检查的下载地址、签名及版本。
    pub async fn check(&self) -> Result<UpdateInfo, String> {
        let mut pending = self.state.operation.try_lock()
            .map_err(|_| "更新操作正在进行，请稍后重试".to_owned())?;
        if pending.restart_pending {
            return Err("更新已安装，应用即将重启".into());
        }

        pending.candidate = None;
        let info = UpdateInfo {
            status: UpdateStatus::UpToDate,
            current_version: self.app_handle.package_info().version.to_string(),
            latest_version: None,
            body: None,
            date: None,
        };
        let updater = match self.app_handle.updater_builder().timeout(CHECK_TIMEOUT).build() {
            Ok(updater) => updater,
            Err(Error::EmptyEndpoints) => {
                // 个人包使用空更新源，打开更新界面时显示手动更新说明。
                return Ok(UpdateInfo { status: UpdateStatus::Manual, ..info });
            }
            Err(error) => return Err(check_error(error)),
        };
        #[cfg(target_os = "macos")]
        {
            // Tauri 会替换 .app 整个目录。裸可执行文件没有应用包，不能对其执行原地更新。
            let executable = tauri::utils::platform::current_exe().map_err(|error| {
                log::error!("读取应用可执行文件路径失败: {error}");
                "系统异常，请稍后重试".to_owned()
            })?;
            if !is_macos_app_executable(&executable) {
                return Ok(UpdateInfo { status: UpdateStatus::Manual, ..info });
            }
        }
        pending.check_release(updater, info).await
    }

    /// 安装已确认的版本，签名验证前不调用平台安装程序。
    pub async fn download_and_install(
        &self,
        version: &str,
        emit: impl Fn(UpdateProgress),
    ) -> Result<(), String> {
        let mut pending = self.state.operation.try_lock()
            .map_err(|_| "更新操作正在进行，请稍后重试".to_owned())?;
        if pending.restart_pending {
            return Err("更新已安装，应用即将重启".into());
        }

        // 使用检查时保存的数据，避免重新查询期间发布的新版本被直接安装。
        let Some(mut update) = pending.candidate.take_if(|update| update.version == version) else {
            return Err("可安装版本已变化，请重新检查更新".into());
        };
        update.timeout = Some(DOWNLOAD_TIMEOUT);
        log::info!("开始下载更新: {}", update.version);

        let mut downloaded = 0u64;
        let mut last_progress = None::<Instant>;
        let bytes = update.download(
            |chunk_size, total| {
                downloaded = downloaded.saturating_add(chunk_size as u64);
                // 降低高频分块下载对页面和 IPC 的压力。
                if last_progress.is_none_or(|last| last.elapsed() >= PROGRESS_INTERVAL)
                    || total.is_some_and(|total| downloaded >= total)
                {
                    emit(UpdateProgress::Downloading {
                        downloaded,
                        total: total.filter(|total| *total > 0),
                    });
                    last_progress = Some(Instant::now());
                }
            },
            || emit(UpdateProgress::Verifying),
        ).await.map_err(download_error)?;

        emit(UpdateProgress::Installing);
        // 平台安装包含阻塞的文件操作或系统对话框，放在阻塞线程执行。
        tokio::task::spawn_blocking(move || update.install(bytes))
            .await
            .map_err(|error| {
                log::error!("更新安装任务异常: {error}");
                "系统异常，请稍后重试".to_owned()
            })?
            .map_err(|error| {
                log::error!("安装更新失败: {error}");
                "安装更新失败，请重试或手动安装新版".to_owned()
            })?;

        pending.restart_pending = true;
        Ok(())
    }
}

impl PendingUpdate {
    /// 根据同一次远程检查填充界面信息和待安装版本。
    async fn check_release(&mut self, updater: Updater, mut info: UpdateInfo) -> Result<UpdateInfo, String> {
        // 限制整个检查耗时，即使配置了多个候选地址。
        match tokio::time::timeout(CHECK_TIMEOUT, updater.check()).await {
            Ok(Ok(Some(update))) => {
                info.status = UpdateStatus::Available;
                info.latest_version = Some(update.version.clone());
                info.body = update.body.clone();
                info.date = update.date.map(|date| date.to_string());
                self.candidate = Some(update);
            }
            Ok(Ok(None)) => {}
            Ok(Err(Error::TargetNotFound(target))) => {
                log::info!("更新源未提供当前平台的产物: {target}");
                info.status = UpdateStatus::Unavailable;
            }
            Ok(Err(error)) => return Err(check_error(error)),
            Err(error) => {
                log::warn!("检查更新超时: {error}");
                return Err("检查更新超时，请稍后重试".into());
            }
        }
        Ok(info)
    }
}

/// macOS 更新的目标必须是完整的应用包，排除 cargo build 生成的裸程序。
#[cfg(target_os = "macos")]
fn is_macos_app_executable(executable: &std::path::Path) -> bool {
    let macos = executable.parent();
    let contents = macos.and_then(std::path::Path::parent);
    let bundle = contents.and_then(std::path::Path::parent);
    macos.and_then(std::path::Path::file_name).is_some_and(|name| name == "MacOS")
        && contents.and_then(std::path::Path::file_name).is_some_and(|name| name == "Contents")
        && bundle.and_then(std::path::Path::extension).is_some_and(|ext| ext == "app")
}

/// 检查失败保留日志细节，界面只给出可操作的提示。
fn check_error(error: Error) -> String {
    log::error!("检查更新失败: {error}");
    match error {
        Error::Reqwest(ref error) if error.is_timeout() => "检查更新超时，请稍后重试",
        Error::Reqwest(_) | Error::ReleaseNotFound | Error::Network(_) =>
            "暂时无法获取更新，请检查网络后重试",
        _ => "系统异常，请稍后重试",
    }.into()
}

/// 插件会在下载后验证签名，失败时不给安装程序任何数据。
fn download_error(error: Error) -> String {
    log::error!("更新下载或验签失败: {error}");
    match error {
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) =>
            "更新包签名校验失败，请重新检查更新",
        Error::Reqwest(ref error) if error.is_timeout() => "下载更新超时，请重试",
        Error::Reqwest(_) | Error::Network(_) => "更新包下载失败，请检查网络后重试",
        _ => "系统异常，请稍后重试",
    }.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    /// 个人包没有更新地址，检查返回手动更新，安装命令无法绕过检查。
    #[test]
    fn personal_build_uses_manual_updates() {
        let mut context = mock_context(noop_assets());
        context.config_mut().plugins.0.insert("updater".into(), serde_json::json!({
            "endpoints": [], "pubkey": "unused-test-key"
        }));
        let app = mock_builder().plugin(tauri_plugin_updater::Builder::new().build())
            .build(context).unwrap();
        let state = UpdateState::default();
        let updater = TauriUpdater::new(app.handle().clone(), &state);

        tauri::async_runtime::block_on(async {
            let result = updater.check().await.unwrap();
            assert!(matches!(result.status, UpdateStatus::Manual));
            assert!(result.latest_version.is_none());
            assert!(updater.download_and_install("2.0.0", |_| panic!("个人包不能启动下载"))
                .await.is_err());

            // 同一进程已有操作时，其他检查或安装会直接返回。
            let _busy = state.operation.lock().await;
            assert_eq!(updater.check().await.unwrap_err(), "更新操作正在进行，请稍后重试");
            assert_eq!(updater.download_and_install("2.0.0", |_| {}).await.unwrap_err(), "更新操作正在进行，请稍后重试");
        });
    }

    /// 用本机的临时更新源验证实际检查和验签。测试包不包含可安装内容。
    #[test]
    #[ignore = "需要绑定本机回环端口，单独运行"]
    fn confirmed_version_is_reused_and_invalid_signature_blocks_install() {
        use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        tauri::async_runtime::block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let manifest = serde_json::json!({
                "version": "99.0.0",
                "url": format!("{base}/package"),
                "signature": "not-a-valid-signature"
            }).to_string();
            let requests = Arc::new(AtomicUsize::new(0));
            let server_requests = requests.clone();
            let server = tokio::spawn(async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut headers = Vec::new();
                    while !headers.ends_with(b"\r\n\r\n") {
                        headers.push(stream.read_u8().await.unwrap());
                    }
                    server_requests.fetch_add(1, Ordering::Relaxed);
                    if headers.starts_with(b"GET /latest.json ") {
                        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{manifest}", manifest.len());
                        stream.write_all(response.as_bytes()).await.unwrap();
                    } else {
                        assert!(headers.starts_with(b"GET /package "));
                        // 用 chunked 响应复现服务端不公布总大小的下载。
                        stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\ntest\r\n0\r\n\r\n").await.unwrap();
                    }
                }
            });
            let mut context = mock_context(noop_assets());
            context.config_mut().plugins.0.insert("updater".into(), serde_json::json!({
                "dangerousInsecureTransportProtocol": true,
                "endpoints": [format!("{base}/latest.json")],
                "pubkey": "invalid-test-key"
            }));
            let app = mock_builder().plugin(tauri_plugin_updater::Builder::new().build())
                .build(context).unwrap();
            let state = UpdateState::default();
            let adapter = TauriUpdater::new(app.handle().clone(), &state);
            let updater = app.handle().updater_builder().build().unwrap();
            let info = state.operation.lock().await.check_release(updater, UpdateInfo {
                status: UpdateStatus::UpToDate,
                current_version: app.package_info().version.to_string(),
                latest_version: None,
                body: None,
                date: None,
            }).await.unwrap();
            assert!(matches!(info.status, UpdateStatus::Available));
            assert_eq!(info.latest_version.as_deref(), Some("99.0.0"));

            assert!(adapter.download_and_install("98.0.0", |_| panic!("版本不符不能开始下载"))
                .await.is_err());
            assert_eq!(requests.load(Ordering::Relaxed), 1);

            let phases = std::cell::RefCell::new(Vec::new());
            let error = adapter.download_and_install("99.0.0", |event| phases.borrow_mut().push(event))
                .await.unwrap_err();
            assert!(error.contains("签名校验失败"));
            assert!(matches!(phases.borrow().as_slice(), [
                UpdateProgress::Downloading { downloaded: 4, total: None }, UpdateProgress::Verifying
            ]));
            assert_eq!(requests.load(Ordering::Relaxed), 2, "安装使用原检查结果，不再次请求清单");
            server.abort();
        });
    }

    /// 裸文件和名称相似的普通目录都不能作为 macOS 更新目标。
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_only_updates_the_app_bundle() {
        use std::path::Path;
        assert!(is_macos_app_executable(Path::new("/Applications/ClipPal.app/Contents/MacOS/ClipPal")));
        assert!(!is_macos_app_executable(Path::new("/repo/target/release/ClipPal")));
        assert!(!is_macos_app_executable(Path::new("/repo/staging/Contents/MacOS/ClipPal")));
    }
}
