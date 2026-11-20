//! 状态栏：底部居中悬浮，无操作后自动淡出，`S` 切换常显
//!
//! 淡出用「定时器 + 动画」两段：定时器只负责在空闲窗口到达时把目标状态
//! 切成隐藏，动画负责 180ms 的淡入淡出。空闲期间不产生动画帧，不空转。

use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Pixels, Size, div, ease_out_quint, px,
    prelude::*,
};

use super::Spike;
use super::theme::{Palette, RADIUS_PANEL};
use crate::decode::Rotation;
use crate::viewer;

/// 无操作多久后开始淡出（信息变多后给足阅读时间）
const IDLE_DELAY: Duration = Duration::from_millis(2500);
/// 淡入淡出动画时长
const FADE: Duration = Duration::from_millis(180);

impl Spike {
    /// 有交互：状态栏重新显示，并重置自动淡出计时
    pub(super) fn touch_status(&mut self, cx: &mut Context<Self>) {
        if self.status_pinned {
            return;
        }
        if self.show_status() {
            // 从隐藏变可见：需要重画一帧，动画才会开始
            cx.notify();
        }
        // 已有定时器在等：只推迟截止时间，不新建任务
        let need_timer = self.status_deadline.is_none();
        self.status_deadline = Some(Instant::now() + IDLE_DELAY);
        if need_timer {
            self.start_status_timer(cx);
        }
    }

    /// `S`：常显 / 自动淡出 切换
    pub(super) fn toggle_status_pin(&mut self, cx: &mut Context<Self>) {
        self.status_pinned = !self.status_pinned;
        self.show_status();
        if self.status_pinned {
            // 取消待淡出
            self.status_deadline = None;
        } else {
            let need_timer = self.status_deadline.is_none();
            self.status_deadline = Some(Instant::now() + IDLE_DELAY);
            if need_timer {
                self.start_status_timer(cx);
            }
        }
        self.settings.status_pinned = self.status_pinned;
        self.settings.save();
        cx.notify();
    }

    /// 显示状态栏；返回状态是否发生变化
    ///
    /// 已在显示时不重放动画，否则每次鼠标移动都会闪一下。
    fn show_status(&mut self) -> bool {
        if self.status_shown {
            return false;
        }
        self.status_shown = true;
        self.status_seq += 1;
        true
    }

    /// 等待空闲窗口到达后隐藏状态栏
    ///
    /// 同一时间只有一个任务在跑：活动只推迟截止时间，任务醒来后重新读截止
    /// 时间继续等。`status_timer_gen` 用来作废旧任务。
    fn start_status_timer(&mut self, cx: &mut Context<Self>) {
        self.status_timer_gen = self.status_timer_gen.wrapping_add(1);
        let timer_gen = self.status_timer_gen;
        cx.spawn(async move |this, cx| {
            loop {
                let Some((gen_now, deadline)) =
                    this.read_with(cx, |s, _| (s.status_timer_gen, s.status_deadline)).ok()
                else {
                    break;
                };
                if gen_now != timer_gen {
                    break;
                }
                let Some(deadline) = deadline else {
                    break;
                };
                let wait = deadline.saturating_duration_since(Instant::now());
                if !wait.is_zero() {
                    cx.background_executor().timer(wait).await;
                }
                // 醒来后复核：期间可能有新的活动把截止时间往后推
                let Some((gen_now, deadline)) =
                    this.read_with(cx, |s, _| (s.status_timer_gen, s.status_deadline)).ok()
                else {
                    break;
                };
                if gen_now != timer_gen {
                    break;
                }
                let Some(deadline) = deadline else {
                    break;
                };
                if Instant::now() < deadline {
                    continue;
                }
                // 到期：淡出
                let _ = this.update(cx, |this, cx| {
                    this.status_deadline = None;
                    if this.status_pinned {
                        return;
                    }
                    if this.status_shown {
                        this.status_shown = false;
                        this.status_seq += 1;
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    /// 状态栏：底部居中悬浮，显示序号、尺寸、缩放模式与百分比
    pub(super) fn render_status_bar(
        &self,
        image: &viewer::LoadedImage,
        view: Size<Pixels>,
        scale_factor: f32,
        palette: &Palette,
    ) -> AnyElement {
        let base = image.logical_size(scale_factor);
        let percent = self.view.percent(view, base);
        // 显隐切换时才换动画方向，平时不动
        let shown = self.status_shown;

        // 一行、最多三块：序号 / 尺寸 / 模式 / 百分比
        let mut row = div().flex().items_center().gap(px(10.0));
        if let Some(folder) = &self.folder
            && folder.files.len() > 1
        {
            row = row.child(
                div()
                    .text_color(palette.text_muted)
                    .child(format!("{} / {}", folder.index + 1, folder.files.len())),
            );
        }
        row = row.child(
            div()
                .text_color(palette.text)
                .child(format!("{} × {}", image.pixel_size.0, image.pixel_size.1)),
        );
        if image.rotation != Rotation::None {
            row = row.child(
                div()
                    .text_color(palette.text_muted)
                    .child(format!("↻ {}°", rotation_degrees(image.rotation))),
            );
        }
        if let Some(mode) = zoom_mode_label(self.view.mode) {
            row = row.child(div().text_color(palette.text_muted).child(mode));
        }
        row = row.child(
            div()
                .text_color(palette.text)
                .child(format!("{percent:.0}%")),
        );

        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .pb(px(12.0))
            .child(
                div()
                    .px(px(10.0))
                    .py(px(5.0))
                    .rounded(px(RADIUS_PANEL))
                    .bg(palette.card_bg)
                    .border_1()
                    .border_color(palette.card_border)
                    .shadow_sm()
                    .text_size(px(12.0))
                    .child(row)
                    .with_animation(
                        ("status-bar", self.status_seq),
                        Animation::new(FADE).with_easing(ease_out_quint()),
                        move |bar, t| bar.opacity(if shown { t } else { 1.0 - t }),
                    ),
            )
            .into_any_element()
    }
}

/// 只显示用户旋转角，不含已烘进像素的 EXIF 方向
fn rotation_degrees(rotation: Rotation) -> u32 {
    match rotation {
        Rotation::None => 0,
        Rotation::Cw90 => 90,
        Rotation::Cw180 => 180,
        Rotation::Cw270 => 270,
    }
}

/// 非默认缩放模式才提示，避免常驻噪音
fn zoom_mode_label(mode: viewer::ViewMode) -> Option<&'static str> {
    match mode {
        viewer::ViewMode::Original => Some("原始"),
        viewer::ViewMode::Fill => Some("填充"),
        _ => None,
    }
}
