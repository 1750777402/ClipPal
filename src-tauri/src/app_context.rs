use std::sync::{Arc, OnceLock, RwLock};

use rbatis::RBatis;
use tauri::{AppHandle, WebviewWindow};

use crate::{
    biz::{clip_async_queue::AsyncQueue, clip_record::ClipRecord, system_setting::Settings},
    errors::{AppError, AppResult},
    utils::lock_utils::{
        create_global_sync_lock,
        lock_utils::{safe_read_lock, safe_write_lock},
        GlobalSyncLock,
    },
    window::{WindowFocusCount, WindowHideFlag},
};

const DEFAULT_CLIP_RECORD_QUEUE_CAPACITY: usize = 1000;

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
/// - 用户操作后才会变化的状态，用 `StateSlot<T>`；
/// - 某个领域自己的底层资源，按领域拆到独立的子 Context。
///
/// 当前项目还保留旧的全局 `CONTEXT` 作为兼容层。
/// 新代码优先使用 `AppContext`，旧代码可以逐步迁移。
pub struct AppContext {
    /// 应用启动阶段必须完成初始化的核心资源。
    core: CoreContext,
    /// Tauri setup / runtime 阶段才能拿到的运行期资源。
    runtime: RuntimeContext,
    /// 剪贴板模块使用的底层运行资源。
    clipboard: ClipboardContext,
    /// 云同步模块使用的底层运行资源。
    sync: SyncContext,
    /// 主窗口相关的运行状态。
    window: WindowContext,
}

/// 只初始化一次的运行期资源。
///
/// 适合 AppHandle、主窗口这类 setup 阶段才能拿到，但设置后不应该再替换的对象。
///
/// 它和 `StateSlot<T>` 的区别：
/// - `InitCell<T>` 表示“晚一点初始化，但初始化后不可变”；
/// - `StateSlot<T>` 表示“运行中可以反复设置、清空、更新”。
///
/// 这里使用标准库 `OnceLock`，可以避免误把只应初始化一次的资源覆盖掉。
#[allow(dead_code)]
pub struct InitCell<T> {
    /// 资源名称，用于构造更明确的错误信息。
    name: &'static str,
    /// 实际保存的资源。
    value: OnceLock<T>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

/// 可反复更新的运行期状态。
///
/// 适合登录用户、VIP 状态、当前同步任务等“启动时可以为空，运行中会变化”的数据。
///
/// 示例：
/// - 用户未登录时，`current_user` 是 `None`；
/// - 登录成功后，调用 `set(user)`；
/// - 退出登录后，调用 `clear()`。
///
/// 这个类型内部使用 `RwLock<Option<T>>`：
/// - `RwLock` 允许多个读者并发读取；
/// - `Option<T>` 表达“这个状态可能还没有值”。
#[allow(dead_code)]
pub struct StateSlot<T> {
    /// 状态名称，用于构造错误信息。
    name: &'static str,
    /// 可选状态值。
    value: RwLock<Option<T>>,
}

#[allow(dead_code)]
impl<T> StateSlot<T> {
    /// 创建一个初始为空的状态槽。
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            value: RwLock::new(None),
        }
    }

    /// 设置或覆盖当前状态。
    ///
    /// 适合登录、刷新 VIP 状态、任务切换等场景。
    pub fn set(&self, value: T) -> AppResult<()> {
        *safe_write_lock(&self.value)? = Some(value);
        Ok(())
    }

    /// 清空当前状态。
    ///
    /// 适合退出登录、任务结束、资源失效等场景。
    pub fn clear(&self) -> AppResult<()> {
        *safe_write_lock(&self.value)? = None;
        Ok(())
    }

    /// 在写锁保护下原地更新状态。
    ///
    /// 当需要基于旧值决定新值时使用，避免调用方先读再写导致状态竞争。
    pub fn update<R>(&self, f: impl FnOnce(&mut Option<T>) -> R) -> AppResult<R> {
        let mut value = safe_write_lock(&self.value)?;
        Ok(f(&mut value))
    }
}

