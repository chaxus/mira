# Trimmed ffmpeg side-car (option B)

Mira **captures** with OS APIs. This folder holds an optional **encode / remux / GIF** binary.

## Layout

```
vendor/ffmpeg/<os>-<arch>/ffmpeg[.exe]
```

Supported folder names (must match `mira::ffmpeg::vendor_triple()`):

| Folder | Target |
|---|---|
| `macos-aarch64` | Apple Silicon |
| `macos-x86_64` | Intel Mac |
| `windows-x86_64` | Windows x64 |
| `windows-aarch64` | Windows ARM64 |
| `linux-x86_64` | Linux x64 |
| `linux-aarch64` | Linux ARM64 |

Binaries are **gitignored**. Use [`scripts/fetch-ffmpeg.sh`](../../scripts/fetch-ffmpeg.sh) or drop a build in by hand.

## Size & license budget

| Rule | Target |
|---|---|
| Uncompressed size | Prefer **≤ ~25MB** per triple |
| Codecs | H.264 + AAC + mp4 (+ gif); mov demux OK for remux only |
| Prefer | LGPL configure if you can avoid GPL encoders; document if libx264 (GPL) is linked |

Do **not** vendor full “essentials+/full” static builds (~80–150MB+) into releases.

## Resolution order at runtime

1. `MIRA_FFMPEG` — absolute path override  
2. Bundled path under this tree (or next to the app binary)  
3. `ffmpeg` on `PATH` (local / CI convenience)

## Build sketch (essentials)

```bash
./configure \
  --disable-everything \
  --disable-network \
  --enable-protocol=file \
  --enable-demuxer=mov,matroska,gif \
  --enable-muxer=mp4,mov,gif \
  --enable-decoder=h264,aac,pcm_s16le \
  --enable-encoder=libx264,aac,gif \
  --enable-filter=scale,fps,palettegen,paletteuse,split \
  --enable-libx264 \
  --enable-gpl \
  --prefix=/tmp/mira-ffmpeg
make -j"$(nproc)" && make install
# copy ffmpeg → vendor/ffmpeg/<triple>/
```

Adjust flags for VideoToolbox / Media Foundation when targeting those platforms.