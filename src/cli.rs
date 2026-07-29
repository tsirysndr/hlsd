//! Command-line interface. Every flag is optional; anything set here overrides
//! the corresponding value from the TOML config file (or the built-in default).

use std::path::PathBuf;

use clap::Parser;

use crate::config::{CodecKind, InputKind};

#[derive(Parser, Debug)]
#[command(
    name = "hlsd",
    version,
    about = "Serve live HLS (and optional MPEG-DASH) from a raw PCM s16le stream",
    long_about = None,
)]
pub struct Cli {
    /// Path to a TOML config file. Missing keys fall back to defaults.
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Input source for the PCM stream.
    #[arg(long, value_enum, value_name = "KIND")]
    pub input: Option<InputKind>,

    /// Path to the FIFO or Unix socket (for --input fifo|unix).
    #[arg(long, value_name = "PATH")]
    pub input_path: Option<PathBuf>,

    /// PCM sample rate in Hz.
    #[arg(long, value_name = "HZ")]
    pub sample_rate: Option<u32>,

    /// Number of interleaved PCM channels.
    #[arg(long, value_name = "N")]
    pub channels: Option<u16>,

    /// Audio codec used for the segments.
    #[arg(long, value_enum, value_name = "CODEC")]
    pub codec: Option<CodecKind>,

    /// Target bitrate for lossy codecs, e.g. 128k or 96000 (ignored by FLAC).
    #[arg(long, value_name = "RATE")]
    pub bitrate: Option<String>,

    /// Output directory for playlists and segments.
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,

    /// Target segment duration in seconds.
    #[arg(long, value_name = "SECS")]
    pub segment_duration: Option<u32>,

    /// Number of segments kept in the live window.
    #[arg(long, value_name = "N")]
    pub playlist_size: Option<u32>,

    /// Enable MPEG-DASH output in addition to HLS.
    #[arg(long)]
    pub dash: bool,

    /// Disable MPEG-DASH output (overrides config).
    #[arg(long, conflicts_with = "dash")]
    pub no_dash: bool,

    /// Disable HLS output (overrides config).
    #[arg(long)]
    pub no_hls: bool,

    /// HTTP server bind host.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// HTTP server bind port.
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,
}
