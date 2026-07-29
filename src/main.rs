//! hlsd — serve live HLS (and optional MPEG-DASH) from a raw PCM s16le stream.

mod boxes;
mod cli;
mod codec;
mod config;
mod input;
mod mux;
mod playlist;
mod segmenter;
mod server;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;
use crate::config::Config;

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    let mut config = Config::load(cli.config.as_deref())?;
    config.apply_cli(&cli);
    config.validate()?;

    banner(&config);

    // The segmenter reads the (blocking) PCM input and writes segments/manifests.
    // Run it on a dedicated thread so the async HTTP server owns the main runtime.
    let seg_config = config.clone();
    std::thread::spawn(move || {
        let reader = match input::open(&seg_config) {
            Ok(r) => r,
            Err(e) => {
                log::error!("input error: {e:#}");
                std::process::exit(1);
            }
        };
        if let Err(e) = segmenter::run(&seg_config, reader) {
            log::error!("segmenter error: {e:#}");
            std::process::exit(1);
        }
        log::info!("segmenter finished; server still running (Ctrl-C to stop)");
    });

    server::serve(config).await?;
    Ok(())
}

fn banner(config: &Config) {
    let base = format!("http://{}:{}", config.server.host, config.server.port);
    log::info!("hlsd serving from {} on {}", config.output.dir.display(), base);
    if config.hls.enabled {
        log::info!("  HLS : {base}/stream.m3u8  (master: {base}/master.m3u8)");
    }
    if config.dash.enabled {
        log::info!("  DASH: {base}/stream.mpd");
    }
    log::info!("  codec: {}", config.encoder.codec);
}
