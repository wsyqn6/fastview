//! 从 `design/logo/f-solid.svg` 生成多尺寸 Windows 图标。
//! 运行：`cargo run --example generate_logo`

use std::fs::{self, File};

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, RgbaImage};
use resvg::{tiny_skia, usvg};

const SIZES: [u32; 8] = [16, 20, 24, 32, 48, 64, 128, 256];

fn main() {
    let source = fs::read_to_string("design/logo/f-solid.svg").expect("read logo svg");
    fs::create_dir_all("assets/logo").expect("create assets/logo");
    let file = File::create("assets/logo/fastview.ico").expect("create ico");
    let frames: Vec<_> = SIZES
        .iter()
        .map(|&size| {
            let image = render(&source, size);
            IcoFrame::as_png(image.as_raw(), size, size, ExtendedColorType::Rgba8)
                .expect("encode ico frame")
        })
        .collect();
    IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("write ico");
}

/// 渲染 SVG 为直 alpha RGBA；tiny-skia 内部是预乘 alpha。
fn render(svg: &str, size: u32) -> RgbaImage {
    let tree = usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default()).expect("parse svg");
    let scale = size as f32 / tree.size().width();
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let mut data = pixmap.data().to_vec();
    for pixel in data.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as u32;
        if alpha == 0 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
        } else if alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    RgbaImage::from_raw(size, size, data).expect("image")
}
