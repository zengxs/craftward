# Scintilla in Qt Quick

`Craftward.Editor.CodeEditor` hosts Scintilla 5.6.6 as a `QQuickItem`. The editor,
completion list, and call tips do not create native child windows or require
Qt Widgets. Context menus and overlay scrollbars use Qt Quick Controls.

## Responsibilities

- `ScintillaEditorBackend` exposes the document, appearance, editing commands,
  and scroll state to QML. It translates Quick events into adapter calls.
- `ScintillaQuickAdapter` owns the Scintilla editor and connects its timers,
  notifications, clipboard, drag and drop, and input method handling to Qt.
- `scintillaquickplatform.cpp` implements Scintilla fonts and drawing surfaces
  with `QFont`, `QTextLayout`, `QPainter`, and `QImage`.
- `scintillaquickwindow.cpp` implements the platform window, completion list,
  menu, and call tip support using Quick items.
- `ScintillaImageItem` owns the backing image and scene graph texture.

Scintilla determines text layout, styling, selections, carets, indicators,
folding, and editor decorations. Qt rasterizes the drawing operations and
composites the resulting image with the rest of the Quick scene.

## Rendering and ownership

Scintilla document access, input, notifications, and painting stay on the GUI
thread. `updatePolish()` paints pending damage into a device-pixel-ratio-aware
image. During scene graph synchronization, `updatePaintNode()` publishes that
image as a texture. The render thread never calls Scintilla. Qt's implicit
image sharing keeps a published image alive while later GUI updates detach it.

Scintilla invalidation rectangles restrict CPU drawing. Vertical scrolling
reuses cached rows when the image is current and the displacement is an exact
number of physical pixels. Other scrolls repaint the viewport. Dirty regions
are cleared before drawing so antialiasing does not accumulate on cached
pixels at fractional scales. Abandoned paints caused by wrapping or styling
are retried over the full viewport before publication.

The current presentation path creates and uploads a complete texture for each
changed image. It does not perform partial GPU uploads. Unchanged frames reuse
the texture. This implementation makes no measured 60 Hz or 120 Hz guarantee.

## Document and input API

The QML control preserves `text`, `readOnly`, `wordWrap`, font, palette,
`lineHeightScale`, and the one-based `revealLocation(startLine, endLine)` API.
It also exposes undo, redo, cut, copy, paste, and select-all commands.

The C++ backend offers `sendMessage()` for Scintilla features such as style
ranges, multiple selections, completion, and call tips.
Call it only on the GUI thread. Use the backend setters for properties exposed
to QML. Scintilla positions and lengths use UTF-8 bytes; Qt input method
positions use UTF-16 code units, with conversion inside the adapter.

Document notifications are coalesced before updating QML. The `text` property
converts the full document only when a consumer reads it after a change. A
binding that reads `text` on every edit still incurs that full-document cost.
IME candidates use tentative undo operations and do not become bound document
text until committed. A committed-text snapshot is retained during composition
and reused across candidate updates. Preedit foreground and background colors
and underlines use Scintilla's reserved IME indicators, with UTF-16 format
ranges mapped to UTF-8 positions for every selection. Committing a replacement
removes the existing selection before applying its relative replacement range.
Text drops resolve the active input method composition before hit-testing and
editing, so the drop remains a separate undo action. If the input context does
not commit, it is reset and any remaining tentative text is cancelled.

Opened-file views allow temporary edits in memory. The file-opening API reads
from disk and provides no save or write-back operation.

The adapter uses UTF-8 documents. Minimaps, shared-document preview policy,
language servers, and debugger UI belong to subsequent ADE features. Trackpad deltas
currently scroll whole Scintilla display lines; there is no pixel-smooth text
scrolling layer.

The adapter applies the same horizontal scroll limits to wheel input,
scrollbars, and Scintilla messages. Position and range callbacks derive the
limits from the current Scintilla scroll width and text viewport, then constrain
the core offset before publishing state to QML. Resizing the viewport or explicitly
reducing the scroll width also constrains the existing offset. Wrapping fixes
the horizontal offset at zero. Caret navigation can expand the core scroll
width before the adapter applies these limits.

The adapter caches measured widths for document lines so the horizontal range
can shrink after deletions, document replacement, or font changes. Initial
measurement and layout changes process the document in short GUI-thread batches;
ordinary edits invalidate the affected lines. Cached widths retain the widest
offscreen line, and the range does not shrink while measurements are incomplete.
The range also includes the current selections' virtual-space carets and anchors,
using Scintilla's layout and line-end font metrics. These selection extents are
kept separate from the document cache, so clearing virtual space removes its
extra range without remeasuring the document. The widest extent includes a small
caret gap. Selection notifications schedule extent updates even for offscreen
selections, independently of repainting.

## Syntax highlighting

