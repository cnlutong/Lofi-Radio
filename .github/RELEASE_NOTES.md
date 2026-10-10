下载对应的 DMG，打开后将 **Lofi Radio** 拖到 **Applications（应用程序）**，再启动。菜单栏出现「lofi」后，左键点击播放，右键打开主界面或选择电台。

- **aarch64.dmg**：Apple 芯片 Mac。
- 要求 **macOS 27 或更新版本**，以及能够访问 YouTube 直播的网络。
- 内置 mpv、yt-dlp、Deno、Python 和所需动态库；无需 Homebrew 或额外安装播放依赖。
- 安装包采用 ad-hoc 签名，尚未经过 Apple Developer ID 签名与公证。首次打开若被拦截，请到「系统设置 → 隐私与安全性」选择「仍要打开」，再确认。参见 [Apple 说明](https://support.apple.com/zh-cn/102445)。
- `SHA256SUMS.txt` 可用于校验下载完整性。
- `third-party-sources` 是内置第三方工具的对应源码、补丁、许可证和构建配方，普通用户无需下载。GitHub 自动提供的 **Source code** 是项目源码，不是安装包。

项目代码采用 MIT 许可证；内置第三方工具遵循各自许可证（包括 GPL/LGPL）。不分发音乐文件，直播音乐的权利归原权利人所有。
