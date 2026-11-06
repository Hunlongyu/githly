# Githly 原生开发进度

当前版本为 0.1.0，优先在 Windows x64 开发和验证。

## 技术与构建

- Rust 1.99.1，MSVC 工具链；GPUI Kit 0.9.0，默认 GPUI 后端。
- 系统 Git CLI，异步读取；不内置 Git，不引入 libgit2。
- 日常使用 `cargo build` / `cargo run`，仅在需要发布构建时添加 `--release`。
- Cargo 使用默认 CPU 并发，没有固定 jobs 限制。
- Windows 图标和版本信息由 `build.rs` 嵌入 EXE；主线程预留 8 MiB 栈，页面按需提交。
- `vendor/gpui-component` 保留 Diff 布局、文本选择和分段控件的必要补丁，详见其中的 `PATCHES.txt` 与 `LICENSE-APACHE`。

## 已实现

- 加载本地 Git 仓库、拖入目录、最近项目、恢复上次打开的项目。
- 搜索与折叠本地 / 远程跟踪分支，显示当前分支、上游及本地记录的领先 / 落后数量。
- 更改列表、已跟踪 / 未跟踪分组、路径筛选；Stash 列表只读。
- 当前 HEAD 最近 200 条历史，支持关系图 / 普通列表、搜索和元数据显示配置。
- 点击提交默认打开第一个变更文件；双栏文本 Diff 等宽换行，保留行号和增删标色。
- 图片 Diff：PNG、JPEG、GIF、WebP、BMP、ICO、TIFF、SVG；GIF / WebP 预览首帧。
- 更改、历史、分支及 Diff 文件列表按可见范围渲染。
- 关闭详情释放文件数据、旧 Diff 状态和图片纹理；过期异步结果不会覆盖当前选择。
- 深浅主题、轻微动效、独立设置弹窗、原生标题栏拖动与窗口按钮。
- Git 缺失提示、重新检测、手动选择现有 git.exe、官网入口。

## 边界与后续工作

- 分支切换、Stash 内容浏览、超过 200 条历史的继续加载仍待实现。
- 暂存区和工作区目前合并为相对 HEAD 的更改视图。
- 文件监测自动刷新、完整键盘导航、布局持久化和安装包仍待完善。
- 不提供提交、暂存、丢弃更改、Pull、Push、Fetch 或冲突解决操作。
- 远程分支及领先 / 落后数量只基于本地已有引用，不代表服务器实时状态。
- Windows 之外的平台尚未完成运行验证。

## 配置和日志

配置：`%APPDATA%\githly\config.json`。保存 Git 路径、最近项目、当前项目、主题、动效和历史显示偏好。损坏或不兼容的配置不被静默覆盖。

普通运行日志：`%LOCALAPPDATA%\githly\logs`，默认记录警告和错误。

临时内存诊断已移除：没有桌面 JSONL 采样、分配器统计、诊断 feature、自检参数或 Ctrl+Shift+M 入口。已完成的内存优化与回归测试继续保留。

## 验证

```powershell
cargo test --features ui-tests
cargo clippy --all-targets --features ui-tests -- -D warnings
cargo build
```

2026-11-06：以上验证通过，26 项测试覆盖真实 Git 仓库读取、不同文件 Diff、图片解码与纹理释放、虚拟列表、双栏布局与文本选择、关闭详情释放、配置迁移等行为。

## 读取限制

Git 标准输出上限 8 MiB，错误输出上限 64 KiB，查询超时 20 秒；单文件文本 Diff 上限 2 MiB，未跟踪文本上限 1 MiB。超限显示错误。

图片单侧原始文件上限 8 MiB，位图解码预算 64 MiB，单边最大 16,384 像素；预览最长边不超过 1,600 像素。SVG 不读取外部文件引用。

这些阈值用于控制读取和渲染成本，并非完整的大型仓库性能承诺。
