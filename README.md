<p align="center">
  <img src="assets/lofi-app-icon.png" width="96" height="96" alt="Lofi Radio icon" />
</p>
<h1 align="center">Lofi Radio</h1>
<p align="center">A little music. A little focus.</p>
<p align="center">A quiet macOS menu bar player for live lofi radio.</p>
<p align="center">
  <a href="https://github.com/cnlutong/Lofi-Radio/releases/latest"><img src="https://img.shields.io/github/v/release/cnlutong/Lofi-Radio?style=flat&amp;label=download&amp;color=687d53" alt="Latest release" /></a>
  <img src="https://img.shields.io/badge/macOS-27%2B-687d53?style=flat" alt="macOS 27 or later" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-687d53?style=flat" alt="MIT license" /></a>
</p>
<p align="center"><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>

---

### Your music, within reach

- **13 live stations** for studying, unwinding, and late nights.
- **One click to play or pause.** Right-click the menu bar icon for stations, volume, and the player panel.
- **Quiet in the background.** Closing the panel keeps the music playing; your station and volume are remembered.
- **Ready to listen.** Playback dependencies are bundled in the installer.

<p align="center">
  <img src="assets/lofi-macos-screenshot.png" width="320" alt="Lofi Radio player window on macOS" />
</p>

### Download

New builds require **macOS 27 or later on Apple Silicon**. Intel builds are discontinued. The app interface is currently in Simplified Chinese.

[Downloads](https://github.com/cnlutong/Lofi-Radio/releases) · The current v0.1.0 installers are historical macOS 15+ builds; the macOS 27 baseline applies to subsequent builds.

Open the DMG, drag **Lofi Radio** into **Applications**, then launch it. Click **lofi** in the menu bar to start listening. Playback starts manually; pausing disconnects the stream, and playing again reconnects to the live broadcast.

> **First launch:** the current release is ad-hoc signed and has not been notarized by Apple. If macOS blocks it, use **System Settings → Privacy & Security → Open Anyway**. [Apple’s guide](https://support.apple.com/en-us/102445).

[All releases & checksums](https://github.com/cnlutong/Lofi-Radio/releases) · [Report an issue](https://github.com/cnlutong/Lofi-Radio/issues)

<details>
<summary><strong>Stations</strong></summary>

Live streams from [Lofi Girl](https://www.youtube.com/@LofiGirl/streams), [Chillhop Music](https://www.youtube.com/@ChillhopMusic/streams), [The Bootleg Boy](https://www.youtube.com/@thebootlegboy/streams), [Homework Radio](https://www.youtube.com/@HomeworkRadio/streams), [Chill with Taiki](https://www.youtube.com/@ChillwithTaiki/streams), and [Purrple Cat](https://www.youtube.com/@PurrpleCat/streams).

This app uses unofficial YouTube playback and does not distribute music files. Streams can change or go offline; update the app if playback stops working. Automatic reconnection after network loss or system sleep is not yet supported.

</details>

<details>
<summary><strong>Development & building</strong></summary>

Built with Tauri 2, Rust 2024, TypeScript 6, Vite 8, and CSS. Playback uses AVFoundation through mpv. Local development requires macOS 27+ on Apple Silicon, Xcode 27 Command Line Tools, Node.js 22.12+, and Homebrew.

```sh
xcode-select --install  # Skip if already installed
brew install rust mpv yt-dlp deno
npm ci
npm run tauri dev
```

```sh
npm test           # Offline player and settings checks
npm run build     # TypeScript checks and frontend build
npm run test:live # Live playback checks; briefly plays at 10% volume
npm run tauri build
```

The local app is at `src-tauri/target/release/bundle/macos/Lofi Radio.app` and uses the playback dependencies installed on your Mac. To build a self-contained installer instead:

```sh
brew install mpv deno python@3.14
npm run release:prepare
npm run release:build
```

Installers are written to `src-tauri/target/release/bundle/dmg/`. The [release workflow](.github/workflows/release.yml) builds Apple Silicon installers on macOS 27 using the [`xcode-27` runner](https://github.com/actions/runner-images/blob/main/images/macos/xcode-27-arm64-Readme.md) and publishes installers, checksums, and third-party source archives when a matching `v*` version tag is pushed. Bundled dependencies are checked against the same macOS 27 baseline.

Settings and playback logs live in `~/Library/Application Support/com.lofiradio.desktop/`. Branding assets are in [`assets/`](assets/); the app icon and menu bar mark share the same rounded lettering.

</details>

### License

Project code is available under the [MIT License](LICENSE). Music and third-party brands belong to their respective owners.

Bundled dependencies retain their own licenses, including GPL/LGPL for mpv and some of its dependencies. License and version records are included in `Contents/Resources/runtime/licenses/`; corresponding source, patches, and Homebrew recipes are provided in each release’s `third-party-sources` archives.
