import { invoke } from "@tauri-apps/api/core";

interface AdbDevice {
  serial: string;
  status: string;
}

interface ScrcpyOptions {
  max_size: number | null;
  bitrate: string | null;
  max_fps: number | null;

  video_codec: string | null;
  video_encoder: string | null;
  video_codec_options: string | null;

  capture_orientation: string | null;
  orientation: string | null;
  angle: number | null;

  crop: string | null;
  display_id: number | null;

  video_buffer: number | null;

  no_video: boolean;
  no_video_playback: boolean;

  no_audio: boolean;
  audio_dup: boolean;

  audio_codec: string | null;
  audio_codec_options: string | null;
  audio_buffer: number | null;

  no_audio_playback: boolean;

  no_control: boolean;
  show_touches: boolean;

  screen_off_timeout: number | null;
  keep_active: boolean;
  stay_awake: boolean;

  no_playback: boolean;

  no_window: boolean;

  window_title: string | null;

  window_x: number | null;
  window_y: number | null;

  window_width: number | null;
  window_height: number | null;

  no_window_aspect_ratio_lock: boolean;

  background_color: string | null;

  borderless: boolean;
  always_on_top: boolean;
  fullscreen: boolean;

  disable_screensaver: boolean;

  render_fit: string | null;

  record: string | null;
  record_format: string | null;
  record_orientation: string | null;

  time_limit: number | null;
}


// ============================================================
// Application state
// ============================================================

let usbSerial: string | null = null;
let deviceIp: string | null = null;

let tcpConnected = false;
let casting = false;


// ============================================================
// Default scrcpy configuration
// ============================================================

function getDefaultScrcpyOptions(): ScrcpyOptions {
  return {
    max_size: 1920,
    bitrate: "8M",
    max_fps: 60,

    video_codec: "h264",
    video_encoder: null,
    video_codec_options: null,

    capture_orientation: null,
    orientation: null,
    angle: null,

    crop: null,
    display_id: null,

    video_buffer: null,

    no_video: false,
    no_video_playback: false,

    no_audio: true,
    audio_dup: false,

    audio_codec: null,
    audio_codec_options: null,
    audio_buffer: null,

    no_audio_playback: false,

    no_control: false,
    show_touches: true,

    screen_off_timeout: 0,
    keep_active: true,
    stay_awake: true,

    no_playback: false,

    no_window: false,

    window_title: "PhoneCasting",

    window_x: null,
    window_y: null,

    window_width: null,
    window_height: null,

    no_window_aspect_ratio_lock: false,

    background_color: null,

    borderless: false,
    always_on_top: false,
    fullscreen: true,

    disable_screensaver: true,

    render_fit: "letterbox",

    record: null,
    record_format: null,
    record_orientation: null,

    time_limit: null,
  };
}


// ============================================================
// DOM helpers
// ============================================================

function element<T extends HTMLElement>(
  id: string
): T | null {
  return document.querySelector<T>(`#${id}`);
}

function setText(
  id: string,
  text: string
) {
  const el = element<HTMLElement>(id);

  if (el) {
    el.textContent = text;
  }
}

function setStatus(message: string) {
  setText("status-output", message);
}

function appendStatus(message: string) {
  const output = element<HTMLPreElement>(
    "status-output"
  );

  if (!output) {
    return;
  }

  const current = output.textContent ?? "";

  output.textContent =
    current === "Ready."
      ? message
      : `${current}\n${message}`;
}

function showError(error: unknown) {
  console.error(error);

  const message = String(error);

  setStatus(`ERROR: ${message}`);
}


// ============================================================
// Connection status
// ============================================================

function updateConnectionStatus() {
  const dot = element<HTMLElement>(
    "status-dot"
  );

  const text = element<HTMLElement>(
    "connection-status-text"
  );

  const deviceState = element<HTMLElement>(
    "device-state"
  );

  if (tcpConnected) {
    dot?.classList.add("connected");

    if (text) {
      text.textContent = "TCP/IP Connected";
    }

    if (deviceState) {
      deviceState.textContent = "TCP/IP connected";
    }

    return;
  }

  if (usbSerial) {
    dot?.classList.add("connected");

    if (text) {
      text.textContent = "USB Connected";
    }

    if (deviceState) {
      deviceState.textContent = "USB connected";
    }

    return;
  }

  dot?.classList.remove("connected");

  if (text) {
    text.textContent = "Disconnected";
  }

  if (deviceState) {
    deviceState.textContent = "Not connected";
  }
}


// ============================================================
// Device detection
// ============================================================

async function refreshDevices() {
  try {
    setStatus("Detecting ADB devices...");

    const devices = await invoke<AdbDevice[]>(
      "get_usb_devices"
    );

    const device = devices.find(
      (item) => item.status === "device"
    );

    if (!device) {
      usbSerial = null;

      setText(
        "device-name",
        "No device detected"
      );

      setText(
        "device-serial",
        "—"
      );

      setText(
        "device-state",
        "Not connected"
      );

      updateConnectionStatus();

      setStatus(
        "No ADB device detected."
      );

      return;
    }

    usbSerial = device.serial;

    setText(
      "device-name",
      "Android Device"
    );

    setText(
      "device-serial",
      device.serial
    );

    setText(
      "device-state",
      "USB connected"
    );

    updateConnectionStatus();

    setStatus(
      `Device detected.\nSerial: ${device.serial}`
    );

  } catch (error) {
    showError(error);
  }
}


// ============================================================
// IP detection
// ============================================================

