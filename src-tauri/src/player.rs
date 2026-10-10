use serde::Serialize;
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::{fs::DirBuilderExt, net::UnixStream as IpcStream};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[cfg(windows)]
use {
    crate::windows::Pipe as IpcStream,
    std::os::windows::{io::OwnedHandle, process::CommandExt},
};

static RUNTIME_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
pub struct Station {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
}

// Verified against the official channels' live tabs on 2026-10-10.
pub const STATIONS: &[Station] = &[
    Station {
        id: "lofi-girl-study",
        name: "Lofi Girl · 学习",
        url: "https://www.youtube.com/watch?v=rFZHOHl-L8A",
    },
    Station {
        id: "lofi-girl-sleep",
        name: "Lofi Girl · 睡眠",
        url: "https://www.youtube.com/watch?v=JD-kMIpDfnY",
    },
    Station {
        id: "lofi-girl-jazz",
        name: "Lofi Girl · 爵士",
        url: "https://www.youtube.com/watch?v=E2vONfzoyRI",
    },
    Station {
        id: "lofi-girl-asian",
        name: "Lofi Girl · 亚洲",
        url: "https://www.youtube.com/watch?v=1Tl2FtV06qo",
    },
    Station {
        id: "lofi-girl-sad",
        name: "Lofi Girl · 雨天",
        url: "https://www.youtube.com/watch?v=CwPCy1GLS38",
    },
    Station {
        id: "lofi-girl-house",
        name: "Lofi Girl · House",
        url: "https://www.youtube.com/watch?v=3PFJ9SETS4M",
    },
    Station {
        id: "chillhop-jazz",
        name: "Chillhop · 爵士",
        url: "https://www.youtube.com/watch?v=5yx6BWlEVcY",
    },
    Station {
        id: "chillhop-lofi",
        name: "Chillhop · 学习",
        url: "https://www.youtube.com/watch?v=7NOSDKb0HlU",
    },
    Station {
        id: "chillhop-night",
        name: "Chillhop · 深夜",
        url: "https://www.youtube.com/watch?v=i6WzngxTnBA",
    },
    Station {
        id: "bootleg-boy",
        name: "The Bootleg Boy · 睡眠",
        url: "https://www.youtube.com/watch?v=FWjZ0x2M8og",
    },
    Station {
        id: "homework-radio",
        name: "Homework Radio · 木屋",
        url: "https://www.youtube.com/watch?v=bREoCmUpE9g",
    },
    Station {
        id: "chill-with-taiki",
        name: "Chill with Taiki",
        url: "https://www.youtube.com/watch?v=qH3fETPsqXU",
    },
    Station {
        id: "purrple-cat",
        name: "Purrple Cat",
        url: "https://www.youtube.com/watch?v=OFsJen4j9VY",
    },
];

#[derive(Clone, Serialize, PartialEq)]
pub struct Snapshot {
    pub station: Station,
    pub status: String,
    pub volume: u8,
    pub prevent_sleep: bool,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn label(&self) -> String {
        let status = match self.status.as_str() {
            "connecting" => "正在连接直播…",
            "playing" => "正在播放",
            "paused" => "已暂停",
            "buffering" => "正在缓冲…",
            "error" => "播放失败 · 点击重试",
            _ => "准备好开始专注",
        };
        format!("{status} · {}", self.station.name)
    }

    pub fn toggle_label(&self) -> &str {
        match self.status.as_str() {
            "playing" | "buffering" => "暂停",
            "paused" => "播放直播",
            "connecting" => "取消连接",
            "error" => "重试播放",
            _ => "播放直播",
        }
    }
}

pub struct Player {
    pub snapshot: Snapshot,
    child: Option<Child>,
    #[cfg(not(windows))]
    sleep_inhibitor: Option<Child>,
    #[cfg(windows)]
    sleep_inhibitor: Option<OwnedHandle>,
    ipc: Option<BufReader<IpcStream>>,
    runtime_dir: PathBuf,
    settings: PathBuf,
    log: PathBuf,
    request_id: u64,
    connecting_since: Option<Instant>,
}

