# Conversation terminals

`TerminalController` owns local PTY sessions for the duration of the application
run. A conversation has its own tabs, active tab, and panel visibility. Switching
conversations, hiding the panel, and closing the main window preserve processes.
Closing a tab ends its process; quitting Craftward ends all local terminals.
Processes and terminal contents are not restored after an application restart.

Open the panel with the terminal button beside the conversation title or
**Window → Terminal** (`Command-J`). The panel has a scrollable tab strip and a
fixed new-terminal button. Drag its upper edge to resize it. Font family and size
are available in **Settings → General** and apply to existing terminals as well
as new ones.

The font selector lists available monospaced families, with the bundled Fira Code
first and selected by default. An unavailable saved family falls back to Fira Code.
Installing or removing fonts refreshes the list while the application is running.
Primary font selection leaves the terminal's emoji and missing-glyph fallback
configuration intact.

New terminals start in the selected conversation's working directory and launch
the account's login shell interactively. User startup files, history, environment,
and pager preferences apply normally. If the directory is unavailable, creation
is disabled. This integration currently runs on the local host.

On macOS, `/usr/bin/login -flp` initializes the login session while preserving
the conversation directory and inherited environment. A short `/bin/zsh -fc`
bootstrap then replaces itself with the selected shell, prefixing its `argv[0]`
with `-` to enable login-shell startup files. The selected shell path is passed
as an argument, not interpolated into shell code. This avoids assuming that every
shell accepts `-l -i`. The bootstrap also removes `STDOUT_FASTPIPE`, since `login`
closes Contour's extra output descriptor; regular PTY output is unaffected.

## Boundaries

The public controller and view headers use Qt and C++20. Contour types stay in the
C++23 implementation. `TerminalView` embeds Contour's Qt Quick display, including
its RHI renderer, IME handling, selection, clipboard, and accessibility support.
It translates macOS modifier keys at the display boundary; the rest of the
application retains Qt's normal Command/Control conventions.

Each conversation uses a Contour window controller in `WindowMode::Embedded`.
The model still owns tabs and sessions, but the controller cannot adopt or close
Craftward's OS window. Session creation accepts an explicit directory so an
existing shell's `cd` cannot change the starting directory of later tabs.

QML can detach an ancestor before destroying the terminal display. Contour keeps
a weak reference to its last render window so teardown can wait for an outstanding
frame before releasing the font and atlas resources it still uses.

The bundled configuration uses a steady block cursor, immediate
cursor updates, grayscale font rendering, CoreText fallback, and font-rendered
Braille characters. The terminal grid follows legacy application width rules by
default. Applications with different Unicode width tables can still disagree
about emoji sequence widths.

## Rendering and compatibility

CoreText locates font faces and character fallbacks on macOS. HarfBuzz shapes
text, FreeType rasterizes the glyphs, and Contour draws its texture atlas through
Qt Quick's RHI renderer on Metal. Qt's native text rendering setting does not
replace this pipeline. Bundled fonts are registered with both CoreText and Qt's
application font database for the lifetime of the process.

Font collection member identity must survive both lookup and the shaping cache;
otherwise regular, bold, and italic faces can incorrectly share glyph rasters.
The font regressions cover Menlo's four faces in both loading orders, resizing,
and Fira Code regular and bold. They accept the bundled static faces or a system
variable font of the same family, while requiring the requested face identities
and distinct regular/bold rasters. Separate fallback checks verify Braille symbols
and color emoji when the initial font cascade is too short.

Emoji artwork can extend beyond its logical terminal cells. Keep each emoji
cluster anchored to its grid position and let the block cursor follow logical
cells. The default `grapheme_clustering: false` preserves compatibility with
legacy editor width rules; applications can opt into DEC mode 2027. Editor width
overrides are outside Craftward's configuration and cannot resolve every Unicode
sequence disagreement.

PTY writes must remain non-blocking. Backpressure leaves unsent bytes in the
input queue and schedules another attempt, including when the receiving program
produces no output. Input transactions serialize the queue and drain local echo
under the existing terminal lock; display callbacks run after that lock is
released. Regressions retain the local-echo deadlock and deferred-delivery cases.

