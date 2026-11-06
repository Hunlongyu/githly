# Githly logo

绿色 G 与分支图案是 Githly 的应用标识，使用透明背景。

- `logo-transparent.png`：使用内置 image_gen 编辑原图，移除浅色矩形背景、白色圆角底板和底板阴影，保留绿色 G 与内部白色分支图案；G 的留白区域也使用透明通道。
- `githly.ico`：16、24、32、48、64、128、256 像素的多尺寸 ICO，32 位 PNG 帧。
- `githly-256.png`：从 ICO 解码得到的 256 像素预览。
- `convert-logo.ps1`：使用 Windows System.Drawing 确定性缩放、封装并逐尺寸解码验证。

`githly.ico` 已通过根目录 `build.rs` 和 `assets/windows/githly.rc` 嵌入 Windows EXE，使用 GPUI 读取的图标资源编号 `1`，供程序文件、原生窗口和任务栏使用。修改 ICO 后，`cargo build` / `cargo run` 会自动重新编译资源；已运行的旧进程需要退出并重新启动。

窗口内部的功能图标使用 GPUI Kit 图标。转换脚本读取透明源图；运行 `./assets/branding/convert-logo.ps1` 可重新生成 ICO 和 256 px PNG。脚本生成的多尺寸校验预览不纳入版本控制。

Windows 进程组与程序文件还需要 EXE 自身的品牌信息，不能仅依赖窗口标题。资源脚本提供 VERSIONINFO：FileDescription / ProductName 为 `Githly`，InternalName / OriginalFilename 保持 `githly` / `githly.exe`；版本号由 build.rs 从 Cargo.toml 生成。图标仍使用同一资源编号 1，不另设白色背景。

