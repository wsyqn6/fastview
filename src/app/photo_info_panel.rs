//! 照片信息浮层：文件、图像、相机与曝光字段

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{AnyElement, Context, Div, Window, div, prelude::*, px};

use super::Spike;
use super::theme::{Palette, RADIUS_ITEM, RADIUS_PANEL};
use super::widgets::{overlay, overlay_panel, panel_header};
use crate::metadata::{self, CACHE_LIMIT, CachedPhotoInfo, PhotoInfoData};
use crate::text::{Text, TextKey};

const PANEL_WIDTH: f32 = 460.0;
const LABEL_WIDTH: f32 = 60.0;
const VALUE_WIDTH: f32 = 320.0;

#[derive(Clone)]
pub(super) enum PhotoInfoLoad {
    Loading,
    Ready(Arc<PhotoInfoData>),
    Error(String),
}

pub(super) struct PhotoInfoState {
    pub path: PathBuf,
    pub load: PhotoInfoLoad,
}

impl Spike {
    pub(super) fn toggle_photo_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.photo_info.is_some() {
            self.close_photo_info(cx);
            return;
        }
        let Some(path) = self.image.as_ref().map(|image| image.path.clone()) else {
            return;
        };

        self.open_menu = None;
        self.menu_selected = None;
        if self.settings_open {
            self.close_settings(window, cx);
        }
        self.help_open = false;
        self.info = None;
        self.load_photo_info(path, cx);
    }

    pub(super) fn close_photo_info(&mut self, cx: &mut Context<Self>) {
        if self.photo_info.take().is_some() {
            cx.notify();
        }
    }

    /// 当前图片变化时刷新信息浮层；未打开则只记下路径
    pub(super) fn refresh_photo_info(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.photo_info.is_some() {
            self.load_photo_info(path, cx);
        }
    }

    fn load_photo_info(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self.photo_info_gen.wrapping_add(1);
        self.photo_info_gen = generation;

        if let Some(data) = self.cached_photo_info(&path) {
            self.photo_info = Some(PhotoInfoState {
                path,
                load: PhotoInfoLoad::Ready(data),
            });
            cx.notify();
            return;
        }

        self.photo_info = Some(PhotoInfoState {
            path: path.clone(),
            load: PhotoInfoLoad::Loading,
        });
        cx.notify();

        let collect_path = path.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { metadata::collect(&collect_path) })
                .await;
            this.update(cx, |this, cx| {
                if this.photo_info_gen != generation
                    || !this
                        .photo_info
                        .as_ref()
                        .is_some_and(|state| state.path == path)
                {
                    return;
                }
                let load = match result {
                    Ok(data) => {
                        let data = Arc::new(data);
                        if let Ok((len, modified)) = metadata::fingerprint(&path) {
                            this.remember_photo_info(path.clone(), len, modified, Arc::clone(&data));
                        }
                        PhotoInfoLoad::Ready(data)
                    }
                    Err(err) => PhotoInfoLoad::Error(err),
                };
                this.photo_info = Some(PhotoInfoState { path, load });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cached_photo_info(&mut self, path: &Path) -> Option<Arc<PhotoInfoData>> {
        let (len, modified) = metadata::fingerprint(path).ok()?;
        let matched = self
            .photo_info_cache
            .get(path)?
            .matches(len, modified);
        if !matched {
            return None;
        }
        let data = Arc::clone(&self.photo_info_cache.get(path)?.data);
        self.touch_photo_info_order(path);
        Some(data)
    }

    fn remember_photo_info(
        &mut self,
        path: PathBuf,
        len: u64,
        modified: SystemTime,
        data: Arc<PhotoInfoData>,
    ) {
        let cached = CachedPhotoInfo { len, modified, data };
        self.photo_info_cache.insert(path.clone(), cached);
        self.touch_photo_info_order(&path);
        while self.photo_info_cache.len() > CACHE_LIMIT {
            let Some(oldest) = self.photo_info_order.pop_front() else {
                break;
            };
            self.photo_info_cache.remove(&oldest);
        }
    }

    fn touch_photo_info_order(&mut self, path: &Path) {
        self.photo_info_order.retain(|item| item != path);
        self.photo_info_order.push_back(path.to_path_buf());
    }

    pub(super) fn render_photo_info_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.photo_info.as_ref()?;
        let palette = self.palette(window);
        let t = self.text();

        let body = match &state.load {
            PhotoInfoLoad::Loading => div().child(t.t(TextKey::PhotoInfoLoading)),
            PhotoInfoLoad::Error(err) => div().child(format!("{}: {}", t.t(TextKey::PhotoInfoError), err)),
            PhotoInfoLoad::Ready(data) => self.render_photo_info_data(&state.path, data, &palette, t),
        };

        let close = div()
            .id("photo-info-close")
            .text_color(palette.text_muted)
            .cursor_pointer()
            .hover(|style| style.text_color(palette.text))
            .on_click(cx.listener(|this, _, _, cx| this.close_photo_info(cx)))
            .child(t.t(TextKey::EscClose));

        let panel = overlay_panel("photo-info-panel", PANEL_WIDTH, palette)
            .max_h(px(520.0))
            .overflow_y_scroll()
            .shadow_md()
            .child(panel_header(t.t(TextKey::PhotoInfo), close, palette))
            .child(body);

        Some(overlay(panel.into_any_element(), cx, |this, _, cx| {
            this.close_photo_info(cx);
        }))
    }

    fn render_photo_info_data(
        &self,
        path: &Path,
        data: &PhotoInfoData,
        palette: &Palette,
        t: Text,
    ) -> Div {
        let image = self.image.as_ref();
        let display = image.map(|image| image.pixel_size).unwrap_or((0, 0));
        let missing = t.t(TextKey::NotRecorded);
        let camera = camera_name(data);
        let has_camera = camera.is_some()
            || data.lens.is_some()
            || data.focal_length.is_some()
            || data.taken_at.is_some();
        let has_exposure = data.iso.is_some() || data.shutter.is_some() || data.aperture.is_some();

        let mut column = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(file_header(path, palette))
            .child(metric_cards(display, data, missing, palette, t));

        if has_camera {
            column = column.child(capture_card(data, camera, missing, palette, t));
        }
        if has_exposure {
            column = column.child(exposure_card(data, missing, palette, t));
        }
        if data.exif_unavailable && !has_camera && !has_exposure {
            column = column.child(
                div()
                    .text_size(px(11.0))
                    .text_color(palette.text_muted)
                    .child(t.t(TextKey::ExifUnavailable)),
            );
        }
        column
    }
}