#[allow(dead_code)]
impl<T: Clone> StateSlot<T> {
    /// 获取当前状态值。
    ///
    /// 当调用方要求状态必须存在时使用。
    /// 如果当前是 `None`，会返回带状态名称的错误。
    pub fn get(&self) -> AppResult<T> {
        safe_read_lock(&self.value)?
            .clone()
            .ok_or_else(|| AppError::General(format!("{} 尚未设置", self.name)))
    }

    /// 尝试获取当前状态值。
    ///
    /// 当状态为空是正常情况时使用，比如判断用户是否登录。
    pub fn try_get(&self) -> AppResult<Option<T>> {
        Ok(safe_read_lock(&self.value)?.clone())
    }
}

/// 应用核心资源。
///
/// 这里放应用启动时必须初始化完成的资源。
/// 如果这些资源缺失，应用通常无法正常运行。
#[allow(dead_code)]
pub struct CoreContext {
    /// SQLite / RBatis 连接实例。
    ///
    /// 这是后端业务数据访问的基础依赖。
    db: RBatis,

    /// 系统设置缓存。
    ///
    /// 使用 `Arc<RwLock<Settings>>` 是为了让多个模块共享同一份设置：
    /// 读取配置时拿读锁，保存配置时拿写锁。
    settings: Arc<RwLock<Settings>>,
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
}

impl Default for RuntimeContext {
    fn default() -> Self {
        Self {
            app_handle: InitCell::new("AppHandle"),
        }
    }
}

/// 剪贴板相关运行资源。
pub struct ClipboardContext {
    /// 剪贴记录同步队列。
    ///
    /// 剪贴板记录发生新增、删除等事件后，会进入这个队列，
    /// 再由后台消费任务执行云同步等后续处理。
    record_queue: AsyncQueue<ClipRecord>,
}

impl Default for ClipboardContext {
    fn default() -> Self {
        Self {
            record_queue: AsyncQueue::new(DEFAULT_CLIP_RECORD_QUEUE_CAPACITY),
        }
    }
}

/// 云同步相关运行资源。
pub struct SyncContext {
    /// 云同步全局互斥锁。
    ///
    /// 用来避免多个同步任务同时执行。
    /// 比如定时同步、手动触发同步、队列消费同步之间应该共享同一把锁。
    lock: GlobalSyncLock,
}

impl Default for SyncContext {
    fn default() -> Self {
        Self {
            lock: create_global_sync_lock(),
        }
    }
}

/// 窗口相关运行状态。
///
/// 这里只保存窗口行为需要共享的底层状态，不放具体窗口业务逻辑。
pub struct WindowContext {
    /// 主窗口焦点计数。
    ///
    /// 用来判断窗口失焦后的隐藏策略，避免启动或切换窗口时误隐藏。
    focus_count: Arc<WindowFocusCount>,

    /// 主窗口是否允许隐藏的标记。
    ///
    /// 例如打开文件保存对话框时，需要临时禁止主窗口因失焦而隐藏。
    hide_flag: Arc<WindowHideFlag>,

    /// 主窗口实例，setup 初始化主窗口后设置。
    ///
    /// 主窗口在 Tauri 窗口系统准备好后才能获取，所以用 `InitCell`。
    main_window: InitCell<WebviewWindow>,
}

impl Default for WindowContext {
    fn default() -> Self {
        Self {
            focus_count: Arc::new(WindowFocusCount::default()),
            hide_flag: Arc::new(WindowHideFlag::default()),
            main_window: InitCell::new("主窗口"),
        }
    }
}

#[allow(dead_code)]
impl AppContext {
    /// 创建一个新的应用上下文。
    ///
    /// 这个构造函数适合完全使用 `AppContext` 管理资源的新代码路径。
    /// 它会自己创建同步锁、剪贴板队列、窗口状态等运行资源。
    pub fn new(db: RBatis, settings: Arc<RwLock<Settings>>) -> Self {
        Self {
            core: CoreContext { db, settings },
            runtime: RuntimeContext::default(),
            clipboard: ClipboardContext::default(),
            sync: SyncContext::default(),
            window: WindowContext::default(),
        }
    }

