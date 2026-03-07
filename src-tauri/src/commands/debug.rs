//! debug.rs
//! 仅在 debug 构建（`pnpm run tauri dev`）下编译的调试专用命令。
//!
//! 所有命令均使用 `#[cfg(debug_assertions)]` 守卫，
/// 主动触发一个 panic，用于测试 panic_report 的崩溃报告是否正常工作。
///
/// # 使用方式
/// 在浏览器控制台或前端代码中调用：
/// await invoke("debug_panic")
///
///
/// 触发后程序会立即崩溃，panic hook 会：
/// 1. 在panic-log文件夹生成日志文件
/// 2. 将报告内容输出到终端 stderr
/// 3. 以退出码 0xFFFF 终止进程
#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_panic() {
    // 主动 panic，消息会出现在 Panic_Report 的 Message 字段中
    panic!("手动触发的测试 panic：用于验证 panic_report 崩溃报告功能是否正常工作");
}
