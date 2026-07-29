Name:           hlsd
Version:        0.1.0
Release:        1%{?dist}
Summary:        Serve live HLS (and optional MPEG-DASH) from a raw PCM audio stream

License:        MIT
URL:            https://github.com/tsirysndr/hlsd

BuildArch:      x86_64

Requires: glibc

%description
hlsd is a pure-Rust server that reads a raw PCM s16le audio stream on stdin and
serves it as live HLS (and optionally MPEG-DASH), segmenting and muxing on the
fly with no external tools required. Optional lossy codecs (AAC, MP3, Opus) can
be enabled at build time.

%prep
# Nothing to prep — the binary is prebuilt.

%build
# Nothing to build — the binary is prebuilt.

%install
mkdir -p %{buildroot}/usr/local/bin
cp -r %{_sourcedir}/amd64/usr %{buildroot}/

%files
/usr/local/bin/hlsd

%post
if [ "$1" -eq 1 ]; then
    echo "hlsd: installed. Run 'hlsd --help' to get started."
fi
