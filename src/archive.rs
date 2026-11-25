//! zip 压缩包：把包内图片解压到临时目录，复用磁盘路径的解码与缓存链路。

use std::fmt::Display;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn failed(error: impl Display) -> ExtractError {
    ExtractError::Failed(error.to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtractError {
    NoImages,
    Failed(String),
}

/// 一个已解压的压缩包；离开作用域时清理临时目录。
pub struct Archive {
    root: PathBuf,
    /// 原始 zip 路径：重复打开同一个包时直接复用已解压结果
    pub source: PathBuf,
    /// 自然序第一张图片，打开压缩包时用它作为入口
    pub first: PathBuf,
}

impl Archive {
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 当前路径是否属于这个包（包内翻页时不能提前清理）
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

impl Drop for Archive {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 解压包内图片；非图片条目忽略，加密或损坏的条目直接报错。
pub fn extract_images(source: &Path) -> Result<Archive, ExtractError> {
    let file = File::open(source).map_err(failed)?;
    let mut zip = zip::ZipArchive::new(file).map_err(failed)?;
    let root = std::env::temp_dir().join(format!(
        "fastview-zip-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).map_err(failed)?;

    let result = extract_entries(&mut zip, &root, source);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result
}

/// 临时根目录内取不冲突的文件名；重名用 `_2`、`_3` 递增
fn unique_path(root: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let candidate = root.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or(name).to_string_lossy();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned());
    let mut suffix = 2u32;
    loop {
        let file_name = match &extension {
            Some(ext) => format!("{stem}_{suffix}.{ext}"),
            None => format!("{stem}_{suffix}"),
        };
        let candidate = root.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
        suffix += 1;
    }
}

/// 逐条写入临时目录；`enclosed_name` 拦掉 `..` 路径穿越。
fn extract_entries(
    zip: &mut zip::ZipArchive<File>,
    root: &Path,
    source: &Path,
) -> Result<Archive, ExtractError> {
    let mut images = Vec::new();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(failed)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if !crate::folder::is_image(&name) {
            continue;
        }
        let Some(file_name) = name.file_name() else {
            continue;
        };
        let target = unique_path(root, file_name);
        io::copy(&mut entry, &mut File::create(&target).map_err(failed)?).map_err(failed)?;
        images.push(target);
    }

    if images.is_empty() {
        return Err(ExtractError::NoImages);
    }
    images.sort_by(|a, b| {
        crate::folder::natural_cmp(&crate::folder::file_name(a), &crate::folder::file_name(b))
    });
    Ok(Archive {
        root: root.to_path_buf(),
        source: source.to_path_buf(),
        first: images.remove(0),
    })
}

#[cfg(test)]
mod tests {
    use super::{ExtractError, extract_images};
    use crate::test_util::TempDir;
    use std::io::Write;
    use std::path::Path;

    fn make_zip(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let path = dir.join("album.zip");
        let file = std::fs::File::create(&path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            zip.start_file(*name, options).expect("start entry");
            zip.write_all(data).expect("write entry");
        }
        zip.finish().expect("finish zip");
        path
    }

    #[test]
    fn extract_images_picks_natural_first_image_and_skips_other_files() {
        let dir = TempDir::new("zip");
        let zip_path = make_zip(
            dir.path(),
            &[
                ("readme.txt", b"ignore"),
                ("photos/img_10.jpg", b"ten"),
                ("photos/img_2.jpg", b"two"),
            ],
        );

        let archive = extract_images(&zip_path).expect("extract images");
        assert_eq!(
            archive.first.file_name().unwrap().to_string_lossy(),
            "img_2.jpg"
        );
        assert_eq!(std::fs::read(&archive.first).unwrap(), b"two");
        assert!(!archive.root().join("readme.txt").exists());
    }

    #[test]
    fn extract_images_returns_no_images_for_non_image_zip() {
        let dir = TempDir::new("zip");
        let zip_path = make_zip(dir.path(), &[("readme.txt", b"ignore")]);

        assert!(matches!(
            extract_images(&zip_path),
            Err(ExtractError::NoImages)
        ));
    }

    #[test]
    fn extract_images_flattens_nested_entries_and_renames_duplicates() {
        let dir = TempDir::new("zip");
        let zip_path = make_zip(
            dir.path(),
            &[("a/photo.jpg", b"one"), ("b/photo.jpg", b"two")],
        );

        let archive = extract_images(&zip_path).expect("extract images");
        assert_eq!(
            std::fs::read(archive.root().join("photo.jpg")).unwrap(),
            b"one"
        );
        assert_eq!(
            std::fs::read(archive.root().join("photo_2.jpg")).unwrap(),
            b"two"
        );
        assert_eq!(
            std::fs::read_dir(archive.root())
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            2
        );
    }

    #[test]
    fn extract_images_keeps_entries_inside_temp_root() {
        let dir = TempDir::new("zip");
        let zip_path = make_zip(
            dir.path(),
            &[("../escape.jpg", b"bad"), ("aaa.jpg", b"good")],
        );

        let archive = extract_images(&zip_path).expect("extract images");
        assert_eq!(
            archive.first.file_name().unwrap().to_string_lossy(),
            "aaa.jpg"
        );
        assert!(archive.first.starts_with(archive.root()));
        assert!(!dir.path().join("escape.jpg").exists());
    }
}
