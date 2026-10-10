import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Station = { id: string; name: string; url: string };
type RadioState = { station: Station; status: string; volume: number; error: string | null };
const station = document.querySelector<HTMLSelectElement>("#station")!;
const play = document.querySelector<HTMLButtonElement>("#play")!;
const volume = document.querySelector<HTMLInputElement>("#volume")!;
const value = document.querySelector<HTMLOutputElement>("#volume-value")!;
const error = document.querySelector<HTMLParagraphElement>("#error")!;
let dragging = false;
let changing = false;

function showError(message: unknown) {
  error.textContent = String(message);
  error.hidden = false;
}

function render(state: RadioState) {
  const labels: Record<string, string> = {
    stopped: "待播放", connecting: "连接中…", playing: "直播中",
    paused: "已暂停", buffering: "缓冲中…", error: "连接失败",
  };
  document.body.dataset.status = state.status;
  station.value = state.station.id;
  document.querySelector("#station-name")!.textContent = state.station.name;
  document.querySelector("#status-text")!.textContent = labels[state.status] ?? state.status;
  const playing = state.status === "playing" || state.status === "buffering";
  document.querySelector("#play-icon")!.textContent = playing ? "Ⅱ" : state.status === "connecting" ? "■" : "▶";
  document.querySelector("#play-text")!.textContent = playing ? "暂停" : state.status === "connecting" ? "取消" : state.status === "error" ? "重试" : "播放";
  if (!dragging && !changing) {
    volume.value = String(state.volume);
    value.textContent = `${state.volume}%`;
  }
  error.textContent = state.error ?? "";
  error.hidden = !state.error;
}

play.addEventListener("click", async () => {
  play.disabled = true;
  try { render(await invoke<RadioState>("toggle_playback")); }
  catch (err) { showError(err); }
  finally { play.disabled = false; }
});

station.addEventListener("change", async () => {
  station.disabled = true;
  try { render(await invoke<RadioState>("select_station", { station: station.value })); }
  catch (err) {
    render(await invoke<RadioState>("get_status"));
    showError(err);
  }
  finally { station.disabled = false; }
});

volume.addEventListener("input", () => {
  dragging = true;
  value.textContent = `${volume.value}%`;
});
volume.addEventListener("change", async () => {
  dragging = false;
  changing = true;
  volume.disabled = true;
  try { render(await invoke<RadioState>("set_volume", { volume: Number(volume.value) })); }
  catch (err) { showError(err); }
  finally { changing = false; volume.disabled = false; }
});
document.querySelector("#hide")!.addEventListener("click", () => invoke("hide_window").catch(showError));
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" || (event.metaKey && event.key === "w")) {
    event.preventDefault();
    invoke("hide_window").catch(showError);
  }
});

async function init() {
  for (const source of await invoke<Station[]>("get_stations")) {
    station.add(new Option(source.name, source.id));
  }
  await listen<RadioState>("radio-state", ({ payload }) => render(payload));
  render(await invoke<RadioState>("get_status"));
  station.disabled = false;
}
init().catch(showError);
