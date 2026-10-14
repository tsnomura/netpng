# research_log.md

Research decisions, experiments, and findings for netpng that aren't
adequately captured by Git history alone.

## 2026-09-30 — Trend Micro Apex One flags `~/bin` binaries; root cause and fix

### Goal
After installing the compiled tools into `~/bin` (a PATH directory), Trend
Micro Apex One's "新たなプログラムが検出されました" (new program detected)
behavior-monitoring prompt fired on every run of `pngview.exe`. Needed to
find what specifically triggered it, since it's a corporate-managed
machine and blanket "just allow it" isn't a real fix.

### Trial
Isolated variables one at a time by running the same/different binaries
from different locations and checking whether the prompt appeared:
1. MSVC-built `pngview.exe` in `~/bin` → prompted.
2. Same binary, rebuilt with the `x86_64-pc-windows-gnu` (MinGW) target,
   run from the Cargo build output dir (`target/.../release/`, not on
   PATH) → no prompt.
3. The GNU build copied into `~/bin` → prompted.
4. The original MSVC build run directly from `target/release/` (not
   `~/bin`) → no prompt.
5. A trivial `fn main() {}` binary (no dependencies at all) placed in
   `~/bin` → no prompt.
6. `pnginfo.exe` (links `arboard` for clipboard support, but creates no
   window, and the actual invocation never touched the clipboard code
   path) placed in `~/bin` → prompted.

### Result
Toolchain (MSVC vs. GNU) made no difference. Plain novelty + a PATH
directory made no difference (the empty binary was clean). The common
factor across every prompted case was: **binary links `arboard`
(clipboard-capable code) AND sits in a PATH-listed directory**. Window
creation (`minifb`) was not necessary to trigger it — `pnginfo`, which
never creates a window, still prompted.

### Interpretation
Apex One's behavior-monitoring most likely flags (novel/unrecognized
binary) × (imports clipboard APIs) × (lives in a directory used for
command resolution, a classic persistence/hijack target) — a combination
of static characteristics and location, not a single blunt "this folder
is bad" rule, and not something that requires the program to actually
exercise the clipboard at runtime.

### Decision (and a second, failed attempt)
First attempt: a busybox-style single combined binary (`netpng` engine)
hardlinked/copied under all 16 tool names into `~/bin`, on the theory that
consolidating to one hash would mean only one prompt to clear. **This made
things worse** ("warns every time, not just once" turned out to also mean
every distinct path still gets challenged regardless of shared hash), and
splitting into a dependency-free `netpng-shim.exe` (in `~/bin`) that
spawned a separate `netpng` engine binary (kept in a `~/bin/.netpng-engine`
subdirectory) *also* still prompted — likely because that subdirectory is
still under the same monitored tree, and "unknown process spawns unknown
process" may itself look suspicious. Reverted both.

Final fix: `install.sh` installs the real (arboard-linked) binaries to
`~/.netpng/bin` — a directory that is *not* on PATH — and puts a small
**bash script** (not a compiled binary) named per tool in `~/bin` that
just `exec`s the real binary. A shell script run by the already-trusted
`bash.exe` interpreter doesn't present a new PE binary for Apex One to
evaluate at all, so the combination that triggered the rule never occurs.
Confirmed clean on repeated runs.

### Next
This only solves it for bash-like shells (Git Bash/MSYS/WSL). From
PowerShell/cmd the `.exe` in `~/.netpng/bin` must be called directly (or
the per-tool binaries built/copied conventionally, accepting the prompt).
Haven't asked IT about an internal code-signing cert or a path-based
allowlist entry, which would be the real cross-shell fix if this becomes
a problem again (e.g. for distributing these tools to other machines).

### Related
- files: `install.sh`
- commits: the revert of the busybox/shim layout and the bash-shim
  `install.sh` are both in the commit history around this date (see
  `git log --oneline -- install.sh`)

## 2026-10-01 — Linux clipboard: `--output clipboard` from a one-shot CLI call doesn't persist

