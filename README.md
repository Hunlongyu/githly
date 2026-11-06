<p align="center">
  <img src="assets/branding/githly-256.png" width="80" alt="Githly logo" />
</p>
<h1 align="center">Githly</h1>
<p align="center">面向 AI 开发流程的原生 Git 观察面板</p>
<p align="center">Rust · GPUI Kit · Windows</p>

当代码修改、提交和同步交给 AI 执行时，你仍需要知道项目发生了什么。

Githly 聚焦本地更改、分支信息、提交历史和简单的文件差异，用一个紧凑的桌面面板跟踪项目状态。

> 当前为早期开发版本，优先支持 Windows x64。分支切换、Stash 内容浏览和历史分页仍在规划中。

## 预览

以下为原生客户端查看 Githly 自身仓库的实际截图。

**深色主题**

![Githly 深色主题：项目历史与双栏 Diff](docs/screenshots/native-dark.png)

**浅色主题**

![Githly 浅色主题：项目历史与双栏 Diff](docs/screenshots/native-light.png)

## 已实现

| 功能 | 说明 |
| --- | --- |
| 本地项目 | 选择或拖入已有仓库、切换最近项目、恢复上次打开的项目 |
| 更改 | 已跟踪 / 未跟踪分组、折叠、路径筛选、单行文件列表 |
| 分支浏览 | 搜索本地与远程跟踪分支，分组折叠，当前分支标记 |
| 上游状态 | 显示本地记录的领先 / 落后提交数，不自动访问远程 |
| 历史 | 当前 HEAD 最近 200 条提交；关系图 / 列表切换，搜索与元数据显示配置 |
| 文本 Diff | 选中提交后默认打开首个文件，双栏自动换行、行号、红绿增删标色 |
| 图片 Diff | 修改前后预览，支持 PNG、JPEG、GIF、WebP、BMP、ICO、TIFF、SVG |
| Stash | 只读列表 |
| 桌面交互 | 深浅主题、轻微动效、独立设置弹窗、可调整面板、原生窗口按钮 |
| 大列表 | 更改、分支、历史与 Diff 文件列表使用虚拟渲染 |

GIF / WebP 预览首帧。文本 Diff 保持简单的差异标色，不加载完整语言语法库；关闭详情释放文件数据和图片资源。

Githly 调用系统已安装的 Git。缺少 Git 时提供提示、重新检测和手动选择 `git.exe`，不会自行下载或安装 Git。

## 功能边界

应用内不提供 Commit、暂存、丢弃更改、Pull、Push、Fetch、Merge、Rebase 或冲突解决操作，也不建立逐行审阅、批注和已读进度流程。

目前分支菜单用于浏览，尚不能切换分支；Stash 暂不展示内容。暂存区与工作区合并为相对 HEAD 的更改视图。远程跟踪分支和领先 / 落后数量来自本地引用，不代表远程服务器的实时状态。

## 构建与运行

Windows 开发环境需要：

- Rust MSVC 工具链，项目通过 `rust-toolchain.toml` 固定为 **1.99.1**。
- Visual Studio C++ Build Tools、Windows SDK 和 CMake。
- 系统 Git，或在应用设置中指定现有 `git.exe`。

```powershell
git clone https://github.com/Hunlongyu/githly.git
cd githly
cargo run
```

也可以在启动时打开指定仓库：

```powershell
cargo run -- E:\path\to\repository
```

日常默认 Debug。需要优化构建时使用：

```powershell
cargo build --release
.\target\release\githly.exe
```

构建不限制 Cargo 的默认 CPU 并发。应用只需要系统 Git，不需要 Node.js、WebView 或额外的 Git 库。其他操作系统尚未完成运行验证。

## 配置与快捷键

| 内容 | 位置 / 操作 |
| --- | --- |
| 配置 | `%APPDATA%\githly\config.json` |
| 警告与错误日志 | `%LOCALAPPDATA%\githly\logs` |
| 加载项目 | `Ctrl+O`，或拖入仓库目录 |
| 刷新本地状态 | `F5` |
| 打开设置 | `Ctrl+,` |
| 关闭菜单 / 设置 | `Esc` |

配置保存最近项目、当前项目、Git 路径、主题、动效和历史显示偏好。主动关闭项目后，下次启动显示加载入口；最近项目记录仍保留。

## 开发

```powershell
cargo test --features ui-tests
cargo clippy --all-targets --features ui-tests -- -D warnings
cargo fmt --all -- --check
```

```text
src/                 原生界面、Git 读取、配置与回归测试
assets/              透明应用图标与 Windows 资源
docs/                需求、实现进度与客户端截图
vendor/gpui-component/  固定版本组件库及本地 UI 补丁
```

- [软件开发文档](docs/software-development.md)：功能范围与验收目标。
- [原生开发进度](docs/native-development.md)：当前实现、验证与读取限制。
- [组件库补丁](vendor/gpui-component/PATCHES.txt)：双栏布局、选择和分段控件修正。第三方源码保留原 [Apache-2.0 许可证](vendor/gpui-component/LICENSE-APACHE)。

后续计划包括分支切换、Stash 内容浏览、历史分页、文件变化自动刷新、布局持久化及安装包。