    /// 使用外部已经创建好的底层资源创建应用上下文。
    ///
    /// 当前项目还在从旧的全局 `CONTEXT` 迁移到 `AppContext`。
    /// 为了保证迁移期间只有一份 `settings`、`sync_lock` 和 `record_queue`，
    /// `lib.rs` 会先创建这些资源，再同时交给旧 `CONTEXT` 和新的 `AppContext`。
    ///
    /// 等旧 `CONTEXT` 完全移除后，可以优先使用 `new`。
    pub fn from_existing_parts(
        db: RBatis,
        settings: Arc<RwLock<Settings>>,
        sync_lock: GlobalSyncLock,
        record_queue: AsyncQueue<ClipRecord>,
    ) -> Self {
        Self {
            core: CoreContext { db, settings },
            runtime: RuntimeContext::default(),
            clipboard: ClipboardContext { record_queue },
            sync: SyncContext { lock: sync_lock },
            window: WindowContext::default(),
        }
    }

    /// 获取核心资源分组。
    ///
    /// 通常业务代码更推荐直接使用 `db()`、`settings()` 等更明确的方法。
    pub fn core(&self) -> &CoreContext {
        &self.core
    }

    /// 获取 Tauri 运行期资源分组。
    pub fn runtime(&self) -> &RuntimeContext {
        &self.runtime
    }

    /// 获取剪贴板资源分组。
    pub fn clipboard(&self) -> &ClipboardContext {
        &self.clipboard
    }

    /// 获取同步资源分组。
    pub fn sync(&self) -> &SyncContext {
        &self.sync
    }

    /// 获取窗口状态分组。
    pub fn window(&self) -> &WindowContext {
        &self.window
    }

    /// 获取数据库连接。
    ///
    /// 返回引用，避免不必要地克隆 RBatis。
    pub fn db(&self) -> &RBatis {
        &self.core.db
    }

    /// 获取系统设置缓存。
    ///
    /// 返回 `Arc` 的 clone，调用方可以自己决定何时读写锁。
    /// 如果只是简单读取或更新，优先使用 `with_settings` / `update_settings`。
    pub fn settings(&self) -> Arc<RwLock<Settings>> {
        self.core.settings.clone()
    }

    /// 在读锁保护下读取系统设置。
    ///
    /// 适合只需要从 `Settings` 中取一个值或计算一个结果的场景。
    /// 闭包结束后读锁会立即释放。
    pub fn with_settings<R>(&self, f: impl FnOnce(&Settings) -> R) -> AppResult<R> {
        let settings = safe_read_lock(&self.core.settings)?;
        Ok(f(&settings))
    }

    /// 在写锁保护下更新系统设置缓存。
    ///
    /// 只负责更新内存中的设置，不负责保存到文件。
    /// 需要持久化时仍应调用对应的设置保存逻辑。
    pub fn update_settings<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> AppResult<R> {
        let mut settings = safe_write_lock(&self.core.settings)?;
        Ok(f(&mut settings))
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

    /// 尝试获取主窗口句柄。
    ///
    /// 适合启动早期或窗口可能不存在的容错逻辑。
    pub fn try_main_window(&self) -> Option<WebviewWindow> {
        self.window.main_window.try_get_cloned()
    }

    /// 获取云同步全局互斥锁。
    pub fn sync_lock(&self) -> &GlobalSyncLock {
        &self.sync.lock
    }

    /// 获取剪贴记录同步队列。
    ///
    /// `AsyncQueue` 内部使用 `Arc` 包装发送端和接收端，
    /// 因此这里返回 clone 即可共享同一个队列。
    pub fn clip_record_queue(&self) -> AsyncQueue<ClipRecord> {
        self.clipboard.record_queue.clone()
    }

    /// 获取主窗口焦点计数器。
    ///
    /// 返回 `Arc`，保证旧 `CONTEXT` 兼容层和新 `AppContext` 使用同一份状态。
    pub fn window_focus_count(&self) -> Arc<WindowFocusCount> {
        self.window.focus_count.clone()
    }

    /// 获取主窗口隐藏控制标记。
    ///
    /// 返回 `Arc`，方便在对话框、自动粘贴等跨作用域场景中共享。
    pub fn window_hide_flag(&self) -> Arc<WindowHideFlag> {
        self.window.hide_flag.clone()
    }
}
