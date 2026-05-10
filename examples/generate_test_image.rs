use image::{Rgba, RgbaImage};
use std::path::PathBuf;

/// 生成用于调试分块加载的测试图片
/// 每个块显示其编号（从1开始，左上到右下）
fn generate_tiled_test_image(width: u32, height: u32, tile_size: u32, output_path: &str) {
    let mut img = RgbaImage::new(width, height);

    // 计算行列数
    let cols = width.div_ceil(tile_size);
    let rows = height.div_ceil(tile_size);

    println!("生成测试图片: {}x{}", width, height);
    println!("块大小: {}x{}", tile_size, tile_size);
    println!("总块数: {}x{} = {}", cols, rows, cols * rows);

    // 为每个块填充不同的颜色和编号
    for row in 0..rows {
        for col in 0..cols {
            let x = col * tile_size;
            let y = row * tile_size;
            let w = tile_size.min(width - x);
            let h = tile_size.min(height - y);

            // 计算块编号（从1开始）
            let tile_number = row * cols + col + 1;

            // 根据块编号生成不同的背景色
            let hue = (tile_number as f32 / (cols * rows) as f32) * 360.0;
            let (r, g, b) = hsl_to_rgb(hue / 360.0, 0.7, 0.6);

            // 填充块的背景色
            for py in y..(y + h) {
                for px in x..(x + w) {
                    img.put_pixel(px, py, Rgba([r, g, b, 255]));
                }
            }

            // 在块中心绘制编号
            draw_number(&mut img, x, y, w, h, tile_number, cols, rows);
        }
    }

    // 保存文件
    let path = PathBuf::from(output_path);
    img.save(&path).expect("Failed to save image");
    println!("测试图片已保存到: {:?}", path);
}

/// 在块中心绘制编号
fn draw_number(
    img: &mut RgbaImage,
    block_x: u32,
    block_y: u32,
    block_w: u32,
    block_h: u32,
    number: u32,
    _total_cols: u32,
    _total_rows: u32,
) {
    let number_str = format!("{}", number);

    // 使用巨型字体（每个字符200x280像素）
    let char_width = 200;
    let char_height = 280;
    let text_width = (number_str.len() as u32) * char_width;
    let text_height = char_height;

    // 计算文字位置（居中）
    let start_x = block_x + (block_w.saturating_sub(text_width)) / 2;
    let start_y = block_y + (block_h.saturating_sub(text_height)) / 2;

    // 绘制每个字符
    for (char_idx, ch) in number_str.chars().enumerate() {
        if let Some(digit) = ch.to_digit(10) {
            let char_x = start_x + (char_idx as u32) * char_width;
            draw_giant_digit(img, char_x, start_y, digit);
        }
    }
}

/// 绘制巨型数字（20x28点阵，放大10倍 = 200x280像素）
fn draw_giant_digit(img: &mut RgbaImage, x: u32, y: u32, digit: u32) {
    let font = get_giant_digit_font();
    if digit >= font.len() as u32 {
        return;
    }

    let pattern = &font[digit as usize];
    let scale = 10; // 放大10倍

    // 白色文字
    let color = Rgba([255, 255, 255, 255]);

    // 黑色描边
    let outline_color = Rgba([0, 0, 0, 255]);

    for (row_idx, row_pattern) in pattern.iter().enumerate() {
        for (col_idx, pixel) in row_pattern.iter().enumerate() {
            if *pixel == 1 {
                // 放大绘制
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + (col_idx * scale + sx) as u32;
                        let py = y + (row_idx * scale + sy) as u32;

                        if px < img.width() && py < img.height() {
                            img.put_pixel(px, py, color);
                        }
                    }
                }

                // 绘制粗描边
                for dy in -5..=5 {
                    for dx in -5..=5 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let outline_x =
                                    (x as i32 + (col_idx * scale + sx) as i32 + dx) as u32;
                                let outline_y =
                                    (y as i32 + (row_idx * scale + sy) as i32 + dy) as u32;
                                if outline_x < img.width() && outline_y < img.height() {
                                    img.put_pixel(outline_x, outline_y, outline_color);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 获取巨型数字字体（20x28点阵，将放大10倍到200x280像素）
fn get_giant_digit_font() -> Vec<Vec<Vec<u8>>> {
    vec![
        // 0
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
            vec![1, 1, 0, 1, 0, 0, 1, 0, 1, 1],
            vec![1, 1, 0, 1, 0, 0, 1, 0, 1, 1],
            vec![1, 1, 0, 1, 0, 0, 1, 0, 1, 1],
            vec![1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 1
        vec![
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 2
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 1, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 1, 1, 0, 0, 0, 0, 0],
            vec![0, 0, 1, 1, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 3
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 4
        vec![
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 1, 1, 0, 1, 1, 0, 0],
            vec![0, 0, 1, 1, 0, 0, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 1, 1, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 5
        vec![
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 1, 1, 1, 1, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 6
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 1, 1, 1, 1, 1, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 7
        vec![
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 1, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
            vec![0, 0, 0, 1, 1, 0, 0, 0, 0, 0],
            vec![0, 0, 1, 1, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 8
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
        // 9
        vec![
            vec![0, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 0, 1, 1, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
            vec![0, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
            vec![0, 0, 0, 0, 0, 0, 0, 1, 1, 0],
            vec![0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 1, 1, 0, 0, 0, 1, 1, 0, 0],
            vec![0, 0, 1, 1, 1, 1, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ],
    ]
}

/// HSL转RGB
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 1.0 / 6.0 {
        (c, x, 0.0)
    } else if h < 2.0 / 6.0 {
        (x, c, 0.0)
    } else if h < 3.0 / 6.0 {
        (0.0, c, x)
    } else if h < 4.0 / 6.0 {
        (0.0, x, c)
    } else if h < 5.0 / 6.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn main() {
    // 生成 6000x6000 的测试图片，块大小 1024
    generate_tiled_test_image(6000, 6000, 1024, "test_tiled_6000.png");

    // 也可以生成其他尺寸的测试图片
    // generate_tiled_test_image(5000, 5000, 1024, "test_tiled_5000.png");
    // generate_tiled_test_image(3000, 3000, 1024, "test_tiled_3000.png");
    // generate_tiled_test_image(8000, 6000, 1024, "test_tiled_8000x6000.png");
}
