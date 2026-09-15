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

The adapter uses UTF-8 documents. Syntax highlighting integration with
`SyntaxHighlightingEngine`, minimaps, shared-document preview policy, language
servers, and debugger UI belong to subsequent ADE features. Trackpad deltas
currently scroll whole Scintilla display lines; there is no pixel-smooth text
scrolling layer.

The adapter applies the same horizontal scroll limits to wheel input,
scrollbars, and Scintilla messages. Position and range callbacks derive the
limits from the current Scintilla scroll width and text viewport, then constrain the core
offset before publishing state to QML. Resizing the viewport or explicitly
reducing the scroll width also constrains the existing offset. Wrapping fixes
the horizontal offset at zero. Caret navigation can expand the core scroll
width before the adapter applies these limits.

Scintilla's width tracking only expands its known width. Deleting a long line
does not automatically shrink that width, so finite trailing space may remain.
The adapter does not measure the whole document to remove that space.

## Validation and provenance

Configure the application with `BUILD_TESTING=ON`, then build
`CraftwardScintillaQuickTest`. `task app:test:cpp` includes this target.
The CTest entries run the editor under Qt's offscreen software backend at
normal and 125% scale. They cover editing, undo, multiple selections, UTF-16
IME replacement and cancellation, completion and call tips, text drops,
wrapping, horizontal input bounds and range changes, range expansion during
caret navigation, the public QML control, scene clipping and composition,
partial painting, and scroll-image equivalence.
The IME drop tests use Qt's private test input-context hook to cover platform
commit and reset responses; this dependency is limited to the test target.

For native rendering checks, run the test executable directly with
`QSG_RHI_BACKEND=metal` and no offscreen platform override. The clipboard tests
write to the active platform clipboard. `CRAFTWARD_EDITOR_TEST_IMAGE` can name
an output image for scene inspection and scroll mismatch diagnostics.

The font and surface implementation is adapted from the bundled Scintilla
5.6.6 Qt adapter. The vendored Scintilla source is unchanged; `Scintilla::Core`
selects its existing `SCINTILLA_QT_QML` platform configuration.