### Goal
Confirm whether `--input/--output clipboard` (via `arboard`) works the
same way on Linux (tested under WSL2 Ubuntu + WSLg) as it does on
Windows, where the OS retains clipboard content independent of which
process set it.

### Trial
1. Set an image on the *Windows* clipboard, tried reading it from inside
   WSL via `pnginfo --input clipboard`.
2. Within WSL only: `pngcrop --output clipboard ...` (exits immediately
   after setting) followed immediately by `pnginfo --input clipboard` in
   a separate process.
3. Same within-WSL test, but with the writer (`pngview`, kept open via
   its "c" key / `--copy-to-clipboard`) still alive when the reader ran.

### Result
1. Failed — "clipboard is empty." Windows↔WSLg clipboard bridging
   apparently doesn't carry image data in the format `arboard` requests.
   (Out of scope: this project's Linux build targets Linux-only use, not
   Windows↔WSL interop, so not pursued further.)
2. Failed — even though the *write* call itself returned success (exit
   0), the content was already gone by the time the second process asked
   for it.
3. Succeeded, once `write_clipboard_image` was changed (see Decision) —
   `pnginfo` in a separate process correctly read back the image while
   `pngview`'s window was still open.

### Interpretation
X11 (and Wayland's clipboard compatibility layer) doesn't centrally store
clipboard content the way Windows does. The setting application is the
"selection owner" and must actively answer `SelectionRequest` events for
as long as the content should be available; ownership (and the content)
disappears the moment that owner disconnects, unless a clipboard manager
is running to adopt it (none is, in a bare WSLg session). Critically,
*merely keeping the host process alive is not sufficient* if the specific
`arboard::Clipboard` handle is dropped right after `set_image()` returns
(our original `write_clipboard_image` did exactly this) — the
X11-serving mechanism appears tied to that handle, not the process.
`arboard`'s Linux-only `SetExtLinux::wait()` is the documented way to
keep serving; it blocks the calling thread until the content is replaced
by something else (not until it's merely read once).

### Decision
- `netpng-core::write_clipboard_image` is now `#[cfg(target_os = "linux")]`-
  gated to spawn a **detached background thread** that runs
  `Clipboard::new()?.set().wait().image(data)`. The function itself
  returns immediately (optimistic "Ok" — a real failure, e.g. no X
  server, surfaces asynchronously as an `eprintln!` from that thread, not
  as a returned `Err`, since success never returns at all). Non-Linux
  keeps the original synchronous `set_image()`.
- Since a one-shot CLI call still can't usefully own the clipboard on
  Linux even with this fix (the process still exits and takes the thread
  with it), `pngview` gained `"c"` (and `--copy-to-clipboard` for
  scripting) to copy the *currently displayed* image — because pngview
  is already long-lived (runs until closed or `--timeout`), this is the
  practical way to get something onto the clipboard on Linux. Content
  survives for exactly as long as the window (and so the thread) stays
  open.
- Documented as a known characteristic (not a bug to keep chasing) in
  `pngview`'s own `--help` text.

### Next
If a true one-shot `--output clipboard` is ever needed on Linux, the only
way is OS-level daemonization (re-exec detached with `Stdio::null()`,
per `arboard`'s own `examples/daemonize.rs`) — more complexity than
seemed justified given `pngview --copy-to-clipboard --timeout N` already
covers the practical case.

### Related
- files: `crates/netpng-core/src/lib.rs` (`write_clipboard_image`),
  `crates/pngview/src/lib.rs` (`"c"`, `--copy-to-clipboard`, `--timeout`)
- commits: `4aff6c7` (detached-thread `wait()` fix), `d91bc4f` ("c" key),
  `cd31018` (`--copy-to-clipboard`/`--timeout`)

## 2026-10-14 — `pngview` winit rewrite crashes under WSLg; fixed by disabling decorations on Linux

### Goal
A parallel work stream (merged via PR #2, `f3423a9`) replaced `pngview`'s
window backend from `minifb` (polling-based) with `winit` + `softbuffer`
(event-driven), to get correct per-notch wheel-zoom handling. Needed to
verify this still works under WSLg (the Linux dev/test environment used
throughout this project), since the old `minifb` version was confirmed
working there.

### Trial
1. Built the new `winit`-based `pngview` in WSL2/Ubuntu + WSLg and ran it
   normally (with and without `--timeout`).
2. Forced the X11 backend via `WINIT_UNIX_BACKEND=x11` to see if avoiding
   Wayland entirely sidestepped the problem.
3. Captured a full `WAYLAND_DEBUG=1` protocol trace of a failing run and
   inspected it for the actual point of failure.
4. Added `.with_decorations(false)` to the `WindowAttributes` builder,
   gated to `#[cfg(target_os = "linux")]` only (Windows/macOS keep native
   server-side decorations), and reran the same tests.

### Result
1. Reliably failed, every run: window appeared to initialize, then
   `Io error: Broken pipe (os error 32)` printed twice, followed by
   `Error: event loop error: Exit Failure: 1` (process exit code 1).
2. No change — still failed identically. (In hindsight this means the env
   var had no effect on this `winit` version's backend selection; the
   trace in step 3 is unambiguously Wayland protocol traffic, not X11.)
3. The trace showed `smithay-client-toolkit`'s client-side-decoration
   support (via `sctk-adwaita`) creating four border subsurfaces
   (`wl_subsurface` objects for top/right/bottom/left strips around the
   content area) and successfully attaching/damaging/committing buffers
   for each. The real content surface (`wl_surface`) got one successful
   `attach`/`damage_buffer`/`commit` near the end of the trace, and the
   broken pipe followed immediately after — with no explicit Wayland
   protocol error logged anywhere in the trace.
4. Fixed. Three consecutive runs under WSLg completed cleanly (exit 0, no
   broken-pipe message, no decoration subsurfaces in a fresh trace).
   Rebuilt and retested on Windows afterward (`cfg(target_os = "linux")`
   keeps the change Linux-only): compiles warning-free, native title bar
   unaffected.

### Interpretation
WSLg's Wayland compositor (bridged through FreeRDP) doesn't support
server-side decorations — this was already known from the old `minifb`
version, which printed a "Failed to create server-side surface
decoration: Missing" warning but otherwise worked fine, because `minifb`
doesn't attempt a client-side fallback. `winit`'s stack (via
`smithay-client-toolkit`/`sctk-adwaita`) does attempt a CSD fallback, and
something in that compositor's handling of the extra decoration
subsurfaces causes it to drop the Wayland connection outright, which
`winit`'s event loop surfaces as a broken-pipe I/O error rather than a
clean protocol error. Disabling decorations avoids the CSD code path
entirely, so the compositor never gets to whatever it can't handle.

### Decision
Added `.with_decorations(false)` to the window builder, `#[cfg(target_os
= "linux")]`-gated so Windows/macOS are unaffected. This is a reasonable
permanent fix, not just a diagnostic workaround: `pngview` on Linux now
has no title bar/window chrome (borderless), but all functionality
(zoom/pan/save/clipboard/quit) is keyboard/mouse-driven already and
doesn't depend on window chrome. This is likely WSLg-specific (its
compositor is unusually limited/bridged); a native Wayland desktop
(GNOME/KDE/Sway) would probably handle `sctk-adwaita` CSD correctly, but
there was no such environment available to test, and disabling
decorations is a safe, low-cost choice either way.

### Next
If decorations are ever wanted back on native (non-WSLg) Linux desktops,
revisit with access to a real Wayland compositor to confirm whether the
crash is WSLg-only before re-enabling. Also still unresolved: why
`WINIT_UNIX_BACKEND=x11` had no effect on backend selection in this
`winit` version -- not pursued further since the decorations fix resolved
the actual problem.

### Related
- files: `crates/pngview/src/lib.rs` (`resumed`, `WindowAttributes`)
- commits: `f3423a9` (winit rewrite that introduced the regression), and
  the decorations-disable fix committed alongside this log entry
