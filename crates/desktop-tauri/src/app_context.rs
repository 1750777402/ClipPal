use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, WebviewWindow};

use {
    clip_core::domain::settings::Settings,
    crate::errors::AppError,
    crate::errors::AppResult,
    clip_core::repositories::AppRepositories,
    clip_core::search::SearchEngine,
    crate::desktop::shell::window::WindowFocusState,
    crate::desktop::shell::window::WindowHideFlag,
};

pub use desktop_app_core::LocalContext as CoreContext;
#[cfg(feature = "commercial-extension")]
pub use crate::commercial::context::CommercialContext;
#[cfg(feature = "sync")]
pub use selfhost_app_core::{SelfhostContext as ApplicationContext, SyncContext};
#[cfg(not(feature = "sync"))]
pub type ApplicationContext = CoreContext;
static APP_CONTEXT: OnceLock<Arc<AppContext>> = OnceLock::new();

/// 应用运行期共享上下文。
///
/// `AppContext` 是后端内部资源的统一入口，主要解决两个问题：
/// 1. 让 command、后台任务、窗口模块、同步模块能够显式拿到依赖；
/// 2. 区分“启动时一定存在”和“运行过程中才会出现”的资源。
///
/// 这里应该只放底层运行资源和运行状态，不放具体业务 service。
/// 比如数据库连接、设置缓存、同步队列、窗口状态适合放在这里；
/// 登录流程、VIP 计算、剪贴板记录处理这类业务逻辑不应该直接塞进这里。
///
/// 后续新增内容时建议遵守这个规则：
/// - 启动时必须存在的资源，放到 `CoreContext`；
/// - Tauri setup 后才能拿到且只应设置一次的资源，用 `InitCell<T>`；
/// - 用户操作后才会变化的状态，放到对应领域上下文；
/// - 某个领域自己的底层资源，按领域拆到独立的子 Context。
///
pub struct AppContext {
    /// 应用启动阶段必须完成初始化的核心资源。
    application: ApplicationContext,
    /// 当前版本的能力限制，单机业务只调用策略而不识别商业实现。
    policy: Arc<dyn crate::core::policy::ApplicationPolicy>,
    /// Tauri setup / runtime 阶段才能拿到的运行期资源。
    runtime: RuntimeContext,
    /// 账号、VIP 等商业扩展使用的底层资源。
    ///
    /// 这些资源独立成组，避免把商业服务误认为本地核心能力。
    #[cfg(feature = "commercial-extension")]
    commercial: CommercialContext,
    /// 主窗口相关的运行状态。
    window: WindowContext,
    /// 自动粘贴流程共享的运行状态。
    auto_paste: AutoPasteState,
    /// 共享更新检查结果和操作锁，多个 IPC 调用使用同一份状态。
    updater: crate::desktop::system::updater::UpdateState,
    /// 串行读取、保存自建同步偏好和启停后台会话。
    #[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
    sync_switch_lock: tokio::sync::Mutex<()>,
}

/// 自动粘贴的目标窗口。
#[derive(Clone, Debug)]
pub enum AutoPasteTarget {
    #[cfg(windows)]
    Windows {
        hwnd: isize,
        process_id: u32,
        title: String,
    },
    #[cfg(target_os = "macos")]
    MacOS {
        process_id: i32,
        bundle_id: Option<String>,
        app_name: String,
    },
    #[cfg(not(any(windows, target_os = "macos")))]
    Unsupported,
}

