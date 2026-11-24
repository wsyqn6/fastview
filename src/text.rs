//! 界面文案：所有可见文字通过 `TextKey` 取，不在绘制代码里写死
//!
//! 新增文案只改下面的表；`System` 在构造 `Text` 时解析成中文或英文，
//! 渲染代码不需要再判断语言。

use crate::app::settings::Language;

macro_rules! strings {
    ($($key:ident => $zh:expr, $en:expr;)*) => {
        /// 可本地化的文案键
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum TextKey {
            $($key,)*
        }

        /// 当前语言下的文案表
        #[derive(Clone, Copy)]
        pub struct Text(Language);

        impl Text {
            pub fn new(language: Language) -> Self {
                Self(language.effective())
            }

            pub fn t(self, key: TextKey) -> &'static str {
                match (self.0, key) {
                    $((Language::Chinese, TextKey::$key) => $zh,)*
                    $((Language::English, TextKey::$key) => $en,)*
                    (Language::System, _) => unreachable!("系统语言已在 new 中解析"),
                }
            }
        }
    };
}

strings! {
    File => "文件", "File";
    Open => "打开文件", "Open Image";
    Exit => "退出", "Exit";
    View => "查看", "View";
    Fit => "适应窗口", "Fit to Window";
    OriginalSize => "原始大小", "Actual Size";
    Fill => "填充窗口", "Fill Window";
    RotateCw => "顺时针旋转", "Rotate Right";
    RotateCcw => "逆时针旋转", "Rotate Left";
    Fullscreen => "全屏", "Fullscreen";
    Thumbnails => "缩略图", "Thumbnails";
    Settings => "设置", "Settings";
    Help => "帮助", "Help";
    Shortcuts => "快捷键", "Shortcuts";
    About => "关于 FastView", "About FastView";
    CheckUpdate => "检查更新", "Check for Updates";
    PhotoInfo => "照片信息", "Photo Info";
    PhotoInfoCapture => "拍摄", "Capture";

    Language => "界面语言", "Language";
    LanguageSystem => "跟随系统", "System";
    LanguageChinese => "中文", "Chinese";
    LanguageEnglish => "English", "English";
    Theme => "主题", "Theme";
    ThemeSystem => "跟随系统", "System";
    ThemeLight => "浅色", "Light";
    ThemeDark => "深色", "Dark";
    Material => "窗口材质", "Window Material";
    MaterialAcrylic => "亚克力", "Acrylic";
    MaterialMica => "云母", "Mica";
    MaterialNone => "无模糊", "No Blur";
    Opacity => "不透明度", "Opacity";
    ThumbnailBar => "缩略图条", "Thumbnail Bar";
    StatusPinned => "状态栏常显", "Keep Status Bar";
    Borderless => "无边框模式", "Borderless";
    AutoWindowSize => "打开时窗口跟随图片", "Resize window to image on open";
    Done => "完成", "Done";

    Loading => "加载中…", "Loading…";
    DropOrOpen => "拖入图片、文件夹或 zip，或按 Ctrl+O 打开", "Drop images, a folder or a zip, or press Ctrl+O";
    Formats => "支持 JPEG / PNG / WebP / GIF / BMP / TIFF / ZIP", "Supports JPEG / PNG / WebP / GIF / BMP / TIFF / ZIP";
    ModeOriginal => "原始", "Actual";
    ModeFill => "填充", "Fill";
    DropHint => "松开以打开", "Drop to open";
    OpenPrompt => "打开图片", "Open Image";
    NoImages => "没有可查看的图片", "No images found";
    ArchiveFailed => "无法打开压缩包", "Cannot open archive";

    ShortcutsTitle => "快捷键", "Shortcuts";
    ShortcutsClose => "H / Esc 关闭", "H / Esc to close";
    GroupFile => "文件", "File";
    GroupView => "查看", "View";
    GroupWindow => "窗口", "Window";
    GroupHelp => "帮助", "Help";
    ActOpen => "打开文件", "Open image";
    ActSettings => "设置", "Settings";
    ActFit => "适应窗口", "Fit to window";
    ActOriginal => "原始大小", "Actual size";
    ActFill => "填充窗口", "Fill window";
    ActRotateCw => "顺时针旋转", "Rotate right";
    ActRotateCcw => "逆时针旋转", "Rotate left";
    ActZoom => "放大 / 缩小", "Zoom in / out";
    ActZoomCursor => "以光标缩放", "Zoom at cursor";
    KeyWheel => "滚轮", "Wheel";
    KeyDrag => "拖拽", "Drag";
    ActStep => "上一张 / 下一张", "Previous / Next";
    ActPan => "平移", "Pan";
    ActThumbnails => "缩略图", "Thumbnails";
    ActStatus => "状态栏常显", "Keep status bar";
    ActPhotoInfo => "照片信息", "Photo info";
    ActFullscreen => "全屏", "Fullscreen";
    ActBorderless => "无边框", "Borderless";
    ActExitFullscreen => "退出全屏 / 关闭浮层", "Exit fullscreen / close overlay";
    ActShortcuts => "快捷键", "Shortcuts";

    EscClose => "Esc 关闭", "Esc to close";
    GotIt => "知道了", "Got it";
    AboutVersion => "版本", "Version";
    AboutStage => "Rust + GPUI 实验版本", "Rust + GPUI experimental build";
    AboutFeatures => "分级解码 · 本地缓存 · 拖拽打开", "Tiered decoding · local cache · drag & drop";
    UpdateBody => "当前为实验版本，暂未接入更新服务。", "Experimental build; update service is not available yet.";

    GroupImage => "图像", "Image";
    GroupCamera => "相机", "Camera";
    GroupExposure => "曝光", "Exposure";
    FieldName => "名称", "Name";
    FieldPath => "路径", "Path";
    FieldDimensions => "尺寸", "Dimensions";
    FieldFileSize => "文件大小", "File Size";
    FieldFormat => "格式", "Format";
    FieldSize => "大小", "Size";
    FieldStorageSize => "存储尺寸", "Stored Size";
    FieldDisplaySize => "显示尺寸", "Display Size";
    FieldOrientation => "EXIF 方向", "EXIF Orientation";
    FieldRotation => "当前旋转", "Rotation";
    FieldMake => "制造商", "Make";
    FieldModel => "型号", "Model";
    FieldCamera => "相机", "Camera";
    FieldLens => "镜头", "Lens";
    FieldFocalLength => "焦距", "Focal Length";
    FieldTakenAt => "拍摄时间", "Taken At";
    FieldIso => "ISO", "ISO";
    FieldShutter => "快门", "Shutter";
    FieldAperture => "光圈", "Aperture";
    NotRecorded => "未记录", "Not recorded";
    ExifUnavailable => "EXIF 不可用", "EXIF unavailable";
    PhotoInfoLoading => "正在读取照片信息…", "Reading photo info…";
    PhotoInfoError => "读取失败", "Read failed";
    OrientationNormal => "正常", "Normal";
    OrientationRotate90 => "旋转 90°", "Rotate 90°";
    OrientationRotate180 => "旋转 180°", "Rotate 180°";
    OrientationRotate270 => "旋转 270°", "Rotate 270°";
    OrientationMirrorHorizontal => "水平镜像", "Mirror horizontal";
    OrientationMirrorVertical => "垂直镜像", "Mirror vertical";
    OrientationRotate90MirrorHorizontal => "旋转 90° 并水平镜像", "Rotate 90° and mirror horizontal";
    OrientationRotate270MirrorHorizontal => "旋转 270° 并水平镜像", "Rotate 270° and mirror horizontal";
}

#[cfg(test)]
mod tests {
    use super::{Language, Text, TextKey};

    #[test]
    fn picks_language_text() {
        assert_eq!(Text::new(Language::Chinese).t(TextKey::File), "文件");
        assert_eq!(Text::new(Language::English).t(TextKey::File), "File");
    }

    #[test]
    fn system_resolves_to_a_concrete_language() {
        assert!(!matches!(Language::System.effective(), Language::System));
    }
}
