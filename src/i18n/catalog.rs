//! Githly interface messages. Keep Git data (paths, refs and commit text) verbatim.
use super::Locale;
use std::fmt::Display;

macro_rules! messages {
    ($( $(#[$attr:meta])* $name:ident => $zh:literal, $en:literal; )*) => { impl Locale {
        $(
            $(#[$attr])*
            pub(crate) fn $name(self) -> &'static str {
                match self {
                    Locale::Chinese => $zh,
                    Locale::English => $en,
                }
            }
        )*
    }};
}

macro_rules! formatted_messages {
    ($( $(#[$attr:meta])* $name:ident($($arg:ident),+) => $zh:literal, $en:literal; )*) => { impl Locale {
        $(
            $(#[$attr])*
            pub(crate) fn $name(self, $($arg: impl Display),+) -> String {
                match self {
                    Locale::Chinese => format!($zh, $($arg = $arg),+),
                    Locale::English => format!($en, $($arg = $arg),+),
                }
            }
        )*
    }};
}

messages! {
    status_columns => "左位为暂存区状态，右位为工作区状态，· 表示无更改", "Left: index status. Right: working tree status. A dot means unchanged.";
    filter_paths => "按路径筛选更改", "Filter changes by path";
    search_history => "搜索说明、作者、哈希", "Search message, author, hash";
    choose_git_exe => "选择 Git 可执行文件", "Select Git executable";
    open_project => "加载项目", "Open project";
    loading_files => "正在读取文件…", "Loading files…";
    commit_no_diff => "此次提交没有文件差异", "This commit has no file changes";
    loading_diff => "正在读取差异…", "Loading diff…";
    no_text_diff => "没有可显示的文本差异", "No text changes to display";
    detect_again => "重新检测", "Detect again";
    choose_git => "选择 Git 程序…", "Select Git executable…";
    git_website => "Git 官网", "Git website";
    recent_projects => "最近打开", "Recent projects";
    open_another => "加载其他项目…", "Open another project…";
    clear_recent => "清除最近记录", "Clear recent projects";
    recent_cleared => "已清除最近项目记录", "Recent projects cleared";
    close_project => "关闭项目", "Close project";
    upstream_missing => "上游跟踪记录在本地不可用，暂时无法计算提交数量", "The upstream ref is unavailable locally. Ahead and behind counts are unknown.";
    no_upstream => "未设置上游", "No upstream";
    no_upstream_tip => "当前分支没有上游跟踪关系，无法计算待推送与待拉取数量", "Set an upstream branch to see ahead and behind counts.";
    toggle_theme => "切换明暗主题", "Toggle light or dark theme";
    switch_light => "切换为浅色", "Switch to light theme";
    switch_dark => "切换为深色", "Switch to dark theme";
    settings => "设置", "Settings";
    general => "通用", "General";
    about => "关于", "About";
    check_updates => "检查更新", "Check for updates";
    current_version => "当前版本", "Current version";
    checking_updates => "正在检查更新…", "Checking for updates…";
    up_to_date => "已是最新版本", "You’re up to date";
    developer => "开发者", "Developer";
    repository_link => "GitHub 仓库", "GitHub repository";
    feedback => "意见反馈", "Feedback";
    update_available_tip => "有新版本，查看更新日志", "Update available · View release notes";
    #[cfg(not(target_os = "macos"))]
    settings_shortcut => "设置  Ctrl+,", "Settings  Ctrl+,";
    #[cfg(target_os = "macos")]
    settings_shortcut => "设置  ⌘,", "Settings  ⌘,";
    switching_branch => "正在切换分支…", "Switching branches…";
    loading_project => "正在读取项目…", "Loading project…";
    project_opened => "已打开项目", "Project opened";
    config_dir_missing => "无法定位配置目录", "Could not locate the configuration directory";
    invalid_config => "配置文件格式无效，原文件已保留", "Invalid settings file. The original file was preserved.";
    unsupported_config => "不支持此配置文件版本，原文件已保留", "Unsupported settings version. The original file was preserved.";
    invalid_config_dir => "配置目录无效", "Invalid configuration directory";
    create_config_dir => "无法创建配置目录", "Could not create the configuration directory";
    write_config => "无法写入配置", "Could not write settings";
    save_config => "无法保存配置文件", "Could not save the settings file";
    worktree_snapshot => "当前提交 → 工作区 · 打开时的快照；重新选择查看最新内容", "HEAD → working tree · Snapshot at open. Select again to refresh.";
    current_branch => "当前分支", "Current branch";
    all_branches => "全部分支", "All branches";
    git_not_on_path => "未在系统搜索路径中找到 Git，请指定已有的 Git 程序或前往官网安装", "Git was not found on PATH. Select an installed executable or download Git from its website.";
    select_git_binary => "请选择 Git 可执行文件，而不是 git-bash 或安装程序", "Select the Git executable (git.exe on Windows).";
    invalid_git_version => "所选程序没有返回有效的 Git 版本", "The selected program did not return a valid Git version";
    start_git => "无法启动 Git 程序", "Could not start Git";
    read_git_output => "无法读取 Git 输出", "Could not read Git output";
    read_git_stderr => "无法读取 Git 错误输出", "Could not read Git error output";
    git_timeout => "Git 读取超时，请稍后重试", "Git timed out. Try again later.";
    invalid_repository => "此目录不是可读取的 Git 工作目录", "This directory is not a readable Git working tree";
    dirty_checkout => "工作区或暂存区存在更改，暂不能切换分支", "Cannot switch branches with working tree or index changes";
    busy_checkout => "仓库正在进行其他操作，暂不能切换分支", "Cannot switch branches during another repository operation";
    local_checkout_only => "仅支持切换已有本地分支", "Only existing local branches can be checked out";
    read_git_error => "无法读取 Git 错误", "Could not read Git errors";
    detached_head => "未检出分支", "Detached HEAD";
    invalid_index_image => "暂存区图片存在冲突或不是普通文件", "The staged image is conflicted or is not a regular file";
    image_too_large => "图片超过 8 MiB，暂不预览", "Image exceeds the 8 MiB preview limit";
    invalid_image_version => "该版本不是普通图片文件", "This revision is not a regular image file";
    read_image => "无法读取图片文件", "Could not read the image";
    invalid_image_path => "此路径不是普通图片文件", "This path is not a regular image file";
    file_too_large => "文件过大，暂不展开内容", "File is too large to display";
    non_text => "此文件无法作为文本显示", "This file cannot be displayed as text";
    diff_too_large => "差异过大，暂不展开内容", "Diff is too large to display";
    output_too_large => "Git 输出超过本轮读取上限，请缩小查看范围", "Git output exceeded the read limit. Narrow the selection.";
    ref_too_large => "单条 Git 引用超过 8 MiB", "A single Git ref exceeds 8 MiB";
    branches_too_large => "Git 分支列表超过 8 MiB", "The Git branch list exceeds 8 MiB";
    merging => "合并中", "Merging";
    rebasing => "变基中", "Rebasing";
    applying_patch => "变基 / 应用补丁中", "Rebasing or applying patches";
    cherry_picking => "拣选中", "Cherry-picking";
    reverting => "还原提交中", "Reverting";
    bisecting => "二分定位中", "Bisecting";
    appdata_missing => "无法定位应用数据目录", "Could not locate the application data directory";
    #[cfg(windows)]
    register_launch => "无法注册 Githly 启动请求", "Could not register the Githly launch handler";
    create_window => "无法创建 Githly 窗口", "Could not create the Githly window";
    #[cfg(windows)]
    instance_lock => "无法创建 Githly 单实例锁", "Could not create the Githly single-instance lock";
    instance_not_ready => "已有 Githly 进程，但主窗口尚未就绪", "Githly is running, but its window is not ready";
    path_too_long => "仓库路径过长", "Repository path is too long";
    instance_unresponsive => "已有 Githly 窗口未响应启动请求", "The existing Githly window did not respond";
    launch_rejected => "已有 Githly 窗口未接受启动请求", "The existing Githly window declined the launch request";
    #[cfg(windows)]
    window_handle => "无法取得主窗口句柄", "Could not get the main window handle";
    #[cfg(windows)]
    no_win32_handle => "缺少 Win32 窗口句柄", "Missing Win32 window handle";
    #[cfg(windows)]
    receive_launch => "无法接收重复启动请求", "Could not receive additional launch requests";
    #[cfg(windows)]
    mark_window => "无法标记主窗口", "Could not register the main window";
    invalid_version => "更新版本号无效", "Invalid update version";
    stable_only => "仅支持正式版本更新", "Only stable updates are supported";
    invalid_notes => "更新说明地址不属于本项目", "Release notes URL does not belong to this project";
    invalid_installer_url => "安装包地址不属于本项目或版本不匹配", "Installer URL does not match this project and version";
    invalid_checksum => "安装包校验信息无效", "Invalid installer checksum";
    installer_too_large => "安装包大小超出限制", "Installer exceeds the size limit";
    invalid_release => "GitHub 版本信息格式无效", "Invalid GitHub release metadata";
    no_release => "尚无正式版本", "No stable release is available";
    invalid_manifest => "更新清单格式无效", "Invalid update manifest";
    update_cache_dir => "无法定位更新缓存目录", "Could not locate the update cache directory";
    installer_failed => "安装包下载失败", "Installer download failed";
    installer_in_use => "旧安装包正在使用，请稍后重试", "The previous installer is in use. Try again later.";
    installer_incomplete => "安装包不完整或大小不匹配", "Installer is incomplete or its size does not match";
    checksum_failed => "安装包 SHA-256 校验失败", "Installer SHA-256 verification failed";
    #[cfg(any(windows, target_os = "linux"))]
    installer_launch_failed => "无法启动安装程序，当前版本仍可使用", "Could not start the installer. You can keep using the current version.";
    search_branches => "搜索分支", "Search branches";
    remote_branches => "远程分支", "Remote branches";
    local_branches => "本地分支", "Local branches";
    collapsed => "已折叠", "collapsed";
    expanded => "已展开", "expanded";
    current => "当前", "Current";
    switch => "切换", "Switch";
    no_branches => "暂无分支", "No branches";
    no_matching_branches => "没有匹配的分支", "No matching branches";
    invalid_image => "文件损坏、格式不支持或解码尺寸过大", "Image is corrupt, unsupported, or too large to decode";
    parse_svg => "SVG 文件无法解析", "Could not parse SVG";
    svg_size => "SVG 尺寸无法预览", "Cannot preview this SVG at its specified size";
    svg_pixels => "SVG 像素无效", "Invalid SVG pixels";
    before => "修改前", "Before";
    after => "修改后", "After";
    first_frame => " · 首帧", " · First frame";
    missing_image => "此版本中不存在此图片", "Image is absent in this revision";
    done => "完成", "Done";
    appearance => "外观", "Appearance";
    light => "浅色", "Light";
    dark => "深色", "Dark";
    motion => "界面动效", "Motion";
    system => "跟随系统", "System default";
    subtle => "轻微", "Subtle";
    off => "关闭", "Off";
    detecting => "正在检测…", "Detecting…";
    git_missing => "未找到 Git", "Git not found";
    use_system_path => "使用系统中的 Git", "Use system PATH";
    loading_stash => "正在读取储藏…", "Loading stash…";
    empty_stash => "这条储藏没有文件更改", "This stash has no file changes";
    downloading => "正在下载更新包…", "Downloading update…";
    download_verified => "更新包已下载并通过校验。", "Update downloaded and verified.";
    opening_installer => "正在校验并应用更新…", "Verifying and applying the update…";
    #[cfg(windows)]
    installer_opened => "安装向导已打开，更新完成后会自动启动新版 Githly。", "The installer is open. Githly will restart after the update.";
    notes_unavailable => "此版本未提供更新说明。", "No release notes are available for this version.";
    #[cfg(windows)]
    invalid_portable_zip => "便携包缺少有效的 Githly.exe。", "The portable ZIP does not contain a valid Githly.exe.";
    restarting => "更新已安装，正在重新启动 Githly…", "Update installed. Restarting Githly…";
    release_notes => "更新日志", "Release notes";
    install_update => "安装更新…", "Install update…";
    download_update => "下载更新", "Download update";
    no_recent => "尚未打开项目", "No recent projects";
    welcome => "查看本地更改与提交历史", "Browse local changes and commit history";
    detect_system_git => "正在检测系统 Git…", "Looking for Git on PATH…";
    no_git => "尚未找到可用的 Git", "No usable Git installation found";
    open_local => "加载本地项目", "Open local project";
    drop_repository => "选择仓库文件夹，或将文件夹拖到这里", "Choose a repository folder or drop it here";
    #[cfg(not(target_os = "macos"))]
    open_shortcut => "打开本地仓库  Ctrl+O", "Open local repository  Ctrl+O";
    #[cfg(target_os = "macos")]
    open_shortcut => "打开本地仓库  ⌘O", "Open local repository  ⌘O";
    history_display => "历史显示配置", "History display options";
    show_hash => "显示提交哈希", "Show commit hash";
    show_tags => "显示标签", "Show tags";
    show_branches => "显示分支", "Show branches";
    metadata_after => "哈希、分支和标签显示在说明后面", "Show hash, branches, and tags after the message";
    untracked => "未跟踪", "Untracked";
    tracked => "已跟踪的更改", "Tracked changes";
    calculating_stats => "正在计算增删行数…", "Counting added and deleted lines…";
    stats_unavailable => "统计暂不可用，稍后自动重试", "Line counts unavailable. Githly will retry.";
    partial_stats => "部分文件无文本统计", "Some files have no line counts";
    partial_stats_suffix => " · 部分文件无文本统计", " · Some files have no line counts";
    no_stashes => "暂无储藏", "No stashes";
    searching => "搜索中…", "Searching…";
    loading => " · 加载中…", " · Loading…";
    new_history => " · 有新历史，回到顶部自动更新", " · New commits. Scroll to the top to refresh.";
    end_of_history => " · 已到底", " · End of history";
    search_loaded => " · 搜索已加载记录", " · Searching loaded commits only";
    graph => "关系图", "Graph";
    list => "列表", "List";
    commit_message => "提交说明", "Commit message";
    author => "作者", "Author";
    commit_time => "提交时间", "Commit time";
    local_changes => "本地更改", "Local changes";
    empty_tree => "空树", "empty tree";
    full_message => "完整说明", "Full message";
    close_details => "关闭详情", "Close details";
    previous_change => "上一处变化", "Previous change";
    next_change => "下一处变化", "Next change";
    no_worktrees => "没有可用的工作树", "No available worktrees";
    loading_worktrees => "正在读取工作树…", "Loading worktrees…";
    no_more_history => "没有可继续读取的历史", "No more history to load";
    read_history => "无法读取历史输出", "Could not read history output";
    history_timeout => "读取更多历史超时", "Loading more history timed out";
    history_limit => "历史批次超过 8 MiB 读取上限", "History batch exceeds the 8 MiB read limit";
    incomplete_history => "Git 历史记录不完整", "Incomplete Git history record";
    stash_missing => "储藏记录不可用", "Stash object is unavailable";
    missing_head => "Git 未返回 HEAD 状态", "Git did not return HEAD status";
    missing_branch => "Git 未返回分支名称", "Git did not return the branch name";
    unknown_status => "无法识别 Git 文件状态", "Unrecognized Git file status";
    incomplete_status => "Git 文件状态不完整", "Incomplete Git file status";
    incomplete_path => "Git 文件路径不完整", "Incomplete Git file path";
    incomplete_rename => "Git 重命名路径不完整", "Incomplete rename path";
    status_unchecked => "未核对状态", "Status not checked";
    bare_repository => "裸仓库", "Bare repository";
    locked => "已锁定", "Locked";
    directory_missing => "目录不可用", "Directory unavailable";
    clean_worktree => "工作区干净", "Working tree clean";
    #[cfg(windows)]
    https_required => "更新连接必须使用 HTTPS", "Update connections require HTTPS";
    #[cfg(windows)]
    missing_host => "缺少服务器地址", "Missing server address";
    update_size_limit => "更新文件超过大小限制", "Update file exceeds the size limit";
    #[cfg(windows)]
    request_timeout => "更新请求超时", "Update request timed out";
    language => "语言", "Language";
    worktrees => "工作树", "Worktrees";
}

formatted_messages! {
    save_failed(error) => "保存配置失败：{error:#}", "Could not save settings: {error:#}";
    last_read(path, time) => "{path}\n自动核对本地状态 · 上次读取 {time}", "{path}\nLocal status checked automatically · Last read {time}";
    upstream_counts(ahead, behind) => "待推送 {ahead} 个提交 · 待拉取 {behind} 个提交\n基于本地远程跟踪记录，不自动连接远程", "Ahead: {ahead} · Behind: {behind}\nBased on local tracking refs. Githly does not fetch automatically.";
    refresh_failed(error) => "自动刷新暂不可用，保留上次内容并稍后重试：{error}", "Automatic refresh failed. Showing the last snapshot until the next check. {error}";
    worktree_retry(error) => "读取工作树列表失败，稍后自动重试：{error}", "Could not read worktrees. Githly will retry. {error}";
    cannot_read(path) => "无法读取 {path}", "Could not read {path}";
    git_path_missing(path) => "Git 程序路径不存在：{path}", "Git executable not found at {path}";
    conflicts(conflicts) => "{conflicts} 个冲突文件", "Conflicted files: {conflicts}";
    truncated_message(body) => "{body}\n\n[提交说明过长，仅显示前 64 KiB]", "{body}\n\n[Commit message truncated to 64 KiB]";
    source_unavailable(status) => "更新源暂不可用（HTTP {status}）", "Update source unavailable (HTTP {status})";
    download_http(status) => "下载失败（HTTP {status}）", "Download failed (HTTP {status})";
    branch_group(name, count, state) => "{name}，{count} 个，{state}", "{name}, {count}, {state}";
    image_preview_failed(error) => "无法预览图片：{error:#}", "Could not preview image: {error:#}";
    draw_image_failed(error) => "图片预览绘制失败：{error}", "Could not draw image preview: {error}";
    download_failed(error) => "下载失败：{error:#}", "Download failed: {error:#}";
    check_failed(error) => "检查更新失败：{error:#}", "Could not check for updates: {error:#}";
    view_update(version) => "查看新版 v{version}", "View update v{version}";
    install_failed(error) => "更新失败：{error:#}", "Update failed: {error:#}";
    file_group(heading, count, state) => "{heading}，{count} 个文件，{state}", "{heading}, files: {count}, {state}";
    stash_tab(count) => "储藏  {count}", "Stash  {count}";
    changes_tab(count) => "更改  {count}", "Changes  {count}";
    history_scope(scope) => "历史 · {scope}", "History · {scope}";
    tag_label(name) => "标签：{name}", "tag: {name}";
    shown_commits(count, status) => "显示 {count} 条提交{status}", "Commits shown: {count}{status}";
    history_retry(error) => "继续读取历史失败，稍后自动重试：{error}", "Could not load more history. Githly will retry. {error}";
    parent_comparison(hash, author, date, parent) => "{hash}   {author}   {date}   相对父提交 {parent}", "{hash}   {author}   {date}   Compared with parent {parent}";
    detail_file_count(meta, count) => "{meta} · {count} 个文件", "{meta} · Files: {count}";
    worktrees_failed(error) => "读取工作树失败：{error:#}", "Could not read worktrees: {error:#}";
    history_failed(status) => "Git 历史读取失败（{status}）", "Could not read Git history ({status})";
    worktree_changes(count) => "{count} 项更改", "Changes: {count}";
    unreadable(error) => "不可读取：{error}", "Cannot read: {error}";
    #[cfg(windows)]
    connection_failed(error) => "网络连接失败：{error}", "Network connection failed: {error}";
    #[cfg(windows)]
    request_failed(error) => "网络请求失败：{error}", "Network request failed: {error}";
}