fn binary(name: &str) -> Result<PathBuf, String> {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let bundled = if cfg!(windows) {
            parent.join("runtime/bin")
        } else {
            parent
                .parent()
                .unwrap_or(parent)
                .join("Resources/runtime/bin")
        };
        if bundled.is_dir() {
            let path = bundled.join(&filename);
            return path
                .is_file()
                .then_some(path)
                .ok_or_else(|| format!("安装包缺少 {name}，请重新下载 Lofi Radio"));
        }
    }
    #[cfg(all(windows, debug_assertions))]
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("runtime/bin")
            .join(&filename);
        if path.is_file() {
            return Ok(path);
        }
    }
    // Finder-launched apps do not inherit the terminal's Homebrew PATH.
    [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]
    .into_iter()
    .chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ))
    .map(|dir| dir.join(&filename))
    .find(|path| path.is_file())
    .ok_or_else(|| {
        if cfg!(windows) {
            format!("缺少 {name}，请重新安装 Lofi Radio，或将播放工具加入 PATH")
        } else {
            format!("缺少 {name}。请运行：brew install mpv yt-dlp deno")
        }
    })
}

impl Player {
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let settings = data_dir.join("settings.json");
        let saved = fs::read(&settings)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or(Value::Null);
        let volume = saved["volume"].as_u64().unwrap_or(30).min(100) as u8;
        let station = STATIONS
            .iter()
            .find(|s| Some(s.id) == saved["station"].as_str())
            .copied()
            .unwrap_or(STATIONS[0]);
        // Private directory: mpv IPC can execute commands and must stay local to this user.
        let runtime_dir = std::env::temp_dir().join(format!(
            "lofi-radio-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos(),
            RUNTIME_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        builder.create(&runtime_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            snapshot: Snapshot {
                station,
                status: "stopped".into(),
                volume,
                prevent_sleep: saved["prevent_sleep"].as_bool().unwrap_or(true),
                error: None,
            },
            child: None,
            sleep_inhibitor: None,
            ipc: None,
            runtime_dir,
            settings,
            log: data_dir.join("mpv.log"),
            request_id: 0,
            connecting_since: None,
        })
    }

    fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            return Ok(());
        }
        let mpv = binary("mpv")?;
        let ytdlp = binary("yt-dlp")?;
        let deno = binary("deno")?;
        let path = std::env::join_paths(
            [
                mpv.parent().unwrap().to_path_buf(),
                ytdlp.parent().unwrap().to_path_buf(),
                deno.parent().unwrap().to_path_buf(),
            ]
            .into_iter()
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            )),
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        let socket = self.runtime_dir.join("mpv.sock");
        #[cfg(windows)]
        let socket = PathBuf::from(format!(
            r"\\.\pipe\{}",
            self.runtime_dir.file_name().unwrap().to_string_lossy()
        ));
        let volume_script = self.runtime_dir.join("volume.lua");
        fs::write(&volume_script, include_str!("volume.lua")).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&socket);
        let mut command = Command::new(&mpv);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW: playback never opens a console.
        command
            .args([
                "--no-config",
                "--idle=yes",
                "--vid=no",
                "--audio-display=no",
                "--force-window=no",
                "--stop-screensaver=no",
                "--no-terminal",
                "--ytdl-format=bestaudio",
                "--cache=yes",
                "--demuxer-max-bytes=8MiB",
            ])
            // Start silent until the Lua hook has set this renderer's saved gain.
            .arg("--volume=0")
            .arg(format!("--script={}", volume_script.display()))
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg(format!("--log-file={}", self.log.display()))
            .arg(format!(
                "--script-opts=ytdl_hook-ytdl_path={},lofi_volume-volume={}",
                ytdlp.display(),
                self.snapshot.volume
            ))
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "macos")]
        command.arg("--ao=avfoundation");
        #[cfg(windows)]
        {
            command.args(["--ao=wasapi", "--audio-exclusive=no"]);
            let certificate = mpv.parent().unwrap().parent().unwrap().join("cacert.pem");
            if certificate.is_file() {
                command.arg(format!("--tls-ca-file={}", certificate.display()));
            }
        }
        #[cfg(all(test, windows))]
        command.arg("--ao=null"); // Hosted Windows runners have no audio device.
        self.child = Some(command.spawn().map_err(|e| format!("无法启动 mpv：{e}"))?);
        for _ in 0..100 {
            #[cfg(unix)]
            let connection = IpcStream::connect(&socket);
            #[cfg(windows)]
            let connection = IpcStream::connect(&socket, self.child.as_ref().unwrap().id());
            if let Ok(stream) = connection {
                #[cfg(unix)]
                {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .map_err(|e| e.to_string())?;
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .map_err(|e| e.to_string())?;
                }
                self.ipc = Some(BufReader::new(stream));
                return Ok(());
            }
            if self
                .child
                .as_mut()
                .unwrap()
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
            {
                break;
            }
            thread::sleep(Duration::from_millis(30));
        }
        self.stop();
        Err("mpv 未能启动，请查看 mpv.log".into())
    }

    fn command(&mut self, command: Value) -> Result<Value, String> {
        self.request_id += 1;
        let id = self.request_id;
        let ipc = self.ipc.as_mut().ok_or("播放器未启动")?;
        writeln!(
            ipc.get_mut(),
            "{}",
            json!({"command": command, "request_id": id})
        )
        .map_err(|e| e.to_string())?;
        loop {
            let mut line = String::new();
            let ipc = self.ipc.as_mut().ok_or("播放器未启动")?;
            if ipc.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("mpv 连接已断开".into());
            }
            let message: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if message["request_id"] == id {
                return if message["error"] == "success" {
                    Ok(message["data"].clone())
                } else {
                    Err(format!("mpv：{}", message["error"]))
                };
            }
            match message["event"].as_str() {
                Some("file-loaded") => {
                    self.connecting_since = None;
                    self.snapshot.status = "playing".into();
                }
                Some("end-file") if message["reason"] == "error" => {
                    self.fail(format!(
                        "直播连接失败：{}。请重试；详细原因见 mpv.log。",
                        message["file_error"].as_str().unwrap_or("未知错误")
                    ));
                }
                _ => {}
            }
        }
    }

    pub fn fail(&mut self, error: String) {
        self.allow_sleep();
        self.snapshot.status = "error".into();
        self.snapshot.error = Some(error);
        self.connecting_since = None;
    }

    pub fn toggle(&mut self) -> Result<(), String> {
        match self.snapshot.status.as_str() {
            "connecting" => {
                self.stop();
                self.snapshot.status = "stopped".into();
            }
            "playing" | "buffering" => {
                // Live radio pause ends the session; playback reconnects at the live edge.
                self.stop();
                self.snapshot.status = "paused".into();
            }
            _ => {
                self.stop();
                self.start()?;
                self.snapshot.status = "connecting".into();
                self.snapshot.error = None;
                self.connecting_since = Some(Instant::now());
                // A fast file-loaded event can arrive before loadfile's response.
                self.command(json!(["loadfile", self.snapshot.station.url, "replace"]))?;
            }
        }
        self.sync_sleep_inhibition()
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<(), String> {
        if volume > 100 {
            return Err("音量必须在 0–100 之间".into());
        }
        if self.ipc.is_some() {
            self.command(json!(["script-message", "lofi-volume", volume.to_string()]))?;
        }
        self.snapshot.volume = volume;
        self.save_settings()
    }

    pub fn set_prevent_sleep(&mut self, enabled: bool) -> Result<(), String> {
        let previous = self.snapshot.prevent_sleep;
        self.snapshot.prevent_sleep = enabled;
        if let Err(error) = self.save_settings() {
            self.snapshot.prevent_sleep = previous;
            return Err(error);
        }
        self.sync_sleep_inhibition()
    }

    pub fn select_station(&mut self, id: &str) -> Result<(), String> {
        let station = STATIONS
            .iter()
            .find(|s| s.id == id)
            .copied()
            .ok_or("未知直播源")?;
        if station == self.snapshot.station
            && matches!(
                self.snapshot.status.as_str(),
                "playing" | "buffering" | "connecting"
            )
        {
            return Ok(());
        }
        // Restart mpv so queued events and buffered audio from the old stream cannot leak into the new one.
        self.stop();
        self.snapshot.station = station;
        self.snapshot.status = "stopped".into();
        self.snapshot.error = None;
        self.save_settings()?;
        self.toggle()
    }

    fn save_settings(&self) -> Result<(), String> {
        let temp = self.settings.with_extension("tmp");
        fs::write(
            &temp,
            json!({
                "volume": self.snapshot.volume,
                "station": self.snapshot.station.id,
                "prevent_sleep": self.snapshot.prevent_sleep,
            })
            .to_string(),
        )
        .map_err(|e| format!("无法保存设置：{e}"))?;
        fs::rename(temp, &self.settings).map_err(|e| format!("无法保存设置：{e}"))?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        if let Err(error) = self.refresh_state() {
            self.allow_sleep();
            return Err(error);
        }
        self.sync_sleep_inhibition()
    }

    fn refresh_state(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            self.child = None;
            self.ipc = None;
            return Err("mpv 意外退出，点击播放重新连接".into());
        }
        let idle = self
            .command(json!(["get_property", "idle-active"]))?
            .as_bool()
            .unwrap_or(true);
        if self.snapshot.status == "error" {
            return Ok(());
        }
        if self
            .connecting_since
            .is_some_and(|t| t.elapsed() > Duration::from_secs(60))
        {
            self.command(json!(["stop"]))?;
            return Err("直播连接超时，请检查网络后重试".into());
        }
        if self.connecting_since.is_some() {
            return Ok(());
        }
        if idle {
            self.snapshot.status = "stopped".into();
            return Ok(());
        }
        let paused = self
            .command(json!(["get_property", "pause"]))?
            .as_bool()
            .unwrap_or(false);
        let buffering = self
            .command(json!(["get_property", "paused-for-cache"]))?
            .as_bool()
            .unwrap_or(false);
        self.snapshot.status = if paused {
            "paused"
        } else if buffering {
            "buffering"
        } else {
            "playing"
        }
        .into();
        Ok(())
    }

    fn sync_sleep_inhibition(&mut self) -> Result<(), String> {
        if !self.snapshot.prevent_sleep
            || self.child.is_none()
            || !matches!(
                self.snapshot.status.as_str(),
                "connecting" | "playing" | "buffering"
            )
        {
            self.allow_sleep();
        }
        #[cfg(windows)]
        if self.snapshot.prevent_sleep
            && self.child.is_some()
            && matches!(
                self.snapshot.status.as_str(),
                "connecting" | "playing" | "buffering"
            )
            && self.sleep_inhibitor.is_none()
        {
            self.sleep_inhibitor = Some(
                crate::windows::prevent_sleep().map_err(|e| format!("无法阻止自动休眠：{e}"))?,
            );
        }
        #[cfg(target_os = "macos")]
        if self.snapshot.prevent_sleep
            && self.child.is_some()
            && matches!(
                self.snapshot.status.as_str(),
                "connecting" | "playing" | "buffering"
            )
        {
            if let Some(inhibitor) = self.sleep_inhibitor.as_mut() {
                if inhibitor.try_wait().map_err(|e| e.to_string())?.is_some() {
                    self.sleep_inhibitor = None;
                    return Err("防休眠进程意外退出，请重新播放".into());
                }
            } else {
                // -i blocks idle system sleep, while allowing display sleep on battery or AC.
                // -w also releases the assertion if mpv exits before our next refresh.
                self.sleep_inhibitor = Some(
                    Command::new("/usr/bin/caffeinate")
                        .args(["-i", "-w", &self.child.as_ref().unwrap().id().to_string()])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|e| format!("无法阻止自动休眠：{e}"))?,
                );
            }
        }
        Ok(())
    }

    fn allow_sleep(&mut self) {
        #[cfg(windows)]
        if let Some(inhibitor) = self.sleep_inhibitor.take() {
            crate::windows::allow_sleep(inhibitor);
        }
        #[cfg(not(windows))]
        if let Some(mut inhibitor) = self.sleep_inhibitor.take() {
            let _ = inhibitor.kill();
            let _ = inhibitor.wait();
        }
    }

    pub fn stop(&mut self) {
        self.allow_sleep();
        #[cfg(windows)]
        if let Some(ipc) = self.ipc.as_mut()
            && writeln!(ipc.get_mut(), "{}", json!({"command": ["quit"]})).is_ok()
            && let Some(child) = self.child.as_mut()
        {
            // Let mpv cancel its yt-dlp/Deno subprocesses before the forced-stop fallback.
            let deadline = Instant::now() + Duration::from_millis(300);
            while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
        }
        self.ipc = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.connecting_since = None;
    }

    pub fn shutdown(&mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.runtime_dir);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;

    #[cfg(target_os = "macos")]
    #[test]
    fn playback_inhibits_only_idle_system_sleep_and_releases_it() {
        let dir = std::env::temp_dir().join(format!("lofi-sleep-test-{}", std::process::id()));
        let mut player = Player::new(&dir).unwrap();
        player.sync_sleep_inhibition().unwrap();
        assert!(player.sleep_inhibitor.is_none());
        for action in [
            "pause",
            "cancel",
            "idle",
            "failure",
            "disconnect",
            "shutdown",
        ] {
            player.child = Some(Command::new("sleep").arg("60").spawn().unwrap());
            player.snapshot.status = "connecting".into();
            player.sync_sleep_inhibition().unwrap();
            let pid = player.sleep_inhibitor.as_ref().unwrap().id();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let output = Command::new("/usr/bin/pmset")
                    .args(["-g", "assertions"])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let output = String::from_utf8(output.stdout).unwrap();
                let assertions: Vec<_> = output
                    .lines()
                    .filter(|line| line.trim_start().starts_with(&format!("pid {pid}(")))
                    .collect();
                if !assertions.is_empty() {
                    assert!(
                        assertions
                            .iter()
                            .any(|line| line.contains("PreventUserIdleSystemSleep"))
                    );
                    assert!(
                        !assertions
                            .iter()
                            .any(|line| line.contains("PreventUserIdleDisplaySleep"))
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "caffeinate did not acquire a sleep assertion"
                );
                thread::sleep(Duration::from_millis(30));
            }
            for status in ["playing", "buffering"] {
                player.snapshot.status = status.into();
                player.sync_sleep_inhibition().unwrap();
                assert_eq!(player.sleep_inhibitor.as_ref().unwrap().id(), pid);
            }
            let playback_pid = player.child.as_ref().unwrap().id();
            player.set_prevent_sleep(false).unwrap();
            assert!(player.sleep_inhibitor.is_none());
            assert!(
                !Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            player.sync_sleep_inhibition().unwrap();
            assert!(player.sleep_inhibitor.is_none());
            player.set_prevent_sleep(true).unwrap();
            let pid = player.sleep_inhibitor.as_ref().unwrap().id();
            assert_eq!(player.child.as_ref().unwrap().id(), playback_pid);
            assert_eq!(player.snapshot.status, "buffering");
            match action {
                "pause" => player.toggle().unwrap(),
                "cancel" => {
                    player.snapshot.status = "connecting".into();
                    player.toggle().unwrap();
                }
                "idle" => {
                    player.snapshot.status = "stopped".into();
                    player.sync_sleep_inhibition().unwrap();
                }
                "failure" => player.fail("test failure".into()),
                "disconnect" => assert!(player.refresh().is_err()),
                _ => player.shutdown(),
            }
            assert!(player.sleep_inhibitor.is_none());
            assert!(
                !Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            player.stop();
        }
        drop(player);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn station_settings_and_validation() {
        let dir = std::env::temp_dir().join(format!("lofi-stations-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");
        fs::write(&settings, r#"{"volume":42}"#).unwrap();
        let mut player = Player::new(&dir).unwrap();
        assert_eq!(player.snapshot.station, STATIONS[0]);
        assert_eq!(player.snapshot.volume, 42);
        assert!(player.snapshot.prevent_sleep); // Existing settings default to enabled.
        player.snapshot.status = "playing".into();
        assert!(player.select_station("invalid").is_err());
        assert_eq!(player.snapshot.status, "playing");
        player.select_station(STATIONS[0].id).unwrap();
        assert_eq!(player.snapshot.status, "playing");
        assert!(player.child.is_none()); // Selecting the active source does not restart it.
        fs::create_dir(player.settings.with_extension("tmp")).unwrap();
        assert!(player.set_prevent_sleep(false).is_err());
        assert!(player.snapshot.prevent_sleep); // A failed save must restore the menu state.
        fs::remove_dir(player.settings.with_extension("tmp")).unwrap();
        player.set_prevent_sleep(false).unwrap();
        player.snapshot.station = STATIONS[1];
        player.set_volume(25).unwrap();
        drop(player);
        let restored = Player::new(&dir).unwrap();
        assert_eq!(restored.snapshot.station, STATIONS[1]);
        assert_eq!(restored.snapshot.volume, 25);
        assert!(!restored.snapshot.prevent_sleep);
        assert_eq!(restored.snapshot.status, "stopped");
        drop(restored);
        fs::write(&settings, r#"{"volume":999,"station":"removed"}"#).unwrap();
        let fallback = Player::new(&dir).unwrap();
        assert_eq!(fallback.snapshot.station, STATIONS[0]);
        assert_eq!(fallback.snapshot.volume, 100);
        assert!(fallback.snapshot.prevent_sleep);
        drop(fallback);
        for (index, station) in STATIONS.iter().enumerate() {
            assert!(station.url.starts_with("https://www.youtube.com/watch?v="));
            assert!(
                !STATIONS[..index]
                    .iter()
                    .any(|s| s.id == station.id || s.url == station.url)
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs bundled Windows playback tools"]
    fn bundled_windows_player_controls() {
        let dir = std::env::temp_dir().join(format!("lofi-packaged-test-{}", std::process::id()));
        let mut player = Player::new(&dir).unwrap();
        // A local silent WAV exercises playback without network or an audio device.
        let bytes = 32_000u32 * 2 * 30;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(bytes + 36).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&32_000u32.to_le_bytes());
        wav.extend_from_slice(&64_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        wav.resize(44 + bytes as usize, 0);
        let audio = dir.join("quiet sample.wav");
        fs::write(&audio, wav).unwrap();
        for _ in 0..2 {
            player.start().unwrap();
            player.snapshot.status = "connecting".into();
            player.connecting_since = Some(Instant::now());
            player
                .command(json!(["loadfile", audio, "replace"]))
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while player.snapshot.status == "connecting" && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
                player.refresh().unwrap();
            }
            assert_eq!(player.snapshot.status, "playing");
            assert!(player.sleep_inhibitor.is_some());
            player.set_volume(25).unwrap();
            assert_eq!(
                player.command(json!(["get_property", "volume"])).unwrap(),
                25.0
            );
            assert_eq!(
                player.command(json!(["get_property", "vid"])).unwrap(),
                false
            );
            player.set_prevent_sleep(false).unwrap();
            assert!(player.sleep_inhibitor.is_none());
            player.set_prevent_sleep(true).unwrap();
            player.toggle().unwrap();
            assert_eq!(player.snapshot.status, "paused");
            assert!(
                player.child.is_none() && player.ipc.is_none() && player.sleep_inhibitor.is_none()
            );
        }
        player.start().unwrap();
        player.child.as_mut().unwrap().kill().unwrap();
        player.child.as_mut().unwrap().wait().unwrap();
        assert!(player.command(json!(["get_property", "pause"])).is_err());
        player.shutdown();
        assert!(!player.runtime_dir.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn player_controls_and_reports_failure() {
        let dir = std::env::temp_dir().join(format!("lofi-test-{}", std::process::id()));
        let mut player = Player::new(&dir).unwrap();
        assert_eq!(player.snapshot.volume, 30);
        assert!(player.set_volume(101).is_err());
        player.set_volume(42).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&player.settings).unwrap()).unwrap()["volume"],
            42
        );
        let socket = player.runtime_dir.join("test.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                request["command"],
                json!(["script-message", "lofi-volume", "25"])
            );
            // An event before a response must not desynchronize the IPC protocol.
            writeln!(reader.get_mut(), "{}", json!({"event": "tick"})).unwrap();
            writeln!(
                reader.get_mut(),
                "{}",
                json!({"request_id": request["request_id"], "error": "success"})
            )
            .unwrap();
        });
        player.ipc = Some(BufReader::new(IpcStream::connect(socket).unwrap()));
        player.snapshot.status = "playing".into();
        player.set_volume(25).unwrap();
        assert_eq!(player.snapshot.status, "playing");
        server.join().unwrap();
        assert!(player.command(json!(["get_property", "pause"])).is_err());
        player.fail("disconnected".into());
        assert_eq!(player.snapshot.toggle_label(), "重试播放");
        drop(player);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pausing_or_canceling_drops_the_live_session() {
        let dir = std::env::temp_dir().join(format!("lofi-stop-test-{}", std::process::id()));
        let mut player = Player::new(&dir).unwrap();
        for status in ["playing", "buffering", "connecting"] {
            let (client, server) = IpcStream::pair().unwrap();
            client
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            player.ipc = Some(BufReader::new(client));
            player.child = Some(Command::new("sleep").arg("60").spawn().unwrap());
            let pid = player.child.as_ref().unwrap().id();
            player.snapshot.status = status.into();
            player.connecting_since = Some(Instant::now());
            player.toggle().unwrap();
            let expected = if status == "connecting" {
                "stopped"
            } else {
                "paused"
            };
            assert_eq!(player.snapshot.status, expected);
            assert!(player.child.is_none() && player.ipc.is_none());
            assert!(player.connecting_since.is_none());
            assert_eq!(
                BufReader::new(server)
                    .read_line(&mut String::new())
                    .unwrap(),
                0
            );
            assert!(
                !Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            player.refresh().unwrap();
            player.set_volume(25).unwrap();
            assert_eq!(player.snapshot.status, expected);
            assert_eq!(player.snapshot.toggle_label(), "播放直播");
        }
        drop(player);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs mpv, yt-dlp, deno and a live internet connection"]
    fn live_stream_controls() {
        let dir = std::env::temp_dir().join(format!("lofi-live-test-{}", std::process::id()));
        let mut player = Player::new(&dir).unwrap();
        player.set_volume(0).unwrap();
        player.toggle().unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while player.snapshot.status == "connecting" && Instant::now() < deadline {
            thread::sleep(Duration::from_secs(1));
            player.refresh().unwrap();
        }
        assert_eq!(
            player.snapshot.status, "playing",
            "{:?}",
            player.snapshot.error
        );
        assert_eq!(
            player.command(json!(["get_property", "vid"])).unwrap(),
            false
        );
        assert_eq!(
            player
                .command(json!(["get_property", "audio-params"]))
                .unwrap()["samplerate"],
            44100
        );
        let position = player
            .command(json!(["get_property", "time-pos"]))
            .unwrap()
            .as_f64()
            .unwrap();
        thread::sleep(Duration::from_secs(2));
        assert!(
            player
                .command(json!(["get_property", "time-pos"]))
                .unwrap()
                .as_f64()
                .unwrap()
                > position
        );
        player.set_volume(10).unwrap();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            let gain = player
                .command(json!(["get_property", "ao-volume"]))
                .unwrap()
                .as_f64()
                .unwrap();
            if (gain - 0.1).abs() < 0.0001 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "renderer volume was not applied within 250ms"
            );
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            player.command(json!(["get_property", "volume"])).unwrap(),
            100.0
        );
        let old_pid = player.child.as_ref().unwrap().id();
        player.toggle().unwrap();
        assert_eq!(player.snapshot.status, "paused");
        assert!(player.child.is_none() && player.ipc.is_none());
        assert!(
            !Command::new("kill")
                .args(["-0", &old_pid.to_string()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        player.set_volume(0).unwrap();
        player.refresh().unwrap();
        assert_eq!(player.snapshot.status, "paused");
        player.toggle().unwrap();
        assert_eq!(player.snapshot.status, "connecting");
        assert_ne!(player.child.as_ref().unwrap().id(), old_pid);
        let deadline = Instant::now() + Duration::from_secs(60);
        while player.snapshot.status == "connecting" && Instant::now() < deadline {
            thread::sleep(Duration::from_secs(1));
            player.refresh().unwrap();
        }
        assert_eq!(
            player.snapshot.status, "playing",
            "{:?}",
            player.snapshot.error
        );
        assert_eq!(
            player.command(json!(["get_property", "pause"])).unwrap(),
            false
        );
        for station in &STATIONS[1..] {
            let old_pid = player.child.as_ref().unwrap().id();
            player.select_station(station.id).unwrap();
            assert!(
                !Command::new("kill")
                    .args(["-0", &old_pid.to_string()])
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            let deadline = Instant::now() + Duration::from_secs(60);
            while player.snapshot.status == "connecting" && Instant::now() < deadline {
                thread::sleep(Duration::from_secs(1));
                player.refresh().unwrap();
            }
            assert_eq!(
                player.snapshot.status, "playing",
                "{}: {:?}",
                station.name, player.snapshot.error
            );
            assert_eq!(
                player.command(json!(["get_property", "path"])).unwrap(),
                station.url
            );
            assert_eq!(player.snapshot.volume, 0);
            println!("verified {}", station.name);
        }
        let pid = player.child.as_ref().unwrap().id();
        player.shutdown();
        assert!(!player.runtime_dir.exists());
        assert!(
            !Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
