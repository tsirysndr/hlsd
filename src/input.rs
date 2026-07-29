//! PCM input sources. Each returns a blocking, byte-oriented reader that yields
//! raw interleaved s16le samples.

use std::fs::File;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{Config, InputKind};

/// Open the configured input source and return a blocking reader over the raw
/// PCM byte stream. For `unix`, this blocks until a producer connects.
pub fn open(config: &Config) -> Result<Box<dyn Read + Send>> {
    match config.input.source {
        InputKind::Stdin => {
            log::info!("reading PCM from stdin");
            Ok(Box::new(std::io::stdin()))
        }
        InputKind::Fifo => {
            let path = config
                .input
                .path
                .as_ref()
                .expect("validated: fifo requires a path");
            log::info!("reading PCM from FIFO {}", path.display());
            let file = File::open(path)
                .with_context(|| format!("opening FIFO {} (create it with `mkfifo`)", path.display()))?;
            Ok(Box::new(file))
        }
        InputKind::Unix => {
            let path = config
                .input
                .path
                .as_ref()
                .expect("validated: unix requires a path");
            open_unix(path)
        }
    }
}

fn open_unix(path: &Path) -> Result<Box<dyn Read + Send>> {
    // A stale socket file would make bind() fail with EADDRINUSE.
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("removing stale socket {}", path.display()))?;
    }
    let listener = UnixListener::bind(path)
        .with_context(|| format!("binding Unix socket {}", path.display()))?;
    log::info!("waiting for a PCM producer on Unix socket {}", path.display());
    let (stream, _addr) = listener
        .accept()
        .with_context(|| format!("accepting connection on {}", path.display()))?;
    log::info!("PCM producer connected");
    Ok(Box::new(stream))
}
