use eframe::egui;
use std::sync::Arc;

/// 设置最小化字体配置 (仅 ASCII,快速启动)
pub fn setup_minimal_fonts(cc: &eframe::CreationContext<'_>) {
    // 使用 egui 默认字体,不加载中文字体
    // 这样可以立即启动,中文稍后异步加载
    let fonts = egui::FontDefinitions::default();
    cc.egui_ctx.set_fonts(fonts);
}

/// 异步加载中文字体并在完成后应用
pub fn start_async_font_loader(ctx: egui::Context) {
    std::thread::spawn(move || {
        use std::time::Instant;

        let t0 = Instant::now();
        debug_log!("[FONT] Starting async font loading...");

        // 尝试加载中文字体
        let font_data = load_chinese_font();

        match font_data {
            Ok(data) => {
                debug_log!(
                    "[FONT] Font loaded in {}ms, applying...",
                    t0.elapsed().as_millis()
                );

                // 构建完整字体配置（包含 emoji 支持）
                let mut fonts = egui::FontDefinitions::default();

                // 添加 emoji 字体支持
                #[cfg(windows)]
                {
                    let emoji_path = std::path::PathBuf::from("C:/Windows/Fonts/seguiemj.ttf");
                    if emoji_path.exists()
                        && let Ok(emoji_data) = std::fs::read(&emoji_path)
                    {
                        fonts.font_data.insert(
                            "emoji".to_owned(),
                            Arc::new(egui::FontData::from_owned(emoji_data)),
                        );
                        fonts
                            .families
                            .entry(egui::FontFamily::Proportional)
                            .or_default()
                            .insert(0, "emoji".to_owned());
                        debug_log!("[FONT] Emoji font loaded: seguiemj.ttf");
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    let emoji_path =
                        std::path::PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc");
                    if emoji_path.exists() {
                        if let Ok(emoji_data) = std::fs::read(&emoji_path) {
                            fonts.font_data.insert(
                                "emoji".to_owned(),
                                Arc::new(egui::FontData::from_owned(emoji_data)),
                            );
                            fonts
                                .families
                                .entry(egui::FontFamily::Proportional)
                                .or_default()
                                .insert(0, "emoji".to_owned());
                            debug_log!("[FONT] Emoji font loaded: Apple Color Emoji");
                        }
                    }
                }

                #[cfg(target_os = "linux")]
                {
                    let emoji_paths = &[
                        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                        "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
                    ];
                    for path in emoji_paths {
                        let emoji_path = std::path::PathBuf::from(path);
                        if emoji_path.exists()
                            && let Ok(emoji_data) = std::fs::read(&emoji_path)
                        {
                            fonts.font_data.insert(
                                "emoji".to_owned(),
                                Arc::new(egui::FontData::from_owned(emoji_data)),
                            );
                            fonts
                                .families
                                .entry(egui::FontFamily::Proportional)
                                .or_default()
                                .insert(0, "emoji".to_owned());
                            debug_log!("[FONT] Emoji font loaded: {}", path);
                            break;
                        }
                    }
                }

                // 添加中文字体
                fonts.font_data.insert(
                    "chinese".to_owned(),
                    Arc::new(egui::FontData::from_owned(data)),
                );

                // 中文字体插入到位置1，保持emoji在位置0
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(1, "chinese".to_owned());

                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("chinese".to_owned());

                // 在主线程上下文中应用字体
                ctx.set_fonts(fonts);
                ctx.request_repaint();

                debug_log!(
                    "[FONT] Font applied successfully in {}ms",
                    t0.elapsed().as_millis()
                );
            }
            Err(e) => {
                debug_log!(
                    "[WARN] Failed to load Chinese font: {}. Using ASCII only.",
                    e
                );
            }
        }
    });
}

/// 加载中文字体数据
fn load_chinese_font() -> Result<Vec<u8>, std::io::Error> {
    #[cfg(windows)]
    {
        load_windows_font("msyh")
    }

    #[cfg(not(windows))]
    {
        // 尝试加载 Noto Sans CJK 或文泉驿字体
        if let Ok(data) = load_system_font("NotoSansCJK") {
            return Ok(data);
        }
        if let Ok(data) = load_system_font("WenQuanYi") {
            return Ok(data);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No Chinese font found",
        ))
    }
}

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

    // 添加 emoji 字体支持 (使用系统自带的 Segoe UI Emoji 或 Noto Color Emoji)
    #[cfg(windows)]
    {
        // Windows: Segoe UI Emoji
        let emoji_path = std::path::PathBuf::from("C:/Windows/Fonts/seguiemj.ttf");
        if emoji_path.exists()
            && let Ok(emoji_data) = std::fs::read(&emoji_path)
        {
            fonts.font_data.insert(
                "emoji".to_owned(),
                Arc::new(egui::FontData::from_owned(emoji_data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "emoji".to_owned());
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: Apple Color Emoji
        let emoji_path = std::path::PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc");
        if emoji_path.exists() {
            if let Ok(emoji_data) = std::fs::read(&emoji_path) {
                fonts.font_data.insert(
                    "emoji".to_owned(),
                    Arc::new(egui::FontData::from_owned(emoji_data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "emoji".to_owned());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Noto Color Emoji
        let emoji_paths = &[
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
        ];
        for path in emoji_paths {
            let emoji_path = std::path::PathBuf::from(path);
            if emoji_path.exists()
                && let Ok(emoji_data) = std::fs::read(&emoji_path)
            {
                fonts.font_data.insert(
                    "emoji".to_owned(),
                    Arc::new(egui::FontData::from_owned(emoji_data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "emoji".to_owned());
                break;
            }
        }
    }

    #[cfg(windows)]
    {
        // 尝试加载微软雅黑字体
        if let Ok(font_data) = load_windows_font("msyh") {
            fonts.font_data.insert(
                "chinese".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );

            // 中文字体插入到位置1，保持emoji在位置0
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(1, "chinese".to_owned());

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
