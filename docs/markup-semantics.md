# Markup Semantic Contract

The inline and message-selection experiments established that explicit semantics
can drive native text layout, inline interactions, and selection independently
of materialized text items. This contract introduces the production data needed
by that renderer. The timeline consumes this contract through its sole production
renderer, a native semantic text adapter.

## Entry Points and Ownership

`ward_markup::parse_semantic(source, format)` parses a complete message snapshot.
`ward_core_markup_parse_semantic` exposes the same operation through the app-only
C interface and returns an owned `ward.markup.v1.SemanticDocument` protobuf.
Destroy its buffer with `ward_core_owned_buffer_destroy`. Input is UTF-8; empty
input is valid. Invalid UTF-8 or a missing nonempty source returns an error.
Parsing is synchronous, retains no state, and performs no text layout.

The source-preserving parser, its `Document` wire payload, and the legacy
renderer have been removed. Semantic snapshots are the only markup parse path.

Semantic parsing requires the complete message because reference definitions
can resolve links in earlier blocks. It is a snapshot operation, not an
incremental parser. Consumers must schedule it away from interaction handlers
and reconcile affected semantic content. They must not independently
parse arbitrary fragments and claim document-wide reference resolution.

## Content and Rendering Segments

A `SemanticBlock` represents one top-level Markdown structure. A paragraph,
list, table, or code block can each be one semantic block. The renderer may use
multiple segments to display a block; a large table does not imply one enormous
text document or permanent materialization of every cell.

Each block contains a flat preorder sequence of typed nodes. `parent_index` is
absent on its root and points to an earlier node for every descendant. This
preserves nesting without recursive protobuf messages or layout references.
Consumers must use node identity, not the parent index, for persistent state.

The contract represents paragraphs, headings, quotes, GFM admonitions, lists,
items, table alignment/rows/cells, emphasis, strong text, strikethrough, links,
images, code, line breaks, rules, task markers, and footnote labels. Link targets
and titles are resolved by the parser; labels retain their nested inline nodes.
Reference definitions need not appear as visible blocks. Code preserves the
parser's decoded whitespace and line breaks; display trimming is a renderer
policy. Plain-text input performs no Markdown or directive interpretation.

Inline HTML is literal text. Unsupported container syntax, including HTML
blocks, is preserved as an opaque source-text node; its descendants are not
duplicated. A consumer must retain a fallback for blocks containing unsupported
nodes. This is a content contract, not a claim that every represented feature
already has a production renderer or complete reference interaction.

## Codex Annotations

Ordinary Markdown text recognizes `:codex-annotation{index="4"}` as a typed
annotation with a positive 32-bit index and visible label `[4]`. Spaces or tabs
around `index`, `=`, and the closing brace are accepted. The original directive
remains recoverable from its source range. Resolving the index to a review object
belongs to the caller; parsing does not open a popup or perform navigation.

Escaped directives and directives inside inline code, fenced/indented code,
link labels, image labels, or opaque HTML remain text. Unknown directive names,
unknown attributes, zero/overflowing indices, and incomplete syntax also remain
text. A colon produced by an entity is not reinterpreted as directive syntax.
Recognition is deliberately limited to this known extension.

## Code Comments

A standalone top-level paragraph can contain one complete review directive:

```text
::code-comment{title="[P2] Preserve block spacing" body="Keep the **ordinary** gap after the table." file="app/src/example.cpp" start=122 end=125 priority=2}
```

`title`, `body`, and `file` are required, nonempty, double-quoted strings.
`start` and `end` are optional positive line numbers within Qt's signed integer
range; `end` requires `start` and cannot precede it. An omitted end means the
start line alone. Optional `priority` accepts 0 through 3, including an explicitly
present zero. Numeric attributes may be bare or double quoted. Unknown or
duplicate attributes and invalid ranges keep the entire paragraph literal.

