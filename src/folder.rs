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
        let target = path.file_name()?;
        let mut folder = Self::scan_dir(path.parent()?)?;
        // `path` 可能是相对路径，比文件名比整条路径可靠
        folder.index = folder
            .files
            .iter()
            .position(|entry| entry.file_name() == Some(target))?;
        Some(folder)
    }

    /// 扫描目录本身；拖入目录或压缩包时使用，游标落在第一张图
    pub fn scan_dir(dir: &Path) -> Option<Self> {
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
        Some(Self {
            dir: dir.to_path_buf(),
            files,
            index: 0,
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

    /// 看相邻的第 `delta` 张（循环），不移动游标；用于预取
    pub fn peek(&self, delta: isize) -> Option<&Path> {
        let len = self.files.len() as isize;
        if len < 2 {
            return None;
        }
        let index = (self.index as isize + delta).rem_euclid(len) as usize;
        if index == self.index {
            return None;
        }
        self.files.get(index).map(PathBuf::as_path)
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

/// 外部传入路径的种类，决定打开方式
pub enum InputKind {
    Image,
    Directory,
    Archive,
}

/// 按路径本身判断来源；不读文件内容，毫秒级
pub fn input_kind(path: &Path) -> InputKind {
    if path.is_dir() {
        return InputKind::Directory;
    }
    let is_zip = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
    if is_zip {
        InputKind::Archive
    } else {
        InputKind::Image
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
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
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
    use super::{Folder, input_kind, natural_cmp};
    use crate::test_util::TempDir;
    use std::cmp::Ordering;
    use std::path::Path;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").expect("write fixture");
    }

    #[test]
    fn scan_dir_starts_at_first_image_in_natural_order() {
        let dir = TempDir::new("folder");
        touch(dir.path(), "img_10.jpg");
        touch(dir.path(), "img_2.jpg");
        touch(dir.path(), "note.txt");

        let folder = Folder::scan_dir(dir.path()).expect("scan dir");
        assert_eq!(folder.index, 0);
        let current = folder.current().expect("current image");
        assert_eq!(current.file_name().unwrap().to_string_lossy(), "img_2.jpg");
    }

    #[test]
    fn input_kind_recognizes_directory_zip_and_image() {
        use super::InputKind;

        let dir = TempDir::new("kind");
        assert!(matches!(
            input_kind(dir.path()),
            InputKind::Directory
        ));
        assert!(matches!(
            input_kind(Path::new("C:\\albums\\ALBUM.ZIP")),
            InputKind::Archive
        ));
        assert!(matches!(
            input_kind(Path::new("photo.jpg")),
            InputKind::Image
        ));
    }

    #[test]
    fn scan_dir_returns_none_when_no_images() {
        let dir = TempDir::new("folder");
        touch(dir.path(), "note.txt");

        assert!(Folder::scan_dir(dir.path()).is_none());
    }

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
