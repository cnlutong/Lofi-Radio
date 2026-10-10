# Lofi Radio

一个安静的 macOS 菜单栏播放器。Tauri 2 + Rust + TypeScript/CSS，使用 mpv 的 yt-dlp hook 播放 13 个 YouTube 直播音频源：
[Lofi Girl](https://www.youtube.com/@LofiGirl/streams)（学习、睡眠、爵士、亚洲、雨天、House）、[Chillhop Music](https://www.youtube.com/@ChillhopMusic/streams)（爵士、学习、深夜）、[The Bootleg Boy](https://www.youtube.com/@thebootlegboy/streams)（睡眠）、[Homework Radio](https://www.youtube.com/@HomeworkRadio/streams)（木屋）、[Chill with Taiki](https://www.youtube.com/@ChillwithTaiki/streams) 和 [Purrple Cat](https://www.youtube.com/@PurrpleCat/streams)。主界面和状态栏右键菜单共用此列表。直播链接于 2026-10-10 从官方频道核对，停播或更换链接时需更新 `src-tauri/src/player.rs` 的 `STATIONS`。

## 运行

需要 macOS 12+、Xcode Command Line Tools、Node.js 22.12+ 和 Homebrew。

```sh
xcode-select --install  # 已安装时跳过
brew install rust mpv yt-dlp deno
npm ci
npm run tauri dev
```

构建可双击运行的本机应用：

```sh
npm run tauri build
open 'src-tauri/target/release/bundle/macos/Lofi Radio.app'
```

本原型使用本机 Homebrew 依赖，尚未把 mpv、yt-dlp、Deno 打包进应用。Finder 启动时会主动查找 `/opt/homebrew/bin` 和 `/usr/local/bin`，不依赖终端 PATH。构建产物仅覆盖当前 Mac 的架构，尚未使用 Developer ID 签名或公证。

## 使用

- 点击「播放」连接直播；点击「暂停」会停止播放器并释放连接和缓存，再次播放会重新抓取最新直播，不保留暂停时的时间点。
- 面板「直播源」下拉框可切换电台，选择后立即播放并保留音量；保存上次选择，重启后仍需手动开始播放。
- 拖动滑块调整 0–100% 音量，松开时应用并保存，首次音量为 30%。
- 左键单击菜单栏「lofi」图标暂停/播放直播，暂停时图标变浅；再次点击重新连接最新直播。
- 右键打开原生控制菜单，通过「直播源」子菜单快速切换，当前源显示勾选，并同步面板；点击「显示主界面」打开面板，启动时面板默认隐藏。菜单也可播放/暂停、选择音量和退出。
- 关闭窗口、按 Escape 或 Cmd+W 只收起面板，音乐继续；不依赖 Chrome，也不会显示 YouTube 页面、视频或推荐。
- 「退出 Lofi Radio」结束应用和播放进程。后台常驻不等于开机自启；本原型不自动播放或开机自启。
- 连接失败或超时会显示原因，可以重试。暂未实现断网后的自动重连或系统睡眠恢复。

## 验证

```sh
npm test           # 控制协议、停止/取消与连接释放、音量范围、直播源校验、设置保存/兼容、连接断开
npm run test:live  # 真实直播：音频输出、播放进度、禁用视频、停止后重新连接、音量、逐源切换、进程清理
npm run build     # TypeScript 检查和前端构建
```

直播检查需要网络和上述播放依赖，测试短暂使用 10% 音量，结束自动关闭 mpv。

设置和日志在 `~/Library/Application Support/com.lofiradio.desktop/`，分别为 `settings.json` 和 `mpv.log`。播放器 IPC 位于用户私有临时目录，权限 0700；退出时清理。播放、暂停和音量均由 Rust 控制，WebView 的生命周期不会控制音频。macOS 固定使用 AVFoundation 输出，通过 mpv 内置 Lua 设置该播放器实例的渲染音量，避免软件音量等待约两秒音频缓存；保持 mpv 原来的三次方音量曲线。直播测试检查输出音量在 250 毫秒内更新，并确认暂停后播放器已退出，调整音量不会重新开始播放，再次播放会创建新播放器并重新加载直播源。

当前仅验证 macOS。Windows 的 Tauri 界面可以复用，但播放器 IPC 的 Unix socket 还需要改用 Windows named pipe，并完成依赖分发与实机验证。

这是使用非官方 YouTube 接入方式的本地技术原型。正式分发前需要处理平台条款和音乐授权；YouTube 变更也可能导致解析失效。依赖更新可运行 `brew upgrade yt-dlp mpv deno`。

## 本机验证记录

已通过离线控制与直播源设置兼容测试、13 个直播源的真实播放及逐源切换测试（包含 Homework Radio、Chill with Taiki 和 Purrple Cat）、TypeScript 构建和 macOS 应用打包。逐源切换验证旧播放进程退出、当前 URL 和音量保留；此前也在不含 Homebrew 的 PATH 下通过了直播测试，并成功启动 `.app` 内的可执行文件。

系统未授予界面自动化权限，因此菜单栏点击、滑块操作和关闭窗口后的持续播放仍需手动验收：播放后收起窗口，确认声音继续；左键「lofi」图标暂停，再次点击播放最新直播；右键显示主界面及选择音量；退出后确认声音停止。上述界面行为已经实现，但未声称完成自动化点击验证。

## 品牌素材

圆润、略向右倾的「lofi」字标使用同一字形用于面板、应用图标和菜单栏。`assets/lofi-source.png` 是内置 Codex ImageGen 生成的透明原稿，提示词保存在 `assets/lofi-prompt.txt`；`lofi-wordmark.png` 和 `lofi-wordmark-sage.png` 分别是裁切后的黑色与鼠尾草绿版本。`assets/lofi-export.json` 记录裁切、颜色和导出尺寸。

应用图标源文件为 `assets/lofi-app-icon.png`，通过 `npm run tauri -- icon assets/lofi-app-icon.png -o src-tauri/icons` 生成桌面图标。菜单栏使用独立的 `src-tauri/icons/tray.png`（72 × 36 像素，显示为 36 × 18 点），保留透明背景和系统模板模式，随 macOS 菜单栏切换黑色或白色。

## 许可证

本项目采用 [MIT 许可证](LICENSE) 开源。第三方依赖遵循各自的许可证；直播音乐及第三方品牌素材的权利归原权利人所有，不包含在本项目的 MIT 授权中。
