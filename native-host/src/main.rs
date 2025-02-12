use std::{
    env,
    io::{self, Read, Write},
    process::Command,
    fs::OpenOptions,
};
use anyhow::{Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use log::{error, info, debug};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "type")]
    message_type: String,
    data: serde_json::Value,
}

fn setup_logging() -> Result<()> {
    // Set up file logging
    let log_path = env::current_dir()?.join("native_host.log");
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    // Configure env_logger to write to both stderr and file
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}

fn read_message<R: Read>(mut input: R) -> Result<Option<Message>> {
    info!("Attempting to read message...");
    
    // Try to read first byte to check if stdin is closed
    let mut first_byte = [0u8; 1];
    match input.read_exact(&mut first_byte) {
        Ok(_) => {
            debug!("Successfully read first byte: {}", first_byte[0]);
        }
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            info!("Stdin closed (EOF on first byte)");
            return Ok(None);
        }
        Err(e) => {
            error!("Error reading first byte: {}", e);
            return Err(e.into());
        }
    }

    // Read remaining 3 bytes of length
    let mut length_bytes = [0u8; 3];
    match input.read_exact(&mut length_bytes) {
        Ok(_) => {
            debug!("Successfully read remaining length bytes");
        }
        Err(e) => {
            error!("Error reading remaining length bytes: {}", e);
            return Err(e.into());
        }
    }

    // Combine all 4 bytes and convert to u32
    let length_buf = [first_byte[0], length_bytes[0], length_bytes[1], length_bytes[2]];
    let length = u32::from_le_bytes(length_buf);
    info!("Message length: {} bytes", length);

    // Read the message content
    let mut buffer = vec![0; length as usize];
    match input.read_exact(&mut buffer) {
        Ok(_) => {
            debug!("Successfully read message content");
        }
        Err(e) => {
            error!("Error reading message content: {}", e);
            return Err(e.into());
        }
    }

    // Try to parse as UTF-8 first for logging
    match String::from_utf8(buffer.clone()) {
        Ok(content) => {
            info!("Raw message content: {}", content);
        }
        Err(_) => {
            info!("Message content is not valid UTF-8");
        }
    }

    // Parse JSON message
    match serde_json::from_slice(&buffer) {
        Ok(message) => {
            info!("Successfully parsed message: {:?}", message);
            Ok(Some(message))
        }
        Err(e) => {
            error!("Failed to parse message as JSON: {}", e);
            Err(e.into())
        }
    }
}

fn write_message<W: Write>(mut output: W, message: &Message) -> Result<()> {
    debug!("Writing message: {:?}", message);
    
    // Serialize message to JSON
    let content = serde_json::to_vec(message)
        .context("Failed to serialize message to JSON")?;
    
    debug!("Message serialized, length: {}", content.len());
    
    // Write message length (little-endian)
    output.write_u32::<LittleEndian>(content.len() as u32)
        .context("Failed to write message length")?;
    
    // Write message content
    output.write_all(&content)
        .context("Failed to write message content")?;
    output.flush()
        .context("Failed to flush output")?;
    
    debug!("Message successfully written");
    Ok(())
}

fn check_hovered_group() -> Result<u32> {
    // Get path of current executable
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to get executable directory"))?;
    
    // Go up to project root: native-host/target/release -> native-host/target -> native-host -> root
    let project_root = exe_dir
        .parent().ok_or_else(|| anyhow::anyhow!("Failed to get parent of release dir"))?
        .parent().ok_or_else(|| anyhow::anyhow!("Failed to get parent of target dir"))?
        .parent().ok_or_else(|| anyhow::anyhow!("Failed to get parent of native-host dir"))?;
    
    // Find hover detector relative to project root
    let detector_path = project_root
        .join("hover-detector")
        .join("target")
        .join("release")
        .join("hover-detector.exe");
    
    let detector_path = detector_path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid path to hover detector"))?;
    
    info!("Running hover detector: {}", detector_path);
    
    // Run hover detector and capture output
    let output = Command::new(detector_path)
        .output()
        .with_context(|| format!("Failed to execute hover detector at {}", detector_path))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        error!("Hover detector failed: {}", error);
        anyhow::bail!("Hover detector failed: {}", error);
    }

    // Convert output to string and parse as number
    let index_str = String::from_utf8_lossy(&output.stdout);
    debug!("Hover detector output: {}", index_str);
    
    let index = index_str.trim().parse::<u32>()
        .context("Failed to parse hover detector output as number")?;
    
    info!("Hover detector returned index: {}", index);
    Ok(index)
}

fn main() -> Result<()> {
    // Set up logging before anything else
    setup_logging()?;
    
    info!("Native messaging host started");
    info!("Process ID: {}", std::process::id());
    info!("Current directory: {:?}", env::current_dir()?);
    
    // Log all environment variables for debugging
    info!("Environment variables:");
    for (key, value) in env::vars() {
        info!("{}: {}", key, value);
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    info!("Starting message processing loop");

    // Process messages from the extension
    while let Some(message) = read_message(&mut reader)? {
        info!("Processing message: {:?}", message);

        match message.message_type.as_str() {
            "check_hover" => {
                info!("Processing check_hover request");
                match check_hovered_group() {
                    Ok(index) => {
                        info!("Hover check successful, index: {}", index);
                        let response = Message {
                            message_type: "hover_result".to_string(),
                            data: serde_json::json!({ 
                                "index": index 
                            }),
                        };
                        write_message(&mut writer, &response)?;
                    }
                    Err(e) => {
                        error!("Error checking hover: {}", e);
                        let response = Message {
                            message_type: "error".to_string(),
                            data: serde_json::json!({ 
                                "message": format!("Failed to check hover: {}", e)
                            }),
                        };
                        write_message(&mut writer, &response)?;
                    }
                }
            }
            _ => {
                error!("Unknown message type: {}", message.message_type);
                let response = Message {
                    message_type: "error".to_string(),
                    data: serde_json::json!({ 
                        "message": format!("Unknown message type: {}", message.message_type)
                    }),
                };
                write_message(&mut writer, &response)?;
            }
        }
    }

    info!("Native messaging host shutting down");
    Ok(())
}
