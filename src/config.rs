//! Configuration model, TOML loading, and CLI-override merging.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::Cli;

/// Where the raw PCM audio is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum InputKind {
    /// Read PCM from standard input.
    Stdin,
    /// Read PCM from a named pipe (FIFO) at `input.path`.
    Fifo,
    /// Read PCM from a Unix domain socket at `input.path` (hlsd binds & accepts).
    Unix,
}

impl Default for InputKind {
    fn default() -> Self {
        InputKind::Stdin
    }
}

/// Audio codec used to encode the segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum CodecKind {
    /// FLAC — lossless, pure Rust, always available.
    Flac,
    /// AAC-LC — requires building with `--features aac`.
    Aac,
    /// MP3 — requires building with `--features mp3`.
    Mp3,
    /// Opus — requires building with `--features opus` (48 kHz input only).
    Opus,
}

impl Default for CodecKind {
    fn default() -> Self {
        CodecKind::Flac
    }
}

impl std::fmt::Display for CodecKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CodecKind::Flac => "flac",
            CodecKind::Aac => "aac",
            CodecKind::Mp3 => "mp3",
            CodecKind::Opus => "opus",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputConfig {
    /// Source kind: stdin, fifo, or unix.
    pub source: InputKind,
    /// Filesystem path for `fifo` / `unix` sources (ignored for stdin).
    pub path: Option<PathBuf>,
    /// PCM sample rate in Hz.
    pub sample_rate: u32,
    /// Number of interleaved channels.
    pub channels: u16,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            source: InputKind::Stdin,
            path: None,
            sample_rate: 44_100,
            channels: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    /// Directory where playlists and segments are written and served from.
    pub dir: PathBuf,
    /// Target segment duration in seconds.
    pub segment_duration: u32,
    /// Number of segments kept in the live playlist / window.
    pub playlist_size: u32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("./stream"),
            segment_duration: 4,
            playlist_size: 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EncoderConfig {
    /// Codec to encode with.
    pub codec: CodecKind,
    /// Target bitrate for lossy codecs (e.g. `128k`, `96000`). Ignored by FLAC.
    pub bitrate: String,
    /// FLAC compression level 0..=8 (higher = smaller, slower). FLAC only.
    pub flac_compression: u8,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            codec: CodecKind::Flac,
            bitrate: "128k".to_string(),
            flac_compression: 5,
        }
    }
}

impl EncoderConfig {
    /// Parse the bitrate string into bits-per-second. Accepts a plain integer or
    /// a `k`/`m` suffix (e.g. `128k` -> 128000, `1M` -> 1_000_000).
    pub fn bitrate_bps(&self) -> Result<u32> {
        let raw = self.bitrate.trim().to_lowercase();
        let (num, mult) = if let Some(n) = raw.strip_suffix('k') {
            (n, 1_000)
        } else if let Some(n) = raw.strip_suffix('m') {
            (n, 1_000_000)
        } else {
            (raw.as_str(), 1)
        };
        let value: f64 = num
            .trim()
            .parse()
            .with_context(|| format!("invalid bitrate {:?}", self.bitrate))?;
        Ok((value * mult as f64).round() as u32)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HlsConfig {
    /// Emit an HLS playlist + segments.
    pub enabled: bool,
}

impl Default for HlsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashConfig {
    /// Emit an MPEG-DASH manifest + segments.
    pub enabled: bool,
}

impl Default for DashConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Bind address for the HTTP server.
    pub host: String,
    /// Bind port for the HTTP server.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub input: InputConfig,
    pub output: OutputConfig,
    pub encoder: EncoderConfig,
    pub hls: HlsConfig,
    pub dash: DashConfig,
    pub server: ServerConfig,
}

impl Config {
    /// Load config from a TOML file, or return defaults if `path` is `None`.
    pub fn load(path: Option<&std::path::Path>) -> Result<Self> {
        match path {
            Some(p) => {
                let text = std::fs::read_to_string(p)
                    .with_context(|| format!("reading config file {}", p.display()))?;
                let cfg: Config = toml::from_str(&text)
                    .with_context(|| format!("parsing config file {}", p.display()))?;
                Ok(cfg)
            }
            None => Ok(Config::default()),
        }
    }

    /// Apply CLI overrides on top of a loaded/default config. CLI wins when set.
    pub fn apply_cli(&mut self, cli: &Cli) {
        if let Some(v) = cli.input {
            self.input.source = v;
        }
        if let Some(v) = cli.input_path.clone() {
            self.input.path = Some(v);
        }
        if let Some(v) = cli.sample_rate {
            self.input.sample_rate = v;
        }
        if let Some(v) = cli.channels {
            self.input.channels = v;
        }
        if let Some(v) = cli.codec {
            self.encoder.codec = v;
        }
        if let Some(v) = cli.bitrate.clone() {
            self.encoder.bitrate = v;
        }
        if let Some(v) = cli.out_dir.clone() {
            self.output.dir = v;
        }
        if let Some(v) = cli.segment_duration {
            self.output.segment_duration = v;
        }
        if let Some(v) = cli.playlist_size {
            self.output.playlist_size = v;
        }
        if cli.dash {
            self.dash.enabled = true;
        }
        if cli.no_dash {
            self.dash.enabled = false;
        }
        if cli.no_hls {
            self.hls.enabled = false;
        }
        if let Some(v) = cli.host.clone() {
            self.server.host = v;
        }
        if let Some(v) = cli.port {
            self.server.port = v;
        }
    }

    /// Validate the resolved config, returning a helpful error for bad combos.
    pub fn validate(&self) -> Result<()> {
        if !self.hls.enabled && !self.dash.enabled {
            anyhow::bail!("both HLS and DASH are disabled — nothing to serve");
        }
        if matches!(self.input.source, InputKind::Fifo | InputKind::Unix)
            && self.input.path.is_none()
        {
            anyhow::bail!(
                "input.source = {} requires a path (set --input-path or [input].path)",
                match self.input.source {
                    InputKind::Fifo => "fifo",
                    InputKind::Unix => "unix",
                    InputKind::Stdin => "stdin",
                }
            );
        }
        if self.input.channels == 0 {
            anyhow::bail!("input.channels must be at least 1");
        }
        if self.input.sample_rate == 0 {
            anyhow::bail!("input.sample_rate must be greater than 0");
        }
        if self.output.segment_duration == 0 {
            anyhow::bail!("output.segment_duration must be greater than 0");
        }
        if self.encoder.flac_compression > 8 {
            anyhow::bail!("encoder.flac_compression must be between 0 and 8");
        }
        Ok(())
    }
}
