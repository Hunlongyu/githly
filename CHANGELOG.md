# 更新日志

## v0.1.9 · 2026-11-10

- Windows 便携版支持随 U 盘携带设置。把 `config.json` 放在 `Githly.exe` 旁，程序就会在该目录读取和保存设置。同目录没有配置时，沿用 AppData 中的配置，两处都没有时在 AppData 中创建默认配置。
- 修复英文界面中提交搜索框提示文字显示不全的问题。

**English**

- Windows users can carry their settings on a USB drive by placing `config.json` beside `Githly.exe`. Githly reads and saves that file when present. Otherwise it uses the AppData configuration, creating it with defaults on first use.
- Fixed clipped placeholder text in the commit search field when using the English interface.

## v0.1.8 · 2026-11-09

- 重新设计设置窗口、通用和关于页面，调整外观、语言、动效、Git 与版本信息的布局。
- 外观支持跟随系统，并在系统主题变化时自动切换。首次使用时，外观、语言和界面动效默认跟随系统。

**English**

- Redesigned the Settings window, General and About pages, including appearance, language, motion, Git and version information.
- Appearance can follow system theme changes while Githly is running. Appearance, language and motion follow system settings by default on a fresh installation.

## v0.1.7 · 2026-11-09

- 在「设置 → 关于」中点击「检查更新」，即可在 Githly 运行期间检查最新稳定版。发现新版后可直接查看更新说明、下载并安装。重复检查同一版本时会保留已验证的安装包。
- Windows 便携版在原目录替换程序，检查更新时会清理旧版留下的空临时目录。用于解压更新包的 `zip` 已升至 9.5.0。

**English**

- Click **Check for updates** in Settings → About to check for the latest stable release while Githly is running. Open the release notes there to download and install the update. Checking again keeps a verified download when the release and package still match.
- Windows portable updates now replace the executable in its own directory and clean up empty staging folders left by older versions. The ZIP extraction library has been updated to 9.5.0.

## v0.1.6 · 2026-11-08

- 历史面板显示当前列表中的提交数，搜索后显示匹配数。移除滚动加载提示，接近列表底部时仍会自动加载。
- 更改面板合并已暂存和未暂存文件，用两位状态标记区分来源。文本差异、图片差异和增删行数使用同一个仓库快照中的基准提交。
- 在「设置 → 关于」中可打开 GitHub Issues 反馈问题。中英文 README 精简为安装、使用和源码构建三节。

**English**

- The history toolbar counts the commits in the list, including just the matches after a search. More commits load automatically as you scroll, without a persistent hint.
- The Changes panel lists staged and unstaged files together. Two status positions distinguish index and working tree changes. Text diffs, image diffs, and line counts use the base commit captured with the repository status.
- Open GitHub Issues from Settings → About. Both READMEs now focus on installation, usage, and building from source.

## v0.1.5 · 2026-11-08

- 在「设置 → 通用 → 语言」切换语言后，主窗口、设置、搜索提示和状态消息立即刷新。保留当前仓库和搜索内容。中文版统一使用「储藏」「工作树」和「标签」。
- 设置新增「关于」，可查看版本、开发者并打开 GitHub 仓库。欢迎页使用应用图标，最近打开列表的清理按钮移至标题旁。
- 改进特殊文件名的差异预览。未跟踪文件保留原始换行，可为名为 `-` 的文件生成可应用的补丁。最近记录中的非 UTF-8 路径也能随设置保存。
- 检查更新时优先从 GitHub 正式版本取得当前平台所需安装包的大小和 SHA-256。缺少摘要时读取对应版本的更新清单，双语更新说明按当前语言显示。

**English**

- Changing the language in Settings → General immediately updates the main window, settings, search placeholders, and status messages. The current repository and search text are preserved.
- Settings now has an About page with the version, developer, and GitHub repository. The welcome screen uses the application icon, and the clear-recents button sits beside the list heading.
- Improved diff previews for unusual filenames. Untracked patches preserve line endings and apply correctly for a file named `-`. Settings also preserve recent project paths that contain non-UTF-8 bytes.
- Update checks read the package size and SHA-256 for the current distribution from the GitHub release, falling back to its versioned manifest when the digest is unavailable. Bilingual release notes follow the selected language.

## v0.1.4 · 2026-11-07

- 修复检测到新版后右上角的云下载按钮显示为空白的问题，一并补齐空 Stash 列表的图标。打开设置即可在标题旁查看正在运行的 Githly 版本。
- Windows 安装版通过应用内更新完成升级后，安装向导会自动打开新版。Windows 便携 ZIP 和 Linux AppImage 会在原位置替换程序，再自动重启。
- macOS 校验并替换应用包后自动重新打开。Ubuntu 安装版会请求系统授权，通过 APT 更新，并在确认新版已安装后重启。
- 从 v0.1.3 或更早的便携版首次升级到本版时，文件替换后仍需手动重新打开一次。旧版 macOS 和 DEB 客户端也沿用原来的安装流程。

**English**

- The cloud download button is now visible when an update is available. The empty stash list icon is also included. Settings shows the running Githly version beside the title.
- On Windows, Setup launches the new version after an in-app upgrade. The portable Windows ZIP and Linux AppImage update in place and restart automatically.
- macOS reopens the updated application bundle after verification and replacement. Ubuntu requests system authorization for APT and restarts after confirming the installed version.
- When upgrading a portable copy from v0.1.3 or earlier to this release, reopen it once after the file is replaced. Older macOS and DEB clients also use their previous installation flow.

## v0.1.3 · 2026-11-07

- Linux x64 新增 AppImage 便携包，下载后赋予执行权限即可运行。应用内更新会替换原 AppImage，重启后使用新版。
- 修复 Git 输出大量警告时读取仓库失败的问题。未跟踪文件的差异现在能正确展示空文件、特殊路径、CRLF 和缺少末尾换行的内容，并遵守 2 MiB 的差异大小上限。
- 大型仓库滚动时复用文件列表索引，搜索和绘制提交图时减少复制。切换分支浏览范围会取消旧搜索，接近列表底部时会合并重复的分页请求。
- 并行读取仓库和提交详情。前台仍约每 3 秒检查文件状态，约每 30 秒核对 stash 和 worktree 列表。展开 worktree 菜单时最多同时查询 4 个目录的状态。
- 图片差异使用原生棋盘格绘制，带文字的 SVG 复用系统字体。关闭面板后更新下载可继续完成，多进程保存配置时使用独立临时文件。
- Windows 便携 ZIP 解压后只包含 Githly.exe。发布页按系统和架构列出下载入口。

**English**

- Linux x64 now has a portable AppImage. Mark the downloaded file as executable to run it. In-app updates replace the original AppImage and take effect on restart.
- Repository reads now tolerate large volumes of Git warnings. Untracked file diffs preserve empty files, quoted paths, CRLF line endings and missing final newlines, and respect the 2 MiB diff limit.
- Scrolling reuses the file list index. Search and commit graph layout make fewer copies. Changing the history scope cancels the old search, and scrolling near the end coalesces page requests.
- Independent repository and commit detail reads run in parallel. Foreground file status checks continue about every 3 seconds. Stash and worktree lists refresh about every 30 seconds. Opening the worktree menu checks up to four directories at a time.
- Image diffs use native checkerboard rendering, and SVG text previews reuse system fonts. Update downloads can finish after the panel closes. Concurrent configuration saves use separate temporary files.
- The Windows portable ZIP extracts to a single Githly.exe. Release downloads are grouped by operating system and architecture.

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
