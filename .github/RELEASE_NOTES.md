Lofi Radio now supports **macOS and Windows**. Playback tools are bundled; no Homebrew, Python, mpv, yt-dlp or Deno setup is needed.

| System | Download |
| --- | --- |
| macOS 15+ · Apple Silicon | `aarch64.dmg` |
| macOS 15+ · Intel | `x64.dmg` |
| Windows 10/11 · Intel / AMD 64-bit | `x64-setup.exe` |

**macOS:** open the DMG and drag Lofi Radio to Applications. **Windows:** run the setup executable, then launch Lofi Radio from the Start menu. WebView2 is included for installation if missing.

Click the menu bar / notification-area icon to play or pause. Right-click for stations, volume, the player panel and the optional keep-awake setting. The app interface is in Simplified Chinese.

The installers do not have publisher certificates. The Mac app is ad-hoc signed, not notarized; if blocked, use System Settings → Privacy & Security → Open Anyway ([Apple’s guide](https://support.apple.com/en-us/102445)). Windows may show a SmartScreen More info → Run anyway prompt.

`SHA256SUMS.txt` verifies downloads. `third-party-sources` archives contain the corresponding sources, patches, licenses and build recipes; ordinary users only need the installer. GitHub’s **Source code** downloads are not installers.

---

新增 **Windows 10/11 64 位安装版**，与两种 Mac 安装包一同提供。播放依赖已内置，无需另外配置工具。Windows 用户运行 `x64-setup.exe`，再从开始菜单启动；如缺少 WebView2，安装程序会自动安装。图标位于任务栏通知区域，可能需要展开隐藏图标。

macOS 用户打开对应 DMG，将应用拖入「应用程序」。单击菜单栏或托盘图标播放 / 暂停，右键切换电台、调整音量或打开面板。「播放时电脑不休眠」可阻止闲置休眠，显示器仍可休眠。

安装包暂未使用发行者证书签名。macOS 首次打开如被拦截，请前往「系统设置 → 隐私与安全性 → 仍要打开」；Windows 如出现 SmartScreen 提示，可选择「更多信息 → 仍要运行」。

Project code is MIT licensed. Bundled dependencies retain their own licenses, including GPL/LGPL. Music belongs to its respective owners and is not distributed with the app.

Build checks: [macOS](https://github.com/cnlutong/Lofi-Radio/actions/runs/38079433236) · [Windows](https://github.com/cnlutong/Lofi-Radio/actions/runs/38080160713). Windows installation, startup and player controls were checked on a hosted runner using silent test audio; playback through a physical Windows audio device has not been verified.
