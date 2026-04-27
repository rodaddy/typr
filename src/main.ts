import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface Settings {
  microphone: string;
  engine: string;
  whisperModel: string;
  groqApiKey: string;
  recordingMode: string;
  hotkey: string;
  outputMode: string;
  outputDir: string;
  streamStep: number;
  streamLength: number;
}

interface MicDevice {
  name: string;
  is_default: boolean;
}

interface DownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
}

// DOM elements -- settings
const statusDot = document.getElementById("status-dot")!;
const statusText = document.getElementById("status-text")!;
const micSelect = document.getElementById("mic-select") as HTMLSelectElement;
const engineLocal = document.getElementById("engine-local")!;
const engineCloud = document.getElementById("engine-cloud")!;
const localSettings = document.getElementById("local-settings")!;
const cloudSettings = document.getElementById("cloud-settings")!;
const modelSelect = document.getElementById("model-select") as HTMLSelectElement;
const downloadBtn = document.getElementById("download-btn")!;
const downloadProgress = document.getElementById("download-progress")!;
const progressFill = document.getElementById("progress-fill")!;
const groqKey = document.getElementById("groq-key") as HTMLInputElement;
const modeToggle = document.getElementById("mode-toggle")!;
const modePtt = document.getElementById("mode-ptt")!;
const hotkeyText = document.getElementById("hotkey-text")!;
const outputClipboard = document.getElementById("output-clipboard")!;
const outputDocument = document.getElementById("output-document")!;
const outputTerminal = document.getElementById("output-terminal")!;
const outputDirInput = document.getElementById("output-dir") as HTMLInputElement;
const outputDirRow = document.getElementById("output-dir-row")!;

// DOM elements -- streaming
const streamOutput = document.getElementById("stream-output") as HTMLTextAreaElement;
const startStreamBtn = document.getElementById("start-stream-btn")!;
const stopStreamBtn = document.getElementById("stop-stream-btn")!;
const clearStreamBtn = document.getElementById("clear-stream-btn")!;
const streamDot = document.getElementById("stream-dot")!;
const streamLabel = document.getElementById("stream-label")!;

// Section navigation
const navItems = document.querySelectorAll(".nav-item");
const sections = document.querySelectorAll(".content-section");

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const target = item.getAttribute("data-section");
    navItems.forEach((n) => n.classList.remove("active"));
    sections.forEach((s) => s.classList.remove("active"));
    item.classList.add("active");
    document.getElementById(`section-${target}`)?.classList.add("active");
  });
});

// Window drag -- titlebar and sidebar empty space
const titlebar = document.getElementById("titlebar")!;
const sidebar = document.getElementById("sidebar")!;
const appWindow = getCurrentWindow();

titlebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow.startDragging();
});

sidebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow.startDragging();
});

let currentSettings: Settings;

async function loadSettings() {
  currentSettings = await invoke<Settings>("get_settings");

  // Populate mic dropdown
  const mics = await invoke<MicDevice[]>("list_microphones");
  micSelect.innerHTML = "";
  mics.forEach((mic) => {
    const option = document.createElement("option");
    option.value = mic.name;
    option.textContent = mic.name + (mic.is_default ? " (default)" : "");
    micSelect.appendChild(option);
  });
  micSelect.value = currentSettings.microphone;

  // Engine
  setEngine(currentSettings.engine);

  // Model
  modelSelect.value = currentSettings.whisperModel;
  await checkModelStatus();

  // Groq key
  groqKey.value = currentSettings.groqApiKey;

  // Recording mode
  setRecordingMode(currentSettings.recordingMode);

  // Output mode
  setOutputMode(currentSettings.outputMode ?? "clipboard");
  outputDirInput.value = currentSettings.outputDir ?? "~/Documents/Typr/";

  // Hotkey
  hotkeyText.textContent = currentSettings.hotkey.replace("CmdOrCtrl", "Cmd");
}

function setEngine(engine: string) {
  currentSettings.engine = engine;
  engineLocal.classList.toggle("active", engine === "local");
  engineCloud.classList.toggle("active", engine === "cloud");
  localSettings.classList.toggle("hidden", engine !== "local");
  cloudSettings.classList.toggle("hidden", engine !== "cloud");
}

function setRecordingMode(mode: string) {
  currentSettings.recordingMode = mode;
  modeToggle.classList.toggle("active", mode === "toggle");
  modePtt.classList.toggle("active", mode === "push-to-talk");
}

function setOutputMode(mode: string) {
  currentSettings.outputMode = mode;
  outputClipboard.classList.toggle("active", mode === "clipboard");
  outputDocument.classList.toggle("active", mode === "document");
  outputTerminal.classList.toggle("active", mode === "terminal");
  outputDirRow.classList.toggle("hidden", mode !== "document");
}

