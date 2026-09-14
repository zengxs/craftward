# Codex User Message Presentation

Codex client envelopes are interpreted in the Codex message adapter before
Markdown parsing or display trimming. `ward_codex::parse_annotated_user_message`
recognizes response annotations and returns structured selections plus a verbatim
request-body slice. This operation has no dependency on `ward-markup`.

The app-facing `Message.text` retains the original input. A successful match adds
`annotated_user_message` with the request body, ordered annotations, optional
comments, and optional source metadata. History snapshots and live updates use
the same message projection. Agent messages do not undergo envelope recognition.

## Supported Envelope

Recognition requires the original text to start with a newline followed by
`# Response annotations:` and a newline. Exactly one nonempty instruction line
follows; its wording is not interpreted. The next line is
`<response-annotations>`, followed by one valid, nonempty JSON array and a closing
`</response-annotations>` line. JSON may span multiple lines. Only blank separator
lines may occur before `## My request:` or `## My request for Codex:` and its
terminating newline. The rest of the input is the request body, including its
leading and trailing whitespace. Structural line endings are LF.

Every annotation requires nonempty string `text`. `annotation` is an optional
string. Unknown object fields are accepted. Invalid required fields reject the
whole envelope; entries are never filtered or renumbered. Array positions define
one-based annotation indices. Invalid or missing `source` metadata does not
discard the selection or comment. Valid source metadata contains a nonempty
`messageId` and nonnegative integer offsets with `endOffset > startOffset`.
Those offsets belong to the originating client's rendered UTF-16 text and must
not be used directly as local Markdown source or selection positions.

Additional client context between the annotations and request heading is not
recognized by this adapter. Missing delimiters, incomplete snapshots, malformed
JSON, and unsupported layouts retain the complete original input. A matching
example embedded in ordinary text or a code fence is not an envelope.

## Timeline Presentation

Each user message retains one source row. The annotation collection is metadata,
while only the request body enters the asynchronous Codex Markdown document.
Viewport segmentation preserves the owning `sourceEntryId`. A count chip appears
above the first body segment. Empty bodies show the chip without an empty message
bubble. Message actions appear after the last segment; whole-message copy includes
numbered selections, comments, and the request body without the client envelope.

One interactive `Popup.Window` belongs to the timeline view. The count chip opens
only its own input's collection. Inline references open the same popup using
native text hit testing, including wrapped labels and table cells. Quotes and
comments are selectable plain text. The popup sizes to the widest unwrapped text,
up to 520 logical pixels, and long content wraps and scrolls within it. Hover
opens after a short delay; a grace period allows moving into the popup. Clicking keeps it open
until Escape, an outside press, or invalidation. Scrolling the timeline, switching
conversations, and recycling or changing an anchor dismiss the popup. Opening it
does not affect row geometry or require the source input's delegate to exist.

The popup may extend beyond the main window. `PopupPositioner` selects the screen
at the anchor, fits the popup within that screen's available geometry, and maps
the result back to the popup parent's coordinates. It prefers opening below the
anchor, switches above when that fits better, and reduces the scrollable height
when space is limited. Screen coordinates use Qt logical pixels, including
negative monitor origins. Text is laid out at the fitted width before determining
the popup height and opening direction. Layout is resolved before opening the native
window. Moving or hiding the owner window dismisses the popup; hover transfer and
text selection operate within the popup's own window.

Native popup windows can clear the source control's hover state without moving
the pointer. Pending dismissal therefore checks the system cursor against the
anchor rectangle and the entire popup panel, including its padding. The leave
grace period starts only after the pointer is outside both regions, preventing
repeated close/reopen cycles and allowing movement across the gap. The window's
`NoDropShadowWindowHint` is cleared to enable the system shadow; the custom
background only draws the rounded panel.

## Reference Resolution

An annotation index is local to one input array. Guidance can submit another
array in the same turn and restart numbering at one. Incoming history preserves
those indices. An assistant reference searches earlier user inputs in its own
turn; later inputs and other turns are excluded.

- No match: show an explicit unavailable-details message.
- One match: show that annotation directly.
- Multiple matches: show every candidate in one popup, newest input first, with
  the annotation number and input ordinal identifying each source.

Candidates are deduplicated by owning user entry ID and annotation index, not by
selection text. Two inputs with identical payloads remain distinct. The optional
`source.messageId` identifies the previously selected assistant response, not the
owning input, and cannot resolve ambiguity on its own. Input ordinals count all
user messages in the turn, including guidance without annotations.
The input ordinal is shown only when an inline reference matches multiple
sources. A single match and an input's own annotation collection omit it.

This implementation reads existing annotations. The composer does not yet create
or send annotation collections. A future authoring feature may allocate unique
indices within a turn, but the history renderer must retain this ambiguity fallback.

## Markdown Preview Boundary

Envelope recognition remains independent of the generic markup parser. Ordinary
`SourceFormat::Markdown` interprets Markdown only. The timeline opts into
`SourceFormat::CodexMarkdown`, which additionally recognizes inline annotation
references and standalone review directives. The native renderer exposes numeric
annotation hits and their geometry; it has no dependency on conversation lookup or
popup state. An editor preview can use ordinary Markdown without either Codex layer.
