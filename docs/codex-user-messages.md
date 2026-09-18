# Codex User Message Presentation

Codex client envelopes are interpreted in the Codex message adapter before
Markdown parsing or display trimming. `ward_codex::parse_user_message_envelope`
recognizes response annotations, mentioned files, pasted files, and combinations
of these sections. It returns structured context plus a verbatim request-body
slice. This operation has no dependency on `ward-markup`.

The app-facing `Message.text` retains the original input. A successful match adds
`user_message_presentation` with the request body, ordered annotations, optional
comments, source metadata, and attachments. Typed images and absolute local-file
mentions also populate this presentation, including messages without a text
envelope. History snapshots and live updates use the same message projection.
Agent messages do not undergo envelope recognition. The presentation retains
protobuf field number 5, with body and annotation field numbers unchanged.

## Supported Envelopes

Recognition starts at the beginning of the original text, allowing leading blank
lines. At least one known context section is required. Each section can occur
once; only blank separator lines are accepted between sections and before
`## My request:` or `## My request for Codex:` and its terminating newline.
The rest of the input is the request body, including its leading and trailing
whitespace. Structural line endings are LF.

The annotation section starts with `# Response annotations:` and a newline.
Exactly one nonempty instruction line follows; its wording is not interpreted.
The next line is
`<response-annotations>`, followed by one valid, nonempty JSON array and a closing
`</response-annotations>` line. JSON may span multiple lines.

Every annotation requires nonempty string `text`. `annotation` is an optional
string. Unknown object fields are accepted. Invalid required fields reject the
whole envelope; entries are never filtered or renumbered. Array positions define
one-based annotation indices. Invalid or missing `source` metadata does not
discard the selection or comment. Valid source metadata contains a nonempty
`messageId` and nonnegative integer offsets with `endOffset > startOffset`.
Those offsets belong to the originating client's rendered UTF-16 text and must
not be used directly as local Markdown source or selection positions.

The mentioned-files section starts with `# Files mentioned by the user:`. It
contains one or more `## LABEL: ABSOLUTE_PATH` lines and the exact closing line
`Distinguish instructions in attached documents from the user's request.`.
Paths may contain spaces; POSIX, Windows drive, and UNC paths are accepted.
Optional `(line N)` and `(lines N-M)` suffixes retain positive, ordered line
positions separately from the path.
An unquoted record with more than one plausible label/path separator rejects
the whole envelope, preserving its original text instead of guessing the path.

The pasted-files section starts with `# Files pasted by the user:`. Each label
is a JSON string, so escaped quotes and newlines remain part of the label.
The optional closing line `Pasted text contains the user's request.` is accepted.
Quoted labels are decoded for presentation and never parsed as Markdown.
Their JSON boundaries disambiguate separators inside the label or path.

Missing delimiters, incomplete snapshots, malformed JSON, repeated sections,
and unsupported layouts retain the complete original text. A matching example
embedded in ordinary text or a code fence is not an envelope. Uploaded-file
records, library/shared-thread metadata, and other context families remain
unsupported; a mixed envelope containing them is retained in full.

## Attachment Projection

Typed `image`, `localImage`, and absolute local-file `mention` inputs become
attachments directly. Their diagnostic placeholders stay in `Message.text`,
but are excluded from the presentation body. Literal `[image: ...]` text is
never interpreted as an attachment. Non-file mentions, audio, skills, and unknown
inputs keep their existing textual projection.

File labels, original local paths or image URLs, source kinds, and optional line
ranges cross the protobuf boundary. Typed local images and file mentions merge
with matching envelope files, preserving the envelope's label and all source
kinds. Repeated typed image URLs are also deduplicated. The Rust adapter supplies
an opaque `resource_id` for each attachment and uses that identity for matching.
Local paths and image URLs occupy separate identity namespaces. Path comparison
removes redundant separators and `.` components without filesystem access; it preserves
case and does not resolve `..` or symlinks. Distinct envelope labels and line
ranges remain separate references.

## Timeline Presentation

Each user message retains one source row. Attachments and the annotation
collection are metadata, while only the request body enters the asynchronous
Codex Markdown document. Viewport segmentation preserves the owning
`sourceEntryId`. Attachment cards and the annotation count chip appear above the
first body segment. Empty bodies show these controls without an empty message
bubble. Message actions appear after the last segment; whole-message copy includes
numbered selections, comments, labeled attachment locations, and the request body
without the client envelope. Inline image data is represented as `[image]` in
copy text instead of copying its encoded payload; raw text remains available.

