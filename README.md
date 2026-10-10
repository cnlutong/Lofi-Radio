<p align="center">
  <img src="assets/lofi-app-icon.png" width="96" height="96" alt="Lofi Radio icon" />
</p>
<h1 align="center">Lofi Radio</h1>
<p align="center">A little music. A little focus.</p>
<p align="center">A quiet tray player for live lofi radio, on macOS and Windows.</p>
<p align="center">
  <a href="https://github.com/cnlutong/Lofi-Radio/releases/latest"><img src="https://img.shields.io/github/v/release/cnlutong/Lofi-Radio?style=flat&amp;label=download&amp;color=687d53" alt="Latest release" /></a>
  <img src="https://img.shields.io/badge/macOS-15%2B-687d53?style=flat" alt="macOS 15 or later" />
  <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-687d53?style=flat" alt="Windows 10 and 11" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-687d53?style=flat" alt="MIT license" /></a>
</p>
<p align="center"><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>

---

### Your music, within reach

- **13 live stations** for studying, unwinding, and late nights.
- **One click to play or pause.** Right-click the tray icon for stations, volume, and the player panel.
- **Quiet in the background.** Closing the panel keeps the music playing; your station and volume are remembered.
- **Ready to listen.** Playback dependencies are bundled in the installer.

### Download

Supports **macOS 15+** and **Windows 10/11 x64**. The app interface is currently in Simplified Chinese.

| Your computer | Installer |
| :--- | :--- |
| macOS · Apple Silicon | [Download for Apple Silicon](https://github.com/cnlutong/Lofi-Radio/releases/download/v0.2.0/Lofi-Radio_0.2.0_aarch64.dmg) |
| macOS · Intel | [Download for Intel](https://github.com/cnlutong/Lofi-Radio/releases/download/v0.2.0/Lofi-Radio_0.2.0_x64.dmg) |
| Windows · Intel / AMD 64-bit | [Download for Windows](https://github.com/cnlutong/Lofi-Radio/releases/download/v0.2.0/Lofi-Radio_0.2.0_x64-setup.exe) |

**macOS:** open the DMG, drag **Lofi Radio** into **Applications**, then launch it. Click **lofi** in the menu bar to start listening.

**Windows:** run the `-setup.exe` installer, then open **Lofi Radio** from the Start menu. Find its icon in the notification area (it may be under the hidden-icons arrow). The installer includes playback tools and installs WebView2 if needed.

Playback starts manually. Pausing disconnects the stream; playing again reconnects to the live broadcast. The optional **Keep awake while playing** tray setting prevents idle system sleep while allowing the display to sleep.

> **First launch:** installers are not signed with a publisher certificate. On macOS, the app is ad-hoc signed but not notarized; use **System Settings → Privacy & Security → Open Anyway** if blocked ([Apple’s guide](https://support.apple.com/en-us/102445)). Windows may show SmartScreen’s **More info → Run anyway** prompt.

[All releases & checksums](https://github.com/cnlutong/Lofi-Radio/releases) · [Report an issue](https://github.com/cnlutong/Lofi-Radio/issues)

<details>
<summary><strong>Stations</strong></summary>

Live streams from [Lofi Girl](https://www.youtube.com/@LofiGirl/streams), [Chillhop Music](https://www.youtube.com/@ChillhopMusic/streams), [The Bootleg Boy](https://www.youtube.com/@thebootlegboy/streams), [Homework Radio](https://www.youtube.com/@HomeworkRadio/streams), [Chill with Taiki](https://www.youtube.com/@ChillwithTaiki/streams), and [Purrple Cat](https://www.youtube.com/@PurrpleCat/streams).

This app uses unofficial YouTube playback and does not distribute music files. Streams can change or go offline; update the app if playback stops working. Automatic reconnection after network loss or system sleep is not yet supported.

</details>

<details>
<summary><strong>Development & building</strong></summary>

Built with Tauri 2, Rust, TypeScript, and CSS. macOS development requires macOS 12+, Xcode Command Line Tools, Node.js 22.12+, and Homebrew. Playback dependencies may require a newer macOS version.

```sh
xcode-select --install  # Skip if already installed
brew install rust mpv yt-dlp deno
npm ci
npm run tauri dev
```

```sh
npm test           # Offline player and settings checks
npm run build     # TypeScript checks and frontend build
npm run test:live # macOS live checks; briefly plays at 10% volume
npm run tauri build
```

The local app is at `src-tauri/target/release/bundle/macos/Lofi Radio.app` and uses the playback dependencies installed on your Mac. To build a self-contained installer instead:

```sh
brew install mpv deno python@3.14
npm run release:prepare
npm run release:build
```

Windows builds require Node.js 22.12+, Python 3.11+, Rust MSVC, Visual Studio C++ Build Tools with the Windows SDK, and MSYS2. In the MSYS2 UCRT64 shell, install `mingw-w64-ucrt-x86_64-mpv`, `mingw-w64-ucrt-x86_64-binutils`. Then, in PowerShell:

```powershell
npm ci
python scripts/prepare-runtime-windows.py --msys-root C:/msys64
npm test
npm run tauri -- build --config src-tauri/tauri.windows.release.conf.json
```

Installers are written to `src-tauri/target/release/bundle/dmg/` or `bundle/nsis/`. The [release workflow](.github/workflows/release.yml) builds both Mac architectures on macOS 15 and Windows x64 on a Windows runner. A matching `v*` version tag publishes installers, checksums, and third-party source archives. Mac dependencies bundled on a newer macOS may require that newer version.

Settings and playback logs live in `~/Library/Application Support/com.lofiradio.desktop/` on macOS and `%APPDATA%/com.lofiradio.desktop/` on Windows. Branding assets are in [`assets/`](assets/); the app icon and menu bar mark share the same rounded lettering.

</details>

### License

Project code is available under the [MIT License](LICENSE). Music and third-party brands belong to their respective owners.

Bundled dependencies retain their own licenses, including GPL/LGPL for mpv and some of its dependencies. License and version records are in `runtime/licenses/` inside the app’s resources. Corresponding source, patches, and Homebrew or MSYS2 build recipes are provided in each release’s `third-party-sources` archives.
