use desktop_app_core::records::ClipRecordService;
use serde::Serialize;
use tauri::{
    Emitter, EventTarget, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::{
    app_context::AppContext,
    errors::{AppError, AppResult},
};

pub(crate) const LABEL: &str = "image-viewer";
const IMAGE_CHANGED: &str = "image-viewer-changed";
const MAX_INITIAL_SIZE: LogicalSize<f64> = LogicalSize::new(1120.0, 800.0);
const MIN_INITIAL_SIZE: LogicalSize<f64> = LogicalSize::new(480.0, 320.0);
// 看图页工具栏的逻辑高度，见 ImageViewer.vue。
const TOOLBAR_HEIGHT: f64 = 44.0;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewerImage {
    revision: u64,
    file_path: String,
}

#[derive(Default)]
pub(crate) struct ImageViewerState {
    image: Option<ViewerImage>,
}

impl ImageViewerState {
    pub(crate) fn image(&self) -> Option<ViewerImage> {
        self.image.clone()
    }
}

/// 串行处理预览请求，让窗口复用和切图顺序与图片查询保持一致。
pub(crate) async fn open<R: Runtime>(
    main: &WebviewWindow<R>,
    context: &AppContext,
    record_id: &str,
) -> AppResult<()> {
    let mut state = context.image_viewer().lock().await;
    let path = ClipRecordService::new(context.core())
        .get_image_path(record_id)
        .await?;
    let app = main.app_handle();
    let existing = app.get_webview_window(LABEL);
    let reused = existing.is_some();
    let window = match existing {
        Some(window) => window,
        None => {
            // 只在创建窗口前读取图片尺寸，切图时保留用户调整过的窗口。
            let file_path = path.file_path.clone();
            let dimensions = tokio::task::spawn_blocking(move || read_dimensions(&file_path))
                .await
                .ok()
                .flatten();
            let window = create_window(main, dimensions).map_err(window_error)?;
            super::window::track_image_viewer_focus(
                &window,
                main,
                context.window_focus_state(),
                context.window_hide_flag(),
            );
            window
        }
    };

    let image = ViewerImage {
        revision: state
            .image
            .as_ref()
            .map_or(1, |current| current.revision + 1),
        file_path: path.file_path,
    };
    // 首次加载由页面主动读取状态，复用时只给看图窗口发送更新通知。
    state.image = Some(image.clone());
    if reused {
        window
            .emit_to(EventTarget::webview_window(LABEL), IMAGE_CHANGED, image)
            .map_err(window_error)?;
    }
    if window.is_minimized().map_err(window_error)? {
        window.unminimize().map_err(window_error)?;
    }
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)?;
    Ok(())
}

fn read_dimensions(file_path: &str) -> Option<(u32, u32)> {
    image::ImageReader::open(file_path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn initial_size(
    dimensions: Option<(u32, u32)>,
    scale_factor: f64,
    work_area: Option<LogicalSize<f64>>,
) -> LogicalSize<f64> {
    let (max_width, max_height) =
        work_area.map_or((MAX_INITIAL_SIZE.width, MAX_INITIAL_SIZE.height), |area| {
            (
                MAX_INITIAL_SIZE.width.min(area.width * 0.8),
                MAX_INITIAL_SIZE.height.min(area.height * 0.8),
            )
        });
    let (width, height) = match dimensions {
        Some((width, height)) if width > 0 && height > 0 => {
            // 按设备像素换算，正常尺寸的图片在高 DPI 屏幕上也能以原始像素显示。
            let width = f64::from(width) / scale_factor;
            let height = f64::from(height) / scale_factor;
            let fit = (max_width / width)
                .min((max_height - TOOLBAR_HEIGHT).max(0.0) / height)
                .min(1.0);
            (width * fit, height * fit + TOOLBAR_HEIGHT)
        }
        _ => (840.0, 580.0),
    };
    LogicalSize::new(
        width.max(MIN_INITIAL_SIZE.width).min(max_width),
        height.max(MIN_INITIAL_SIZE.height).min(max_height),
    )
}

fn create_window<R: Runtime>(
    main: &WebviewWindow<R>,
    dimensions: Option<(u32, u32)>,
) -> tauri::Result<WebviewWindow<R>> {
    let monitor = main
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| main.primary_monitor().ok().flatten());
    let scale_factor = monitor.as_ref().map_or_else(
        || main.scale_factor().unwrap_or(1.0),
        |monitor| monitor.scale_factor(),
    );
    let work_area = monitor
        .as_ref()
        .map(|monitor| monitor.work_area().size.to_logical(scale_factor));
    let size = initial_size(dimensions, scale_factor, work_area);
    let mut builder = WebviewWindowBuilder::new(
        main.app_handle(),
        LABEL,
        WebviewUrl::App("image-viewer.html".into()),
    )
    .title("ClipPal 图片预览")
    // 看图页在 CSS 中设置深色。Windows 的 WebView2 主题会影响同一配置中的其他窗口。
    .background_color(tauri::webview::Color(25, 27, 30, 255))
    .inner_size(size.width, size.height)
    .min_inner_size(360.0_f64.min(size.width), 240.0_f64.min(size.height))
    .visible(false)
    .focused(false);

    #[cfg(target_os = "macos")]
    {
        // 内容延伸到标题栏，保留系统窗口按钮和原生全屏。
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }

    if monitor.is_none() {
        builder = builder.center();
    }
    let window = builder.build()?;
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let target = size.to_physical::<u32>(scale_factor);
        let center = |width, height| {
            PhysicalPosition::new(
                area.position.x + (area.size.width.saturating_sub(width) / 2) as i32,
                area.position.y + (area.size.height.saturating_sub(height) / 2) as i32,
            )
        };
        // 先移到目标屏幕再设置物理尺寸，最后按含系统边框的尺寸居中。
        let place = || -> tauri::Result<()> {
            window.set_position(center(target.width, target.height))?;
            window.set_size(target)?;
            let outer = window.outer_size().unwrap_or(target);
            window.set_position(center(outer.width, outer.height))
        };
        if let Err(error) = place() {
            log::warn!("调整图片预览窗口位置或大小失败: {error}");
        }
    }
    Ok(window)
}

fn window_error(error: tauri::Error) -> AppError {
    AppError::Window(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_size_uses_image_orientation_and_device_pixels() {
        let area = Some(LogicalSize::new(1920.0, 1080.0));
        assert_eq!(initial_size(Some((64, 64)), 1.0, area), MIN_INITIAL_SIZE);
        assert_eq!(
            initial_size(Some((1600, 1000)), 2.0, area),
            LogicalSize::new(800.0, 544.0)
        );
        assert_eq!(
            initial_size(Some((3840, 2160)), 1.0, area),
            LogicalSize::new(1120.0, 674.0)
        );
        assert_eq!(
            initial_size(Some((600, 3000)), 1.0, area),
            LogicalSize::new(480.0, 800.0)
        );
    }

    #[test]
    fn initial_size_fits_small_work_area_and_falls_back_for_missing_dimensions() {
        let area = Some(LogicalSize::new(500.0, 400.0));
        assert_eq!(
            initial_size(None, 1.0, area),
            LogicalSize::new(400.0, 320.0)
        );
        assert_eq!(
            initial_size(Some((4000, 4000)), 1.0, area),
            LogicalSize::new(400.0, 320.0)
        );
        assert_eq!(
            initial_size(Some((0, 100)), 1.0, None),
            LogicalSize::new(840.0, 580.0)
        );
    }
}
