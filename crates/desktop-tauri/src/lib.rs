//! ClipPal 桌面应用适配与启动编排。由打包壳注入 Tauri context 和静态资产。
//! 单机业务仍属于 desktop-app-core；本包负责 IPC、系统交互和可选版本扩展。

/// 应用壳打包的静态资产；适配包不读取宿主仓库路径。
/// 配置只用于当前应用初始化，不向前端暴露密钥或部署令牌。
pub struct PackagedAssets {
    pub config_json: &'static str,
    pub tray_icon_png: &'static [u8],
}

#[cfg(any(
    all(feature = "local", feature = "sync"),
    all(feature = "local", feature = "self-hosted"),
    all(feature = "local", feature = "commercial"),
))]
compile_error!("local 构建不能同时启用同步或商业 feature，请使用 --no-default-features");

mod app_context;
mod desktop;
#[cfg(any(feature = "sync", feature = "commercial-extension"))]
mod network;
mod bootstrap;
mod commands;
#[cfg(feature = "commercial-extension")]
mod commercial;
mod core;
mod errors;
mod response;
#[cfg(feature = "sync")]
mod sync;

/// 组装并运行 ClipPal Tauri 应用。
///
/// 该入口只负责初始化核心依赖、注册插件和 command、连接生命周期事件，
/// 具体业务由共享核心承担，系统能力由 desktop 适配。
pub async fn run(
    context: tauri::Context<tauri::Wry>,
    assets: PackagedAssets,
) -> Result<(), Box<dyn std::error::Error>> {
    desktop::config::initialize(assets)?;
    // 在构建 Tauri 应用前完成数据库、仓储、搜索引擎和共享上下文初始化。
    let core = bootstrap::init_core().await?;
    let app_context = core.app_context.clone();
    let core_for_setup = core.clone();
    let core_for_run = core.clone();

    // AppContext 作为 Tauri managed state 注入，所有 command 从同一上下文获取依赖。
    let autostart = bootstrap::is_autostart(std::env::args_os().skip(1));
    let builder = bootstrap::register_plugins(tauri::Builder::default(), autostart)
        .manage(app_context)
        .setup(move |app| Ok(bootstrap::setup_app(app, core_for_setup.clone(), autostart)?));
    // 不含商业扩展的包不注册账号、VIP 和支付 IPC。
    commands::register_commands(builder)
        .build(context)?
        // 将 Tauri RunEvent 交给 bootstrap 统一处理 Ready 和退出清理。
        .run(move |_, event| {
            bootstrap::handle_run_event(event, core_for_run.clone());
        });

    Ok(())
}
