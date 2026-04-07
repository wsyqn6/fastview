use eframe::egui;
use std::sync::Arc;

#[cfg(windows)]
fn load_windows_font(name: &str) -> Result<Vec<u8>, std::io::Error> {
    let font_path = std::path::PathBuf::from("C:/Windows/Fonts").join(format!("{}.ttf", name));
    if font_path.exists() {
        return std::fs::read(font_path);
    }

    let font_path = std::path::PathBuf::from("C:/Windows/Fonts").join(format!("{}.ttc", name));
    if font_path.exists() {
        return std::fs::read(font_path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("Font {} not found", name),
    ))
}

#[cfg(not(windows))]
fn load_system_font(name: &str) -> Result<Vec<u8>, std::io::Error> {
    // Common locations for system fonts on Unix-like systems
    let paths = &[
        "/System/Library/Fonts/",
        "/Library/Fonts/",
        "/usr/share/fonts/",
        "/usr/share/fonts/truetype/",
        "/usr/share/fonts/opentype/",
    ];

    let extensions = &[".ttf", ".ttc", ".otf"];

    for &prefix in paths {
        for &ext in extensions {
            let path = std::path::PathBuf::from(prefix).join(format!("{}{}", name, ext));
            if path.exists() {
                return std::fs::read(path);
            }
            // Try lowercase
            let path =
                std::path::PathBuf::from(prefix).join(format!("{}{}", name.to_lowercase(), ext));
            if path.exists() {
                return std::fs::read(path);
            }
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Font not found",
    ))
}

pub fn setup_fonts(cc: &eframe::CreationContext<'_>) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(windows)]
    {
        // 尝试加载微软雅黑字体
        if let Ok(font_data) = load_windows_font("msyh") {
            fonts.font_data.insert(
                "chinese".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese".to_owned());
        }
    }

    #[cfg(not(windows))]
    {
        // 尝试加载 Noto Sans CJK 或文泉驿字体
        if let Ok(font_data) = load_system_font("NotoSansCJK") {
            fonts.font_data.insert(
                "chinese".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese".to_owned());
        } else if let Ok(font_data) = load_system_font("WenQuanYi") {
            fonts.font_data.insert(
                "chinese".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese".to_owned());
        }
    }

    cc.egui_ctx.set_fonts(fonts);
}
