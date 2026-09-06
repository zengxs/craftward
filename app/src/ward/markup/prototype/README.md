# Inline Renderer Prototype

This is a throwaway native Qt capability experiment. Its question is whether
explicit inline semantics can drive Qt text layout and selection while real QML
controls occupy positions in the same text flow. It is not a production renderer
or a benchmark. The production application does not link this standalone target.

The user approved developing inline functionality before concentrating on frame
time optimization. Existing geometry regressions and strict performance metrics
remain the baseline; this experiment does not change their acceptance limits.

## Run

From the repository root, with the normal `.env` configuration:

```bash
task app:prototype:inline
```

The task builds only this small Qt executable. Build products, scratch files, and
captures stay under `.tmp/inline-renderer-prototype/`. No Codex history, network
connection, backend process, or application database is involved. Link activation
is recorded in the inspector instead of opening a browser. The copy shortcut
writes the selected sample text to the system clipboard.

## Walkthrough

1. Select text across ordinary text, inline code, the wrapped link, and `[4]`.
   Use the platform copy shortcut. The inspector displays the plain-text result.
2. Hover and click the link and annotation. Observe their distinct targets in the
   inspector. Hover shows a tooltip without changing paragraph geometry.
3. Click the inline Review control. It changes its label and reserved width while
   keeping the same QML object. Narrow the text width to move it across a line
   boundary. Change the font size to inspect its alignment and reservation.
4. Append several chunks to the final paragraph. The `stream:tail` ID stays the
   same. Existing documents keep `documentBuilds: 1`; the control keeps
   `overlayCreations: 1`.
5. Change the theme and inspect selected and unselected content, including the
   Chinese, Arabic, combining-accent, and emoji samples.
6. Use Capture state to save a window image and JSON snapshot under the task's
   evidence directory. Capture numbers restart with each process.

## Experimental Contract

Each fixture supplies explicit nodes with stable `id`, `kind`, and text or target
data. The demonstrated kinds are `text`, `code`, `link`, `annotation`, and
`control`. No regular expression or Qt Markdown parser interprets the fixtures.
The annotation's original directive is retained as metadata; the same directive
inside a code node remains literal text.

`InlineDocument` inserts text with `QTextCursor` and `QTextCharFormat` directly
into a native TextEdit document. Qt Quick owns the document and supplies text
rendering, wrapping, mouse/keyboard selection, and link hit testing.
`QQuickTextDocument` explicitly supports document modifications through this
interface ([Qt documentation](https://doc.qt.io/qt-6/qquicktextdocument.html)).

For this experiment, a transparent image object reserves the control's inline
box. The control remains a semantic `control` node, not an image in the content
model. `QTextImageFormat` supplies the reservation size; the existing block's
`QTextLayout` provides line and cursor geometry for positioning a real QML Button
([image format](https://doc.qt.io/qt-6/qtextimageformat.html),
[text layout](https://doc.qt.io/qt-6/qtextlayout.html)).

Each sample paragraph owns one document. Tail append edits the last text node
through a cursor; control resize replaces one object's format. Neither operation
rebuilds another paragraph. Theme changes reformat the materialized paragraphs.
Copying substitutes the control's readable label for its object replacement
character. Annotation copy currently yields its visible `[4]` label; the final
plain-text and Markdown export policies remain product decisions.

## Observations and Limits

The initial experiment builds and runs on Qt 6.11.2. Native window inspection
shows mixed styles, wrapped links, annotation rendering, Unicode text, and a QML
control in the text flow. Captured state after six tail appends, repeated control
activation, a width/font change, and a theme change retains one build per
document and one control creation. These are bounded observations, not a full
compatibility or performance certification.

The experiment supports pursuing a semantic-node-to-native-document boundary.
It does not yet choose the final renderer implementation. A standalone
QTextLayout renderer and QTextObjectInterface backend were not implemented or
compared here; the transparent-image reservation is an experimental adapter.

### User feedback follow-up

The first button label sat about 6.6 px above the surrounding text baseline.
The coordinate conversion omitted the font descent, and the button introduced
additional vertical padding. The compact inline control now uses the text's font
and line height with no vertical padding; its reservation and placement include
the native descent. A `--probe` mode checks the actual QML control baseline and
paragraph containment after a presented frame. Twelve combinations of 14/17/24
px fonts, 280/690 px widths, and short/expanded labels pass, with maximum absolute
baseline error below 0.37 px. This probe addresses this concrete feedback; it is
not a renderer acceptance suite.
The user accepted label-to-text baseline alignment for the inline Review control.

Native selection of the entire Arabic sample highlighted the expected text.
Dragging from the preceding LTR text into that RTL run produced a discontinuous
visual highlight for a continuous logical selection; the native selected text,
inspector output, and copy action agreed in the observed cases. This reported
behavior is classified as expected RTL selection, not an open defect. The checks
do not cover all bidi selection cases.

The prototype still has separate selection per block. The user confirmed that
continuous selection must span paragraphs, code blocks, and tables within one
message. Global selection across different messages is out of scope. The intended
design uses a message-level logical selection range, rendered by the materialized
blocks and copied from semantic data. Selection endpoints should use stable
semantic identities and text offsets rather than delegate identities. The shared
range coordinates selection across rendering segments; individual content types
still need their own hit testing and copy serialization, including table cell
order and preservation of code whitespace. This selection contract is accepted
but not yet implemented.

Remaining work before production includes:

- Real Markdown/Codex parsing in `ward-markup`, inline nesting, malformed and
  partial directives, and the FFI representation. These fixtures do not test
  source-to-node parsing. Reference resolution also remains outside this demo.
- Explicit conversion between UTF-8 source ranges and Qt UTF-16 cursor ranges.
  The prototype's `start` and `length` values are UTF-16 positions in the rendered
  document, not source offsets.
- Selection across independently virtualized blocks within one message, keyboard
  traversal through controls, rich clipboard export, and complete screen-reader
  validation. Accessible text and a named QML control are exposed, but VoiceOver
  behavior has not been certified.
- Exhaustive geometry validation around mixed text direction, line boundaries,
  tall controls, viewport clipping, DPI changes, and selection over controls.
- A stable production overlay model/pool, recycling, source replacement,
  layout-generation handling, and integration at the existing viewport seam.
- Later profiling and extreme-history memory/initialization acceptance. This
  three-paragraph demo establishes neither throughput nor bounded total storage.

Retain this prototype separately for review. Git capture and any throwaway branch
are human actions under repository policy. Production adoption should introduce
the semantic contract and its regressions deliberately rather than import this
fixture implementation wholesale.
