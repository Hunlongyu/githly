<div align="center">
  <img src="assets/branding/githly-256.png" width="88" alt="Githly 图标" />
  <h1>Githly</h1>
  <p><strong>面向 AI 开发流程的 原生 Git 观察面板</strong></p>

  <p>
    <a href="https://github.com/Hunlongyu/githly/releases/latest"><img src="https://img.shields.io/github/v/release/Hunlongyu/githly?style=flat-square&label=Release&color=2d7d9a" alt="最新版本" /></a>
    <a href="https://github.com/Hunlongyu/githly/actions/workflows/windows.yml"><img src="https://img.shields.io/github/actions/workflow/status/Hunlongyu/githly/windows.yml?style=flat-square&label=Windows%20Build" alt="Windows 构建状态" /></a>
    <a href="https://github.com/Hunlongyu/githly/releases"><img src="https://img.shields.io/github/downloads/Hunlongyu/githly/total?style=flat-square&label=Downloads&color=67b7d1" alt="GitHub Release 累计下载次数" /></a>
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-0078D4?style=flat-square" alt="Windows、macOS 和 Linux" />
    <img src="https://img.shields.io/badge/Built%20with-Rust%20%2B%20GPUI-dea584?style=flat-square" alt="基于 Rust 与 GPUI 构建" />
  </p>

  <p>
    <a href="README.md">English</a> · <strong>简体中文</strong>
  </p>

  <p>
    <a href="#核心特性">核心特性</a> ·
    <a href="#界面预览">界面预览</a> ·
    <a href="#下载与安装">下载与安装</a> ·
    <a href="#快速上手">快速上手</a> ·
    <a href="#功能指南">功能指南</a> ·
    <a href="#快捷键与数据位置">快捷键</a> ·
    <a href="#从源码构建">源码构建</a>
  </p>
</div>

---

**Githly** 是一款轻量、专注于代码状态观察的 原生 Git 桌面面板，专为 AI 辅助开发工作流打造。当 AI 编程助手、命令行工具或其他编辑器在后台修改代码、创建提交时，Githly 会自动刷新本地仓库状态。你可以在同一窗口内直观查看工作区与暂存区变更、双栏文件差异、分支关系图、Stash 以及多个 Worktree 状态，而不会干扰正在运行的开发工具。

## 界面预览

![Githly 深色主题，左侧为文件更改，右侧为提交历史和双栏差异](docs/screenshots/native-dark.png)

<details>
<summary><strong>查看浅色主题</strong></summary>
<br />

![Githly 浅色主题，显示同一仓库的历史和双栏差异](docs/screenshots/native-light.png)

</details>

## 核心特性

- **专为 AI 与命令行流程设计** — 前台约每 3 秒、后台约每 600 秒自动核对本地状态，不争抢仓库锁，也不干扰外部 Git 操作。
- **保留阅读位置与快照** — 后台发生代码变更时，已打开的 Diff 视图会保留当前阅读位置与快照；浏览较早历史时若有新提交产生，会温和提示而不强行跳转视口。
- **双栏文本与图片 Diff** — 支持按「全部」「未暂存」「已暂存」查看变更，提供变化位置快速跳转，并支持常见图片格式（`PNG`、`JPEG`、`GIF`、`WebP`、`BMP`、`ICO`、`TIFF`、`SVG`）修改前后并排预览。
- **流式分支关系图** — 支持当前分支、指定分支或全部分支的关系图与列表视图切换，随滚动按需流式续载历史记录。
- **Stash 与 Worktree 浏览** — 直接查看 Stash 中保存的文件及差异（含未跟踪文件），并通过顶栏菜单查看和打开其他 Git Worktree 工作目录。
- **Rust + GPUI 原生性能** — 异步调用系统 Git，配合视口虚拟化列表与严格的内存预算，保持轻量响应。

## 下载与安装

