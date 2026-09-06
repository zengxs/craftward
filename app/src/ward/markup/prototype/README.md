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

The original inline walkthrough has separate selection per block. The user
confirmed that continuous selection must span paragraphs, code blocks, and tables
within one message. Global selection across different messages is out of scope. The intended
design uses a message-level logical selection range, rendered by the materialized
blocks and copied from semantic data. Selection endpoints should use stable
semantic identities and text offsets rather than delegate identities. The shared
range coordinates selection across rendering segments; individual content types
still need their own hit testing and copy serialization, including table cell
order and preservation of code whitespace. The separate selection experiment
below now exercises this accepted scope with immutable fixtures; production
integration remains open.

## Message Selection Experiment

Run the second experiment with:

```bash
task app:prototype:selection
```

It builds the same standalone executable into `.tmp/inline-selection-prototype/`
and opens its `--selection` view. The original inline walkthrough remains the
default mode. Both targets remain outside the production application.

The question is whether one logical selection can span paragraphs, code blocks,
and tables without retaining offscreen text documents. `MessageSelection` owns
the immutable semantic fixture and selection endpoints. `SelectionText` projects
the shared range into each materialized TextEdit. The viewport queries only live
text items for pointer geometry. Destroying a delegate does not destroy the
selection; a recreated delegate applies the same logical range.

The fixture has two messages and 42 segments. One prose segment contains two
paragraphs, and one table segment contains nine independently laid-out cells.
Each cell uses the same inline renderer as prose. The table demonstrates inline
code, a link, and an annotation. Their text participates in the shared selection;
clicking the link or annotation records its target locally. No browser opens.

Selection behavior in this experiment:

- Drag in either direction across content; a drag into another message clamps
  to the message where the selection began. Starting a new selection in another
  message clears the previous highlights.
- Double-click selects a word. Shift-click extends the current selection. The
  platform Copy and Select All shortcuts operate on the originating message;
  Escape clears the selection. Dragging near a viewport edge scrolls the view.
- Plain-text copy preserves code indentation and line breaks. Separators between
  segments are blank lines. Table cells use row order, with tabs between columns
  and newlines between rows; partial cell selection remains partial. Annotation
  copy uses its visible label, including `[4]` in the table fixture.
- Endpoints use globally unique fixture node IDs and rendered UTF-16 offsets,
  snapped to grapheme boundaries. The selected message is retained by the
  coordinator. Production identity and source-range mapping still need the real
  semantic contract.
- Only nearby text documents are materialized by ListView. The inspector limits
  its preview to 600 UTF-16 units; full serialization occurs on explicit copy or
  state capture. The semantic fixture itself stays in memory.

Try Select example, Jump away, then Back to start. The created/destroyed counters
show actual text-item destruction, and the restored highlight should match the
original copy. Show message boundary exposes the end of Message A and the start
of Message B. Try selecting only part of a table cell, then extending through the
next row. The `a()` code span, Ready link, and `[4]` annotation exercise inline
content inside cells.

`--selection-probe` runs a bounded walkthrough against actual TextEdits. Its 21
checks cover copy order and whitespace, partial cell highlights, inline content
inside cells, reverse selection, joined emoji, message boundaries, and delegate
destruction/recreation. All checks passed at 14/17/24 px fonts and 960/1180 px
window widths. The original 12 baseline checks also passed after sharing the
updated inline item, with maximum absolute baseline error below 0.37 px. Layout
and selection refresh timers are owned by their items so pending work is removed
when a virtualized delegate is destroyed.

The user tried the first selection build and reported stable cross-content
selection with no obvious issues. The follow-up added the table inline examples.
Native observation showed their rendering and recorded link activation. A later
automated drag was cancelled because the user was changing the window; it is not
counted as a completed automated pointer/copy check.

### Inline hover feedback

The first selection view attached its tooltip to the MouseArea covering the
entire viewport. The Basic style positioned that popup relative to the large
parent, leaving a measured gap of about 454 px from the Ready link. A dedicated
`--tooltip-probe` reproduced this geometry failure on the actual popup before
the correction. Qt documents both the shared attached tooltip and local tooltip
instances for custom placement in its
[ToolTip reference](https://doc.qt.io/qt-6/qml-qtquick-controls-tooltip.html).

`InlineDocument::linkFragmentAt` now reports the link or annotation's rectangle
on the text line under the pointer. `InlineHelpTip` places the hint 8 px from that
rectangle, flips above it when space below is insufficient, and constrains the
popup to the available area. Hover waits 400 ms; the hint is noninteractive and
does not take focus. Scroll, layout changes, and source destruction invalidate
the descriptor and dismiss the hint. The original inline walkthrough also uses
this placement adapter; the coordinated view owns one shared hint instance.

All eight tooltip checks passed for fonts 14/17/24 px and window widths 960/1180
px, with measured gaps of 8 px. They cover table links and annotations, viewport
containment, placement above the source under a constrained available area, and
dismissal after scrolling and source destruction. The 21 selection checks in six
configurations and the 12 original baseline configurations also still pass.

Tooltip and popover are different interaction behaviors. A tooltip supplies a
short, noninteractive hint. A future popover may contain selectable text, links,
or buttons and needs its own focus, dismissal, and pointer-transition policy.
They can share the semantic node identity and text anchor geometry. This change
implements tooltip placement only; clicking links and annotations still records
their targets rather than opening a full popover.

This is a selection-state experiment, not production acceptance. Full keyboard
navigation, accessibility selection, rich/Markdown clipboard export, mutable or
streaming content, and interactive QML controls inside table cells remain open.
The small table is materialized as one segment; large-table virtualization is
not established. Integration with the production parser, FFI, semantic identity,
viewport geometry, and renderer remains separate work. The transparent image
reservation is still an experimental adapter, not a final backend decision.

Remaining work before production includes:

- Real Markdown/Codex parsing in `ward-markup`, inline nesting, malformed and
  partial directives, and the FFI representation. These fixtures do not test
  source-to-node parsing. Reference resolution also remains outside this demo.
- Explicit conversion between UTF-8 source ranges and Qt UTF-16 cursor ranges.
  The prototype's `start` and `length` values are UTF-16 positions in the rendered
  document, not source offsets.
- Production selection integration, keyboard traversal through controls,
  rich clipboard export, and complete screen-reader validation. Accessible text
  and a named QML control are exposed, but VoiceOver
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
