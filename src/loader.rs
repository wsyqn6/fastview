use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::debug_log;
use crate::log_error;
use crate::types::{DecodedImage, TileInfo, TiledImage};
use crate::utils::lock_or_recover;
use rayon::ThreadPool;

// 全局启动时间（用于相对时间日志）
static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

fn elapsed_ms() -> u64 {
    START_TIME.elapsed().as_millis() as u64
}

/// 加载优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadPriority {
    Low = 0,      // 预加载
    High = 2,     // 当前显示图片
    Critical = 3, // 立即需要（用户刚切换）
}

/// 发送给后台线程的命令
pub enum LoadCommand {
    /// 加载指定路径的图片
    #[allow(dead_code)]
    Load {
        path: PathBuf,
        priority: LoadPriority,
    },
    /// 预加载一组路径
    Prefetch {
        paths: Vec<PathBuf>,
        priority: LoadPriority,
    },
    /// 异步扫描目录
    ScanDirectory { dir_path: PathBuf },
    /// 创建分块图片元数据
    CreateTiledImage {
        path: PathBuf,
        priority: LoadPriority,
    },
    /// 加载特定块
    LoadTile {
        path: PathBuf,
        col: u32,
        row: u32,
        priority: LoadPriority,
    },
    /// 生成缩略图
    GenerateThumbnail {
        path: PathBuf,
        size: u32,
        priority: LoadPriority,
    },
}

/// 加载结果（发回 UI 线程）
pub enum LoadResult {
    /// 高清图已解码完成
    ImageReady {
        path: PathBuf,
        image: Arc<DecodedImage>,
    },
    /// 分块图片元数据已创建
    TiledImageMetaReady {
        path: PathBuf,
        tiled_image: Arc<TiledImage>,
    },
    /// 单个块已加载
    TileReady {
        path: PathBuf,
        col: u32,
        row: u32,
        data: Vec<u8>,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
    },
    /// 加载失败
    #[allow(dead_code)]
    Error { path: PathBuf, error: String },
    /// 目录扫描完成
    DirectoryScanned { images: Vec<PathBuf> },
    /// 缩略图生成完成
    ThumbnailReady {
        path: PathBuf,
        image: Arc<DecodedImage>,
    },
    /// 缩略图生成失败
    ThumbnailFailed { path: PathBuf, error: String },
}

struct PendingTask {
    path: PathBuf,
    priority: LoadPriority,
    task_type: TaskType,
}

#[derive(Debug, Clone)]
enum TaskType {
    LoadFull,
    LoadTile { col: u32, row: u32 },
    CreateTiled,
    GenerateThumbnail { size: u32 },
}

pub struct ImageLoader {
    cmd_rx: Receiver<LoadCommand>,
    result_tx: Sender<LoadResult>,

    cache: lru::LruCache<PathBuf, Arc<DecodedImage>>,

    pending: VecDeque<PendingTask>,

    // 跟踪正在执行的任务（避免重复解码）
    active_tasks: Arc<Mutex<HashSet<PathBuf>>>,

    // Rayon线程池（替代每次spawn）
    pool: Arc<ThreadPool>,

    #[allow(dead_code)]
    max_memory: usize,
    #[allow(dead_code)]
    current_memory: usize,
}

impl ImageLoader {
    pub fn new(result_tx: Sender<LoadResult>) -> (Self, Sender<LoadCommand>) {
        let (cmd_tx, cmd_rx) = channel();

        // 创建rayon线程池（根据CPU核心数动态调整）
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // 使用 CPU 核心数，但不超过 8 个线程（避免过度并发）
        let num_threads = num_cpus.clamp(2, 8);

        debug_log!(
            "[LOADER] 初始化线程池: {} 个线程 (CPU核心数: {})",
            num_threads,
            num_cpus
        );

        let pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .thread_name(|i| format!("image-loader-{}", i))
                .build()
                .unwrap_or_else(|e| {
                    log_error!(
                        "Failed to create thread pool: {}, falling back to single thread",
                        e
                    );
                    // 降级为单线程执行
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(1)
                        .build()
                        .expect("Failed to create fallback thread pool")
                }),
        );