各平台均需安装 Git。如果环境变量中找不到 Git，可在设置中选择可执行文件。

Githly 默认按系统语言偏好顺序选择简体中文或英文，均不匹配时使用英文。在 **设置 → 语言** 中可手动切换，重启 Githly 后生效。

前往 **[GitHub Releases 最新正式版](https://github.com/Hunlongyu/githly/releases/latest)** 选择安装包或便携 ZIP。每个正式版本均附带更新说明和 `SHA256SUMS.txt` 校验文件。

| 版本类型 | 文件名后缀 | 启动与使用方式 |
| --- | --- | --- |
| **Windows x64 安装版** | `-windows-x64-setup.exe` | 为当前用户安装后，从开始菜单或桌面快捷方式打开 **Githly**。 |
| **Windows x64 便携版** | `-windows-x64.zip` | 解压到任意目录，直接运行其中的 `Githly.exe`。 |
| **macOS** | `-macos-arm64.dmg`（Apple Silicon）或 `-macos-x64.dmg`（Intel） | 打开 DMG，将 Githly 拖入「应用程序」。应用使用临时签名，尚未经过 Apple 公证。 |
| **Ubuntu 24.04 amd64** | `-linux-x64.deb` | 在下载目录运行 `sudo apt install ./Githly-*-linux-x64.deb`，之后从应用菜单或通过 `githly /path/to/repository` 启动。需要 Vulkan 驱动和 X11 或 Wayland。 |
| **Linux x86_64 AppImage**（CI 测试包，v0.1.2 后的正式版提供） | `-linux-x64.AppImage` | 运行 `chmod +x Githly-*-linux-x64.AppImage` 后直接启动文件。构建基线为 Ubuntu 22.04，需要系统 Git、Vulkan 驱动和 X11 或 Wayland。 |

登录 GitHub 后，也可在 main 分支成功构建的 Artifacts 中下载测试包，保留 14 天。[Windows 构建](https://github.com/Hunlongyu/githly/actions/workflows/windows.yml) · [macOS 和 Linux 构建](https://github.com/Hunlongyu/githly/actions/workflows/desktop.yml)

macOS 上的应用快捷键使用 Command，对应下文的 Ctrl。

## 快速上手

同一用户会话只运行一个 Githly 窗口。再次从命令行启动会唤回已有窗口，传入仓库目录会在该窗口中打开对应项目。

1. **打开仓库** — 把本地 Git 仓库目录拖进窗口，或按 `Ctrl+O` 选取目录；用过的仓库可直接从「最近打开」进入。
2. **查看工作区更改** — 在左侧「更改」面板切换「全部」「未暂存」「已暂存」，点击文件查看双栏差异。同一文件既有已暂存又有未暂存修改时，可分别查看两部分。
3. **浏览提交历史** — 在右侧点击任意一次提交，下方会默认打开它的第一个变更文件。点击其他文件可切换差异，使用「上一处变化」「下一处变化」在修改位置之间快速跳转。
4. **浏览与切换分支** — 点击顶部分支名（`Ctrl+B`）可浏览任意本地或远程跟踪分支的历史。若要改变当前工作目录实际检出的分支，请点击已有本地分支旁的「切换」按钮（要求工作区干净，且没有冲突或进行中的合并、变基）。

## 功能指南

| 想了解什么 | 在 Githly 中怎么看 |
| --- | --- |
| **改动有多大** | 「更改」面板按已跟踪和未跟踪文件分组，支持路径筛选，并在后台异步计算单文件及合计增删行数。 |
| **分支如何分叉、合并** | 历史支持切换「关系图」和「列表」，可选当前分支、指定分支或全部分支。首批按列表高度读取约两屏，滚动到距末尾约半屏时自动续载（每批最多 100 条）。搜索范围限定在已加载的提交。 |
| **分支与上游相差多少** | 顶栏显示领先、落后提交数。远程跟踪分支及计数基于本地引用，外部工具执行 `git fetch` 后面板会在下次刷新时自动更新。 |
| **阅读期间仓库又有了修改** | 前台约每 3 秒核对一次本地状态，后台约每 10 分钟核对一次。窗口重新聚焦时，距上次成功读取满 3 秒就立即核对。浏览较早历史时会提示有新提交，回到顶部即可更新。已打开的差异保留阅读位置，重新选择文件即可查看最新内容。 |
| **一次提交具体改了什么** | 查看完整提交正文和逐文件双栏等宽文本差异（含行号与增删标色）。常见图片可并排预览修改前后版本，`GIF` 和 `WebP` 显示首帧。 |
| **Stash 和其他工作目录里有什么** | 「Stash」可查看保存的文件清单及差异，包括保存时的未跟踪文件。顶栏的 `worktree` 菜单可查看其他工作目录的路径、分支和更改状态，并直接打开对应目录。 |

> **职责边界说明：** 提交、暂存、丢弃修改、推送、拉取和解决冲突等写入操作请继续使用 Git CLI、IDE 或 AI 编程工具完成。Githly 专注于零干扰的本地状态观察（仅提供干净工作区下的本地分支切换），所有外部改动都会在下一次自动刷新时呈现。

## 软件更新

Githly 启动时自动检查更新。有新版本时，窗口右上角会出现下载图标。点击图标即可查看更新日志，再点「下载更新」会显示进度条、百分比和已下载量。关闭弹窗后下载会继续，可从同一图标重新查看。

Windows 安装版通过安装向导升级，并创建桌面快捷方式。便携版下载 ZIP，在原目录替换程序，重启后生效。macOS 和 Linux 打开相应平台的安装包。直连下载失败时，Githly 自动尝试内置镜像，并使用 GitHub 提供的文件大小和 SHA-256 校验下载内容。

## 快捷键与数据位置

| 操作 | 快捷键 |
| --- | --- |
| 打开本地仓库 | `Ctrl+O` |
| 搜索已加载的提交 | `Ctrl+F` |
| 打开分支菜单 | `Ctrl+B` |
| 在已聚焦的历史或文件列表中选择 | `↑` / `↓`，`Home` / `End` |
| 打开设置 / 关闭弹层 | `Ctrl+,` / `Esc` |

### 配置文件与日志

macOS 在 `~/Library/Application Support/githly` 保存配置和日志。Linux 使用 XDG 配置和数据目录，默认分别为 `~/.config/githly` 和 `~/.local/share/githly`。Windows 路径如下。

- **配置文件：** `%APPDATA%\githly\config.json`（保存最近项目、主题、面板尺寸和 Git 路径）。
- **运行日志：** `%LOCALAPPDATA%\githly\logs`（遇到读取或更新错误时可在此查看诊断日志）。

## 从源码构建

### 环境要求

- Windows 使用 **Rust MSVC 工具链**（仓库通过 `rust-toolchain.toml` 自动固定版本）
- **Visual Studio C++ Build Tools** 与 **Windows SDK**
- **CMake**
- macOS 需安装 Xcode 和命令行工具。Linux 原生依赖见[构建工作流](.github/workflows/desktop.yml)。

### 克隆与运行

```powershell
git clone https://github.com/Hunlongyu/githly.git
cd githly

# 编译并启动 Githly
cargo run --locked

# 或直接打开指定本地仓库
cargo run --locked -- "E:\path\to\repository"
```

### 相关文档

- **[原生开发进度](docs/native-development.md)** — 技术架构、内存与读取边界、流式历史设计与验证基准。
- **[发布与更新文档](docs/releases.md)** — 本地打包脚本、Inno Setup 安装包构建、更新清单与镜像规则。
- **[开发清单 (TODO)](TODO.md)** — 已完成功能与暂缓事项记录。
- **[更新日志 (CHANGELOG)](CHANGELOG.md)** — 版本演进与发布说明。
