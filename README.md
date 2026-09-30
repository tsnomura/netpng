# netpng

A small toolkit of single-purpose PNG command-line tools, in the spirit of
[netpbm](https://netpbm.sourceforge.net/): instead of one big multi-flag
image editor, each tool does one thing and tools are meant to be connected
with pipes.

```
pngchroma --input photo.png --background 0,255,0 --blend 200 \
  | pngdecontaminate --background 0,255,0 \
  | pngcompose --base newbg.png --overlay - --output result.png
```

## Design

- **PNG only.** No other image format in, no other format out. `--compression
  fast|default|best` controls PNG compression level explicitly — there's no
  auto-detection of "am I in a pipe or writing a file", since that would make
  a tool's output depend on how it's invoked.
- **One job per tool.** No tool takes a chain of operations; if you want crop
  then rotate then compose, that's three tools in a pipe.
- **Uniform `--input`/`--output` convention**, shared by every tool:
  - a file path
  - `-` for stdin/stdout
  - `clipboard` for the system clipboard
- Internally, every tool normalizes images to 8-bit RGBA (`netpng-core`'s
  `Image` type) regardless of the source PNG's original color type or bit
  depth.

## Tools

| Tool | Like | What it does |
|---|---|---|
| `pngcrop` | pamcut | Crop to a rectangle (`--x --y --width --height`) |
| `pngtrim` | pnmcrop | Auto-detect and remove a uniform-color border |
| `pngcat` | — | Concatenate images horizontally or vertically |
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

Run any tool with `--help` for its full flags — most of the color/geometry
ones (`pngtrim`, `pngchroma`, `pngrotate`, `pngcompose`) have more knobs than
fit in the table above.

## Building

```
cargo build --workspace --release
```

Binaries land in `target/release/`. Requires a Windows desktop session for
`pngview` (it opens a native window); every other tool is headless.

## Installing on PATH (Git Bash / MSYS / WSL)

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

- `crates/netpng-core` — shared library: PNG decode/encode, the `Image`
  type, and every tool's actual image-processing logic (`crop`, `rotate`,
  `chroma_key`, `quantize`, ...)
- `crates/png*` — one thin CLI binary crate per tool, each just parsing
  flags and calling into `netpng-core`