Quoted attributes decode `\"`, `\\`, `\n`, `\r`, and `\t`; other backslash escapes
remain available to the embedded Markdown parser. The decoded body is parsed as
a complete Markdown document with its own reference definitions. Its ordinary
flat nodes become children of the typed comment node. Their byte provenance maps
back through attribute escapes to the original message. Title and file metadata
also retain decoded text and source mappings.

Recognition happens before Markdown tokenizes attribute contents. Incomplete
streaming directives remain literal until complete, and completed comments retain
their identities and payloads when following blocks are appended. Separate cards
require separate paragraphs. Use escaped newlines inside the body for paragraph
breaks. Directives inside lists, quotes, code, HTML, link/image labels, or another
comment body remain inert; nested review cards are not supported.

The native card shows an optional priority badge beside the title, followed by
the file location and an expanded Markdown body. It removes a leading `[P2] `
from the title only when it matches the explicit priority and leaves a nonempty
title; it never infers priority from the title. At a 14 px base font, the title uses 15 px bold text,
the body 13 px text, and metadata 11 px text. Only the title reserves badge width;
the location and body use the full padded content width. Badge and title text
share the actual first-line baseline from TextEdit cursor geometry and the
existing QTextLayout ascent, including when the title wraps.
`MarkupPartsView` renders ordinary text and table parts for both the surrounding
message and the embedded body, without a recursive card/component dependency.

File paths resolve against the selected conversation's known working directory,
and locations within that directory display a relative path with any line range.
A leading workspace directory name is accepted as a qualifier: for a directory
ending in `Craftward`, `Craftward/app/file.cpp` addresses `app/file.cpp` in that
directory. Matching uses a complete path segment. A leading `./` explicitly
addresses an ordinary relative path, including a nested directory with the same
name as the workspace. Absolute paths outside the directory remain absolute in
the label. Navigation and display preserve symlinks and parent-directory segments;
they do not lexically collapse `link/..` before the filesystem resolves it.
Changing the directory reprojects existing message models. Without directory
metadata, relative paths remain unresolved rather than inheriting the application
process directory. The location tooltip retains the full path. File clicks emit a typed
path/start/end navigation request. The current window handler opens an existing
absolute local file with the system default application. It resolves the target
through the filesystem at click time before passing a canonical file URL to the
operating system. It reports failure and does not jump to a line. The line range
remains available for a future editor.

## Identity and Text Positions

Block IDs combine the root kind and its starting source byte position. Node IDs
combine kind and block-relative starting byte position, with a local occurrence
suffix when required for uniqueness. A persistent address consists of the
owning message/document identity, block ID, and node ID. IDs are opaque strings.

On append, nodes whose kind and starting position remain unchanged retain their
IDs. Completed unaffected blocks remain equal. Completing delimiters or adding
a reference definition may reinterpret earlier content and replace nodes; this
is a semantic update, not an identity guarantee across arbitrary edits. Snapshot
indices are never persistent identities. The message selection owner reconciles endpoints after each accepted snapshot.
It retains surviving node identities, clamps shortened text to grapheme boundaries,
and clears a selection whose endpoint was replaced.

Source ranges are half-open UTF-8 byte ranges in the complete source. Container
ranges are normalized to cover their descendants before IDs are assigned. This
includes task markers that the Markdown parser places outside its paragraph's
reported range in loose lists. Text
mapping ranges are half-open UTF-16 code-unit ranges local to one decoded text
value. They are not offsets in a segment, QTextDocument, grapheme sequence, or
Markdown source. The Qt adapter can use decoded QString positions directly and
must apply its own layout/selection and grapheme-boundary rules.

A verbatim mapping means its source slice equals its decoded text slice. A
consumer can translate an interior character boundary by decoding that source
slice, never by adding UTF-8 and UTF-16 offsets. A replacement mapping associates
the entire decoded range with its source token and promises no interior source
cursor mapping. Entities, normalized inline code, and annotation labels are
examples. Source ranges may include syntax or omit non-rendered delimiters;
they are provenance, not a lossless source-edit script. Keeping logical selection
on node ID plus decoded text offset avoids requiring a fabricated source cursor.

