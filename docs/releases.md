# Windows 发布与更新

## 构建与发布

`.github/workflows/windows.yml` 使用 Windows 2025 Runner、仓库固定的 Rust 1.99.1 和 Cargo.lock。推送、PR、手动运行执行格式检查、测试、Clippy、Release 构建及打包；仅稳定 `vX.Y.Z` 标签会发布 GitHub Release，且必须与 Cargo.toml 版本完全一致。

本地打包使用 PowerShell 7：

```powershell
cargo build --release --locked
./scripts/install-inno.ps1
./scripts/package.ps1 -Tag v0.1.0
```

编译器脚本从官方不可变版本地址下载 Inno Setup 6.7.4，验证固定 SHA-256、Windows 签名及发布者，便携解压到 `target/tools`，不注册文件关联。也可使用 PATH 中已有的 Inno Setup 6。仅需要便携包时使用 `./scripts/package.ps1 -SkipInstaller`。

`dist/release` 包含：

- `Githly-X.Y.Z-windows-x64.zip`
- `Githly-X.Y.Z-windows-x64-setup.exe`
- `update.json`：版本、版本说明地址、安装包地址 / 字节数 / SHA-256。
- `SHA256SUMS.txt`：以上文件的校验值。

确认版本和产物后，由维护者提交变更并推送对应版本标签。工作流上传附件并生成 Release 说明。本轮仅提供配置与实现，没有代替维护者推送标签或发布版本。

安装包最低 Windows 10，按用户安装到 `%LOCALAPPDATA%/Programs/Githly`，不要求管理员权限。升级关闭占用文件的应用前使用正常安装向导，不强行终止；完成后可启动新版。用户配置保留在 `%APPDATA%/githly`，卸载也不删除配置。便携版的更新按钮同样打开安装向导，不原地静默替换任意目录中的 EXE。

## 客户端检查与镜像

设置中可选手动、每天（默认）、每三天或每周检查。启动至少等待 30 秒，之后每小时轻量判断是否到期；最近检查时间跨启动保留。检查和下载各只有一个活动任务，手动重试至少相隔一分钟；不会在 UI 线程执行网络或文件 I/O。

先查询 GitHub 最新正式 Release。403 / 429 会保存 `Retry-After` / `X-RateLimit-Reset` 冷却时间；冷却期间不重复请求 GitHub API，改查 Release 的 `update.json`。若配置镜像，则通过镜像请求清单与安装包；空配置使用 GitHub Release 直连。

镜像必须支持“HTTPS 前缀 + 完整 GitHub Release URL”的代理形式。以下使用保留示例域名说明格式，不是可用服务：

```text
配置：https://mirror.example/
请求：https://mirror.example/https://github.com/Hunlongyu/githly/releases/latest/download/update.json
```

界面显示查询来源。已知 GitHub 新版本时拒绝版本不匹配的镜像清单；只从镜像查询且没有较新版本时显示“镜像暂未提供较新版本”，不推断 GitHub 最新版本。镜像不可用、文件截断、大小或摘要不符均显示错误，可稍后重试；下载失败会删除本次临时文件。

网络使用 Windows WinHTTP 的系统代理及证书校验，禁止 HTTPS 降级跳转。清单上限 1 MiB；安装包上限 256 MiB，使用 64 KiB 缓冲写入缓存，下载后核对字节数与 SHA-256，启动安装前再核对一次。保留单个已验证安装包，避免每个版本堆积完整下载。网络有连接 / 读取超时和总读取预算。

## 当前验收边界

本机 Release 构建、Inno Setup 6.7.4 编译、ZIP 打包及清单 / 文件摘要核对已通过，错误标签也会被拒绝。自动化测试覆盖新旧版本判断、限流冷却、镜像切换 / 滞后 / 失败、下载截断 / 中断 / 校验失败及安装程序无法启动。尚无正式版本可做端到端升级验收，干净 Windows 安装 / 跨版本升级 / 卸载及真实安装失败恢复仍列在 TODO。

没有代码签名证书，也没有独立签名的更新清单。SHA-256 只能证明安装包与清单一致，无法抵御清单和安装包同时被替换；应只配置自己信任的镜像。公共镜像服务与清单签名方案留待正式发布前确定。

实现依据：[GitHub 限流规则](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)、[WinHTTP 重定向选项](https://learn.microsoft.com/en-us/windows/win32/winhttp/option-flags)、[Inno Setup 官方下载](https://jrsoftware.org/isdl.php)及[安装命令行参数](https://jrsoftware.org/ishelp/topic_setupcmdline.htm)。
