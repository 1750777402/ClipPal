use crate::{app_context::AppContext, errors::{AppError, AppResult}};
use crate::desktop::shell::shortcut::{parse_shortcut_strict, register_shortcut_handler};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// 设置相关系统能力适配器，统一封装快捷键、自启动、权限和同步触发操作。
pub struct SettingsSystem<'a> { context: &'a AppContext }

impl<'a> SettingsSystem<'a> {
    /// 绑定应用上下文，后续系统操作从中取得 AppHandle 和运行状态。
    pub fn new(context: &'a AppContext) -> Self { Self { context } }

    /// 使用系统快捷键解析器验证完整键位，成功时丢弃解析对象。
    pub fn parse_shortcut(&self, shortcut: &str) -> Result<(), String> {
        parse_shortcut_strict(shortcut).map(|_| ())
    }

    /// 返回实际注册的快捷键，不能用配置文件推断注册是否成功。
    pub fn registered_shortcut(&self) -> AppResult<Option<String>> {
        self.cleanup_shortcut_probe()?;
        let actual = self.context.registered_shortcut()?;
        let app = self.context.app_handle()?;
        Ok(actual.filter(|value| parse_shortcut_strict(value).is_ok_and(|key| app.global_shortcut().is_registered(key))))
    }

    /// 先注册新快捷键，成功后再释放旧快捷键；失败时保留旧入口。
    /// next 为空时用于恢复此前没有快捷键的运行状态。
    pub fn update_shortcut(&self, next: Option<&str>) -> AppResult<()> {
        let app = self.context.app_handle()?;
        let previous = self.registered_shortcut()?.as_deref().map(parse_shortcut_strict).transpose().map_err(AppError::GlobalShortcut)?;
        let next_key = next.map(parse_shortcut_strict).transpose().map_err(AppError::GlobalShortcut)?;
        let context = crate::app_context::app_context()?;
        replace_registration(previous, next_key,
            |shortcut| app.global_shortcut().is_registered(shortcut),
            |shortcut| register_shortcut_handler(&app, context.clone(), shortcut),
            |shortcut| app.global_shortcut().unregister(shortcut).map_err(|error| error.to_string()),
        ).map_err(AppError::GlobalShortcut)?;
        self.context.set_registered_shortcut(next.map(str::to_owned))
    }

    /// 真正尝试系统注册以判断占用；测试结束释放候选键，不影响原快捷键。
    pub fn validate_shortcut(&self, value: &str) -> AppResult<bool> {
        let key = parse_shortcut_strict(value).map_err(AppError::Config)?;
        self.cleanup_shortcut_probe()?;
        let app = self.context.app_handle()?;
        probe_registration(key,
            |key| app.global_shortcut().is_registered(key),
            |key| {
                app.global_shortcut().register(key).map_err(|error| error.to_string())?;
                self.context.set_pending_shortcut_probe(Some(key)).map_err(String::from)
            },
            |key| {
                app.global_shortcut().unregister(key).map_err(|error| error.to_string())?;
                self.context.set_pending_shortcut_probe(None).map_err(String::from)
            },
        ).map_err(AppError::GlobalShortcut)
    }

    /// 清理失败的候选检查在下次加载、校验或保存前重试，避免误当成有效唤起键。
    fn cleanup_shortcut_probe(&self) -> AppResult<()> {
        if let Some(key) = self.context.pending_shortcut_probe()? {
            self.context.app_handle()?.global_shortcut().unregister(key)
                .map_err(|error| AppError::GlobalShortcut(error.to_string()))?;
            self.context.set_pending_shortcut_probe(None)?;
        }
        Ok(())
    }

    /// 读取系统实际自启状态，系统设置被外部修改后也能正确显示。
    pub fn is_auto_start_enabled(&self) -> AppResult<bool> {
        self.context.app_handle()?.autolaunch().is_enabled()
            .map_err(|error| AppError::System(format!("读取开机自启动状态失败: {}", error)))
    }

    /// 通过系统自启动插件开启或关闭开机启动，并核对系统结果。
    pub fn set_auto_start(&self, enabled: bool) -> AppResult<()> {
        let app_handle = self.context.app_handle()?;
        let manager = app_handle.autolaunch();
        (if enabled { manager.enable() } else { manager.disable() })
            .map_err(|error| AppError::System(format!("开机自启动设置失败: {}", error)))?;
        if self.is_auto_start_enabled()? != enabled {
            return Err(AppError::System("开机自启动设置后系统状态未生效".into()));
        }
        Ok(())
    }
}

/// 系统调用通过闭包注入，失败路径无需操作用户的实际快捷键即可验证。
fn replace_registration(
    previous: Option<Shortcut>, next: Option<Shortcut>,
    is_registered: impl Fn(Shortcut) -> bool,
    register: impl Fn(Shortcut) -> Result<(), String>,
    unregister: impl Fn(Shortcut) -> Result<(), String>,
) -> Result<(), String> {
    if previous == next && next.is_none_or(&is_registered) { return Ok(()); }
    if let Some(next) = next { register(next)?; }
    if let Some(previous) = previous.filter(|key| Some(*key) != next && is_registered(*key)) {
        if let Err(error) = unregister(previous) {
            if let Some(next) = next {
                if let Err(rollback_error) = unregister(next) { log::error!("回滚新快捷键失败: {}", rollback_error); }
            }
            return Err(error);
        }
    }
    Ok(())
}

fn probe_registration(
    key: Shortcut,
    is_registered: impl Fn(Shortcut) -> bool,
    register: impl Fn(Shortcut) -> Result<(), String>,
    unregister: impl Fn(Shortcut) -> Result<(), String>,
) -> Result<bool, String> {
    if is_registered(key) { return Ok(true); }
    if let Err(error) = register(key) {
        log::warn!("候选快捷键注册失败: {}", error);
        return Ok(false);
    }
    unregister(key)?;
    Ok(true)
}

/// 系统状态与文件配置作为一次事务应用，失败按实际旧状态逆序恢复。
/// 自启调用即使返回错误也可能已修改系统，因此仍要尝试恢复。
pub(crate) fn apply_settings_transaction(
    previous_shortcut: Option<&str>, next_shortcut: &str,
    previous_autostart: bool, next_autostart: bool,
    shortcut: impl Fn(Option<&str>) -> AppResult<()>,
    autostart: impl Fn(bool) -> AppResult<()>,
    persist: impl FnOnce() -> AppResult<()>,
) -> AppResult<()> {
    shortcut(Some(next_shortcut))?;
    let result = (|| {
        if previous_autostart != next_autostart { autostart(next_autostart)?; }
        persist()
    })();
    if result.is_err() {
        if previous_autostart != next_autostart {
            if let Err(error) = autostart(previous_autostart) { log::error!("回滚自启动设置失败: {}", error); }
        }
        if let Err(error) = shortcut(previous_shortcut) { log::error!("回滚快捷键失败: {}", error); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    fn keys() -> (Shortcut, Shortcut) {
        (parse_shortcut_strict("Ctrl+A").unwrap(), parse_shortcut_strict("Ctrl+B").unwrap())
    }
    #[test]
    fn failed_new_registration_keeps_previous_shortcut() {
        let (old, new) = keys();
        let removed = RefCell::new(Vec::new());
        assert!(replace_registration(Some(old), Some(new), |_| true, |_| Err("occupied".into()), |key| { removed.borrow_mut().push(key); Ok(()) }).is_err());
        assert!(removed.borrow().is_empty());
    }
    #[test]
    fn failed_old_removal_unregisters_new_and_retains_previous() {
        let (old, new) = keys();
        let active = RefCell::new(vec![old]);
        assert!(replace_registration(Some(old), Some(new), |key| active.borrow().contains(&key), |key| { active.borrow_mut().push(key); Ok(()) }, |key| {
            if key == old { return Err("test failure".into()); }
            active.borrow_mut().retain(|value| *value != key); Ok(())
        }).is_err());
        assert_eq!(*active.borrow(), vec![old]);
    }
    #[test]
    fn successful_replacement_can_restore_previous_after_persistence_failure() {
        let (old, new) = keys();
        for initial in [Some(old), None] {
            let active = RefCell::new(initial.into_iter().collect::<Vec<_>>());
            for (from, to) in [(initial, Some(new)), (Some(new), initial)] {
                replace_registration(from, to, |key| active.borrow().contains(&key), |key| { active.borrow_mut().push(key); Ok(()) }, |key| { active.borrow_mut().retain(|value| *value != key); Ok(()) }).unwrap();
                assert_eq!(*active.borrow(), to.into_iter().collect::<Vec<_>>());
            }
        }
    }
    #[test]
    fn saving_same_key_repairs_missing_registration() {
        let (key, _) = keys();
        let active = RefCell::new(Vec::new());
        replace_registration(Some(key), Some(key), |key| active.borrow().contains(&key), |key| { active.borrow_mut().push(key); Ok(()) }, |_| panic!("must not unregister repaired key")).unwrap();
        assert_eq!(*active.borrow(), vec![key]);
    }
    #[test]
    fn candidate_probe_never_removes_existing_key_and_releases_new_key() {
        let (old, candidate) = keys();
        let active = RefCell::new(vec![old]);
        for key in [old, candidate] {
            assert!(probe_registration(key, |key| active.borrow().contains(&key), |key| { active.borrow_mut().push(key); Ok(()) }, |key| { active.borrow_mut().retain(|value| *value != key); Ok(()) }).unwrap());
            assert_eq!(*active.borrow(), vec![old]);
        }
        assert!(!probe_registration(candidate, |_| false, |_| Err("occupied".into()), |_| panic!("not registered")).unwrap());
        assert!(probe_registration(candidate, |_| false, |_| Ok(()), |_| Err("cleanup failed".into())).is_err());
    }
    #[test]
    fn failed_persistence_restores_actual_os_state_even_if_configuration_differs() {
        let key = RefCell::new(None::<String>);
        let enabled = RefCell::new(false);
        assert!(apply_settings_transaction(None, "Ctrl+B", false, true,
            |value| { *key.borrow_mut() = value.map(str::to_owned); Ok(()) },
            |value| { *enabled.borrow_mut() = value; Ok(()) },
            || Err(AppError::Config("disk full".into()))).is_err());
        assert_eq!(*key.borrow(), None);
        assert!(!*enabled.borrow());
    }
    #[test]
    fn failed_autostart_after_partial_os_change_rolls_back_and_does_not_persist() {
        let key = RefCell::new(Some("Ctrl+A".to_owned()));
        let enabled = RefCell::new(false);
        assert!(apply_settings_transaction(Some("Ctrl+A"), "Ctrl+B", false, true,
            |value| { *key.borrow_mut() = value.map(str::to_owned); Ok(()) },
            |value| { *enabled.borrow_mut() = value; if value { Err(AppError::System("verification failed".into())) } else { Ok(()) } },
            || panic!("must not persist failed system settings")).is_err());
        assert_eq!(key.borrow().as_deref(), Some("Ctrl+A"));
        assert!(!*enabled.borrow());
    }
}