## Native Timeline Adapter

`MarkupDocumentModel` owns the semantic segments for one message. It coalesces
complete-message snapshots on workers and discards obsolete generations. A
timeline message creates its document model when first requested. The viewport
model currently requests all loaded message models while building its segment
index; parsing is not yet scheduled by viewport proximity. There is one parse
pipeline per document, with no parallel legacy parse. An empty initial snapshot
remains an empty segment until the worker completes. It never requests
synchronous parsing or materializes an entire message through a repeater.

Semantic data is grouped at content boundaries, with an 8 KiB source target and
at most eight ordinary top-level blocks or sixteen immediate list items/table
rows per group. Long top-level lists and tables therefore span several segments.
An indivisible paragraph, cell, or nested structure can exceed the byte target;
this is not a strict per-segment memory limit. Arbitrary character slicing and
whole-history text layouts are not introduced. Grouping avoids creating a native
document and QML row for every short paragraph or list item.

The `semanticSegment` role retains the group's typed semantic document, including
original block IDs, node IDs, and decoded-text mappings. The `renderParts` role
carries a value-only projection produced by the same worker. A part is a text
surface, a table with rows of independent text surfaces, or a code-comment card
with metadata surfaces and body parts. Cards occupy their own segments and keep
the embedded body together as an indivisible structure. The projection
contains text runs and formatting values, never a text document or measured
geometry. Split outer list/table containers describe the selected child range;
their ordered-list start is adjusted while child identities remain unchanged.
Unchanged completed groups compare equal and reconcile without resetting rows.
Ordered lists share their maximum marker digit count across segments. Appending
an item that adds a digit updates earlier render parts to keep the text aligned;
their semantic identities and selection endpoints remain stable.

`MarkupTextDocument` writes one projected surface into a materialized TextEdit's
`QTextDocument`. It handles paragraphs, headings, quotes, lists and task markers,
rules, nested emphasis, inline code, resolved links, literal inline HTML, and
annotation labels. Native Qt shaping, wrapping, and link hit testing operate on
decoded UTF-16 text. Top-level code retains its syntax highlighter, horizontal
scrolling, and copy toolbar.

List indentation starts at two font ems and grows when the widest digit advance,
marker suffix, and marker gap require more room. Native text, continuation
paragraphs, and nested tables share this font-dependent indentation. Tight lists
have no extra item gap; loose lists add ten pixels. Direct paragraphs within
list items distinguish loose lists from tight lists without inheriting a nested
list's spacing. The complete outer list's item gap is retained before splitting,
so a segment containing only empty items keeps the same spacing as the rest of
its list. Item gaps also apply after a table that ends the preceding sibling item
and between segments of the same list. A table followed by a new independent or
nested list retains the ordinary eight-pixel block gap.
Unordered markers cycle through a filled disc, hollow circle, and square at
successive nesting levels. Numbering and hanging text alignment remain owned by
Qt's native list layout. A marker decoration uses the same Qt Quick text renderer
as the body and aligns each marker with the first line's baseline. Its advance
ends 0.7 em before ordered-list text and 0.9 em before unordered-list text, with
mirrored placement for right-to-left paragraphs. Native marker ink is transparent
to avoid drawing it twice; body characters and selection offsets are unchanged.

Solid bullet markers use a heavy font weight at the body font size, producing a
diameter of approximately 0.3 em with the macOS system font. This preserves their
vertical alignment. Marker formatting is scoped to the block character format;
body runs, numbered markers, hollow circles, squares, and task checkboxes retain
their existing fonts.

