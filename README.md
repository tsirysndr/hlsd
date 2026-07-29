# hlsd

A small, self-contained daemon that takes a raw **PCM s16le** audio stream and
serves it live as **HLS** — and optionally **MPEG-DASH** — over HTTP.

Audio is read from **stdin**, a **FIFO**, or a **Unix socket**, encoded, packaged
into fragmented-MP4 (CMAF) segments, and published behind an
[actix-web](https://actix.rs) server with a rolling live window.

The default build is **pure Rust with no system dependencies** (FLAC via
[`flacenc`](https://crates.io/crates/flacenc)). AAC, MP3, and Opus are available
as opt-in Cargo features that compile *vendored* C sources — they need a C
compiler at build time but **no pre-installed system library**. Works on Linux
and macOS.

## Features

- Input from **stdin**, **FIFO**, or **Unix domain socket**
- **HLS** (fMP4, `#EXT-X-VERSION:7`) and optional **MPEG-DASH** from the same segments
- Codecs: **FLAC** (default, pure Rust), **AAC-LC**, **MP3**, **Opus** (feature-gated)
- Configurable via **TOML** and/or **CLI flags** (CLI overrides the file)
- Live sliding-window playlist with automatic old-segment eviction
- Correct MIME types, permissive CORS, atomic manifest writes

## Install / Build

```sh
# Default: pure Rust, FLAC only, no system deps
cargo build --release

# With extra codecs (compiles vendored C — needs a C compiler, no system libs)
cargo build --release --features aac
cargo build --release --features mp3
cargo build --release --features opus
cargo build --release --features all-codecs
```

The binary is `target/release/hlsd`.

## Quick start

Pipe any PCM s16le / 44.1 kHz / stereo source into hlsd over stdin:

```sh
# Example producer (any tool that emits raw PCM works):
some-audio-source | ./target/release/hlsd --out-dir ./stream --dash
```

Then open:

- HLS media playlist: `http://127.0.0.1:8080/stream.m3u8`
- HLS multivariant:   `http://127.0.0.1:8080/master.m3u8`
- MPEG-DASH manifest: `http://127.0.0.1:8080/stream.mpd` (with `--dash`)
- Landing page:       `http://127.0.0.1:8080/`

### Feeding hlsd with ffmpeg

hlsd doesn't use ffmpeg internally, but ffmpeg is a handy way to *produce* the
raw PCM. Emit `s16le`, `44100` Hz, `2` channels to stdout (`pipe:1`) and pipe it
into hlsd's stdin — the two ffmpeg output flags must match hlsd's
`--sample-rate` / `--channels`:

```sh
# Stream a file (looped, in real time) as live HLS + DASH
ffmpeg -re -stream_loop -1 -i input.mp3 \
    -f s16le -ar 44100 -ac 2 pipe:1 \
  | ./target/release/hlsd --out-dir ./stream --dash
```

```sh
# Capture a live source (e.g. an internet radio) and republish it
ffmpeg -re -i https://example.com/stream.aac \
    -f s16le -ar 44100 -ac 2 pipe:1 \
  | ./target/release/hlsd --out-dir ./stream
```

```sh
# Generate a 440 Hz test tone — useful for a quick smoke check
ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=44100" \
    -ac 2 -f s16le pipe:1 \
  | ./target/release/hlsd --out-dir ./stream
```

For **Opus**, produce 48 kHz PCM and tell hlsd to match:

```sh
ffmpeg -re -i input.wav -f s16le -ar 48000 -ac 2 pipe:1 \
  | ./target/release/hlsd --codec opus --sample-rate 48000 --out-dir ./stream
```

> `-re` makes ffmpeg read/emit at real-time (wall-clock) speed, which is what
> you want for live output. Drop it to transcode as fast as possible.

### Playing the stream

`ffplay` is a *player*, so it consumes hlsd's HTTP output — it does not feed
hlsd's stdin (use `ffmpeg` for that). Point any HLS/DASH player at the URL:

```sh
ffplay -autoexit http://127.0.0.1:8080/stream.m3u8   # HLS
ffplay http://127.0.0.1:8080/stream.mpd              # DASH
```

Other options: `mpv http://127.0.0.1:8080/stream.m3u8`, VLC, Safari (native
HLS), or the built-in landing page at `http://127.0.0.1:8080/`.

### Input from a FIFO

```sh
mkfifo /tmp/pcm.fifo
./target/release/hlsd --input fifo --input-path /tmp/pcm.fifo &
some-audio-source > /tmp/pcm.fifo
```

### Input from a Unix socket

hlsd binds the socket and waits for a producer to connect:

```sh
./target/release/hlsd --input unix --input-path /tmp/pcm.sock &
some-audio-source | nc -U /tmp/pcm.sock
```

## Configuration

Load a TOML file with `--config`, override individual values with flags:

```sh
./target/release/hlsd --config hlsd.toml --port 9000 --codec flac
```

### Full example config (all options)

Every key below is optional and shows its default value.

```toml
[input]
# Where the raw PCM s16le stream comes from: "stdin" | "fifo" | "unix".
source = "stdin"
# Filesystem path for the "fifo" or "unix" source (ignored for stdin).
# path = "/tmp/hlsd.sock"
sample_rate = 44100
channels = 2

[encoder]
# Codec: "flac" (pure Rust, always available) | "aac" | "mp3" | "opus".
# aac/mp3/opus require building with the matching Cargo feature.
codec = "flac"
# Target bitrate for lossy codecs (accepts 96000, "128k", "1M"). Ignored by FLAC.
bitrate = "128k"
# FLAC compression level 0..=8 (higher = smaller & slower). FLAC only.
flac_compression = 5

[output]
# Directory where manifests and segments are written and served from.
dir = "./stream"
# Target segment length in seconds.
segment_duration = 4
# Number of segments kept in the live window (older ones are deleted).
playlist_size = 6

[hls]
# Emit an HLS playlist (stream.m3u8 + master.m3u8).
enabled = true

[dash]
# Emit an MPEG-DASH manifest (stream.mpd).
enabled = false

[server]
host = "127.0.0.1"
port = 8080
```

### CLI flags

| Flag | Config key | Description |
|------|------------|-------------|
| `-c, --config <FILE>` | — | TOML config file |
| `--input <stdin\|fifo\|unix>` | `input.source` | Input source |
| `--input-path <PATH>` | `input.path` | FIFO / socket path |
| `--sample-rate <HZ>` | `input.sample_rate` | PCM sample rate |
| `--channels <N>` | `input.channels` | PCM channels |
| `--codec <flac\|aac\|mp3\|opus>` | `encoder.codec` | Codec |
| `--bitrate <RATE>` | `encoder.bitrate` | Lossy bitrate (`128k`, `96000`, `1M`) |
| `--out-dir <DIR>` | `output.dir` | Output/serve directory |
| `--segment-duration <SECS>` | `output.segment_duration` | Segment length |
| `--playlist-size <N>` | `output.playlist_size` | Live window size |
| `--dash` / `--no-dash` | `dash.enabled` | Toggle DASH output |
| `--no-hls` | `hls.enabled` | Disable HLS output |
| `--host <HOST>` | `server.host` | Bind host |
| `-p, --port <PORT>` | `server.port` | Bind port |

## Codec support & compatibility

All codecs are packaged in fragmented MP4 (CMAF), shared by HLS and DASH.

| Codec | Build | System deps | Notes |
|-------|-------|-------------|-------|
| **FLAC** | default | none (pure Rust) | Lossless; plays in Safari and MSE browsers |
| **AAC-LC** | `--features aac` | none (vendored C) | Best broad player support |
| **MP3** | `--features mp3` | none (vendored C) | `mp4a.6B`; MSE support varies |
| **Opus** | `--features opus` | none (vendored C) | **48 kHz input only**; MSE support varies |

For Opus, feed 48 kHz PCM (`--sample-rate 48000`); other rates are rejected.

## How it works

```
PCM (stdin/fifo/unix) ──▶ encoder ──▶ CMAF fMP4 muxer ──▶ segment files + manifests
                                                                      │
                                                          actix-web HTTP server
```

1. **Input** reads blocking s16le bytes and frames them into i16 samples.
2. **Encoder** turns samples into codec frames (each independently muxable).
3. **Muxer** writes one immutable `init.mp4` and a `moof`+`mdat` media segment
   per fragment.
4. **Segmenter** maintains the sliding window, evicting and deleting old
   segments, and rewrites `stream.m3u8` / `master.m3u8` / `stream.mpd`
   atomically.
5. **Server** serves everything with correct content types and CORS.

## Development

```sh
cargo test --release                    # unit + end-to-end smoke test (FLAC, no ffmpeg)
cargo build --release --features all-codecs
```

The smoke test synthesizes PCM, runs the binary over stdin, and asserts a valid
playlist and fMP4 segments are produced.

## License

MIT — see [LICENSE](LICENSE).
