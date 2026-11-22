use tauri::{Manager, Runtime};
use tauri_plugin_autostart::MacosLauncher;

/// 注册 Tauri 插件。
///
/// 这里只负责插件装配，不处理业务初始化。
pub fn register_plugins<R>(builder: tauri::Builder<R>, autostart: bool) -> tauri::Builder<R>
where
    R: Runtime,
{
    #[cfg(feature = "sync")]
    let builder = builder.plugin(tauri_plugin_http::init());

    // 后台启动也跳过位置、全屏等首次恢复，避免系统恢复操作间接唤醒窗口。
    let mut window_state = tauri_plugin_window_state::Builder::new()
        .with_state_flags(tauri_plugin_window_state::StateFlags::all() - tauri_plugin_window_state::StateFlags::VISIBLE);
    if autostart {
        window_state = window_state.skip_initial_state("main");
    }

    builder
        // 软件自动更新
        .plugin(tauri_plugin_updater::Builder::new().build())
        // 本机系统对话框，用于打开、保存文件和展示消息弹窗
        .plugin(tauri_plugin_dialog::init())
        // 保存窗口位置和大小，并在应用重新打开时恢复
        .plugin(window_state.build())
        // 使用系统默认应用打开文件或 URL
        .plugin(tauri_plugin_opener::init())
        // ClipPal 自定义剪贴板插件
        .plugin(tauri_plugin_clipboard_pal::init())
        // 开机自启插件
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![super::AUTOSTART_ARG]),
        ))
        // 单实例插件：第二次启动时显示并聚焦已有主窗口
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if super::is_autostart(args.iter().skip(1)) { return; }
            if let Some(context) = app.try_state::<std::sync::Arc<crate::app_context::AppContext>>() {
                if let Err(error) = crate::desktop::shell::window::show_main_window(context.inner()) {
                    log::error!("重复启动唤起主窗口失败: {}", error);
                }
            }
        }))
}
