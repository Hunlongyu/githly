<p align="center">
  <img src="assets/branding/githly-256.png" width="80" alt="Githly 图标" />
</p>
<h1 align="center">Githly</h1>
<p align="center">面向 AI 开发流程的 Windows 原生 Git 观察面板</p>
<p align="center">
  <a href="https://github.com/Hunlongyu/githly/releases"><img src="https://img.shields.io/github/downloads/Hunlongyu/githly/total?style=flat-square" alt="GitHub Release 资产累计下载次数" /></a>
</p>

当 AI 或其他工具在后台修改代码、创建提交时，Githly 会自动刷新本地仓库状态。你可以在同一窗口看到哪些文件变了、工作区和暂存区各改了什么，也可以沿分支图找到一次提交，查看它的文件差异。

## 下载与运行

需要 **Windows 10 或 11（x64）** 和系统 Git。若 Githly 未找到 Git，可在设置中指定已有的 `git.exe`。

从正式发布页选择 Windows x64 安装包或便携 ZIP。每个版本都附有更新说明和 `SHA256SUMS.txt`，可用来核对下载文件。[下载最新正式版](https://github.com/Hunlongyu/githly/releases/latest)

| 使用方式 | 如何启动 |
| --- | --- |
| 安装版 | 运行文件名以 `-windows-x64-setup.exe` 结尾的安装包，跟随向导为当前用户安装，之后从开始菜单打开 Githly。 |
| 便携版 | 解压文件名以 `-windows-x64.zip` 结尾的便携包，直接运行其中的 `Githly.exe`。 |

如需试用开发版，可登录 GitHub，在成功构建的 **Artifacts** 区域下载 `windows-x64`。开发构建保留 14 天。[Windows 构建列表](https://github.com/Hunlongyu/githly/actions/workflows/windows.yml)

## 第一次使用

同一 Windows 登录会话只运行一个 Githly 窗口。再次启动会唤回已有窗口，已最小化时会恢复。启动时传入仓库目录，会在该窗口打开对应项目。

1. 把已有的本地仓库目录拖进窗口，或按 `Ctrl+O` 选取目录。用过的仓库可以从「最近打开」中直接进入。
2. 在左侧「更改」面板切换「全部」「未暂存」「已暂存」，点击文件查看差异。同一文件既有已暂存又有未暂存修改时，可分别查看两部分。
3. 在右侧点击一次提交，下方会打开它的第一个变更文件。点击其他文件可切换差异，用「上一处变化」「下一处变化」在修改位置之间跳转。
4. 点击顶部分支名可浏览其他分支的历史。要改变工作目录的检出分支，请使用已有本地分支旁的「切换」按钮。切换要求工作区干净且没有冲突或进行中的合并、变基。

## 界面预览

![Githly 深色主题，左侧为文件更改，右侧为提交历史和双栏差异](docs/screenshots/native-dark.png)

<details>
<summary>查看浅色主题</summary>

![Githly 浅色主题，显示同一仓库的历史和双栏差异](docs/screenshots/native-light.png)

</details>

## 查看仓库

| 想了解什么 | 在 Githly 中怎么看 |
| --- | --- |
| 改动有多大 | 「更改」按已跟踪和未跟踪文件分组，支持路径筛选，显示文件及合计增删行数。 |
| 分支如何分叉、合并 | 历史可切换关系图和列表，支持当前分支、指定分支或全部分支。当前源码首批读取约两屏，滚动到距末尾约半屏时再加载最多 100 条。搜索范围是已加载的提交。 |
| 分支与上游相差多少 | 查看领先、落后提交数。远程跟踪分支及这些计数来自本地引用，外部工具 Fetch 后面板会自动刷新。 |
| 仓库在阅读期间又有了修改 | 前台约每 15 秒核对本地状态，后台约每 60 秒核对一次。浏览较早历史时会提示有新提交，回到顶部即可更新。已打开的差异保留阅读位置，重新选择文件可查看最新内容。 |
| 一次提交具体改了什么 | 查看提交正文和逐文件双栏文本差异。常见图片可并排预览修改前后版本，GIF 和 WebP 显示首帧。 |
| Stash 和其他工作目录里有什么 | 「Stash」可查看保存的文件及差异，包括当时保存的未跟踪文件。顶栏的 worktree 菜单可查看其他工作目录的路径、分支和更改状态，并打开对应目录。 |

提交、暂存、推送、拉取和解决冲突等操作请使用 Git 或已有开发工具。Githly 会在下一次自动刷新时显示结果。

## 软件更新

按 `Ctrl+,` 打开设置，在「软件更新」中可以立即检查，也可选择每天（默认）、每三天、每周或仅手动检查。更新来源是 GitHub 正式 Release。

发现新版后，点击「更新说明」查看变化，或点击「下载更新」。客户端核对安装包大小和 SHA-256 后会显示「安装更新…」，点击后启动 Windows 安装向导。便携版用户若想保留解压运行的方式，可从发布页下载新版 ZIP。设置中也可配置 GitHub 限流时使用的 HTTPS 镜像。[发布与更新说明](docs/releases.md)

## 快捷键与数据位置

| 操作 | 快捷键 |
| --- | --- |
| 打开本地仓库 | `Ctrl+O` |
| 搜索已加载的提交 | `Ctrl+F` |
| 打开分支菜单 | `Ctrl+B` |
| 在已聚焦的历史或文件列表中选择 | `↑` / `↓`，`Home` / `End` |
| 打开设置 / 关闭弹层 | `Ctrl+,` / `Esc` |

最近项目、主题、面板尺寸、Git 路径和更新偏好保存在 `%APPDATA%\githly\config.json`。遇到读取或更新错误时，可在 `%LOCALAPPDATA%\githly\logs` 查看日志。

## 从源码运行

准备 Rust MSVC 工具链、Visual Studio C++ Build Tools、Windows SDK 和 CMake。仓库通过 `rust-toolchain.toml` 固定 Rust 版本。

```powershell
git clone https://github.com/Hunlongyu/githly.git
cd githly
cargo run --locked
```

在已有源码目录中，可用 `cargo run --locked -- "E:\path\to\repository"` 直接打开指定仓库。实现进度、测试与打包步骤见[原生开发进度](docs/native-development.md)、[开发清单](TODO.md)和[发布文档](docs/releases.md)。
