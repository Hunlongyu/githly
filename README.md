<p align="center">
  <img src="assets/branding/githly-256.png" width="80" alt="Githly logo" />
</p>
<h1 align="center">Githly</h1>
<p align="center">面向 AI 开发流程的原生 Git 观察面板</p>
<p align="center">Rust · GPUI Kit · Windows</p>

当代码修改、提交和同步交给 AI 执行时，你仍需要知道项目发生了什么。

Githly 聚焦本地更改、分支信息、提交历史和简单的文件差异，用一个紧凑的桌面面板跟踪项目状态。

> 当前为早期开发版本，优先支持 Windows x64。已接入低频自动刷新、历史滚动续载和软件更新客户端；真实发布升级验收仍待完成。

## 预览

以下为原生客户端查看 Githly 自身仓库的实际截图。

**深色主题**

![Githly 深色主题：项目历史与双栏 Diff](docs/screenshots/native-dark.png)

**浅色主题**

![Githly 浅色主题：项目历史与双栏 Diff](docs/screenshots/native-light.png)

## 已实现

| 功能 | 说明 |
| --- | --- |
| 本地项目 | 选择或拖入已有仓库、最近项目、恢复上次项目、已有 Worktree 浏览 |
| 更改 | 全部 / 未暂存 / 已暂存范围，路径筛选、双重状态及异步增删统计 |
| 分支 | 浏览本地 / 远程 / 全部分支历史，独立入口切换干净工作区的已有本地分支 |
| 上游状态 | 显示本地记录的领先 / 落后提交数，不自动访问远程 |
| 历史 | 首次读取 200 条，接近底部自动续载；关系图 / 列表切换，搜索已加载记录 |
| 自动刷新 | 前台约 15 秒、后台约 60 秒核对本地状态，保留当前阅读快照 |
| 文本 Diff | 默认打开首个文件，双栏换行、增删标色、上一处 / 下一处变化、完整提交说明 |
| 图片 Diff | 修改前后预览，支持 PNG、JPEG、GIF、WebP、BMP、ICO、TIFF、SVG |
| Stash | 固定对象的文件清单与 Diff，包含保存的未跟踪文件 |
| 桌面交互 | 深浅主题、轻微动效、独立设置、面板尺寸持久化、常用快捷键 |
| 大列表 | 更改、分支、历史与 Diff 文件列表使用虚拟渲染 |
| 软件更新 | 每天 / 每三天 / 每周或手动检查，限流时回退到配置的 Release 镜像，下载校验后启动安装向导 |

GIF / WebP 预览首帧。文本 Diff 保持简单的差异标色，不加载完整语言语法库；关闭详情释放文件数据和图片资源。

Githly 调用系统已安装的 Git。缺少 Git 时提供提示、重新检测和手动选择 `git.exe`，不会自行下载或安装 Git。

## 功能边界

应用内不提供 Commit、暂存、丢弃更改、Pull、Push、Fetch、Merge、Rebase 或冲突解决操作，也不建立逐行审阅、批注和已读进度流程。

分支名称用于浏览历史，独立“切换”按钮会改变工作目录。工作区有更改、存在冲突或正在合并 / 变基时拒绝切换，不自动 stash 或强制覆盖。远程跟踪分支和领先 / 落后数量来自本地引用，不代表远程服务器的实时状态。

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
| 刷新本地状态 | 后台自动核对，无刷新按钮和 F5 快捷键 |
| 打开设置 | `Ctrl+,` |
| 搜索历史 / 打开分支 | `Ctrl+F` / `Ctrl+B` |
| 历史及文件列表选择 | 聚焦列表后使用方向键、`Home` / `End` |
| 关闭菜单 / 设置 | `Esc` |

配置保存最近项目、当前项目、Git 路径、主题、动效、历史显示、面板尺寸和更新设置。主动关闭项目后，下次启动显示加载入口；最近项目记录仍保留，可从项目菜单清除。

## 发布与更新

GitHub Actions 在推送 / PR 时执行检查并生成 Windows 产物，`vX.Y.Z` 标签触发 GitHub Release 发布。安装包、便携 ZIP 和更新清单由同一打包脚本生成。具体步骤、镜像格式和当前限制见 [发布与更新](docs/releases.md)。

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
- [开发 TODO](TODO.md)：已完成实现、验收结果和本轮暂缓事项。
- [组件库补丁](vendor/gpui-component/PATCHES.txt)：双栏布局、选择和分段控件修正。第三方源码保留原 [Apache-2.0 许可证](vendor/gpui-component/LICENSE-APACHE)。

历史接近底部时自动加载更早提交，没有页码或翻页按钮。正在浏览旧历史时，外部新增提交会提示有更新，回到顶部后自动核对；已经打开的 Diff 保持阅读快照，重新选择文件可查看最新内容。刷新失败保留原有内容，并放慢重试频率。

仍待完善历史缓存淘汰重载、完整键盘流程、真实仓库长期性能和真实跨版本安装验收。已加载历史随浏览量增长，关闭项目后释放；不将生成历史测试当作 UI 流畅度承诺。完整清单见 [TODO](TODO.md)。
