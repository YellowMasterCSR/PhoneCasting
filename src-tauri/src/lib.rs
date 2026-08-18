use std::process::Command;
use tauri::Manager;

const ADB_TCP_PORT: &str = "5555";

#[derive(serde::Serialize)]
struct AdbDevice {
    serial: String,
    status: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct ScrcpyOptions {
    // =========================
    // Video
    // =========================
    max_size: Option<u32>,
    bitrate: Option<String>,
    max_fps: Option<u32>,

    video_codec: Option<String>,
    video_encoder: Option<String>,
    video_codec_options: Option<String>,

    capture_orientation: Option<String>,
    orientation: Option<String>,
    angle: Option<f32>,

    crop: Option<String>,
    display_id: Option<u32>,

    video_buffer: Option<u32>,

    no_video: bool,
    no_video_playback: bool,

    // =========================
    // Audio
    // =========================
    no_audio: bool,
    audio_dup: bool,

    audio_codec: Option<String>,
    audio_codec_options: Option<String>,
    audio_buffer: Option<u32>,

    no_audio_playback: bool,

    // =========================
    // Control
    // =========================
    no_control: bool,
    show_touches: bool,

    screen_off_timeout: Option<u32>,
    keep_active: bool,
    stay_awake: bool,

    // =========================
    // Playback
    // =========================
    no_playback: bool,

    // =========================
    // Window
    // =========================
    no_window: bool,

    window_title: Option<String>,

    window_x: Option<i32>,
    window_y: Option<i32>,

    window_width: Option<u32>,
    window_height: Option<u32>,

    no_window_aspect_ratio_lock: bool,

    background_color: Option<String>,

    borderless: bool,
    always_on_top: bool,
    fullscreen: bool,

    disable_screensaver: bool,

    render_fit: Option<String>,

    // =========================
    // Recording
    // =========================
    record: Option<String>,
    record_format: Option<String>,

    record_orientation: Option<String>,

    time_limit: Option<u64>,
}

/// Run the bundled ADB executable.
fn run_adb(app: &tauri::AppHandle, args: &[&str]) -> Result<std::process::Output, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let scrcpy_dir = resource_path.join("bin").join("scrcpy");
    let adb_exe = scrcpy_dir.join("adb.exe");

    if !adb_exe.exists() {
        return Err(format!("adb.exe not found at: {}", adb_exe.display()));
    }

    Command::new(&adb_exe)
        .args(args)
        .current_dir(&scrcpy_dir)
        .output()
        .map_err(|e| format!("Failed to run ADB: {}", e))
}