## Build and validation

`app/third_party/cmake/contour.cmake` defines the embedded Contour targets directly.
Contour's upstream `CMakeLists.txt` files, standalone QML interface, executable,
and packaging rules are not part of the Craftward build. Source lists, module
dependencies, resource paths, and shader compilation must be checked when updating
the Contour submodule. The shell-data generator is reused because `ContourGuiApp`
still inherits the CLI model that references those resources.
Craftward and Scintilla remain C++20; Contour and the adapter use C++23 with the
same Apple compiler and system C++ runtime. The deployment target is macOS 13.3,
with both Intel and Apple Silicon slices.

The ten source dependencies live in submodules alongside Contour under
`app/third_party`; their revisions are pinned by the parent repository. Initialize
those submodules before configuring. When libunicode needs to regenerate its
tables, Unicode character data is downloaded into the build directory.
`contour-fonts.cmake` adds FreeType, HarfBuzz, and libpng through their native CMake
targets. These libraries remain optimized in Debug builds. The libpng adapter
preserves SSE and NEON selection for Universal builds, and the HarfBuzz adapter
probes the capabilities of the source-built FreeType headers.

`contour-meson.cmake` builds Pixman and Cairo through Meson using CMake
`ExternalProject` targets. Each architecture has its own configuration and install
prefix; `lipo` combines only these two libraries. Build-local pkg-config metadata
points Cairo to the CMake-built FreeType and libpng archives and their source and
generated headers. Native source edits and generated-header dependencies are
tracked by the build graph. Meson configuration caches are reset when its version
changes. Discovery is restricted to these build-local dependencies and the macOS
SDK, so Homebrew font libraries do not enter the application.

Meson and pkg-config are build-machine tools discovered by CMake; install them
with `brew install meson pkgconf`. No separate Python build driver is maintained.

Contour's builtin SSH and TLS support are disabled. TLS context and certificate
creation return an explicit unavailable error. Token comparison uses macOS's
`timingsafe_bcmp`; OpenSSL is not built or linked. Remote connection capabilities
belong in the Rust core.

The Qt SDK must include `qtmultimedia` and `qt5compat` in addition to the existing
Craftward modules. CMake 3.28+, Ninja, Python 3, Meson 1.3+, pkg-config, and the
Xcode command-line tools are required. The application test task includes
`CraftwardTerminalIntegrationTest`; this test uses a native macOS window and the
real PTY/rendering path. Its coverage includes conversation ownership, directory
selection, font updates, Control-C,
an 11,000-character paste into Vim, and terminal/window teardown. A separate
compile target verifies that the public headers still work with C++20.

`task app:test:cpp` also builds and runs `CraftwardTerminalInputTest`,
`CraftwardTerminalCursorTest`, and `CraftwardTerminalFontTest`. Input cases each
run in a separate process with a three-second timeout to catch deadlocks. The
cursor test reads the production profile and checks the first updated render
buffer. Font tests use production font registration and run offscreen. These
targets use the normal CMake dependency graph and require no separate SDK or
download/bootstrap scripts.

The integration test exercises the macOS login launcher with zsh, bash, tcsh,
and csh, checking login and interactive mode, the initial directory, inherited
environment, login identity, and shell paths containing spaces and metacharacters.
The native window test uses the same launcher with zsh. Startup files and history
are isolated in fixture directories, including a HOME override applied after
`login`. Environment overrides affect terminal child processes only. The
application's default constructor selects the account's login shell.

For a short manual comparison, use the same font size and panel dimensions in
Craftward and another terminal:

- Type and move the cursor in the shell and `cat`; stop continuous `yes` output
  with Control-C and type again immediately.
- Check regular/bold text, Chinese, Braille spinners, adjacent emoji, ligatures,
  and box-drawing alignment at the display's native scale.
- Copy and paste multiline and wrapped text, scroll while output continues,
  and resize a full-screen editor such as Vim.
- Switch between the composer and terminal, hide and reopen the panel, and
  close one of several tabs while checking that the other sessions remain usable.

Command duration and FPS alone do not measure input-to-display latency. Record
the visible symptom first; use targeted measurements to investigate regressions.
