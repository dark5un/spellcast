// SPDX-License-Identifier: Apache-2.0

//! CLI entry point for Spellcast.
//!
//! Parses CLI arguments, loads configuration, initializes subsystems,
//! and runs the main event loop.

use std::path::PathBuf;

use clap::Parser;
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("spellcast=debug"),
        )
        .init();
    } else {
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("spellcast=info"),
        )
        .init();
    }

    info!("Spellcast v{} starting up", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config_path = cli.config;
    let config = spellcast::config::load_config(&config_path)?;
    info!("Configuration loaded from {:?}", config_path);

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

    // Initialize ASR engine
    #[cfg(feature = "whisper-rs")]
    let _asr_engine = {
        let model_path = shellexpand::tilde(&config.asr.model_path).to_string();
        spellcast::asr::WhisperAsr::new(&model_path, backend_type)?
    };
    #[cfg(not(feature = "whisper-rs"))]
    let _asr_engine = spellcast::asr::NoopAsr::new();

    // Run the main terminal loop
    spellcast::terminal::run_terminal_loop(&config, &mut mode_ctrl, &memory, cli.shell.as_deref())?;

    info!("Spellcast shutting down");
    Ok(())
}