Once a surface is available, the native adapter exclusively owns its document
content. Code delegates may display fallback text while waiting for the surface;
that binding is disabled without restoring a previous value when the adapter
takes over. Code TextEdits keep PlainText format throughout their lifetime so
delayed payloads cannot trigger HTML reinterpretation or discard line breaks.

Syntax highlighting distinguishes source changes from document notifications.
Selection and formatting may emit contentsChanged without changing plain text;
those notifications retain existing syntax spans. Actual source, language, or
theme changes still invalidate spans and schedule asynchronous highlighting.

Code blocks preserve syntax foreground colors inside the selection.
`MarkupSelectionBackground` paints beneath the existing TextEdit while its native
selection foreground and background are transparent. The TextEdit still owns
the projected selection and layout; this item adds no text document or input
handler. It derives visual rectangles from public QTextLine glyph ranges,
including bidi runs and partial ligatures, and accounts for tabs and selected
paragraph separators. Layout changes refresh those rectangles, and the existing
code viewport supplies scrolling and clipping. Transparent native selection also
suppresses selected text decorations, so this layer restores syntax underlines
from the existing glyph runs and formats. Its small Qt Quick Shapes component
uses curve strokes to retain Qt's antialiasing and is created only for decorated
selected runs. Prose and table cells retain their normal selection colors.

Selected hard breaks and paragraph separators receive a visible cell one space
wide, using the break's font. Selection backgrounds share the added leading
between adjacent selected lines, including empty lines. Prose and table cells
keep this connection within each paragraph; their overlay supplements native
selection with leading and break markers. Within a code surface, the connection
also spans QTextBlock boundaries because each source line is a separate block.
These backgrounds do not change selection endpoints or copied whitespace.

`MarkupTable` coordinates equal column widths, column alignment, and the maximum
cell height in each row. Every cell uses the same `MarkupSelectableText` and
native text adapter as prose. Only materialized timeline segments create cells;
long top-level tables still split at the existing structural limits. Nested
tables retain their quote/list indentation and appear after preceding prose.
There is no `QTextTable`, alternate table renderer, or private Qt Quick dependency.
The mouse-release image regression remains in both Qt and native font modes.

`MarkupDocumentModel` owns one `MarkupSelection` for the complete message. Its
endpoints contain block-scoped node identities and decoded UTF-16 offsets, snapped
to grapheme boundaries. Synthetic identities cover block separators and literal
or top-level code surfaces. The selection survives delegate destruction and
reprojects when text is materialized or restyled. Appending text retains existing
endpoints; replacing an endpoint's semantic node clears the selection.

The selection index and native renderer consume the same text projection. Plain
text copy uses newlines between blocks and table rows, tabs between cells, and
preserves internal code whitespace. Lists copy their text without generated
markers. Annotation labels copy their visible text. Review cards copy the badge
and title on one line, the displayed file location on the next, and the decoded
body after a blank line. These fields participate in the same message selection
as surrounding prose and tables. Internal file links use the typed navigation
request rather than passing an internal URL scheme to the operating system.
Copy does not materialize
offscreen documents and does not include another message.

The native adapter maps between semantic UTF-16 offsets and document positions
when Qt collapses CRLF to a single paragraph separator. Hit testing and word
selection convert to semantic offsets; projected selection ranges convert back
to document positions. The semantic text retains its original line endings for
copying. The map records only collapsed CRLF pairs within each inserted run and
is rebuilt when the surface changes, independently of document materialization.

`MarkupSelectionHost` registers live surfaces and routes mouse dragging,
shift-click, word selection, Copy, Select All, and Escape to the message owner.
Dragging near the viewport edge scrolls and extends the selection once pointer
movement reaches the platform drag threshold. A stationary press does not start
edge scrolling. Accepted selection gestures share the viewport movement
lifecycle: they stop live-tail following, defer geometry changes, and settle only
after both selection dragging and Flickable motion end. The release endpoint is
committed before settlement.
Starting a
selection in another message clears the previous one; extending an existing
selection stays in its original message. Code toolbars and scrollbars retain
their own input handling. Links activate on a click without a drag or selection.

