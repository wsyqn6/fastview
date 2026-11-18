//! 同目录图片列表
//!
//! 只做两件事：扫出同目录的图片文件、按自然序排好。翻页与缩略图条共用这份列表。

use std::cmp::Ordering;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

/// 能被 WIC 或 `image` 解码的常见格式
const EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "bmp", "webp", "ico", "tif", "tiff", "avif", "heic",
    "heif",
];

/// 当前目录里的图片与游标
pub struct Folder {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub index: usize,
}

impl Folder {
    /// 扫描 `path` 所在目录；文件不在目录里时返回 `None`
    pub fn scan(path: &Path) -> Option<Self> {
        let dir = path.parent()?;
        let target = path.file_name()?;
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|entry| is_image(entry))
            .collect();
        if files.is_empty() {
            return None;
        }
        files.sort_by(|a, b| natural_cmp(&file_name(a), &file_name(b)));

        // `path` 可能是相对路径，比文件名比整条路径可靠
        let index = files
            .iter()
            .position(|entry| entry.file_name() == Some(target))?;
        Some(Self {
            dir: dir.to_path_buf(),
            files,
            index,
        })
    }

    pub fn current(&self) -> Option<&Path> {
        self.files.get(self.index).map(PathBuf::as_path)
    }

    /// 游标移动到 `path`，返回是否发生变化
    pub fn select(&mut self, path: &Path) -> bool {
        let Some(target) = path.file_name() else {
            return false;
        };
        let Some(index) = self
            .files
            .iter()
            .position(|entry| entry.file_name() == Some(target))
        else {
            return false;
        };
        if index == self.index {
            return false;
        }
        self.index = index;
        true
    }

    /// 循环前进 / 后退
    pub fn step(&mut self, forward: bool) -> Option<&Path> {
        if self.files.len() < 2 {
            return None;
        }
        self.index = if forward {
            (self.index + 1) % self.files.len()
        } else {
            (self.index + self.files.len() - 1) % self.files.len()
        };
        self.current()
    }
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 自然序比较：`2.jpg` 排在 `10.jpg` 之前
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();

    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                match take_number(&mut left).cmp(&take_number(&mut right)) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(l), Some(r)) => {
                let (l, r) = (l.to_ascii_lowercase(), r.to_ascii_lowercase());
                match l.cmp(&r) {
                    Ordering::Equal => {
                        left.next();
                        right.next();
                    }
                    other => return other,
                }
            }
        }
    }
}

fn take_number(chars: &mut Peekable<Chars<'_>>) -> u64 {
    let mut value = 0u64;
    while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
        value = value.saturating_mul(10).saturating_add(digit as u64);
        chars.next();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;
    use std::cmp::Ordering;

    #[test]
    fn orders_numbers_by_value() {
        assert_eq!(natural_cmp("2.jpg", "10.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("img_9.png", "img_10.png"), Ordering::Less);
        assert_eq!(natural_cmp("a.jpg", "a.jpg"), Ordering::Equal);
    }

    #[test]
    fn ignores_case() {
        assert_eq!(natural_cmp("IMG.JPG", "img.jpg"), Ordering::Equal);
    }
}
