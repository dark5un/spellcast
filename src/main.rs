// SPDX-License-Identifier: Apache-2.0

//! CLI entry point for Spellcast.
//!
//! Parses CLI arguments, loads configuration, initializes subsystems,
//! and runs the main event loop.

use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait};
use log::info;

/// Spellcast — Dictation-first terminal keyboard multiplexer.
#[derive(Parser, Debug)]
#[command(name = "spellcast", version, about)]
struct Cli {
    /// Path to configuration file
    #[arg(
        short = 'c',
        long = "config",
        default_value = "~/.config/spellcast/config.toml"
    )]
    config: PathBuf,

    /// Compute backend override
    #[arg(short = 'b', long = "backend")]
    backend: Option<String>,

    /// Shell to spawn (default: $SHELL or /bin/bash)
    #[arg(short = 's', long = "shell")]
    shell: Option<String>,

    /// Enable verbose logging
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// List audio devices and check microphone
    #[arg(long = "check-audio")]
    check_audio: bool,

    /// Set audio input device name (saves to config)
    #[arg(long = "set-input-device")]
    set_input_device: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging — write to file, NOT stderr, to avoid
    // corrupting the alternate screen terminal display
    let log_path = shellexpand::tilde("~/.config/spellcast/spellcast.log").to_string();
    if let Some(parent) = std::path::Path::new(&log_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Failed to open log file");
    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    log::info!("Spellcast v{} starting up", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config_path = cli.config.clone();
    let mut config = spellcast::config::load_config(&config_path)?;
    info!("Configuration loaded from {:?}", config_path);
    log::info!("Audio device: {}", config.audio.device);
    log::info!("Backend: {:?}", config.backend.backend_type);
    log::info!("Shell: {:?}", cli.shell.as_deref().unwrap_or("$SHELL"));
    log::info!("Log file: ~/.config/spellcast/spellcast.log");

    // Handle --check-audio: list input devices and exit
    if cli.check_audio {
        let host = cpal::default_host();
        println!("Audio host: {}\n", host.id().name());

        println!("Input devices:");
        let mut seen: HashSet<String> = HashSet::new();
        let mut idx = 0;
        let default_name = host
            .default_input_device()
            .and_then(|d| d.description().ok())
            .map(|d| d.name().to_string());
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(name) = device.description().map(|d| d.name().to_string())
                    && seen.insert(name.clone())
                {
                    idx += 1;
                    let marker = if Some(name.as_str()) == default_name.as_deref() {
                        " * (default)"
                    } else {
                        ""
                    };
                    println!("  {:3}. {}{}", idx, name, marker);
                }
            }
        }
        return Ok(());
    }

    // Handle --set-input-device: write device name to config and use it
    if let Some(ref dev_name) = cli.set_input_device {
        config.audio.device = dev_name.clone();
        let config_path = shellexpand::tilde(
            cli.config
                .to_str()
                .unwrap_or("~/.config/spellcast/config.toml"),
        )
        .to_string();
        let toml_str = toml::to_string_pretty(&config).expect("Config should serialize");
        if let Some(parent) = std::path::Path::new(&config_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&config_path, &toml_str).ok();
        info!("Audio device set to \"{}\" in {}", dev_name, config_path);
    }

    // Create mode controller
    let mut mode_ctrl = spellcast::ModeController::new();
    info!("Mode controller initialized");

    // Initialize memory database
    let db_path = shellexpand::tilde(&config.database.path).to_string();
    let memory = spellcast::memory::MemoryStore::open(&db_path)?;
    info!("Memory store opened at {}", db_path);

    // Detect and initialize compute backend
    let backend_type_str = config.backend.backend_type.to_string();
    let backend_type = cli.backend.as_deref().unwrap_or(&backend_type_str);
    let _compute_backend = spellcast::backend::detect_backend(backend_type)?;
    info!("Compute backend: {}", backend_type);

    // Initialize audio capture
    let _audio_config = spellcast::audio::AudioConfig {
        sample_rate: config.audio.sample_rate,
        channels: config.audio.channels,
        device: config.audio.device.clone(),
    };

    // Initialize ASR engine — suppress whisper.cpp stdout output
    // so it doesn't corrupt the terminal before alternate screen is entered
    #[cfg(feature = "cpu")]
    let _asr_engine = {
        use std::os::fd::AsRawFd;
        // Save original stdout and stderr fds
        let saved_stdout = unsafe { libc::dup(1) };
        let saved_stderr = unsafe { libc::dup(2) };
        let dev_null = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("Failed to open /dev/null");
        unsafe {
            libc::dup2(dev_null.as_raw_fd(), 1);
            libc::dup2(dev_null.as_raw_fd(), 2);
        }

        let model_path = shellexpand::tilde(&config.asr.model_path).to_string();
        log::info!("Loading ASR model from {}", model_path);
        let asr = spellcast::asr::WhisperAsr::new(&model_path, backend_type)?;
        log::info!("ASR model loaded successfully");

        // Restore original stdout and stderr
        unsafe {
            libc::dup2(saved_stdout, 1);
            libc::dup2(saved_stderr, 2);
            libc::close(saved_stdout);
            libc::close(saved_stderr);
        }
        drop(dev_null);

        asr
    };
    #[cfg(not(feature = "cpu"))]
    let _asr_engine = spellcast::asr::NoopAsr::new();

    // Run the main terminal loop
    spellcast::terminal::run_terminal_loop(&config, &mut mode_ctrl, &memory, cli.shell.as_deref())?;

    info!("Spellcast shutting down");
    Ok(())
}
