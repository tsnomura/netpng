# netpng

A small toolkit of single-purpose PNG command-line tools, in the spirit of
[netpbm](https://netpbm.sourceforge.net/): instead of one big multi-flag
image editor, each tool does one thing and tools are meant to be connected
with pipes.

```
pngchroma photo.png --background 0,255,0 --blend 200 \
  | pngdecontaminate --background 0,255,0 \
  | pngcompose newbg.png - > result.png
```

## Design

- **PNG only** for the processing tools: no other image format in, no
  other format out. The four converters (`jpg2png`, `png2jpg`, `ico2png`,
  `png2ico`) are the exception, in the spirit of netpbm's `jpegtopnm` and
  `pnmtojpeg`. `--compression fast|default|best` controls PNG compression
  level explicitly — there's no auto-detection of "am I in a pipe or writing
  a file", since that would make a tool's output depend on how it's invoked.
- **One job per tool.** No tool takes a chain of operations; if you want crop
  then rotate then compose, that's three tools in a pipe.
- **netpbm-flavored common conventions**, shared by every tool:
  - input is a **positional argument** at the end of the command, or the
    equivalent `--input` flag (same value either way; the flag wins if
    both are given): a file path, `-` (or omitted) for stdin, or
    `clipboard` for the system clipboard (PNG tools only)
  - **output goes to stdout** by default — redirect it with the shell
    (`pngtrim in.png > out.png`), or write a file (or the clipboard) with
    `--output` (`pngtrim in.png --output out.png`)
  - `-version` (also `-v` / `--version`) prints the version and exits
  - `-quiet` (also `-q` / `--quiet`) suppresses informational messages;
    errors are still printed
- Internally, every tool normalizes images to 8-bit RGBA (`netpng-core`'s
  `Image` type) regardless of the source PNG's original color type or bit
  depth.

## Tools

| Tool | Like | What it does |
|---|---|---|
| `pngcrop` | pamcut | Crop to a rectangle (`--x --y --width --height`) |
| `pngtrim` | pnmcrop | Auto-detect and remove a uniform-color border |
| `pngcat` | — | Concatenate images horizontally or vertically; a single input passes through unchanged |
| `pngcompose` | — | Alpha-composite (Porter-Duff "over") an overlay onto a base image at an offset |
| `pngflip` | pamflip | Mirror or rotate by a multiple of 90 degrees |
| `pngrotate` | pnmrotate | Rotate by an arbitrary angle, bilinear-sampled, canvas auto-expanded |
| `pngscale` | pamscale | Resize (`--scale`, or `--width`/`--height`), bilinear-sampled |
| `pnginvert` | pnminvert | Invert RGB (alpha untouched) |
| `pngnorm` | pnmnorm | Per-channel contrast stretch, clipping a percentile of dark/light pixels |
| `pngdist` | ppmdist | Map a low-color-count image to maximum-contrast gray levels |
| `pngreduce` | pnmquant | Median-cut color quantization to at most `--colors` colors |
| `pnginfo` | pamfile | Report width/height/color type/bit depth without decoding pixels |
| `pngchroma` | — | Chroma-key a solid background into alpha, with an anti-aliased (not binary) edge |
| `pngdecontaminate` | — | Remove background-color spill from an already-keyed image's edge pixels |
| `pngblur` | — | Gaussian blur; `--channels alpha` feathers a mask without touching its colors |
| `pngview` | — | Display an image in a window; reads from a pipe, a file, or the clipboard |
| `jpg2png` | jpegtopnm | Convert a JPEG to PNG (the JPEG is already lossy) |
| `png2jpg` | pnmtojpeg | Flatten alpha onto `--background` (default white) and encode as baseline JPEG at `--quality` |
| `ico2png` | — | Extract one size from an ICO as PNG (default largest; `--size` picks the closest) |
| `png2ico` | — | Pack an image into an ICO, one entry per size (default `16,32,48,256`) |
| `clip2png` | — | Copy the PNG image on the clipboard to stdout or a file, unchanged (no input image) |
| `pngmake` | pnmmake | Create a new solid-color PNG of a given size (no input image) |
| `pngpad` | pnmpad | Add a border of a solid color around an image |
| `pngalpha` | — | Extract the alpha channel as grayscale, or apply a grayscale `--mask` as the new alpha channel |
| `pngflatten` | pnmflatten | Flatten transparency onto a solid `--background`, producing an opaque PNG |
| `pnggamma` | pnmgamma | Apply gamma correction to the RGB channels |

Run any tool with `--help` for its full flags — most of the color/geometry
ones (`pngtrim`, `pngchroma`, `pngrotate`, `pngcompose`) have more knobs than
fit in the table above.

## Building

```
cargo build --workspace --release
```

Binaries land in `target/release/`. `pngview` opens a native window via
`winit` + `softbuffer` (software rendering — no GPU/OpenGL needed), so it
needs a real display session (Windows, macOS, or a Linux X11 session); it
cannot run headless, and on Linux it uses X11 (not Wayland). Input is
event-driven, so quick key taps are never missed, and OS key auto-repeat is
ignored for the one-shot actions. One known environment quirk: some window
managers (observed under xpra) shrink the client area slightly right after
the window is mapped (e.g. 800x600 becomes 790x568); the image and all
controls keep working in the smaller window, which is freely resizable. Its
`clipboard` input / `--copy-to-clipboard` / "c" work on all of those via
the system clipboard. Every other tool is headless.

## Installing on PATH (bash-like shells: Git Bash / MSYS / WSL / Linux / macOS)

```
./install.sh          # shims go to ~/bin by default
./install.sh <dir>     # or somewhere else on PATH
```

This copies the real binaries to `~/.netpng/bin` and generates a small bash
shim per tool (that just `exec`s the real binary) onto PATH. Two separate
locations, on purpose: some endpoint-security products treat a brand-new,
unrecognized *binary* sitting in a PATH directory as more suspect than the
same binary elsewhere, or than a *script* run by an already-trusted
interpreter (bash) — splitting "real binary, off PATH" from "thin shim
script, on PATH" avoided a false-positive "unknown program" prompt that
copying the binaries directly into a PATH directory reliably triggered
during development. The shims only work from a bash-like shell; from
PowerShell/cmd, call the `.exe` in `~/.netpng/bin` directly.

## Layout

- `crates/netpng-core` — shared library: PNG decode/encode, JPEG/ICO bridge
  codecs, the `Image` type, and every tool's actual image-processing logic
  (`crop`, `rotate`, `chroma_key`, `quantize`, ...)
- `crates/<tool>` — one thin CLI binary crate per tool, each just parsing
  flags and calling into `netpng-core`
