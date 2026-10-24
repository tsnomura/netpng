---
name: netpng
description: Edit PNG images from the shell with netpng, a netpbm-style toolkit of single-purpose PNG commands (pngtrim, pngcrop, pngscale, pngpad, pngcat, pngcompose, pngchroma, clip2png, png2jpg, ...) installed in ~/bin. Use this skill whenever the user wants to crop, trim whitespace/margins, resize, rotate, flip, pad, tile side by side, overlay/composite, remove a solid or green-screen background, feather, blur, invert, gamma-correct, normalize, reduce colors, flatten transparency, check an image's size, convert PNG to/from JPEG or ICO, or move an image to/from the clipboard -- screenshots and figures included, and even when netpng is not named (画像の切り抜き・余白削除・リサイズ・背景透過・合成・並べる・クリップボードの画像など). Prefer it over writing ImageMagick commands or Python/Pillow scripts.
---

# netpng

netpng is a set of small PNG command-line tools in the spirit of netpbm: each
tool does one job, and jobs are chained with pipes
(<https://github.com/tsnomura/netpng>). Its `install.sh` puts shims in
`~/bin` and the real binaries in `~/.netpng/bin`. Using it instead of ad-hoc
ImageMagick/Pillow code keeps the user's image workflow in one consistent
set of tools.

If a tool is missing (`command -v pngtrim` fails), build and install it
from a clone of the repository:
`cargo build --workspace --release && ./install.sh`.
`<tool> --help` is always the authoritative reference for flags.

## Conventions shared by every tool

- **Input**: a positional argument, or `-i`/`--input` (not both). `-` or no
  input means stdin; `clipboard` reads the clipboard image. Exceptions:
  `pngcompose` takes `BASE OVERLAY` (or `--base`/`--overlay`), `pngcat` takes
  any number of inputs, `pngmake` and `clip2png` take none, and
  `jpg2png`/`ico2png` read only files or stdin.
- **Output**: PNG on stdout by default; `-o FILE` writes a file, `-o clipboard`
  sets the clipboard. `png2jpg`/`png2ico` write JPEG/ICO instead.
- **Colors** are `R,G,B` or `R,G,B,A`, each 0-255 (`255,255,255`, `0,0,0,0`).
- **Options** are Unix style: `-q`/`--quiet`, `-v`/`--version`, long options
  with two dashes. Single-dash long forms like `-quiet` do not exist.
- **Pixels are 8-bit RGBA** inside every tool, so output is always an 8-bit
  RGBA PNG: 16-bit precision and PNG metadata are not preserved, and
  grayscale/palette inputs come out as RGBA. `--compression best` gives
  smaller final files (the default `fast` is meant for pipelines).
- **stdin can feed only one image per command**: in `pngcompose`, `pngcat`
  and `pngalpha --mask`, at most one input may be `-`. Give the others as
  files.

## Running netpng from Claude Code

These matter because Claude's shell is non-interactive and Claude cannot
see windows:

1. **Never let PNG bytes reach the terminal.** End every pipeline in `-o FILE`,
   a `>` redirect, or another tool (`| pnginfo` to check the result).
2. **To look at a result, Read the PNG file** with the Read tool, which
   shows the image. Do this to check trims, cutouts and compositions before
   reporting success. Do not launch `pngview` to look: you cannot see its
   window, and it blocks until the window is closed.
3. **Write to a new file** by default (`shot-trimmed.png`); overwrite the
   input only when the user asks for it.
4. **Reading the clipboard** (`clip2png`, or `clipboard` as an input) needs a
   display (`$DISPLAY` or `$WAYLAND_DISPLAY`).
5. **Writing the clipboard on Linux blocks**: X11/Wayland clipboards are
   served by the process that set them, so `-o clipboard` keeps running
   until something else is copied. Detach it so the Bash call returns and
   the image stays available:
   ```bash
   setsid -f pngtrim shot.png -o clipboard >/dev/null 2>&1
   clip2png | pngtrim | setsid -f pngcat -o clipboard >/dev/null 2>&1   # pipelines too
   ```
   (`pngcat` with one input passes it through, handy as a final
   "write somewhere" stage.) On Windows/macOS `-o clipboard` returns at once.
6. **pngview** opens a window for the *user*. Run it only when they ask to
   see something, in the background or with `--timeout SECONDS`.
   `pngview --copy-to-clipboard` shows the image and keeps it on the
   clipboard while the window is open.

## Tools

| Tool | Does | Key options |
|---|---|---|
| `pnginfo` | Print `WxH color=... depth=... bytes=...` | |
| `pngcrop` | Crop a rectangle | `--x --y --width --height` (all required) |
| `pngtrim` | Remove a uniform border | `--background R,G,B` (default: top-left pixel), `--fuzz N` tolerance, `--margin N`, `--top/--bottom/--left/--right` (default: all four) |
| `pngpad` | Add a border | `--top --bottom --left --right`, `--color` (default transparent) |
| `pngscale` | Resize, bilinear | `--scale F`, or `--width`/`--height` (one keeps aspect ratio) |
| `pngrotate` | Rotate any angle, clockwise, canvas grows | `--angle DEG`, `--background` (default transparent) |
| `pngflip` | Mirror / rotate by 90s | `--mode horizontal\|vertical\|rotate90\|rotate180\|rotate270` |
| `pngcat` | Join images in order | `--direction horizontal\|vertical`; heights (or widths) must match |
| `pngcompose` | Overlay onto base ("over") | `BASE OVERLAY`, `--x --y` offset (may be negative); output has base's size, overlay is clipped |
| `pngchroma` | Key a solid background to alpha, soft edge | `--background R,G,B`, `--similarity` (30), `--blend` (30); see Chroma keying |
| `pngdecontaminate` | Remove background spill from keyed edges | `--background R,G,B` (same as pngchroma) |
| `pngblur` | Gaussian blur | `--sigma PX`, `--channels all\|rgb\|alpha` (alpha-weighted; `alpha` feathers a cutout) |
| `pngalpha` | Extract alpha as gray, or apply a gray mask as alpha | `--mask MASK.png` (same size) |
| `pngflatten` | Composite onto a solid color, opaque result | `--background R,G,B` (default white) |
| `pnginvert` | Invert RGB (alpha kept) | |
| `pnggamma` | Gamma on RGB | `--gamma G` (>1 brightens midtones) |
| `pngnorm` | Per-channel contrast stretch | `--bpercent` (2), `--wpercent` (1) |
| `pngreduce` | Median-cut to N colors | `--colors N` |
| `pngdist` | Few-color image to max-contrast grays | `--order intensity\|frequency` |
| `pngmake` | New solid image | `--width --height --color` (default white) |
| `clip2png` | Clipboard image to PNG | |
| `jpg2png` / `png2jpg` | JPEG to/from PNG | png2jpg: `--quality` (90), `--background` for alpha (white) |
| `ico2png` / `png2ico` | ICO to/from PNG | ico2png: `--size N`; png2ico: `--sizes 16,32,48,256` |
| `pngview` | Window for the user (see above) | `--timeout S`, `--copy-to-clipboard`, `--zoom` |

## Recipes

```bash
# Trim a screenshot's background, then give it an even 16px white margin
pngtrim shot.png --fuzz 8 | pngpad --top 16 --bottom 16 --left 16 --right 16 --color 255,255,255 -o shot-clean.png

# Green-screen cutout (choose --blend as in "Chroma keying" below)
pngchroma in.png --background 0,255,0 --similarity 0 --blend 200 \
  | pngdecontaminate --background 0,255,0 -o cutout.png

# Put the cutout on a new background (base = background, overlay = cutout)
pngcompose newbg.png cutout.png -o result.png

# Before/after side by side (match heights first)
pngscale after.png --height "$(pnginfo before.png | sed -E 's/^[0-9]+x([0-9]+).*/\1/')" -o after-h.png
pngcat before.png after-h.png -o compare.png

# Center an overlay: offsets from both sizes
read bw bh < <(pnginfo base.png | sed -E 's/^([0-9]+)x([0-9]+).*/\1 \2/')
read ow oh < <(pnginfo logo.png | sed -E 's/^([0-9]+)x([0-9]+).*/\1 \2/')
pngcompose base.png logo.png --x $(( (bw - ow) / 2 )) --y $(( (bh - oh) / 2 )) -o centered.png

# Clipboard in, trimmed, back to the clipboard (Linux: detached, see above)
clip2png | pngtrim | setsid -f pngcat -o clipboard >/dev/null 2>&1

# Batch
mkdir -p trimmed && for f in *.png; do pngtrim "$f" -o "trimmed/$f"; done

# Formats
jpg2png photo.jpg | pngscale --scale 0.5 | png2jpg --quality 85 -o small.jpg
png2ico logo.png -o favicon.ico
```

**Large reductions**: `pngscale` samples bilinearly with no area averaging,
so shrinking below about half size aliases (jagged text, moire). Shrink in
halving steps instead (`pngscale --scale 0.5 | pngscale --scale 0.5 | ...`;
an exact 0.5 averages each 2x2 block), then make the last, smaller step.

**Rotation leaves transparent corners**; add `| pngflatten --background R,G,B`
(or pass `--background` to pngrotate) when the result must be opaque.

## Chroma keying

`pngchroma` sets each pixel's alpha from its RGB distance `d` to the key
color, `d = sqrt(dR^2 + dG^2 + dB^2)` (0 to about 441): transparent up to
`--similarity`, opaque from `--similarity + --blend`, a linear ramp between.
Choosing the band:

- **Keep `similarity + blend` below the distance from the key to the
  closest color inside the subject.** Any subject color nearer than that
  becomes partly transparent. Example: key `0,255,0` vs a yellow label
  `250,220,60` is d = 259, so a band ending at 280 leaves the label at
  alpha 234/255 and the background shows through it.
- **Make the band as wide as that allows.** Antialiased edge pixels are
  mixes of the key and the edge color; a band that ends well short of the
  edge color's distance turns most of them fully opaque, which looks
  hard-cut and keeps a green rim.
- **Always follow with `pngdecontaminate` using the same `--background`**;
  it removes the key's tint from the partly transparent edge pixels.
- **To soften edges further, feather after decontaminating**:
  `| pngblur --sigma 0.7 --channels alpha`. The blur is alpha-weighted, so
  the key color hidden in transparent pixels does not come back.

Check every cutout before using it: `pngalpha cutout.png -o alpha.png` and
Read it (the subject must be solid white, the background black, gray only
along the edge), then Read `pngflatten cutout.png --background 128,128,128`
for leftover tint.

When a subject color is so close to the key that no single band works
(edges need a wide band but an interior color then turns translucent), key
twice and stack the results: a wide pass for the edges, plus a hard pass
whose threshold sits just below the closest interior color's distance so
the whole interior is opaque:

```bash
pngchroma in.png --background 0,255,0 --similarity 0 --blend 296 \
  | pngdecontaminate --background 0,255,0 -o wide.png
pngchroma in.png --background 0,255,0 --similarity 255 --blend 1 \
  | pngdecontaminate --background 0,255,0 -o core.png
pngcompose wide.png core.png -o cutout.png      # core over wide
```

## When netpng is not enough

netpng has no text or shape drawing, no general grayscale/desaturate for
photos (`pngdist` is only for few-color images), no sharpening or
brightness/contrast arithmetic beyond `pnggamma`/`pngnorm`, no formats other
than PNG/JPEG/ICO (no WebP, GIF, TIFF, SVG), and no metadata editing. For
those steps, say netpng doesn't cover it and use ImageMagick or Pillow for
just that step, keeping netpng for the rest of the pipeline.