        let loader = Self {
            cmd_rx,
            result_tx,
            cache: lru::LruCache::new(crate::utils::to_non_zero_usize(8, 10)),
            pending: VecDeque::new(),
            active_tasks: Arc::new(Mutex::new(HashSet::new())),
            pool,
            max_memory: 30 * 1024 * 1024, // 30 MB
            current_memory: 0,
        };
        (loader, cmd_tx)
    }

    pub fn run(mut self) {
        loop {
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    LoadCommand::Load { path, priority } => {
                        self.handle_load(path, priority);
                        // 高优先级任务立即处理
                        if priority >= LoadPriority::High {
                            self.process_pending();
                        }
                    }
                    LoadCommand::Prefetch { paths, priority } => {
                        for path in paths {
                            self.handle_load(path, priority);
                        }
                    }
                    LoadCommand::ScanDirectory { dir_path } => {
                        self.scan_directory_async(dir_path);
                    }
                    LoadCommand::CreateTiledImage { path, priority } => {
                        self.handle_create_tiled(path, priority);
                        if priority >= LoadPriority::High {
                            self.process_pending();
                        }
                    }
                    LoadCommand::LoadTile {
                        path,
                        col,
                        row,
                        priority,
                    } => {
                        self.handle_load_tile(path, col, row, priority);
                        if priority >= LoadPriority::High {
                            self.process_pending();
                        }
                    }
                    LoadCommand::GenerateThumbnail {
                        path,
                        size,
                        priority,
                    } => {
                        self.handle_generate_thumbnail(path, size, priority);
                        if priority >= LoadPriority::High {
                            self.process_pending();
                        }
                    }
                }
            }

            self.process_pending();

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn handle_load(&mut self, path: PathBuf, priority: LoadPriority) {
        // 检查缓存
        if let Some(cached) = self.cache.get(&path) {
            let _ = self.result_tx.send(LoadResult::ImageReady {
                path,
                image: cached.clone(),
            });
            return;
        }

        // 检查是否已经在执行中
        {
            let active = lock_or_recover(&self.active_tasks);
            if active.contains(&path) {
                debug_log!(
                    "[{:.3}s] [LOADER] 跳过重复任务（正在执行）: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
                return;
            }
        }

        // 检查pending队列中是否已有相同路径的任务
        if let Some(existing_task) = self
            .pending
            .iter_mut()
            .find(|t| t.path == path && matches!(t.task_type, TaskType::LoadFull))
        {
            if priority > existing_task.priority {
                debug_log!(
                    "[{:.3}s] [LOADER] 升级任务优先级: {:?} ({:?} -> {:?})",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name(),
                    existing_task.priority,
                    priority
                );
                existing_task.priority = priority;
            } else {
                debug_log!(
                    "[{:.3}s] [LOADER] 跳过重复任务: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
                return;
            }
        } else {
            let task = PendingTask {
                path,
                priority,
                task_type: TaskType::LoadFull,
            };

            // 插入队列，保持按优先级降序排列
            let pos = self
                .pending
                .iter()
                .position(|t| t.priority < priority)
                .unwrap_or(self.pending.len());
            self.pending.insert(pos, task);
        }

        if priority >= LoadPriority::High {
            self.pending.retain(|t| t.priority >= LoadPriority::High);
        }
    }

    fn handle_create_tiled(&mut self, path: PathBuf, priority: LoadPriority) {
        // 首先读取图片尺寸，判断是否需要分块加载
        let needs_tiled = match image::image_dimensions(&path) {
            Ok((width, height)) => {
                // 分辨率 >= 6000 时使用分块加载
                let need = width >= 6000 || height >= 6000;
                debug_log!(
                    "[{:.3}s] [LOADER] 图片尺寸: {}x{}, 需要分块: {}",
                    elapsed_ms() as f64 / 1000.0,
                    width,
                    height,
                    need
                );
                need
            }
            Err(e) => {
                debug_log!(
                    "[{:.3}s] [LOADER] 无法读取图片尺寸: {:?}, 错误: {}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name(),
                    e
                );
                false
            }
        };

        if !needs_tiled {
            // 小图片，直接使用普通加载
            debug_log!(
                "[{:.3}s] [LOADER] 小图片，使用普通加载",
                elapsed_ms() as f64 / 1000.0
            );
            self.handle_load(path, priority);
            return;
        }

        // 检查是否已经在执行中
        let task_key = format!("{:?}_tiled", path);
        {
            let active = lock_or_recover(&self.active_tasks);
            if active.contains(&PathBuf::from(&task_key)) {
                return;
            }
        }

        // 添加到pending队列
        let task = PendingTask {
            path: path.clone(),
            priority,
            task_type: TaskType::CreateTiled,
        };

        let pos = self
            .pending
            .iter()
            .position(|t| t.priority < priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(pos, task);
    }

    fn handle_load_tile(&mut self, path: PathBuf, col: u32, row: u32, priority: LoadPriority) {
        // 为每个块生成唯一标识
        let task_key = format!("{:?}_tile_{}_{}", path, col, row);

        {
            let active = lock_or_recover(&self.active_tasks);
            if active.contains(&PathBuf::from(&task_key)) {
                debug_log!(
                    "[{:.3}s] [LOADER] 跳过重复块加载: {:?} ({},{})",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name(),
                    col,
                    row
                );
                return;
            }
        }

        // 检查是否已在pending队列中
        if self.pending.iter().any(|t| {
            t.path == path
                && matches!(&t.task_type, TaskType::LoadTile { col: c, row: r } if *c == col && *r == row)
        }) {
            return;
        }

        let task = PendingTask {
            path,
            priority,
            task_type: TaskType::LoadTile { col, row },
        };

        let pos = self
            .pending
            .iter()
            .position(|t| t.priority < priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(pos, task);
    }

    fn process_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        self.pending
            .make_contiguous()
            .sort_by(|a, b| b.priority.cmp(&a.priority));

        if let Some(task) = self.pending.pop_front() {
            let path = task.path.clone();
            let priority = task.priority;
            let task_type = task.task_type.clone();
            let result_tx = self.result_tx.clone();
            let pool = self.pool.clone();
            let active_tasks = self.active_tasks.clone();

            // 标记为正在执行
            {
                let mut active = lock_or_recover(&active_tasks);
                match &task_type {
                    TaskType::LoadFull | TaskType::CreateTiled => {
                        active.insert(path.clone());
                    }
                    TaskType::LoadTile { col, row } => {
                        let key = format!("{:?}_tile_{}_{}", path, col, row);
                        active.insert(PathBuf::from(key));
                    }
                    TaskType::GenerateThumbnail { .. } => {
                        let key = format!("{:?}_thumb", path);
                        active.insert(PathBuf::from(key));
                    }
                }
            }

            debug_log!(
                "[{:.3}s] [LOADER] 开始处理: {:?} (type={:?}, priority={:?})",
                elapsed_ms() as f64 / 1000.0,
                path.file_name(),
                task_type,
                priority
            );

            pool.spawn(move || {
                let start = Instant::now();

                let result =
                    match &task_type {
                        TaskType::LoadFull => {
                            decode_image_file(&path).map(|image| LoadResult::ImageReady {
                                path: path.clone(),
                                image: Arc::new(image),
                            })
                        }
                        TaskType::CreateTiled => create_tiled_image_meta(&path).map(|tiled| {
                            LoadResult::TiledImageMetaReady {
                                path: path.clone(),
                                tiled_image: Arc::new(tiled),
                            }
                        }),
                        TaskType::LoadTile { col, row } => decode_tile(&path, *col, *row, 1024)
                            .map(|(data, w, h, x, y)| LoadResult::TileReady {
                                path: path.clone(),
                                col: *col,
                                row: *row,
                                data,
                                width: w,
                                height: h,
                                x,
                                y,
                            }),
                        TaskType::GenerateThumbnail { size } => {
                            execute_thumbnail_generation(&path, *size).map(|image| {
                                LoadResult::ThumbnailReady {
                                    path: path.clone(),
                                    image,
                                }
                            })
                        }
                    };

                // 从 active_tasks 中移除
                {
                    let mut active = lock_or_recover(&active_tasks);
                    match &task_type {
                        TaskType::LoadFull | TaskType::CreateTiled => {
                            active.remove(&path);
                        }
                        TaskType::LoadTile { col, row } => {
                            let key = format!("{:?}_tile_{}_{}", path, col, row);
                            active.remove(&PathBuf::from(key));
                        }
                        TaskType::GenerateThumbnail { .. } => {
                            let key = format!("{:?}_thumb", path);
                            active.remove(&PathBuf::from(key));
                        }
                    }
                }

                match result {
                    Ok(load_result) => {
                        let duration = start.elapsed();
                        debug_log!(
                            "[{:.3}s] [LOADER] 完成: {:?} ({}ms)",
                            elapsed_ms() as f64 / 1000.0,
                            path.file_name(),
                            duration.as_millis()
                        );
                        let _ = result_tx.send(load_result);
                    }
                    Err(e) => {
                        debug_log!(
                            "[{:.3}s] [LOADER] 失败: {:?}, error={}",
                            elapsed_ms() as f64 / 1000.0,
                            path.file_name(),
                            e
                        );
                        let _ = result_tx.send(LoadResult::Error {
                            path,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    /// 异步扫描目录（使用rayon并行过滤）
    fn scan_directory_async(&self, dir_path: PathBuf) {
        let result_tx = self.result_tx.clone();
        let pool = self.pool.clone();

        pool.spawn(move || {
            use rayon::prelude::*;

            let images: Vec<PathBuf> = match std::fs::read_dir(&dir_path) {
                Ok(entries) => {
                    entries
                        .par_bridge() // 并行迭代
                        .filter_map(|entry| {
                            entry.ok().and_then(|e| {
                                let path = e.path();
                                if !path.is_file() {
                                    return None;
                                }
                                let ext = path
                                    .extension()
                                    .and_then(|ext| ext.to_str())?
                                    .to_lowercase();
                                if matches!(
                                    ext.as_str(),
                                    "jpg"
                                        | "jpeg"
                                        | "png"
                                        | "webp"
                                        | "gif"
                                        | "bmp"
                                        | "tiff"
                                        | "tif"
                                ) {
                                    Some(path)
                                } else {
                                    None
                                }
                            })
                        })
                        .collect::<Vec<_>>()
                }
                Err(_) => Vec::new(),
            };

            // 排序结果
            let mut sorted_images = images;
            sorted_images.sort();

            // 发送结果回UI线程
            let _ = result_tx.send(LoadResult::DirectoryScanned {
                images: sorted_images,
            });
        });
    }
}

fn decode_image_file(
    path: &PathBuf,
) -> Result<DecodedImage, Box<dyn std::error::Error + Send + Sync>> {
    use image::{ImageDecoder, ImageReader};
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path)?;
    let reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    // 获取 decoder 以读取 EXIF 方向信息
    let mut decoder = reader.into_decoder()?;

    // 从 EXIF 数据中获取方向
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    // 解码图片
    let mut img = image::DynamicImage::from_decoder(decoder)?;

    // 应用 EXIF 方向变换
    img.apply_orientation(orientation);

    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let data = rgba.into_raw();

    Ok(DecodedImage {
        data,
        width,
        height,
    })
}

/// 创建分块图片元数据（不加载实际像素数据）
fn create_tiled_image_meta(
    path: &PathBuf,
) -> Result<TiledImage, Box<dyn std::error::Error + Send + Sync>> {
    use image::{GenericImageView, ImageDecoder, ImageReader};
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path)?;
    let reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    let mut decoder = reader.into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut img = image::DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);

    let (width, height) = img.dimensions();

    // 定义块大小
    let tile_size = 1024;

    // 计算需要的行列数
    let cols = width.div_ceil(tile_size);
    let rows = height.div_ceil(tile_size);

    // 创建缩略图（最大边长512px）
    let max_thumb_size = 512;
    let scale = (max_thumb_size as f32 / width.max(height) as f32).min(1.0);
    let thumb_w = (width as f32 * scale) as u32;
    let thumb_h = (height as f32 * scale) as u32;

    // 使用 Nearest 快速生成缩略图（质量足够用于预览）
    let thumbnail_img = img.resize(thumb_w, thumb_h, image::imageops::FilterType::Nearest);

    // 获取实际的缩略图尺寸（resize 可能会调整）
    let (actual_thumb_w, actual_thumb_h) = thumbnail_img.dimensions();

    let thumb_rgba = thumbnail_img.into_rgba8();
    let thumb_data = thumb_rgba.into_raw();

    debug_log!(
        "[{:.3}s] [LOADER] 缩略图: {}x{} -> {}x{}, 数据长度: {}",
        elapsed_ms() as f64 / 1000.0,
        width,
        height,
        actual_thumb_w,
        actual_thumb_h,
        thumb_data.len()
    );

    let thumbnail = Arc::new(DecodedImage {
        data: thumb_data,
        width: actual_thumb_w,
        height: actual_thumb_h,
    });

    // 创建空的块信息
    let mut tiles = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let x = col * tile_size;
            let y = row * tile_size;
            let w = tile_size.min(width - x);
            let h = tile_size.min(height - y);

            tiles.push(TileInfo {
                col,
                row,
                x,
                y,
                width: w,
                height: h,
                loaded: false,
            });
        }
    }

    Ok(TiledImage {
        width,
        height,
        thumbnail,
        tiles,
        tile_size,
        cols,
        rows,
    })
}

/// 解码单个块的结果类型
type TileDecodeResult = (Vec<u8>, u32, u32, u32, u32);

/// 解码单个块
fn decode_tile(
    path: &PathBuf,
    col: u32,
    row: u32,
    tile_size: u32,
) -> Result<TileDecodeResult, Box<dyn std::error::Error + Send + Sync>> {
    use image::{GenericImageView, ImageDecoder, ImageReader};
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path)?;
    let reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    let mut decoder = reader.into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut img = image::DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);

    let (width, height) = img.dimensions();

    // 计算块的位置和尺寸
    let x = col * tile_size;
    let y = row * tile_size;
    let w = tile_size.min(width - x);
    let h = tile_size.min(height - y);

    // 裁剪出对应的块
    let tile_img = img.crop_imm(x, y, w, h);
    let tile_rgba = tile_img.into_rgba8();
    let tile_data = tile_rgba.into_raw();

    Ok((tile_data, w, h, x, y))
}

/// 生成缩略图
impl ImageLoader {
    fn handle_generate_thumbnail(&mut self, path: PathBuf, size: u32, priority: LoadPriority) {
        // 添加到待处理队列
        let task = PendingTask {
            path: path.clone(),
            priority,
            task_type: TaskType::GenerateThumbnail { size },
        };

        self.pending.push_back(task);
    }
}

/// 执行缩略图生成
fn execute_thumbnail_generation(
    path: &PathBuf,
    size: u32,
) -> Result<Arc<DecodedImage>, Box<dyn std::error::Error + Send + Sync>> {
    use image::{ImageDecoder, ImageReader};
    use std::fs::File;
    use std::io::BufReader;

    // 使用 ImageReader 以支持 EXIF 自动旋转
    let file = File::open(path)?;
    let reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    // 获取 decoder 以读取 EXIF 方向信息
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    // 解码图片
    let mut img = image::DynamicImage::from_decoder(decoder)?;

    // 应用 EXIF 方向信息 (自动旋转，原地修改)
    img.apply_orientation(orientation);

    // 使用 thumbnail() - 专为缩略图优化的整数算法（比 resize 快）
    let thumbnail = img.thumbnail(size, size);

    // 转换为 RGBA
    let rgba = thumbnail.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let data = rgba.into_raw();

    Ok(Arc::new(DecodedImage {
        data,
        width,
        height,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_priority_ordering() {
        // 测试优先级排序：Critical > High > Low
        assert!(LoadPriority::Critical > LoadPriority::High);
        assert!(LoadPriority::High > LoadPriority::Low);
        assert_eq!(LoadPriority::Low, LoadPriority::Low);
    }

    #[test]
    fn test_task_type_variants() {
        // 确保所有 TaskType 变体都能正确创建
        let _full = TaskType::LoadFull;
        let _tile = TaskType::LoadTile { col: 0, row: 0 };
        let _tiled = TaskType::CreateTiled;
    }
}