Document replacements and style refreshes use a single cursor edit transaction
with layout enabled. TextEdit measures its content when `contentsChanged`
arrives; suspending layout while emitting that notification can leave a zero
implicit height even after the document has been laid out. Palette changes on
window activation must therefore preserve the measured height and adjacent
segment positions throughout the update.

Images, footnotes, admonitions, opaque unsupported nodes, and ordered starts that
Qt cannot represent use a literal source fallback for their group. This preserves
content without claiming complete visual support for those structures.
Unsupported content inside a review body preserves the complete directive as
literal source, following the same group fallback rule.
Annotation labels are styled text; resolving their index, activating references,
attaching controls, and displaying tooltips/popovers remain separate work.

The adapter owns no timeline coordinates, scrolling correction, global cache,
or offscreen text document. The shared viewport continues to own materialization,
measurement, and movement-end geometry transactions. Run the application with
`task app:run BUILD_TYPE=Debug`. The renderer selection environment variable has
been removed; benchmark results identify the renderer as `semantic`.

## Validation and Next Integration

Rust tests exercise the public parser with nested inline content, Unicode
mapping, table structure, document-wide references, directive escape/malformed
cases, code whitespace, and append/reinterpretation behavior. C-interface tests
decode the actual protobuf and verify ownership/error behavior and optional or
oneof zero/false values. The Qt model test also decodes Rust's semantic payload
using Qt Protobuf and compares UTF-16 positions with real QString lengths.

Native Qt tests now cover production QML wiring, resolved links, nested code
formatting, Unicode selection, table alignment and inline content, structural
grouping, stale snapshots, layout release, cold-snapshot routing, palette refresh
geometry, and replacements between paragraphs, lists, and tables. Selection tests
drive real mouse events across prose, code, and cells, verify offscreen copy and
streaming reconciliation, and destroy/rematerialize selected table delegates.
Deferred code tests create the delegate before its payload arrives and verify
both assignment orders, literal characters, whitespace, measured height,
selection, palette refresh, and streaming append. Existing
viewport identity, shutdown, and scroll-settlement regressions remain required.
List regressions cover tight and loose bullet, numbered, and task lists, nested
markers, font-size changes, table alignment, and streaming across a marker digit
boundary while retaining the projected selection. Empty-item segments retain the
complete list's spacing when text is appended in a later segment. Table boundaries
distinguish new lists from sibling items. Rendered numbered markers are
compared with inline numbers beside Chinese bold text to verify their baseline
at multiple font sizes. Selection must not move the marker or reveal duplicate
native ink in either Qt or native rendering.
Code-selection tests verify that syntax formats survive mouse press, dragging,
release, and deselection in both Qt and native font rendering, including lines
outside the selection. Image checks confirm that selected tokens retain their
foreground colors in light and dark themes. Background geometry is compared
against Qt's native selection for partial words, spaces, tabs, bidi text,
ligatures, and wrapping. Further checks cover blank lines, font changes,
horizontal scrolling and clipping, streaming append, and deselection.
Complete and partial selections also retain bold, italic, and underlined syntax
in both font rendering modes, allowing one 8-bit color step of compositing
roundoff where underlines intersect glyphs.
Review-card regressions cover attribute decoding and source provenance, optional
priority, title deduplication, embedded Markdown, streaming, directory changes,
native baseline alignment, full-width body layout, selection across fields,
typed file-click routing, and local file URLs containing spaces and `#`.

The next submission can add real inline controls and reference interactions.
It must retain viewport materialization limits, stable identities, local model
notifications, and the established scroll geometry regressions. Expanded keyboard
navigation and accessibility selection, rich clipboard export, overlay pooling,
and interactive popovers remain open. Concentrated frame-time optimization
remains deferred under the user's existing decision.
