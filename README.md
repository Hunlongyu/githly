<div align="center">
  <img src="assets/branding/githly-256.png" width="72" alt="" />
  <h1>Githly</h1>
  <p>A desktop browser for local Git repositories. File changes and commit history refresh as you work in your editor, in the terminal, or with an AI coding assistant.</p>
  <p><a href="https://github.com/Hunlongyu/githly/releases/latest"><strong>Download the latest release</strong></a> · <a href="CHANGELOG.md">Changelog</a> · <a href="README.zh-CN.md">简体中文</a></p>
</div>

![Githly in dark mode with changed files on the left and commit history and a side-by-side diff on the right](docs/screenshots/native-dark.png)

[View the light theme](docs/screenshots/native-light.png)

## Install

Install Git, then choose the package for your system.

| System | Installation |
| --- | --- |
| Windows x64 | Run the `-setup.exe` installer, or extract the portable ZIP and run `Githly.exe`. |
| macOS | Choose `arm64.dmg` for Apple Silicon or `x64.dmg` for Intel. Open the disk image and drag Githly to Applications. |
| Ubuntu 24.04 x64 | Run `sudo apt install ./Githly-*-linux-x64.deb` in the download directory. |
| Linux x64 portable | Run `chmod +x Githly-*-linux-x64.AppImage`, then open the file. |

Githly checks for updates at startup. To check again, open **Settings → About** and click **Check for updates**. When a release is available, click **View update** there or use the download icon in the upper right of the main window to read the release notes and update.

To carry your settings on a USB drive on Windows, quit Githly, copy `%APPDATA%\githly\config.json` next to `Githly.exe`, then start that copy. You can also create a `config.json` containing `{}` beside the executable. Githly reads and saves that file when present. Otherwise it uses the AppData configuration, creating it with defaults if needed.

## Use

Drag a repository folder into the window or press `Ctrl+O` to open it. On macOS, use `⌘` in place of `Ctrl`.

- Select a file in **Changes** to compare text or images in your working tree with the current commit. The two status positions show index and working tree changes, respectively.
- Select a commit to inspect its changes. Press `Ctrl+F` to search loaded commits or `Ctrl+B` to browse another branch.
- Open **Stash** to inspect saved changes or **Worktrees** in the top bar to open another working directory.

## Build from source

Clone the repository and run `cargo run --locked` from its root. Windows requires Visual Studio C++ Build Tools, the Windows SDK, and CMake. On macOS, install Xcode and its command-line tools. For Linux dependencies, see the [build workflow](.github/workflows/desktop.yml).

[Settings and logs](docs/native-development.md#配置和日志) · [Development notes](docs/native-development.md) · [Packaging and releases](docs/releases.md)
