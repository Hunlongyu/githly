# Githly

面向 AI 开发流程的原生 Git 状态与历史观察面板。Rust + GPUI Kit，使用用户系统中已有的 Git。

## Windows 开发

需要 Rust MSVC 工具链、Visual Studio C++ Build Tools、Windows SDK、CMake，以及系统 Git。

```powershell
cargo run
cargo run -- E:\path\to\repository
cargo test
cargo test --features ui-tests
cargo clippy --all-targets -- -D warnings
```

当前固定 Rust 1.99.1 与 GPUI Kit 0.9.0；`rust-toolchain.toml` 自动选择工具链。使用参考 VS Code 2026 的原生深浅色主题。

配置存放在 `%APPDATA%\githly\config.json`，保存最近项目、当前项目、主题与历史显示偏好；下次启动恢复上次打开的项目。设置通过右上角齿轮或 Ctrl+, 打开独立弹窗，可切换主题、选择自定义安装位置的 `git.exe`，不内置 Git，不修改系统 PATH。

运行日志位于 `%LOCALAPPDATA%\githly\logs`。工具链安装在当前用户的 `%USERPROFILE%\.cargo` / `.rustup`，可供所有项目使用；新终端会自动读取更新后的 PATH。

日常默认 Debug；需要 Release 时使用 `cargo build --release`。临时内存诊断已移除，运行日志仅保留 AppData 中的警告与错误日志。

完整需求见 [软件开发文档](docs/software-development.md)，当前实现范围与限制见 [原生开发进度](docs/native-development.md)。
