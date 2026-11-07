# Windows 发布与更新

## 构建与发布

`.github/workflows/windows.yml` 使用 Windows 2025 Runner、仓库固定的 Rust 1.99.1 和 Cargo.lock。推送、PR、手动运行执行格式检查、测试、Clippy、Release 构建及打包；仅稳定 `vX.Y.Z` 标签会发布 GitHub Release，且必须与 Cargo.toml 版本完全一致。

本地打包使用 PowerShell 7：

```powershell
cargo build --release --locked
./scripts/install-inno.ps1
./scripts/package.ps1 -Tag v0.1.2
```

编译器脚本从官方不可变版本地址下载 Inno Setup 6.7.4，验证固定 SHA-256、Windows 签名及发布者，便携解压到 `target/tools`，不注册文件关联。也可使用 PATH 中已有的 Inno Setup 6。仅需要便携包时使用 `./scripts/package.ps1 -SkipInstaller`。

`dist/release` 包含：

- `Githly-X.Y.Z-windows-x64.zip`
- `Githly-X.Y.Z-windows-x64-setup.exe`
- `update-windows-x64-portable.json`：便携 ZIP 的版本、更新说明、字节数和 SHA-256。
- `update.json`：版本、版本说明地址、安装包地址 / 字节数 / SHA-256。
- `SHA256SUMS.txt`：以上文件的校验值。

确认版本和产物后，由维护者提交变更并推送对应版本标签。工作流使用 CHANGELOG.md 中对应版本的条目生成 Release 说明，并附上该标签的安装包和便携包下载入口。

安装包最低 Windows 10，按用户安装到 `%LOCALAPPDATA%/Programs/Githly`，不要求管理员权限。升级关闭占用文件的应用前使用正常安装向导，不强行终止；完成后可启动新版。用户配置保留在 `%APPDATA%/githly`，卸载也不删除配置。安装和升级都会创建当前用户的桌面快捷方式。便携版使用独立 ZIP，应用更新时在原目录暂存并替换当前程序，关闭再打开后生效。

## macOS 和 Linux 安装包

`.github/workflows/desktop.yml` 在原生运行器上执行测试、Clippy、Release 构建、打包和启动检查。macOS 无 Metal 设备时验证应用签名、包结构及无窗口界面测试。普通分支构建按架构上传 DMG 和 Debian 测试包。发布稳定标签时，发布任务会等待各平台构建完成，核对清单后将安装包上传到 GitHub Release。

在对应平台运行 `cargo build --release --locked` 后，可用 `bash scripts/package-unix.sh <platform>` 生成同样的包。平台标识为 `macos-arm64`、`macos-x64`、`linux-x64`。Windows x64 客户端继续读取 `update.json`；新平台读取 `update-<platform>.json`，校验版本、下载地址、字节数和 SHA-256，用户点击安装后由系统打开相应包。

## 客户端检查与镜像

每次启动自动检查一次，沿用服务端限流冷却时间。检测到新版本后在窗口右上角显示更新图标。

客户端先查询 GitHub 最新正式 Release，按平台、架构和发行形式选择资产，并读取 Release 正文、文件大小与 SHA-256。API 不可用或处于限流冷却期时，从 GitHub Release 读取对应发行形式的更新清单。点击更新图标直接打开新版本日志。用户在同一弹窗启动下载、查看进度并应用更新。

更新包优先直连，失败后依次尝试内置的 [GH-Proxy](https://gh-proxy.com/en/docs/github-accelerator/) 和 [GHFast](https://ghfast.top/)。镜像只负责传输包内容，版本和摘要由 GitHub 提供。重试从零重新显示进度，长度或摘要不匹配的文件不会交给安装程序。

网络使用 Windows WinHTTP 的系统代理及证书校验，禁止 HTTPS 降级跳转。清单上限 1 MiB；安装包上限 256 MiB，使用 64 KiB 缓冲写入缓存，下载后核对字节数与 SHA-256，启动安装前再核对一次。保留单个已验证安装包，避免每个版本堆积完整下载。网络有连接 / 读取超时和总读取预算。

## 当前验收边界

本机 Release 构建、Inno Setup 6.7.4 编译、ZIP 打包及清单 / 文件摘要核对已通过，错误标签也会被拒绝。自动化测试覆盖新旧版本判断、限流冷却、下载镜像回退与发行形式隔离、下载截断 / 中断 / 校验失败及安装程序无法启动。干净 Windows 安装、跨版本升级、卸载及真实安装失败恢复仍待验收。

当前使用 GitHub 的 HTTPS 元数据和 SHA-256 校验更新包。代码签名和独立清单签名仍待实现。

实现依据：[GitHub 限流规则](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)、[WinHTTP 重定向选项](https://learn.microsoft.com/en-us/windows/win32/winhttp/option-flags)、[Inno Setup 官方下载](https://jrsoftware.org/isdl.php)及[安装命令行参数](https://jrsoftware.org/ishelp/topic_setupcmdline.htm)。