/// Get all devices currently known to ADB.
#[tauri::command]
fn get_usb_devices(app: tauri::AppHandle) -> Result<Vec<AdbDevice>, String> {
    let output = run_adb(&app, &["devices"])?;

    if !output.status.success() {
        return Err(format!(
            "ADB failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let devices = stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();

            let serial = parts.next()?.to_string();
            let status = parts.next()?.to_string();

            Some(AdbDevice { serial, status })
        })
        .collect();

    Ok(devices)
}

/// Get the IPv4 address of a USB-connected Android device.
#[tauri::command]
fn get_device_ip(app: tauri::AppHandle, serial: String) -> Result<String, String> {
    let output = run_adb(&app, &["-s", &serial, "shell", "ip", "-4", "addr", "show"])?;

    if !output.status.success() {
        return Err(format!(
            "Failed to get device network information: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let trimmed = line.trim();

        if let Some(pos) = trimmed.find("inet ") {
            let address = trimmed[pos + 5..].split_whitespace().next().unwrap_or("");

            let ip = address.split('/').next().unwrap_or("");

            if !ip.is_empty() && !ip.starts_with("127.") {
                return Ok(ip.to_string());
            }
        }
    }

    Err("Could not find a non-loopback IPv4 address on the device.".to_string())
}

/// Connect to an Android device using ADB over TCP/IP.
///
/// PhoneCasting always uses port 5555.
#[tauri::command]
fn connect_tcp(app: tauri::AppHandle, ip: String) -> Result<String, String> {
    let address = format!("{}:{}", ip.trim(), ADB_TCP_PORT);

    let output = run_adb(&app, &["connect", &address])?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!(
            "ADB TCP connection failed: {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    Ok(stdout.trim().to_string())
}

/// Disconnect an ADB TCP/IP device.
#[tauri::command]
fn disconnect_tcp(app: tauri::AppHandle, ip: String) -> Result<String, String> {
    let address = format!("{}:{}", ip.trim(), ADB_TCP_PORT);

    let output = run_adb(&app, &["disconnect", &address])?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!(
            "ADB disconnect failed: {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    Ok(stdout.trim().to_string())
}

/// Switch a USB-connected Android device back to USB ADB.
///
/// This disables the TCP/IP ADB listener by running:
/// adb -s <USB_SERIAL> usb
#[tauri::command]
fn disable_tcpip(app: tauri::AppHandle, serial: String) -> Result<String, String> {
    let output = run_adb(&app, &["-s", &serial, "usb"])?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!(
            "Failed to disable TCP/IP ADB: {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    Ok(if stdout.trim().is_empty() {
        "ADB switched back to USB mode.".to_string()
    } else {
        stdout.trim().to_string()
    })
}

/// Build the scrcpy command-line arguments from the selected options.
///
/// This does NOT launch scrcpy.
fn build_scrcpy_args(serial: &str, options: &ScrcpyOptions) -> Result<Vec<String>, String> {
    let mut args = Vec::new();

    // =========================
    // Device
    // =========================

    if !serial.trim().is_empty() {
        args.push("-s".to_string());
        args.push(serial.trim().to_string());
    }

    // =========================
    // Video
    // =========================

    if let Some(max_size) = options.max_size {
        if max_size == 0 {
            return Err("Max size must be greater than 0.".to_string());
        }

        args.push(format!("--max-size={}", max_size));
    }

    if let Some(bitrate) = &options.bitrate {
        if !bitrate.trim().is_empty() {
            args.push(format!("--video-bit-rate={}", bitrate.trim()));
        }
    }

    if let Some(max_fps) = options.max_fps {
        if max_fps == 0 {
            return Err("Maximum FPS must be greater than 0.".to_string());
        }

        args.push(format!("--max-fps={}", max_fps));
    }

    if let Some(codec) = &options.video_codec {
        if !codec.trim().is_empty() {
            match codec.trim() {
                "h264" | "h265" | "av1" => {
                    args.push(format!("--video-codec={}", codec.trim()));
                }

                _ => {
                    return Err(format!(
                        "Invalid video codec '{}'. Expected h264, h265, or av1.",
                        codec
                    ));
                }
            }
        }
    }

    if let Some(encoder) = &options.video_encoder {
        if !encoder.trim().is_empty() {
            args.push(format!("--video-encoder={}", encoder.trim()));
        }
    }

    if let Some(codec_options) = &options.video_codec_options {
        if !codec_options.trim().is_empty() {
            args.push(format!("--video-codec-options={}", codec_options.trim()));
        }
    }

    // =========================
    // Orientation
    // =========================

    if let Some(orientation) = &options.capture_orientation {
        if !orientation.trim().is_empty() {
            args.push(format!("--capture-orientation={}", orientation.trim()));
        }
    }

    if let Some(orientation) = &options.orientation {
        if !orientation.trim().is_empty() {
            args.push(format!("--orientation={}", orientation.trim()));
        }
    }

    if let Some(angle) = options.angle {
        args.push(format!("--angle={}", angle));
    }

    // =========================
    // Crop
    // =========================

    if let Some(crop) = &options.crop {
        if !crop.trim().is_empty() {
            args.push(format!("--crop={}", crop.trim()));
        }
    }

    // =========================
    // Display
    // =========================

    if let Some(display_id) = options.display_id {
        args.push(format!("--display-id={}", display_id));
    }

    // =========================
    // Video buffering
    // =========================

    if let Some(buffer) = options.video_buffer {
        args.push(format!("--video-buffer={}", buffer));
    }

    // =========================
    // Video playback
    // =========================

    if options.no_video {
        args.push("--no-video".to_string());
    }

    if options.no_video_playback {
        args.push("--no-video-playback".to_string());
    }

    // =========================
    // Audio
    // =========================

    if options.no_audio {
        args.push("--no-audio".to_string());
    }

    if options.audio_dup {
        args.push("--audio-dup".to_string());
    }

    if let Some(codec) = &options.audio_codec {
        if !codec.trim().is_empty() {
            args.push(format!("--audio-codec={}", codec.trim()));
        }
    }

    if let Some(codec_options) = &options.audio_codec_options {
        if !codec_options.trim().is_empty() {
            args.push(format!("--audio-codec-options={}", codec_options.trim()));
        }
    }

    if let Some(buffer) = options.audio_buffer {
        args.push(format!("--audio-buffer={}", buffer));
    }

    if options.no_audio_playback {
        args.push("--no-audio-playback".to_string());
    }

    // =========================
    // Control
    // =========================

    if options.no_control {
        args.push("--no-control".to_string());
    }

    if options.show_touches {
        args.push("--show-touches".to_string());
    }

    if let Some(timeout) = options.screen_off_timeout {
        args.push(format!("--screen-off-timeout={}", timeout));
    }

    if options.keep_active {
        args.push("--keep-active".to_string());
    }

    if options.stay_awake {
        args.push("--stay-awake".to_string());
    }

    // =========================
    // Playback
    // =========================

    if options.no_playback {
        args.push("--no-playback".to_string());
    }

    // =========================
    // Window
    // =========================

    if options.no_window {
        args.push("--no-window".to_string());
    }

    if let Some(title) = &options.window_title {
        if !title.trim().is_empty() {
            args.push(format!("--window-title={}", title.trim()));
        }
    }

    if let Some(x) = options.window_x {
        args.push(format!("--window-x={}", x));
    }

    if let Some(y) = options.window_y {
        args.push(format!("--window-y={}", y));
    }

    if let Some(width) = options.window_width {
        if width == 0 {
            return Err("Window width must be greater than 0.".to_string());
        }

        args.push(format!("--window-width={}", width));
    }

    if let Some(height) = options.window_height {
        if height == 0 {
            return Err("Window height must be greater than 0.".to_string());
        }

        args.push(format!("--window-height={}", height));
    }

    if options.no_window_aspect_ratio_lock {
        args.push("--no-window-aspect-ratio-lock".to_string());
    }

    if let Some(background) = &options.background_color {
        if !background.trim().is_empty() {
            args.push(format!("--background-color={}", background.trim()));
        }
    }

    if options.borderless {
        args.push("--window-borderless".to_string());
    }

    if options.always_on_top {
        args.push("--always-on-top".to_string());
    }

    if options.fullscreen {
        args.push("--fullscreen".to_string());
    }

    if options.disable_screensaver {
        args.push("--disable-screensaver".to_string());
    }

    if let Some(render_fit) = &options.render_fit {
        if !render_fit.trim().is_empty() {
            match render_fit.trim() {
                "letterbox" | "unscaled" | "stretched" => {
                    args.push(format!("--render-fit={}", render_fit.trim()));
                }

                _ => {
                    return Err(format!(
                        "Invalid render fit '{}'. Expected letterbox, unscaled, or stretched.",
                        render_fit
                    ));
                }
            }
        }
    }

    // =========================
    // Recording
    // =========================

    if let Some(record) = &options.record {
        if !record.trim().is_empty() {
            args.push(format!("--record={}", record.trim()));
        }
    }

    if let Some(format) = &options.record_format {
        if !format.trim().is_empty() {
            args.push(format!("--record-format={}", format.trim()));
        }
    }

    if let Some(orientation) = &options.record_orientation {
        if !orientation.trim().is_empty() {
            args.push(format!("--record-orientation={}", orientation.trim()));
        }
    }

    if let Some(time_limit) = options.time_limit {
        if time_limit == 0 {
            return Err("Time limit must be greater than 0.".to_string());
        }

        args.push(format!("--time-limit={}", time_limit));
    }

    Ok(args)
}

/// Test command.
///
/// Builds the scrcpy command but does NOT launch scrcpy.
#[tauri::command]
fn build_scrcpy_command(serial: String, options: ScrcpyOptions) -> Result<Vec<String>, String> {
    build_scrcpy_args(&serial, &options)
}

#[tauri::command]
fn disconnect_device(
    app: tauri::AppHandle,
    tcp_ip: Option<String>,
    usb_serial: Option<String>,
) -> Result<String, String> {
    let mut messages = Vec::new();

    // First disconnect the TCP connection.
    if let Some(ip) = tcp_ip {
        if !ip.trim().is_empty() {
            let address = format!("{}:{}", ip.trim(), ADB_TCP_PORT);

            let output = run_adb(&app, &["disconnect", &address])?;

            if output.status.success() {
                messages.push(format!("Disconnected {}", address));
            } else {
                messages.push(format!(
                    "TCP disconnect returned an error: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
    }

    // If USB is still connected, switch the phone back to USB ADB.
    if let Some(serial) = usb_serial {
        if !serial.trim().is_empty() {
            let output = run_adb(&app, &["-s", &serial, "usb"])?;

            if output.status.success() {
                messages.push("ADB switched back to USB mode.".to_string());
            } else {
                messages.push(format!(
                    "Could not switch ADB back to USB mode: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
    }

    if messages.is_empty() {
        Ok("Nothing to disconnect.".to_string())
    } else {
        Ok(messages.join("\n"))
    }
}

/// Start scrcpy for a specific ADB device using the supplied options.
#[tauri::command]
fn start_scrcpy(
    app: tauri::AppHandle,
    serial: String,
    options: ScrcpyOptions,
) -> Result<String, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let scrcpy_dir = resource_path.join("bin").join("scrcpy");

    let scrcpy_exe = scrcpy_dir.join("scrcpy.exe");

    if !scrcpy_exe.exists() {
        return Err(format!("scrcpy.exe not found at: {}", scrcpy_exe.display()));
    }

    let args = build_scrcpy_args(&serial, &options)?;

    Command::new(&scrcpy_exe)
        .args(&args)
        .current_dir(&scrcpy_dir)
        .spawn()
        .map_err(|e| format!("Failed to start scrcpy: {}", e))?;

    Ok("scrcpy started".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_usb_devices,
            get_device_ip,
            connect_tcp,
            disconnect_tcp,
            disable_tcpip,
            build_scrcpy_command,
            disconnect_device,
            start_scrcpy
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhoneCasting");
}