fn file_header(path: &Path, palette: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(palette.text)
                .child(folder_name(path)),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(palette.text_muted)
                .child(path.to_string_lossy().into_owned()),
        )
}

fn metric_cards(
    display: (u32, u32),
    data: &PhotoInfoData,
    missing: &str,
    palette: &Palette,
    t: Text,
) -> Div {
    div()
        .flex()
        .gap(px(8.0))
        .child(metric_card(
            t.t(TextKey::FieldDimensions),
            format!("{} × {}", display.0, display.1),
            palette,
        ))
        .child(metric_card(
            t.t(TextKey::FieldFileSize),
            metadata::format_size(data.size_bytes),
            palette,
        ))
        .child(metric_card(
            t.t(TextKey::FieldFormat),
            data.format.clone().unwrap_or_else(|| missing.to_string()),
            palette,
        ))
}

fn capture_card(
    data: &PhotoInfoData,
    camera: Option<String>,
    missing: &str,
    palette: &Palette,
    t: Text,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .p(px(12.0))
        .rounded(px(RADIUS_PANEL))
        .bg(palette.card_bg)
        .border_1()
        .border_color(palette.menu_border)
        .child(
            div()
                .text_size(px(10.0))
                .text_color(palette.text_muted)
                .child(t.t(TextKey::PhotoInfoCapture)),
        )
        .child(detail_row(
            t.t(TextKey::FieldCamera),
            camera,
            missing,
            palette,
        ))
        .child(detail_row(
            t.t(TextKey::FieldLens),
            data.lens.clone(),
            missing,
            palette,
        ))
        .child(detail_row(
            t.t(TextKey::FieldTakenAt),
            data.taken_at.clone(),
            missing,
            palette,
        ))
        .child(detail_row(
            t.t(TextKey::FieldFocalLength),
            data.focal_length.clone(),
            missing,
            palette,
        ))
}

fn exposure_card(data: &PhotoInfoData, missing: &str, palette: &Palette, t: Text) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .p(px(12.0))
        .rounded(px(RADIUS_PANEL))
        .bg(palette.card_bg)
        .border_1()
        .border_color(palette.menu_border)
        .child(
            div()
                .text_size(px(10.0))
                .text_color(palette.text_muted)
                .child(t.t(TextKey::GroupExposure)),
        )
        .child(
            div()
                .flex()
                .gap(px(12.0))
                .child(exposure_cell(
                    t.t(TextKey::FieldIso),
                    data.iso.map(|value| value.to_string()),
                    missing,
                    palette,
                ))
                .child(exposure_cell(
                    t.t(TextKey::FieldShutter),
                    data.shutter.clone(),
                    missing,
                    palette,
                ))
                .child(exposure_cell(
                    t.t(TextKey::FieldAperture),
                    data.aperture.clone(),
                    missing,
                    palette,
                )),
        )
}

fn metric_card(label: &str, value: String, palette: &Palette) -> Div {
    div()
        .flex_1()
        .p(px(12.0))
        .rounded(px(RADIUS_ITEM + 2.0))
        .bg(palette.card_bg)
        .border_1()
        .border_color(palette.menu_border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(palette.text_muted)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(palette.text)
                        .child(value),
                ),
        )
}

fn detail_row(label: &str, value: Option<String>, missing: &str, palette: &Palette) -> Div {
    div()
        .flex()
        .gap(px(8.0))
        .child(
            div()
                .w(px(LABEL_WIDTH))
                .text_size(px(11.0))
                .text_color(palette.text_muted)
                .child(label.to_string()),
        )
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .text_size(px(11.0))
                .text_color(palette.text)
                .child(value.unwrap_or_else(|| missing.to_string())),
        )
}

fn exposure_cell(label: &str, value: Option<String>, missing: &str, palette: &Palette) -> Div {
    div()
        .flex_1()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(palette.text_muted)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(palette.text)
                        .child(value.unwrap_or_else(|| missing.to_string())),
                ),
        )
}

fn camera_name(data: &PhotoInfoData) -> Option<String> {
    let make = data.make.as_deref().unwrap_or("");
    let model = data.model.as_deref().unwrap_or("");
    let name = if make.is_empty() || model.starts_with(make) {
        model.to_string()
    } else if model.is_empty() {
        make.to_string()
    } else {
        format!("{make} {model}")
    };
    (!name.trim().is_empty()).then(|| name.trim().to_string())
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
