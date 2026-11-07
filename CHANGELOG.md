# 更新日志

## v0.1.2 · 2026-11-07

- 首次发布 macOS 的 Apple Silicon、Intel 两种 DMG，以及 Ubuntu 24.04 amd64 的 Debian 包。macOS 应用使用临时签名，尚未经过 Apple 公证。
- 启动时自动检查更新。有新版本时，点击右上角更新图标即可在应用内阅读更新说明，点击下载可查看进度。下载支持内置镜像，并按官方清单校验 SHA-256。
- Windows 安装版升级时创建桌面快捷方式。便携版在原目录更新。**正在使用 v0.1.1 或更早便携版的用户**，请退出 Githly，手动下载本版 ZIP 并解压到原目录；后续版本可在应用内升级。
- 仓库状态在前台每 3 秒、后台每 10 分钟核对一次。窗口重新聚焦时，距上次成功读取满 3 秒才立即核对。打开较慢的项目时会在右下角显示提示，空 Stash 提示改为在列表中居中显示。

**English**

- Native packages are available for macOS on Apple Silicon and Intel (DMG) and for Ubuntu 24.04 amd64 (Debian package). The macOS app is ad-hoc signed and has not been notarized by Apple.
- Githly checks for updates at startup. When an update is available, click the icon in the upper right to read the release notes, then click Download to see progress. Downloads support built-in mirrors and are verified against the SHA-256 digest in the official manifest.
- Windows installer upgrades create a desktop shortcut. Portable upgrades keep the app in its existing folder. **If you use portable v0.1.1 or earlier**, quit Githly and extract this release’s ZIP into your current folder. Subsequent portable updates can be applied in the app.
- Githly checks repository status every 3 seconds in the foreground and every 10 minutes in the background. Refocusing checks immediately if the last successful read was at least 3 seconds ago. Slow project opens show a tip in the lower right, and the empty Stash message is centered in the list.

## v0.1.1 · 2026-11-07

- 界面支持简体中文和英文，默认跟随系统语言。在「设置 → 语言」中可手动选择，重启后生效。
- 自动刷新合并了重复的 Git 状态查询，构建提交图时减少了复制。大型标签列表改为流式读取，连续输入历史搜索时可取消旧查询。
- 切换分支前的检查更轻量。切换浏览范围会清除旧刷新错误，读取 worktree 列表失败时会显示原因。未跟踪文件统计也限制了文件系统查询次数。

**English**

- Githly now supports English and Simplified Chinese and follows your system language preferences by default. Choose a language in Settings → Language and restart to apply it.
- Refresh now reads Git status once, graph layout uses fewer copies, and large tag lists are read incrementally. Typing a new history search cancels the previous scan.
- Branch switching uses a lighter preflight check. Changing the history scope clears stale refresh errors, worktree lookup failures are visible, and untracked-file statistics limit filesystem queries.

## v0.1.0 · 2026-11-07

Githly 首个正式版面向 Windows 10/11 x64，使用系统中已安装的 Git。

- 在同一窗口查看工作区、暂存区和未跟踪文件，按路径筛选，查看增删行数。仓库变化后自动刷新，保留当前阅读位置。
- 按分支浏览提交列表或关系图，首批加载约两屏，接近末尾时每次续载最多 100 条。支持查看完整提交说明、双栏文本差异和图片预览。
- 浏览 Stash 和已有 worktree，也可在工作区干净时切换到已有本地分支。
- 支持拖入仓库、最近项目、深浅主题和快捷键。重复启动会唤回原窗口，从命令行传入目录可在原窗口打开项目。
- 提供安装包和便携 ZIP。客户端默认每日检查正式版本，下载后校验安装包大小和 SHA-256，由用户启动安装向导。
