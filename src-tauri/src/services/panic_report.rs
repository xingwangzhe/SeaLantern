//! panic_report.rs
//! 负责在程序崩溃（panic）时收集系统信息并生成崩溃报告。
//!
//! 通过 Rust 标准库的 `std::panic::set_hook` 注册全局 panic 回调，
//! 无需汇编或 unsafe 代码，跨平台兼容。
//! Linux 平台可读取 /proc、/sys 等虚拟文件系统获取更详细的硬件信息；
//! 其他平台则使用通用回退值。

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// 记录程序启动时间，用于在崩溃报告中计算运行时长。
/// 使用 OnceLock 保证只初始化一次，线程安全。
static START_TIME: OnceLock<SystemTime> = OnceLock::new();

/// 防止 panic hook 重入的标志。
/// 若 panic hook 自身触发了新的 panic，此标志可避免无限递归。
static PANIC_HOOK_RUNNING: AtomicBool = AtomicBool::new(false);

/// 注册全局 panic hook。
///
/// 应在程序启动时尽早调用（早于任何可能 panic 的代码），
/// 以确保所有 panic 都能被捕获并生成报告。
///
/// hook 触发时会：
/// 1. 收集崩溃时刻、启动时刻、OS 信息、CPU 温度、内存占用、文件句柄数、CPU 核心数；
/// 2. 获取 panic 发生的源码位置（文件名、行号、列号）及错误消息；
/// 3. 将报告写入当前工作目录下的 `Panic_Report` 文件；
/// 4. 同时将报告输出到 stderr；
/// 5. 以退出码 0xFFFF 终止进程。
pub fn panic_report() {
    // 记录程序启动时间；若已初始化则忽略（幂等）
    START_TIME.get_or_init(SystemTime::now);

    std::panic::set_hook(Box::new(|panic_info| {
        // 使用原子交换防止 hook 重入：若已有一个 hook 正在运行则直接返回
        if PANIC_HOOK_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        // 格式化启动时间与崩溃时间
        let start_time = format_time(*START_TIME.get().expect("start time not set"));
        let crash_time = format_time(SystemTime::now());

        // 获取 panic 的完整消息字符串
        let panic_message = panic_info.to_string();

        // 获取 panic 发生的源码位置（文件:行:列），若无则标记为 unknown
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        // 收集系统环境信息
        let os_info = get_os_info();
        let cpu_temp = get_cpu_temperature();
        let mem_load = get_memory_load();
        let handle_count = get_handle_count();
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        // 拼装崩溃报告正文
        let report = format!(
            "============!Panicked!============\n\
             ===============Info===============\n\
             Panic Time  : {crash_time}\n\
             Start Time  : {start_time}\n\
             OS          : {os_info}\n\
             CPU Temp    : {cpu_temp}\n\
             Loaded Mem  : {mem_load:.2}%\n\
             Handle Count: {handle_count}\n\
             CPU Cores   : {cpu_cores}\n\
             ============Panic Info============\n\
             Location    : {location}\n\
             Message     : {panic_message}\n\
             ============ReportEnds============\n",
        );

        // 将报告写入文件，写入失败时打印错误但不中断流程
        let report_path = "Panic_Report";
        if let Err(e) = fs::write(report_path, &report) {
            eprintln!("Failed to write panic report to '{report_path}': {e}");
        } else {
            println!("Panic report written to '{report_path}'");
        }

        // 同时将报告输出到 stderr，方便终端或日志系统捕获
        eprintln!("{report}");
        eprintln!("Sea Lantern PANICKED!!");

        // 重置标志（虽然随后会退出进程，保持语义完整性）
        PANIC_HOOK_RUNNING.store(false, Ordering::SeqCst);

        // 以异常退出码终止进程，告知外部监控程序发生了崩溃
        std::process::exit(0xFFFF);
    }));
}

/// 将 `SystemTime` 格式化为人类可读的字符串。
///
/// 输出格式：`epoch+<天>d HH:MM:SS.mmm (<毫秒数> ms since epoch)`
fn format_time(t: SystemTime) -> String {
    let since_epoch = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs();
    let millis = since_epoch.subsec_millis();

    // 将总秒数拆分为天、时、分、秒
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    format!(
        "epoch+{}d {:02}:{:02}:{:02}.{:03} ({} ms since epoch)",
        days,
        h,
        m,
        s,
        millis,
        since_epoch.as_millis()
    )
}

/// 获取操作系统信息字符串。
///
/// - Linux：读取 `/proc/version`，包含内核版本及编译信息；
/// - 其他平台：返回 `std::env::consts::OS`（如 "windows"、"macos"）。
fn get_os_info() -> String {
    #[cfg(target_os = "linux")]
    {
        fs::read_to_string("/proc/version")
            .unwrap_or_else(|_| "Unknown".to_string())
            .trim()
            .to_string()
    }
    #[cfg(not(target_os = "linux"))]
    {
        // 非 Linux 平台无法读取 /proc/version，返回平台名称作为回退
        std::env::consts::OS.to_string()
    }
}

/// 获取 CPU 温度（摄氏度）。
///
/// - Linux：遍历 `/sys/class/thermal/thermal_zone*` 读取第一个有效温度值；
///   内核以毫摄氏度为单位存储，需除以 1000 转换；
/// - 其他平台或读取失败：返回 `"N/A"`。
fn get_cpu_temperature() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let path = entry.path();
                // 只处理 thermal_zone* 目录
                if path.to_string_lossy().contains("thermal_zone") {
                    let temp_path = path.join("temp");
                    if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                        if let Ok(millideg) = temp_str.trim().parse::<f64>() {
                            // 内核单位为毫摄氏度，除以 1000 得到摄氏度
                            return format!("{:.2} C", millideg / 1000.0);
                        }
                    }
                }
            }
        }
        "N/A".to_string()
    }
    #[cfg(not(target_os = "linux"))]
    {
        // 非 Linux 平台暂不支持读取 CPU 温度
        "N/A".to_string()
    }
}

/// 获取当前内存占用百分比（0.0 ~ 100.0）。
///
/// - Linux：解析 `/proc/meminfo` 中的 `MemTotal` 与 `MemAvailable` 字段，
///   计算 `(total - available) / total * 100`；
/// - 其他平台或读取失败：返回 `0.0`。
fn get_memory_load() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    // 格式：MemTotal:    <数值> kB
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total = val.parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:") {
                    // 格式：MemAvailable: <数值> kB
                    if let Some(val) = line.split_whitespace().nth(1) {
                        available = val.parse().unwrap_or(0);
                    }
                }
            }
            if total > 0 {
                return (total.saturating_sub(available) as f64 / total as f64) * 100.0;
            }
        }
        0.0
    }
    #[cfg(not(target_os = "linux"))]
    {
        // 非 Linux 平台暂不支持读取内存信息
        0.0
    }
}

/// 获取当前进程打开的文件句柄数量。
///
/// - Linux：统计 `/proc/self/fd` 目录下的条目数，每个条目对应一个打开的文件描述符；
/// - 其他平台：返回 `0`。
fn get_handle_count() -> usize {
    #[cfg(target_os = "linux")]
    {
        fs::read_dir("/proc/self/fd")
            .map(|e| e.count())
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // 非 Linux 平台暂不支持读取文件句柄数
        0
    }
}
