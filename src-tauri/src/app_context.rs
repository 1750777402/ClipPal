use std::sync::{Arc, RwLock};

use rbatis::RBatis;

use crate::{
    biz::{
        clip_async_queue::AsyncQueue,
        clip_record::ClipRecord,
        system_setting::{load_settings_context, Settings},
    },
    utils::lock_utils::{create_global_sync_lock, GlobalSyncLock},
    window::{WindowFocusCount, WindowHideFlag},
};

/// 应用运行期共享上下文。
///
/// 这里只放底层运行资源，不放业务 service。
/// 目标是逐步替代隐式的全局 CONTEXT，让 command、后台任务和系统模块
/// 能通过显式依赖获取数据库、设置、队列、窗口状态等对象。
pub struct AppContext {
    /// SQLite / RBatis 连接实例。
    pub db: RBatis,

    /// 系统设置缓存。
    pub settings: Arc<RwLock<Settings>>,

    /// 云同步全局互斥锁。
    sync_lock: GlobalSyncLock,

    /// 剪贴记录同步队列。
    clip_record_queue: AsyncQueue<ClipRecord>,

    /// 主窗口焦点计数。
    window_focus_count: WindowFocusCount,

    /// 主窗口是否允许隐藏的标记。
    window_hide_flag: WindowHideFlag,
}

impl AppContext {
    fn new(
        db: RBatis,
        settings: Arc<RwLock<Settings>>,
        sync_lock: GlobalSyncLock,
        clip_record_queue: AsyncQueue<ClipRecord>,
        window_focus_count: WindowFocusCount,
        window_hide_flag: WindowHideFlag,
    ) -> Self {
        Self {
            db,
            settings,
            sync_lock,
            clip_record_queue,
            window_focus_count,
            window_hide_flag,
        }
    }

    pub fn sync_lock(&self) -> &GlobalSyncLock {
        &self.sync_lock
    }

    pub fn clip_record_queue(&self) -> AsyncQueue<ClipRecord> {
        self.clip_record_queue.clone()
    }

    pub fn window_focus_count(&self) -> &WindowFocusCount {
        &self.window_focus_count
    }

    pub fn window_hide_flag(&self) -> &WindowHideFlag {
        &self.window_hide_flag
    }
}

/// AppContext 创建工厂。
///
/// AppContext 本身只持有资源，不负责任务启动和生命周期编排。
pub struct AppContextConfig;

impl AppContextConfig {
    pub fn create(db: RBatis) -> AppContext {
        Self::create_with_settings(db, load_settings_context())
    }

    pub fn create_with_settings(db: RBatis, settings: Arc<RwLock<Settings>>) -> AppContext {
        AppContext::new(
            db,
            settings,
            create_global_sync_lock(),
            AsyncQueue::new(1000),
            WindowFocusCount::default(),
            WindowHideFlag::default(),
        )
    }
}