async function detectIp() {
  try {
    if (!usbSerial) {
      await refreshDevices();
    }

    if (!usbSerial) {
      showError(
        "Connect the phone through USB first so its IP address can be detected."
      );

      return;
    }

    setStatus(
      "Detecting phone IP address..."
    );

    deviceIp = await invoke<string>(
      "get_device_ip",
      {
        serial: usbSerial,
      }
    );

    setText(
      "device-ip",
      deviceIp
    );

    setStatus(
      `Phone IP detected: ${deviceIp}`
    );

  } catch (error) {
    showError(error);
  }
}


// ============================================================
// TCP/IP connection
// ============================================================

async function connectTcp() {
  try {
    if (!deviceIp) {
      await detectIp();
    }

    if (!deviceIp) {
      showError(
        "Could not determine the phone IP address."
      );

      return;
    }

    setStatus(
      `Connecting to ${deviceIp}:5555...`
    );

    const result = await invoke<string>(
      "connect_tcp",
      {
        ip: deviceIp,
      }
    );

    tcpConnected = true;

    updateConnectionStatus();

    setStatus(
      `TCP/IP connection established.\n\n${result}`
    );

  } catch (error) {
    tcpConnected = false;

    updateConnectionStatus();

    showError(error);
  }
}


// ============================================================
// Disconnect
// ============================================================

async function disconnectDevice() {
  try {
    setStatus(
      "Disconnecting device..."
    );

    // Refresh the USB serial if necessary.
    if (!usbSerial) {
      try {
        const devices =
          await invoke<AdbDevice[]>(
            "get_usb_devices"
          );

        const device = devices.find(
          (item) => item.status === "device"
        );

        if (device) {
          usbSerial = device.serial;
        }
      } catch {
        // Continue. USB may already be disconnected.
      }
    }

    // If we don't have an IP but USB is available,
    // automatically detect it.
    if (!deviceIp && usbSerial) {
      try {
        deviceIp = await invoke<string>(
          "get_device_ip",
          {
            serial: usbSerial,
          }
        );
      } catch {
        deviceIp = null;
      }
    }

    const result =
      await invoke<string>(
        "disconnect_device",
        {
          tcpIp: deviceIp,
          usbSerial: usbSerial,
        }
      );

    tcpConnected = false;

    deviceIp = null;

    setText(
      "device-ip",
      "—"
    );

    updateConnectionStatus();

    setStatus(result);

  } catch (error) {
    showError(error);
  }
}


// ============================================================
// Start scrcpy
// ============================================================

async function startCasting() {
  try {
    let serial: string | null = null;

    // TCP/IP has priority when it is connected.
    if (tcpConnected && deviceIp) {
      serial = `${deviceIp}:5555`;
    } else {
      // Otherwise use the USB device.
      if (!usbSerial) {
        await refreshDevices();
      }

      serial = usbSerial;
    }

    if (!serial) {
      showError(
        "No Android device is connected."
      );

      return;
    }

    const options = getDefaultScrcpyOptions();

    setStatus(
      `Starting scrcpy...\nDevice: ${serial}`
    );

    await invoke<string>(
      "start_scrcpy",
      {
        serial,
        options,
      }
    );

    setText(
      "casting-description",
      "Casting is running."
    );

    const startButton =
      element<HTMLButtonElement>(
        "start-casting"
      );

    const stopButton =
      element<HTMLButtonElement>(
        "stop-casting"
      );

    if (startButton) {
      startButton.disabled = true;
    }

    if (stopButton) {
      stopButton.disabled = false;
    }

    appendStatus(
      `scrcpy started successfully.\nTarget: ${serial}`
    );

  } catch (error) {
    showError(error);
  }
}


// ============================================================
// Stop casting
// ============================================================

async function stopCasting() {
  /*
   * scrcpy is currently launched as an independent process.
   *
   * We will add proper process tracking/termination
   * in the backend rather than trying to kill it from
   * the frontend.
   */

  casting = false;

  setText(
    "casting-description",
    "Ready to cast your Android screen."
  );

  const startButton =
    element<HTMLButtonElement>(
      "start-casting"
    );

  const stopButton =
    element<HTMLButtonElement>(
      "stop-casting"
    );

  if (startButton) {
    startButton.disabled = false;
  }

  if (stopButton) {
    stopButton.disabled = true;
  }

  appendStatus(
    "Casting state reset."
  );
}


// ============================================================
// Connection method buttons
// ============================================================

function selectUsb() {
  element("connection-usb")
    ?.classList.add("active");

  element("connection-wifi")
    ?.classList.remove("active");

  setStatus(
    "USB connection selected."
  );
}

function selectWifi() {
  element("connection-wifi")
    ?.classList.add("active");

  element("connection-usb")
    ?.classList.remove("active");

  setStatus(
    "TCP/IP connection selected."
  );
}


// ============================================================
// Event registration
// ============================================================

window.addEventListener(
  "DOMContentLoaded",
  () => {

    element<HTMLButtonElement>(
      "refresh-devices"
    )?.addEventListener(
      "click",
      refreshDevices
    );

    element<HTMLButtonElement>(
      "detect-ip"
    )?.addEventListener(
      "click",
      detectIp
    );

    element<HTMLButtonElement>(
      "connect-tcp"
    )?.addEventListener(
      "click",
      connectTcp
    );

    element<HTMLButtonElement>(
      "disconnect-device"
    )?.addEventListener(
      "click",
      disconnectDevice
    );

    element<HTMLButtonElement>(
      "start-casting"
    )?.addEventListener(
      "click",
      startCasting
    );

    element<HTMLButtonElement>(
      "stop-casting"
    )?.addEventListener(
      "click",
      stopCasting
    );

    element<HTMLButtonElement>(
      "connection-usb"
    )?.addEventListener(
      "click",
      selectUsb
    );

    element<HTMLButtonElement>(
      "connection-wifi"
    )?.addEventListener(
      "click",
      selectWifi
    );

    // Initial device detection.
    refreshDevices();
  }
);