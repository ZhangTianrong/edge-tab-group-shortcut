use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use log::{debug, error, info, warn, LevelFilter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "type")]
    message_type: String,
    data: serde_json::Value,
}

fn debug_enabled() -> bool {
    env::var("TABGROUP_NATIVE_HOST_DEBUG")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && normalized != "0" && normalized != "false"
        })
        .unwrap_or(false)
}

fn app_root_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("."))
        })
        .join("TabGroupShortcut")
}

fn setup_logging() -> Result<()> {
    if !debug_enabled() {
        return Ok(());
    }

    let log_dir = app_root_dir().join("logs");
    fs::create_dir_all(&log_dir).context("Failed to create the native-host log directory")?;

    let log_path = log_dir.join("native-host.log");
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .context("Failed to open native-host.log")?;

    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp_secs()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}

fn read_message<R: Read>(input: &mut R) -> Result<Option<Message>> {
    let mut first_byte = [0u8; 1];
    match input.read_exact(&mut first_byte) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(None);
        }
        Err(error) => {
            return Err(error).context("Failed to read the first message-length byte");
        }
    }

    let mut remaining_length_bytes = [0u8; 3];
    input
        .read_exact(&mut remaining_length_bytes)
        .context("Failed to read the remaining message-length bytes")?;

    let length = u32::from_le_bytes([
        first_byte[0],
        remaining_length_bytes[0],
        remaining_length_bytes[1],
        remaining_length_bytes[2],
    ]) as usize;

    let mut buffer = vec![0; length];
    input
        .read_exact(&mut buffer)
        .context("Failed to read the native messaging payload")?;

    let message = serde_json::from_slice(&buffer)
        .context("Failed to parse the native messaging payload as JSON")?;

    Ok(Some(message))
}

fn write_message<W: Write>(output: &mut W, message: &Message) -> Result<()> {
    let content = serde_json::to_vec(message).context("Failed to serialize the response")?;
    output
        .write_u32::<LittleEndian>(content.len() as u32)
        .context("Failed to write the response length")?;
    output
        .write_all(&content)
        .context("Failed to write the response body")?;
    output.flush().context("Failed to flush the response")?;
    Ok(())
}

fn resolve_detector_path() -> Result<PathBuf> {
    let exe_path = env::current_exe().context("Failed to locate native-host.exe")?;
    let sibling_detector_path = exe_path.with_file_name("hover-detector.exe");
    if sibling_detector_path.exists() {
        return Ok(sibling_detector_path);
    }

    let legacy_detector_path = exe_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|project_root| {
            project_root
                .join("hover-detector")
                .join("target")
                .join("release")
                .join("hover-detector.exe")
        });

    if let Some(path) = legacy_detector_path {
        if path.exists() {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "hover-detector.exe was not found. Re-run install.ps1 to install the Windows companion."
    )
}

fn check_hovered_group() -> Result<u32> {
    let detector_path = resolve_detector_path()?;
    debug!("Running hover detector at {}", detector_path.display());

    let output = Command::new(&detector_path)
        .output()
        .with_context(|| format!("Failed to execute {}", detector_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            anyhow::bail!("Hover detector exited with status {}", output.status);
        }

        anyhow::bail!(stderr);
    }

    let index = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .context("Hover detector returned an invalid group index")?;

    Ok(index)
}

fn main() -> Result<()> {
    setup_logging()?;
    info!("Native messaging host started.");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    while let Some(message) = read_message(&mut reader)? {
        debug!("Received message type {}", message.message_type);

        let response = match message.message_type.as_str() {
            "check_hover" => match check_hovered_group() {
                Ok(index) => Message {
                    message_type: "hover_result".to_string(),
                    data: serde_json::json!({ "index": index }),
                },
                Err(error) => {
                    warn!("Hover check failed: {error}");
                    Message {
                        message_type: "error".to_string(),
                        data: serde_json::json!({
                            "message": format!("Failed to check hover: {error}")
                        }),
                    }
                }
            },
            unknown_type => {
                error!("Unknown message type: {}", unknown_type);
                Message {
                    message_type: "error".to_string(),
                    data: serde_json::json!({
                        "message": format!("Unknown message type: {unknown_type}")
                    }),
                }
            }
        };

        write_message(&mut writer, &response)?;
    }

    info!("Native messaging host shutting down.");
    Ok(())
}
