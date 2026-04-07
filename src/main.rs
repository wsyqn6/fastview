mod app;
mod fonts;
mod types;

use app::FastViewApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([400.0, 300.0])
            .with_decorations(true) // 使用系统原生窗口装饰
            .with_fullscreen(false),
        ..Default::default()
    };

    eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| Ok(Box::new(FastViewApp::new(cc)))),
    )
}
