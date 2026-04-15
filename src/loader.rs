use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::types::DecodedImage;
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
}

/// 加载结果（发回 UI 线程）
pub enum LoadResult {
    /// 高清图已解码完成
    ImageReady {
        path: PathBuf,
        image: Arc<DecodedImage>,
    },
    /// 加载失败
    #[allow(dead_code)]
    Error { path: PathBuf, error: String },
    /// 目录扫描完成
    DirectoryScanned { images: Vec<PathBuf> },
}

struct PendingTask {
    path: PathBuf,
    priority: LoadPriority,
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

        // 创建rayon线程池（2-4个线程，根据CPU核心数调整）
        let pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .thread_name(|i| format!("image-loader-{}", i))
                .build()
                .expect("Failed to create thread pool"),
        );

        let loader = Self {
            cmd_rx,
            result_tx,
            cache: lru::LruCache::new(8.try_into().unwrap()),
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
            let active = self.active_tasks.lock().unwrap();
            if active.contains(&path) {
                eprintln!(
                    "[{:.3}s] [LOADER] 跳过重复任务（正在执行）: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
                return;
            }
        }

        // 检查pending队列中是否已有相同路径的任务
        if let Some(existing_task) = self.pending.iter_mut().find(|t| t.path == path) {
            // 如果新任务优先级更高，更新优先级
            if priority > existing_task.priority {
                eprintln!(
                    "[{:.3}s] [LOADER] 升级任务优先级: {:?} ({:?} -> {:?})",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name(),
                    existing_task.priority,
                    priority
                );
                existing_task.priority = priority;
            } else {
                // 否则跳过，不重复添加
                eprintln!(
                    "[{:.3}s] [LOADER] 跳过重复任务: {:?} (当前优先级={:?}, 请求优先级={:?})",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name(),
                    existing_task.priority,
                    priority
                );
                return;
            }
        } else {
            // 创建新任务并加入队列
            let task = PendingTask { path, priority };

            // 插入队列，保持按优先级降序排列
            let pos = self
                .pending
                .iter()
                .position(|t| t.priority < priority)
                .unwrap_or(self.pending.len());
            self.pending.insert(pos, task);
        }

        // 如果高优先级任务出现，清理过期的低优先级任务
        if priority >= LoadPriority::High {
            // 保留高优先级任务，删除低优先级任务
            self.pending.retain(|t| t.priority >= LoadPriority::High);
        }
    }

    fn process_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        // 按优先级排序：Critical > High > Low
        // 确保当前图片优先于预加载图片
        self.pending
            .make_contiguous()
            .sort_by(|a, b| b.priority.cmp(&a.priority));

        // 取最高优先级的任务执行
        if let Some(task) = self.pending.pop_front() {
            let path = task.path.clone();
            let priority = task.priority;
            let result_tx = self.result_tx.clone();
            let pool = self.pool.clone();
            let active_tasks = self.active_tasks.clone();

            // 标记为正在执行
            {
                let mut active = active_tasks.lock().unwrap();
                active.insert(path.clone());
            }

            eprintln!(
                "[{:.3}s] [LOADER] 开始解码: {:?} (priority={:?})",
                elapsed_ms() as f64 / 1000.0,
                path.file_name(),
                priority
            );

            // 使用rayon线程池异步执行解码
            pool.spawn(move || {
                let start = Instant::now();
                match decode_image_file(&path) {
                    Ok(image) => {
                        let duration = start.elapsed();
                        eprintln!(
                            "[{:.3}s] [LOADER] 解码完成: {:?} ({}x{}, {}ms)",
                            elapsed_ms() as f64 / 1000.0,
                            path.file_name(),
                            image.width,
                            image.height,
                            duration.as_millis()
                        );

                        // 从active_tasks中移除
                        {
                            let mut active = active_tasks.lock().unwrap();
                            active.remove(&path);
                        }

                        let send_start = Instant::now();
                        let _ = result_tx.send(LoadResult::ImageReady {
                            path: path.clone(),
                            image: Arc::new(image),
                        });
                        let send_duration = send_start.elapsed();
                        if send_duration.as_millis() > 100 {
                            eprintln!(
                                "[{:.3}s] [LOADER] 警告：发送结果耗时 {}ms",
                                elapsed_ms() as f64 / 1000.0,
                                send_duration.as_millis()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[{:.3}s] [LOADER] 解码失败: {:?}, error={}",
                            elapsed_ms() as f64 / 1000.0,
                            path.file_name(),
                            e
                        );

                        // 从active_tasks中移除
                        {
                            let mut active = active_tasks.lock().unwrap();
                            active.remove(&path);
                        }

                        let _ = result_tx.send(LoadResult::Error {
                            path,
                            error: String::from("Failed to decode"),
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
    use image::ImageReader;
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path)?;
    let reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    // 解码图片（image 库自动处理 EXIF 方向）
    let img = reader
        .decode()
        .map_err(|e| format!("Decode error: {}", e))?;

    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let data = rgba.into_raw();

    Ok(DecodedImage {
        data,
        width,
        height,
    })
}

