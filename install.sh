#!/usr/bin/env bash
# Installs netpng's tools for use from the shell: the real binaries go to
# a dedicated, non-PATH directory (~/.netpng/bin), and a small bash shim
# per tool (just `exec`s the real binary) goes onto a PATH directory.
#
# Why: some endpoint-security products treat a brand-new, unrecognized
# binary sitting in a PATH directory (a common persistence/hijack target)
# as more suspect than the same binary elsewhere, or than a shell script
# invoked by an already-trusted interpreter (bash). Splitting "real binary,
# off PATH" from "thin shim script, on PATH" avoids that combination.
#
# Usage: ./install.sh [shim-dir]   (shim-dir defaults to ~/bin)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/target/release"
INSTALL_DIR="$HOME/.netpng/bin"
SHIM_DIR="${1:-$HOME/bin}"

TOOLS=(
  pngcrop pngcat pngtrim pngcompose pnginfo pnginvert pngflip pngrotate
  pngscale pngnorm pngdist pngreduce pngchroma pngblur pngdecontaminate pngview
)

if [ ! -d "$RELEASE_DIR" ]; then
  echo "error: $RELEASE_DIR not found -- run 'cargo build --workspace --release' first" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR" "$SHIM_DIR"

for t in "${TOOLS[@]}"; do
  exe="$RELEASE_DIR/$t.exe"
  if [ ! -f "$exe" ]; then
    echo "error: $exe not found -- run 'cargo build --workspace --release' first" >&2
    exit 1
  fi
  cp -f "$exe" "$INSTALL_DIR/$t.exe"

  shim="$SHIM_DIR/$t"
  cat > "$shim" <<EOF
#!/usr/bin/env bash
exec "$INSTALL_DIR/$t.exe" "\$@"
EOF
  chmod +x "$shim"
done

echo "Installed ${#TOOLS[@]} tools:"
echo "  binaries -> $INSTALL_DIR"
echo "  shims    -> $SHIM_DIR"
echo
echo "Make sure $SHIM_DIR is on PATH. These shims only work from a bash-like"
echo "shell (Git Bash, MSYS, WSL); from PowerShell/cmd, call the .exe in"
echo "$INSTALL_DIR directly, or build+copy per-tool binaries instead."
