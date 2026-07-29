//! End-to-end smoke test: feed synthetic PCM into the built binary over stdin
//! and assert it produces a valid HLS playlist, an fMP4 init segment, and media
//! segments. Uses the default (pure-Rust FLAC) build — no ffmpeg required.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Generate `secs` seconds of interleaved s16le stereo sine at 44.1 kHz.
fn make_pcm(secs: u32) -> Vec<u8> {
    let sample_rate = 44_100u32;
    let channels = 2u32;
    let total = sample_rate * secs;
    let mut out = Vec::with_capacity((total * channels * 2) as usize);
    for n in 0..total {
        let t = n as f64 / sample_rate as f64;
        let v = (2.0 * std::f64::consts::PI * 440.0 * t).sin();
        let s = (v * 16_000.0) as i16;
        for _ in 0..channels {
            out.extend_from_slice(&s.to_le_bytes());
        }
    }
    out
}

fn wait_for(path: &std::path::Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn produces_hls_and_fmp4() {
    let dir = std::env::temp_dir().join(format!("hlsd-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_hlsd"))
        .args([
            "--input",
            "stdin",
            "--out-dir",
            dir.to_str().unwrap(),
            "--segment-duration",
            "1",
            "--port",
            "0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hlsd");

    // Feed 3 s of PCM, then close stdin so the segmenter sees EOF.
    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&make_pcm(3)).unwrap();
    }

    let m3u8 = dir.join("stream.m3u8");
    let init = dir.join("init.mp4");
    let seg = dir.join("seg-00001.m4s");

    let ok = wait_for(&m3u8, Duration::from_secs(15))
        && wait_for(&init, Duration::from_secs(5))
        && wait_for(&seg, Duration::from_secs(5));

    // Capture assertions before killing the still-running HTTP server.
    let playlist = std::fs::read_to_string(&m3u8).unwrap_or_default();
    let init_bytes = std::fs::read(&init).unwrap_or_default();
    let seg_bytes = std::fs::read(&seg).unwrap_or_default();

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(ok, "expected playlist, init and segment files to appear");
    assert!(playlist.contains("#EXTM3U"), "playlist header missing");
    assert!(playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""), "EXT-X-MAP missing");
    assert!(playlist.contains(".m4s"), "no media segment referenced");

    // init.mp4 must start with an `ftyp` box (bytes 4..8).
    assert!(init_bytes.len() > 8 && &init_bytes[4..8] == b"ftyp", "init is not fMP4");
    // Media segment must contain `moof` and `mdat`.
    assert!(contains(&seg_bytes, b"moof"), "segment missing moof");
    assert!(contains(&seg_bytes, b"mdat"), "segment missing mdat");
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