/// 自动粘贴流程需要跨入口共享的状态数据。
pub struct AutoPasteState {
    target: Mutex<Option<AutoPasteTarget>>,
    /// 新的唤起请求会使尚未发送按键的自动粘贴失效。
    pub wake_generation: Arc<std::sync::atomic::AtomicU64>,
    operation_lock: tokio::sync::Mutex<()>,
    pub permission_prompt_open: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for AutoPasteState {
    /// 初始化空的自动粘贴目标、唤起序号和互斥锁，不默认绑定目标应用。
    fn default() -> Self {
        Self {
            target: Mutex::new(None),
            wake_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            operation_lock: tokio::sync::Mutex::new(()),
            permission_prompt_open: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

impl AutoPasteState {
    /// 替换最近一次唤起 ClipPal 前捕获的目标窗口。
    pub fn replace_target(&self, target: Option<AutoPasteTarget>) -> AppResult<()> {
        *self.target.lock()? = target;
        Ok(())
    }

    /// 当前台是 ClipPal 时保留外部目标；捕获失败或没有前台窗口时清空旧目标。
    pub fn record_capture(&self, capture: AppResult<crate::desktop::system::auto_paste::TargetCapture>) -> AppResult<()> {
        use crate::desktop::system::auto_paste::TargetCapture;
        match capture {
            Ok(TargetCapture::CurrentApp) => Ok(()),
            Ok(TargetCapture::External(target)) => self.replace_target(Some(target)),
            Ok(TargetCapture::Unavailable) => self.replace_target(None),
            Err(error) => { self.replace_target(None)?; Err(error) }
        }
    }

    /// 获取目标窗口快照，避免在系统调用期间长期持有状态锁。
    pub fn target(&self) -> AppResult<Option<AutoPasteTarget>> {
        Ok(self.target.lock()?.clone())
    }

    /// 获取完整复制与自动粘贴流程使用的串行锁。
    pub fn operation_lock(&self) -> &tokio::sync::Mutex<()> {
        &self.operation_lock
    }
}

/// 只初始化一次的运行期资源。
///
/// 适合 AppHandle、主窗口这类 setup 阶段才能拿到，但设置后不应该再替换的对象。
///
/// `InitCell<T>` 表示“晚一点初始化，但初始化后不可变”；
/// 会反复更新的状态使用对应领域上下文的锁保护。
///
/// 这里使用标准库 `OnceLock`，可以避免误把只应初始化一次的资源覆盖掉。
pub struct InitCell<T> {
    /// 资源名称，用于构造更明确的错误信息。
    name: &'static str,
    /// 实际保存的资源。
    value: OnceLock<T>,
}

impl<T> InitCell<T> {
    /// 创建一个还没有初始化的资源槽。
    ///
    /// `name` 只用于错误提示，不参与逻辑判断。
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            value: OnceLock::new(),
        }
    }

    /// 设置资源值。
    ///
    /// 第一次调用会成功；如果已经设置过，会返回错误。
    /// 这可以及时暴露重复初始化的问题，避免静默覆盖运行期资源。
    pub fn set(&self, value: T) -> AppResult<()> {
        self.value
            .set(value)
            .map_err(|_| AppError::General(format!("{} 已初始化，不能重复设置", self.name)))
    }

    /// 获取已经初始化的资源引用。
    ///
    /// 当调用方认为资源在当前阶段必须存在时使用这个方法。
    /// 如果资源还没有设置，会返回 `AppError`，方便向上层明确暴露初始化顺序问题。
    pub fn get(&self) -> AppResult<&T> {
        self.value
            .get()
            .ok_or_else(|| AppError::General(format!("{} 尚未初始化", self.name)))
    }

    /// 尝试获取资源引用。
    ///
    /// 当资源不存在是可接受状态时使用，比如某些启动阶段或可选能力判断。
    pub fn try_get(&self) -> Option<&T> {
        self.value.get()
    }
}

impl<T: Clone> InitCell<T> {
    /// 获取资源并克隆一份返回。
    ///
    /// `AppHandle`、`WebviewWindow` 这类 Tauri 类型本身就是轻量可克隆句柄，
    /// 对外返回 clone 比暴露内部引用更方便，也更适合跨 async/task 使用。
    pub fn get_cloned(&self) -> AppResult<T> {
        self.get().cloned()
    }

    /// 尝试获取资源并克隆一份返回。
    pub fn try_get_cloned(&self) -> Option<T> {
        self.try_get().cloned()
    }
}

/// Tauri 运行期资源。
///
/// 这里放 Tauri setup 阶段或窗口创建后才能拿到的对象。
pub struct RuntimeContext {
    /// Tauri 应用句柄。
    ///
    /// AppHandle 需要在 `.setup()` 中通过 `app.handle()` 获取。
    /// 初始化后不应该被替换，所以使用 `InitCell`。
    app_handle: InitCell<AppHandle>,
    /// 成功注册的实际快捷键；配置字符串不能代替系统注册状态。
    shortcut: Mutex<Option<String>>,
    /// 需要重试释放的临时占用检查键，不绑定唤起处理器。
    shortcut_probe: Mutex<Option<tauri_plugin_global_shortcut::Shortcut>>,
}

impl Default for RuntimeContext {
    /// 创建尚未注入 AppHandle 的运行期上下文；AppHandle 会在 Tauri setup 阶段设置。
    fn default() -> Self {
        Self {
            app_handle: InitCell::new("AppHandle"),
            shortcut: Mutex::new(None),
            shortcut_probe: Mutex::new(None),
        }
    }
}

/// 窗口相关运行状态。
///
/// 这里只保存窗口行为需要共享的底层状态，不放具体窗口业务逻辑。
pub struct WindowContext {
    /// 主窗口是否已真正获得焦点。
    ///
    /// 用来区分真实失焦和启动期间的初始失焦事件。
    focus_state: Arc<WindowFocusState>,

    /// 主窗口是否允许隐藏的标记。
    ///
    /// 例如打开文件保存对话框时，需要临时禁止主窗口因失焦而隐藏。
    hide_flag: Arc<WindowHideFlag>,

    /// 主窗口实例，setup 初始化主窗口后设置。
    ///
    /// 主窗口在 Tauri 窗口系统准备好后才能获取，所以用 `InitCell`。
    main_window: InitCell<WebviewWindow>,
    image_viewer: tokio::sync::Mutex<crate::desktop::shell::image_viewer::ImageViewerState>,
}

impl Default for WindowContext {
    /// 创建窗口焦点、隐藏标记和待初始化主窗口句柄。
    fn default() -> Self {
        Self {
            focus_state: Arc::new(WindowFocusState::default()),
            hide_flag: Arc::new(WindowHideFlag::default()),
            main_window: InitCell::new("主窗口"),
            image_viewer: Default::default(),
        }
    }
}

impl AppContext {
    pub(crate) fn image_viewer(
        &self,
    ) -> &tokio::sync::Mutex<crate::desktop::shell::image_viewer::ImageViewerState> {
        &self.window.image_viewer
    }

    /// 创建应用上下文。
    ///
    /// 应用核心及可选同步资源由外部注入；此处只创建桌面运行状态。
    pub fn new(
        application: ApplicationContext,
        #[cfg(feature = "commercial-extension")]
        commercial: CommercialContext,
        policy: Arc<dyn crate::core::policy::ApplicationPolicy>,
    ) -> Self {
        Self {
            application,
            policy,
            runtime: RuntimeContext::default(),
            #[cfg(feature = "commercial-extension")]
            commercial,
            window: WindowContext::default(),
            auto_paste: AutoPasteState::default(),
            updater: crate::desktop::system::updater::UpdateState::default(),
            #[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
            sync_switch_lock: Default::default(),
        }
    }

    /// 取得当前版本注入的能力策略，业务入口不自行判断账号或套餐。
    pub fn policy(&self) -> &dyn crate::core::policy::ApplicationPolicy { self.policy.as_ref() }

    /// 返回当前进程唯一的更新状态。
    pub fn updater(&self) -> &crate::desktop::system::updater::UpdateState {
        &self.updater
    }

    /// 获取核心资源分组。
    ///
    /// 本地业务服务使用该上下文；桌面调用仓储时可直接使用 repositories()。
    pub fn core(&self) -> &CoreContext {
        #[cfg(feature = "sync")]
        {
            self.application.local()
        }
        #[cfg(not(feature = "sync"))]
        {
            &self.application
        }
    }

    /// 分发剪贴板采集。自建版可以追加同步动作，单机服务仍负责保存记录。
    pub async fn capture_clipboard(
        &self, input: desktop_app_core::capture::CaptureInput<'_>,
    ) -> AppResult<()> {
        let encoder = crate::desktop::security::encrypt_content;
        #[cfg(feature = "sync")]
        self.application.capture(input, &encoder).await?;
        #[cfg(not(feature = "sync"))]
        desktop_app_core::capture::CaptureService::new(self.core(), &encoder).capture(input).await?;
        Ok(())
    }

    /// 获取同步资源分组。
    #[cfg(feature = "sync")]
    pub fn sync(&self) -> &SyncContext {
        self.application.sync()
    }

    #[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
    pub(crate) fn sync_switch_lock(&self) -> &tokio::sync::Mutex<()> {
        &self.sync_switch_lock
    }

    /// 获取商业扩展资源分组。
    #[cfg(feature = "commercial-extension")]
    pub fn commercial(&self) -> &CommercialContext {
        &self.commercial
    }

    /// 获取自动粘贴共享状态。
    pub fn auto_paste_state(&self) -> &AutoPasteState {
        &self.auto_paste
    }

    /// 获取应用数据访问接口。
    pub fn repositories(&self) -> &AppRepositories {
        self.core().repositories()
    }

    /// 获取剪贴记录搜索引擎。
    pub fn search_engine(&self) -> &dyn SearchEngine {
        self.core().search_engine()
    }

    /// 在读锁保护下读取系统设置。
    ///
    /// 适合只需要从 `Settings` 中取一个值或计算一个结果的场景。
    /// 闭包结束后读锁会立即释放。
    pub fn with_settings<R>(&self, f: impl FnOnce(&Settings) -> R) -> AppResult<R> {
        self.core().with_settings(f)
    }

    /// 在写锁保护下更新系统设置缓存。
    ///
    /// 只负责更新内存中的设置，不负责保存到文件。
    /// 需要持久化时仍应调用对应的设置保存逻辑。
    pub fn update_settings<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> AppResult<R> {
        self.core().update_settings(f)
    }

    /// 设置 Tauri AppHandle。
    ///
    /// 应在 Tauri `.setup()` 阶段调用一次。
    /// 重复调用会返回错误。
    pub fn set_app_handle(&self, app_handle: AppHandle) -> AppResult<()> {
        self.runtime.app_handle.set(app_handle)
    }

    /// 获取 AppHandle。
    ///
    /// 当当前调用链要求 AppHandle 必须已经存在时使用。
    pub fn app_handle(&self) -> AppResult<AppHandle> {
        self.runtime.app_handle.get_cloned()
    }

    /// 尝试获取 AppHandle。
    ///
    /// 当调用发生在 setup 之前，或 AppHandle 只是可选能力时使用。
    pub fn try_app_handle(&self) -> Option<AppHandle> {
        self.runtime.app_handle.try_get_cloned()
    }

    /// 设置主窗口句柄。
    ///
    /// 应在主窗口初始化完成后调用一次。
    pub fn set_main_window(&self, window: WebviewWindow) -> AppResult<()> {
        self.window.main_window.set(window)
    }

    /// 获取主窗口句柄。
    ///
    /// 当当前逻辑要求主窗口必须已经初始化时使用。
    pub fn main_window(&self) -> AppResult<WebviewWindow> {
        self.window.main_window.get_cloned()
    }

    /// 获取主窗口焦点状态。
    pub fn window_focus_state(&self) -> Arc<WindowFocusState> {
        self.window.focus_state.clone()
    }

    /// 读取系统成功注册的快捷键快照，不以配置值代替真实状态。
    pub fn registered_shortcut(&self) -> AppResult<Option<String>> {
        Ok(self.runtime.shortcut.lock()?.clone())
    }

    /// 保存当前已成功注册的快捷键状态，供设置和退出清理共用。
    pub fn set_registered_shortcut(&self, shortcut: Option<String>) -> AppResult<()> {
        *self.runtime.shortcut.lock()? = shortcut;
        Ok(())
    }

    /// 读取尚待释放的快捷键试注册句柄，便于后续重试清理。
    pub fn pending_shortcut_probe(&self) -> AppResult<Option<tauri_plugin_global_shortcut::Shortcut>> {
        Ok(*self.runtime.shortcut_probe.lock()?)
    }

    /// 记录或清除待释放的试注册句柄，不绑定实际唤起动作。
    pub fn set_pending_shortcut_probe(&self, key: Option<tauri_plugin_global_shortcut::Shortcut>) -> AppResult<()> {
        *self.runtime.shortcut_probe.lock()? = key;
        Ok(())
    }

    /// 获取主窗口隐藏控制标记。
    ///
    /// 返回 `Arc`，方便在对话框、自动粘贴等跨作用域场景中共享。
    pub fn window_hide_flag(&self) -> Arc<WindowHideFlag> {
        self.window.hide_flag.clone()
    }
}

/// 设置应用级上下文。
///
/// 应用启动时设置一次，后续模块通过 `app_context()` 获取同一份运行资源。
pub fn set_app_context(context: Arc<AppContext>) -> AppResult<()> {
    APP_CONTEXT
        .set(context)
        .map_err(|_| AppError::General("AppContext 已初始化，不能重复设置".to_string()))
}

/// 获取应用级上下文。
pub fn app_context() -> AppResult<Arc<AppContext>> {
    APP_CONTEXT
        .get()
        .cloned()
        .ok_or_else(|| AppError::General("AppContext 尚未初始化".to_string()))
}

/// 尝试获取应用级上下文。
#[cfg(feature = "commercial-extension")]
pub fn try_app_context() -> Option<Arc<AppContext>> {
    APP_CONTEXT.get().cloned()
}

#[cfg(test)]
mod auto_paste_tests {
    use super::*;
    use crate::desktop::system::auto_paste::TargetCapture;

    /// 构造平台对应的测试目标，不查找或操作实际前台应用。
    fn target() -> AutoPasteTarget {
        #[cfg(target_os = "macos")]
        { AutoPasteTarget::MacOS { process_id: 123, bundle_id: Some("test.editor".into()), app_name: "Editor".into() } }
        #[cfg(windows)]
        { AutoPasteTarget::Windows { hwnd: 123, process_id: 123, title: "Editor".into() } }
        #[cfg(not(any(windows, target_os = "macos")))]
        { AutoPasteTarget::Unsupported }
    }

    /// 验证重复唤起保留外部目标，无目标或捕获失败则清空旧目标。
    #[test]
    fn repeated_self_wake_retains_target_but_unavailable_or_failed_capture_clears_it() {
        let state = AutoPasteState::default();
        state.record_capture(Ok(TargetCapture::External(target()))).unwrap();
        let original = format!("{:?}", state.target().unwrap());
        for _ in 0..3 { state.record_capture(Ok(TargetCapture::CurrentApp)).unwrap(); }
        assert_eq!(format!("{:?}", state.target().unwrap()), original);
        state.record_capture(Ok(TargetCapture::Unavailable)).unwrap();
        assert!(state.target().unwrap().is_none());
        state.record_capture(Ok(TargetCapture::External(target()))).unwrap();
        assert!(state.record_capture(Err(AppError::AutoPaste("capture failed".into()))).is_err());
        assert!(state.target().unwrap().is_none());
    }
}