async function checkModelStatus() {
  const downloaded = await invoke<boolean>("check_model_downloaded", {
    modelSize: modelSelect.value,
  });
  downloadBtn.textContent = downloaded ? "✓" : "Download";
  (downloadBtn as HTMLButtonElement).disabled = downloaded;
}

async function saveSettings() {
  currentSettings.microphone = micSelect.value;
  currentSettings.whisperModel = modelSelect.value;
  currentSettings.groqApiKey = groqKey.value;
  currentSettings.outputDir = outputDirInput.value;
  await invoke("save_settings", { settings: currentSettings });
}

// Settings event listeners
engineLocal.addEventListener("click", () => {
  setEngine("local");
  saveSettings();
});

engineCloud.addEventListener("click", () => {
  setEngine("cloud");
  saveSettings();
});

micSelect.addEventListener("change", () => saveSettings());

modelSelect.addEventListener("change", async () => {
  await checkModelStatus();
  saveSettings();
});

downloadBtn.addEventListener("click", async () => {
  (downloadBtn as HTMLButtonElement).disabled = true;
  downloadProgress.classList.remove("hidden");
  progressFill.style.width = "0%";

  try {
    await invoke("download_model", { modelSize: modelSelect.value });
    downloadBtn.textContent = "✓";
  } catch (e) {
    downloadBtn.textContent = "Retry";
    (downloadBtn as HTMLButtonElement).disabled = false;
    console.error("Download failed:", e);
  }
  downloadProgress.classList.add("hidden");
});

groqKey.addEventListener("change", () => saveSettings());

modeToggle.addEventListener("click", () => {
  setRecordingMode("toggle");
  saveSettings();
});

modePtt.addEventListener("click", () => {
  setRecordingMode("push-to-talk");
  saveSettings();
});

outputClipboard.addEventListener("click", () => { setOutputMode("clipboard"); saveSettings(); });
outputDocument.addEventListener("click", () => { setOutputMode("document"); saveSettings(); });
outputTerminal.addEventListener("click", () => { setOutputMode("terminal"); saveSettings(); });
outputDirInput.addEventListener("change", () => saveSettings());

// Listen for recording state changes
listen<string>("recording-state", (event) => {
  const state = event.payload;
  statusDot.className = "";
  if (state === "Recording") {
    statusDot.classList.add("recording");
    statusText.textContent = "Recording...";
  } else if (state === "Transcribing") {
    statusDot.classList.add("transcribing");
    statusText.textContent = "Transcribing...";
  } else {
    statusDot.classList.add("ready");
    statusText.textContent = "Ready";
  }
});

// Listen for download progress
listen<DownloadProgress>("download-progress", (event) => {
  const { percent } = event.payload;
  progressFill.style.width = `${percent}%`;
});

// --- Streaming ---

function setStreamingUI(streaming: boolean) {
  if (streaming) {
    streamDot.classList.add("active");
    streamLabel.textContent = "Streaming...";
    streamLabel.classList.add("active");
    startStreamBtn.classList.add("hidden");
    stopStreamBtn.classList.remove("hidden");
    statusDot.className = "streaming";
    statusText.textContent = "Streaming...";
  } else {
    streamDot.classList.remove("active");
    streamLabel.textContent = "Stopped";
    streamLabel.classList.remove("active");
    startStreamBtn.classList.remove("hidden");
    stopStreamBtn.classList.add("hidden");
    statusDot.className = "ready";
    statusText.textContent = "Ready";
  }
}

startStreamBtn.addEventListener("click", async () => {
  try {
    await invoke("start_stream");
    setStreamingUI(true);
  } catch (e) {
    console.error("Failed to start stream:", e);
    alert(`Failed to start streaming: ${e}`);
  }
});

stopStreamBtn.addEventListener("click", async () => {
  try {
    await invoke("stop_stream");
    setStreamingUI(false);
  } catch (e) {
    console.error("Failed to stop stream:", e);
  }
});

clearStreamBtn.addEventListener("click", () => {
  streamOutput.value = "";
});

listen<string>("stream-text", (event) => {
  const text = event.payload;
  const atBottom =
    streamOutput.scrollHeight - streamOutput.scrollTop <=
    streamOutput.clientHeight + 20;
  streamOutput.value += text + "\n";
  if (atBottom) {
    streamOutput.scrollTop = streamOutput.scrollHeight;
  }
});

listen<null>("stream-stopped", () => {
  setStreamingUI(false);
});

// Sync initial streaming state on load
invoke<boolean>("is_streaming").then((streaming) => {
  if (streaming) setStreamingUI(true);
});

// Initialize settings
loadSettings();