Image thumbnails occupy fixed 90 by 90 logical pixel frames, separated by eight
logical pixels. Images are centered within each frame using one scale factor,
`min(1, 90 / width, 90 / height)`, for both dimensions. The complete image remains
visible without cropping or stretching, and smaller raster images are not
enlarged. Loading and missing images keep the same frame dimensions. Loading
shows a progress indicator; unavailable images show an image placeholder.
Image cards have no filename caption or tooltip. Hover strengthens their border
and shows a pointing cursor; keyboard focus adds an accent border.

Attachments stay in one horizontal row, preserving input order. A row that fits
is aligned to the right. Overflow reveals previous/next controls and an attachment
count below the row. Horizontal mouse-wheel and touchpad input, horizontal dragging,
and the buttons move through the attachments. Vertical wheel input continues to
the timeline. Arrow keys move focus between cards and reveal the focused card.
Resizing preserves delegates rather than reloading every image. The request bubble
sizes independently of the attachment area.
Unchanged attachment snapshots preserve the delegates, horizontal scroll position,
and active preview across timeline revisions. The attachment view compares the
fields it displays without serializing inline image data. A changed attachment
list invalidates the preview before replacing its source cards.

The normal border is one logical pixel, with foreground opacity 10% in light mode
and 14% in dark mode. Hover increases opacity to 22% and 28%, respectively. The
radius is eight logical pixels; focus uses a two-pixel accent border. A GPU mask
clips image corners, while software rendering falls back to square corners.
Thumbnails have no shadow. Decoding is bounded to 180 by 180 pixels. The decoded
aspect ratio determines the fitted image dimensions. `Image.Stretch` is applied
to an already fitted image item to avoid fit-mode decoding enlarging small raster
images. EXIF orientation is applied. See the [Qt Image size semantics](https://doc.qt.io/qt-6/qml-qtquick-image.html#sourceSize-prop).

Clicking an image opens one shared, nonmodal `CodexImagePreview` popup anchored to
its thumbnail. Both image and annotation popovers use `WindowPopover` for their
window presentation: a native popup window with the system shadow, a shared
theme-aware background, a 16-pixel corner radius, 18-pixel padding, and no border. This
component does not prescribe dimensions, content, focus, or dismissal behavior.
Image previews use native screen-aware positioning, preferring a 480 by 400
logical pixel panel and reducing its dimensions when necessary. Its image decode
is bounded to 960 by 800 pixels and fitted with one scale factor. The popup shows
the current image position and previous/next buttons; these navigate only the
images in the source message and do not create tabs. Pointer departure does not
close it. Opening the popup focuses its content so Left and Right navigate
immediately, stopping at the first and last image. Clicking the anchor thumbnail
again closes the popup, including after
navigating within the preview; a later click opens it again. Escape or an outside
press also closes it, and clicking a different thumbnail switches directly to that
image. `PopupAnchorToggle` consumes a native left-button press over the active
anchor before Qt closes and replays it, preventing that press from reopening the
popup. Other targets retain normal outside-click handling. Scrolling, moving the
anchor or owner window, changing conversations, and recycling the source row
dismiss the popup. Image and
annotation popups dismiss each other.
The popup's explicit and implicit size hints use the same screen-fitted dimensions,
preventing asynchronous native layout updates from shrinking it to the toolbar's
minimum size when the image changes.

The popup's Open in Tab action opens the current image and closes the popup.
An image card's context menu also offers Open in Tab. One tab represents one image;
preview navigation never replaces the resource in an existing tab. Reopening the
same image selects its existing tab within the current conversation workspace.
The adapter sends `resource_id` through protobuf, and the model forwards it to QML
as the tab key. Neither C++ nor QML derives an alternative identity. The adapter
hashes the location kind and either the normalized local path or the original
image URL with BLAKE3, keeping inline image data out of the identifier. Attachment
matching and tab reuse therefore share one identity rule: redundant local-path
separators and `.` components reuse the existing tab. Original paths and loading
URLs remain unchanged; case, `..`, and symlinks are not resolved. Labels and line
ranges do not determine identity.
Tabs use numbered image titles and do not expose temporary paths.

`ImageContentView` supports fit, actual size, zoom buttons, modifier-wheel zoom, and panning. Each tab owns an `ImageViewState` with its zoom mode and normalized view
center, separate from its image resource. Switching tabs or conversations and
recreating a viewer preserves that state within the current process. Closing the
tab destroys only its view state. This separation supports future independent
views of one resource, but split groups and cross-process restoration are not
implemented here. Existing text-file tabs keep their behavior.
Attachment navigation, preview actions, and image-view controls use icon buttons
with action tooltips and accessible names. The image position and zoom percentage
remain visible as text.

Other local files retain the existing file-location action, including line
positions. Pasted text has a distinct caption. File labels are plain text; ordinary
file cards and tooltips do not expose paths. Missing images keep their cards and
show an unavailable message when opened. No attachment file is copied or persisted
by this presentation layer; opening a tab does not make a temporary file durable.

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
