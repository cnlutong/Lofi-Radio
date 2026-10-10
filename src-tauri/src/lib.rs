mod player;

use player::{Player, STATIONS, Snapshot, Station};
use std::{sync::Mutex, thread, time::Duration};
use tauri::{
    Emitter, Manager,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

type Radio = Mutex<Player>;

fn tray_icon(status: &str) -> tauri::Result<Image<'static>> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    if matches!(status, "paused" | "stopped" | "error") {
        let mut rgba = icon.rgba().to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] /= 2;
        }
        Ok(Image::new_owned(rgba, icon.width(), icon.height()))
    } else {
        Ok(icon)
    }
}

fn publish(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let _ = app.emit("radio-state", snapshot);
    if let Some(tray) = app.tray_by_id("radio") {
        let _ = tray.set_tooltip(Some(snapshot.label()));
        if let Ok(icon) = tray_icon(&snapshot.status) {
            let _ = tray.set_icon_with_as_template(Some(icon), true);
        }
    }
    // These handles refer to the menu already attached to the tray.
    if let Some(controls) = app.try_state::<Controls>() {
        let _ = controls.status.set_text(snapshot.label());
        let _ = controls.toggle.set_text(snapshot.toggle_label());
        let _ = controls.prevent_sleep.set_checked(snapshot.prevent_sleep);
        let _ = controls
            .volume
            .set_text(format!("音量 · {}%", snapshot.volume));
        for (id, item) in &controls.stations {
            let _ = item.set_checked(*id == snapshot.station.id);
        }
    }
}

struct Controls {
    status: MenuItem<tauri::Wry>,
    toggle: MenuItem<tauri::Wry>,
    prevent_sleep: CheckMenuItem<tauri::Wry>,
    volume: Submenu<tauri::Wry>,
    stations: Vec<(&'static str, CheckMenuItem<tauri::Wry>)>,
}

enum Action {
    Toggle,
    ToggleSleepPrevention,
    Volume(u8),
    Station(String),
}

async fn control(app: tauri::AppHandle, action: Action) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Radio>();
        let mut player = state.lock().map_err(|e| e.to_string())?;
        let result = match &action {
            Action::Volume(v) => player.set_volume(*v),
            Action::Toggle => player.toggle(),
            Action::ToggleSleepPrevention => {
                let enabled = !player.snapshot.prevent_sleep;
                player.set_prevent_sleep(enabled)
            }
            Action::Station(id) => player.select_station(id),
        };
        if let Err(error) = &result {
            if matches!(action, Action::Toggle | Action::Station(_)) {
                player.fail(error.clone());
            } else {
                player.snapshot.error = Some(error.clone());
            }
        }
        let snapshot = player.snapshot.clone();
        drop(player);
        publish(&app, &snapshot);
        result.map(|_| snapshot)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn toggle_playback(app: tauri::AppHandle) -> Result<Snapshot, String> {
    control(app, Action::Toggle).await
}

#[tauri::command]
async fn set_volume(app: tauri::AppHandle, volume: u8) -> Result<Snapshot, String> {
    control(app, Action::Volume(volume)).await
}

#[tauri::command]
fn get_stations() -> &'static [Station] {
    STATIONS
}

#[tauri::command]
async fn select_station(app: tauri::AppHandle, station: String) -> Result<Snapshot, String> {
    if !STATIONS.iter().any(|s| s.id == station) {
        return Err("未知直播源".into());
    }
    control(app, Action::Station(station)).await
}

#[tauri::command]
fn get_status(state: tauri::State<'_, Radio>) -> Result<Snapshot, String> {
    state
        .lock()
        .map(|player| player.snapshot.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            toggle_playback,
            set_volume,
            select_station,
            get_stations,
            get_status,
            hide_window
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let player = Player::new(&app.path().app_data_dir()?)?;
            app.manage(Mutex::new(player));
            let status = MenuItem::with_id(app, "status", "准备好开始专注", false, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "播放直播", true, None::<&str>)?;
            let prevent_sleep = CheckMenuItem::with_id(
                app,
                "prevent-sleep",
                "播放时电脑不休眠",
                true,
                true,
                None::<&str>,
            )?;
            let volume = Submenu::new(app, "音量", true)?;
            let sources = Submenu::new(app, "直播源", true)?;
            let mut stations = Vec::new();
            for station in STATIONS {
                let item = CheckMenuItem::with_id(
                    app,
                    format!("station-{}", station.id),
                    station.name,
                    true,
                    false,
                    None::<&str>,
                )?;
                sources.append(&item)?;
                stations.push((station.id, item));
            }
            for level in [0, 10, 25, 50, 75, 100] {
                volume.append(&MenuItem::with_id(
                    app,
                    format!("volume-{level}"),
                    format!("{level}%"),
                    true,
                    None::<&str>,
                )?)?;
            }
            let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 Lofi Radio", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &status,
                    &toggle,
                    &sources,
                    &volume,
                    &prevent_sleep,
                    &show,
                    &quit,
                ],
            )?;
            app.manage(Controls {
                status,
                toggle,
                prevent_sleep,
                volume,
                stations,
            });
            TrayIconBuilder::with_id("radio")
                .icon(tray_icon("stopped")?)
                .icon_as_template(true)
                .tooltip("Lofi Radio")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "quit" => app.exit(0),
                    "toggle" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = control(app, Action::Toggle).await;
                        });
                    }
                    "prevent-sleep" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = control(app, Action::ToggleSleepPrevention).await;
                        });
                    }
                    id if id.starts_with("volume-") => {
                        if let Ok(volume) = id[7..].parse::<u8>() {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = control(app, Action::Volume(volume)).await;
                            });
                        }
                    }
                    id if id.starts_with("station-") => {
                        let station = id[8..].to_owned();
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = select_station(app, station).await;
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = control(app, Action::Toggle).await;
                        });
                    }
                })
                .build(app)?;
            publish(app.handle(), &app.state::<Radio>().lock().unwrap().snapshot);
            let handle = app.handle().clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    let Some(state) = handle.try_state::<Radio>() else {
                        break;
                    };
                    let Ok(mut player) = state.lock() else {
                        break;
                    };
                    let before = player.snapshot.clone();
                    if let Err(error) = player.refresh() {
                        player.stop();
                        player.fail(error);
                    }
                    let after = player.snapshot.clone();
                    drop(player);
                    if before != after {
                        publish(&handle, &after);
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("无法启动 Lofi Radio");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event
            && let Some(state) = app.try_state::<Radio>()
            && let Ok(mut player) = state.lock()
        {
            player.shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::tray_icon;

    #[test]
    fn inactive_tray_icon_preserves_shape_with_half_opacity() {
        let original = tray_icon("playing").unwrap();
        assert!(original.rgba().chunks_exact(4).any(|pixel| pixel[3] > 0));
        for status in ["paused", "stopped", "error", "connecting", "buffering"] {
            let icon = tray_icon(status).unwrap();
            assert_eq!(
                (icon.width(), icon.height()),
                (original.width(), original.height())
            );
            for (pixel, source) in icon
                .rgba()
                .chunks_exact(4)
                .zip(original.rgba().chunks_exact(4))
            {
                assert_eq!(&pixel[..3], &source[..3]);
                assert_eq!(
                    pixel[3],
                    if matches!(status, "paused" | "stopped" | "error") {
                        source[3] / 2
                    } else {
                        source[3]
                    }
                );
            }
        }
    }
}
