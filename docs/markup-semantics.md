# Markup Semantic Contract

The inline and message-selection experiments established that explicit semantics
can drive native text layout, inline interactions, and selection independently
of materialized text items. This contract introduces the production data needed
by that renderer. The optional `semantic` timeline renderer now consumes this
contract through a production native text adapter. The default `current`
renderer continues to use the legacy source-preserving path.

## Entry Points and Ownership

`ward_markup::parse_semantic(source, format)` parses a complete message snapshot.
`ward_core_markup_parse_semantic` exposes the same operation through the app-only
C interface and returns an owned `ward.markup.v1.SemanticDocument` protobuf.
Destroy its buffer with `ward_core_owned_buffer_destroy`. Input is UTF-8; empty
input is valid. Invalid UTF-8 or a missing nonempty source returns an error.
Parsing is synchronous, retains no state, and performs no text layout.

The legacy `parse` / `ward_core_markup_parse` interface continues to return the
source-preserving `Document` used by existing renderers. Its bounded tail parsing
is unchanged. The new payload has a separate type and entry point so consumers
cannot accidentally treat decoded semantics as Markdown source.

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

## Identity and Text Positions

Block IDs combine the root kind and its starting source byte position. Node IDs
combine kind and block-relative starting byte position, with a local occurrence
suffix when required for uniqueness. A persistent address consists of the
owning message/document identity, block ID, and node ID. IDs are opaque strings.

On append, nodes whose kind and starting position remain unchanged retain their
IDs. Completed unaffected blocks remain equal. Completing delimiters or adding
a reference definition may reinterpret earlier content and replace nodes; this
is a semantic update, not an identity guarantee across arbitrary edits. Snapshot
indices are never persistent identities. Selection reconciliation for replaced
nodes and document generations belongs to the subsequent message-selection
integration.

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

`MarkupDocumentModel.semanticModel` lazily creates a `MarkupSemanticModel` for
the optional renderer. It coalesces complete-message snapshots on workers and
discards obsolete generations. An empty initial snapshot remains an empty native
segment; it never requests synchronous legacy parsing or materializes an entire
message through the legacy repeater. The legacy parser and `renderModel` remain
available to the default renderer.

Semantic data is grouped at content boundaries, with an 8 KiB source target and
at most eight ordinary top-level blocks or sixteen immediate list items/table
rows per group. Long top-level lists and tables therefore span several segments.
An indivisible paragraph, cell, or nested structure can exceed the byte target;
this is not a strict per-segment memory limit. Arbitrary character slicing and
whole-history text layouts are not introduced. Grouping avoids creating a native
document and QML row for every short paragraph or list item.

The `semanticSegment` role carries a `SemanticDocument` containing only that
group's blocks. Original block IDs, node IDs, and decoded-text mappings are
retained. Split outer list/table containers describe the selected child range;
their ordered-list start is adjusted while child identities remain unchanged.
Unchanged completed groups compare equal. Updates reconcile model rows locally
and leave equal materialized documents and their native selections untouched.
An affected group is rebuilt; preserving selection across a changed group or
across delegate destruction requires the subsequent logical selection layer.

`MarkupTextDocument` writes typed nodes directly into a materialized TextEdit's
`QTextDocument`. It handles paragraphs, headings, quotes, lists and task markers,
tables with equal-width aligned columns, rules, nested emphasis, inline code,
resolved links, literal inline HTML, and annotation labels. Native Qt shaping,
wrapping, link hit testing, and selection operate on decoded UTF-16 text.
Top-level code retains its syntax highlighter and copy toolbar. List/table groups
use native text layout rather than a QML object per cell or inline node.

Document replacements and style refreshes use a single cursor edit transaction
with layout enabled. TextEdit measures its content when `contentsChanged`
arrives; suspending layout while emitting that notification can leave a zero
implicit height even after the document has been laid out. Palette changes on
window activation must therefore preserve the measured height and adjacent
segment positions throughout the update.

Images, footnotes, admonitions, opaque unsupported nodes, and ordered starts that
Qt cannot represent use a literal source fallback for their group. This preserves
content but does not promise visual parity with the legacy Markdown adapter.
Annotation labels are styled text; resolving their index, activating references,
attaching controls, and displaying tooltips/popovers remain separate work.

The adapter owns no timeline coordinates, scrolling correction, global cache,
or offscreen text document. The shared viewport continues to own materialization,
measurement, and movement-end geometry transactions. Run the integrated path with
`CRAFTWARD_TIMELINE_RENDER_BENCHMARK_RENDERER=semantic task app:run BUILD_TYPE=Debug`.

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
geometry, and replacements between paragraphs, lists, and tables. Existing
viewport identity, shutdown, and scroll-settlement regressions remain required.

The next submission can add message-wide logical selection and real interactions.
It must retain viewport materialization limits, stable identities, local model
notifications, and the established scroll geometry regressions. Streaming
selection, overlay pooling, expanded keyboard/accessibility behavior, rich
clipboard export, and interactive popovers remain open. Concentrated frame-time
optimization remains deferred under the user's existing decision.