`CodeEditor` exposes `language`, `filePath`, `darkTheme`, and the read-only
`syntaxName`, `languageRecognized`, and `highlightingReady` properties. An
explicit language takes precedence; `text` selects plain text. Otherwise a
nonempty file path enables matching the full filename, suffixes from longest to
shortest, and the current buffer's first line, in that order. Compound suffixes
such as `h.in` and `go.yaml` take precedence over generic shorter extensions.
No highlighting operation reads the edited file from disk. Editors with neither
hint remain plain text. Opened files supply their existing path metadata.

`ScintillaHighlighter` maps the shared engine's UTF-8 ranges to Scintilla text
styles. It reserves the predefined styles, deduplicates effective appearances,
and restores definitions after default-font or palette changes. Token foreground,
bold, italic, and underline follow One Light / One Dark; backgrounds remain owned
by the editor palette. Style changes preserve text, selections, and undo history.

Selection text retains syntax foregrounds by default. The QML control blends the
selection background with its base palette. An explicit backend
`selectionForegroundColor` can override token colors; resetting that property,
or assigning an invalid `QColor` in C++, restores styled foregrounds for all
selection states.

Opening text and configuration changes within one event-loop turn are coalesced
before reaching the worker. New documents display their background until the
initial visible range has received its styles; a delayed loading indicator covers
longer waits. Readiness uses Scintilla's styled layout, wrapping, and current
viewport, including files opened at a specific location. It does not wait for
the entire document, and parser completion alone does not imply that all style
batches have been applied. Empty documents and parse failures also leave the
loading state. Editors with neither a language nor a file hint can immediately
present plain text. Ordinary edits retain the presented text and cached styles while
incremental replacements arrive. Later cross-line replay may still correct
speculative styles.

`SyntaxHighlightingDocument` sends actual document edits, including tentative IME
changes, to a worker-affine Rust session on a shared background thread. Parsing
uses settled state checkpoints and reuses suffixes only after state convergence.
Reparsing discards an old checkpoint if its boundary becomes speculative, so a
later edit cannot restore a state belonging to an earlier revision.
Cross-line parser replay can replace styles before the edited line. A small
maintained Syntect fork supplies replay positions and safe checkpoint relocation;
see `core/third_party/README.md` for its integration requirements.

Results carry document epochs and text/configuration revisions. Unacknowledged
style changes survive rejected results and intervening edits. Parse work is
cooperatively budgeted, and each GUI application covers at most 16 KiB of style
bytes. Styling requests during paint only advance Scintilla's request cursor;
the worker independently tracks syntax validity. Existing style bytes remain
visible until their replacement arrives. IME composition pauses application of
syntax results while retaining all tentative edits in the mirror.

Parser failures fall back to plain text and log a diagnostic. A later edit or
configuration change can rebuild the session. A configuration update still
waiting for dispatch is promoted to a full reset when the current session fails;
only a pending full reset can supersede that failure. Individual
regular-expression matches cannot be interrupted by the cooperative budget.
Initial coloring of large documents progresses asynchronously; this is not a
frame-rate guarantee.
Each Scintilla document may be attached to only one Quick editor at a time.
`SCI_SETDOCPOINTER` rejects a document already attached to another editor with
`SC_STATUS_FAILURE` (read through `SCI_GETSTATUS`), preserving the current
document, selection, and highlighting session. Reattaching the editor's own
document is a no-op. A retained document can be transferred after its previous
editor switches documents or is destroyed. Successful switches, including a
null pointer that creates an empty document, reset the highlighting session.
Supporting simultaneous views requires a document-level highlighting owner and
coordinated style-ID mappings; different themes also need an explicit policy.

## Validation and provenance

Configure the application with `BUILD_TESTING=ON`, then build
`CraftwardScintillaQuickTest`. `task app:test:cpp` includes this target.
The CTest entries run the editor under Qt's offscreen software backend at
normal and 125% scale. They cover editing, undo, multiple selections, UTF-16
IME replacement and cancellation, completion and call tips, text drops,
wrapping, horizontal input bounds and range changes, range expansion during
caret navigation, virtual-space extents across appearance and selection changes
(including offscreen carets and anchors),
the public QML control, scene clipping and composition,
partial painting, and scroll-image equivalence.
Highlighting regressions also cover theme and font changes, tentative IME edits,
rapid document replacement, obsolete errors, and rendered token colors. Rust
tests exercise replay, checkpoint relocation, mixed UTF-8 edits, and rejected
style batches against complete parsing. The optional ignored large-document
test in `ward-highlighting` measures an 8 MiB source and a subsequent edit.
The IME drop tests use Qt's private test input-context hook to cover platform
commit and reset responses; this dependency is limited to the test target.

For native rendering checks, run the test executable directly with
`QSG_RHI_BACKEND=metal` and no offscreen platform override. The clipboard tests
write to the active platform clipboard. `CRAFTWARD_EDITOR_TEST_IMAGE` can name
an output image for scene inspection and scroll mismatch diagnostics.

The font and surface implementation is adapted from the bundled Scintilla
5.6.6 Qt adapter. The vendored Scintilla source is unchanged; `Scintilla::Core`
selects its existing `SCINTILLA_QT_QML` platform configuration.
