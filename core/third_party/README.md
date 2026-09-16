# Core dependencies

## Syntect

`syntect/` is a Git submodule of
[Craftward's Syntect fork](https://github.com/craftward/syntect), based on
[upstream Syntect](https://github.com/trishume/syntect). Cargo depends directly
on this local path. The submodule commit pins the source revision; there is no
separate Cargo Git revision or additional source snapshot to synchronize.
The MIT license is read from the submodule's `LICENSE.txt` for application
license resources.

The current highlighting integration was developed against upstream commit
`4aa78031e93ebd3e0be7278120d0bd9d2508b1a3`. Its package version is still 5.3.0,
but its parser includes features added after the 5.3.0 release. In particular,
the maintained syntax packages and replay-aware driver depend on syntax
inheritance, `branch` / `fail`, `ParseLineOutput`, and `ParseState::is_speculative`.
The original 5.3.0 release is not an interchangeable implementation.

The incremental driver additionally requires two fork interfaces:

- `ParseLineOutput::replay_ranges` associates consecutive groups of corrected
  operations with absolute, zero-based, half-open source line ranges. Multiple
  failures during one parse call may replay the same range more than once.
- `ParseState::relocate_line` safely updates the next-line counter of a settled
  checkpoint after line insertion or deletion. Active branches or replay buffers
  must prevent relocation, and the first-line invariant must be preserved.

Maintain parser changes and their upstream-level regression tests in the fork.
Application-level full-snapshot and incremental regressions belong in
`ward-highlighting`. Recorded scope operations must preserve their execution
order and valid UTF-8 positions, including nested and repeated cross-line replay.

The fork replays buffered text with its original source line numbers and history
positions. Each replay starts with the complete saved line prefix, allowing
subsequent failures to rewind that same operation buffer. Descendant branches
from discarded alternatives are removed, and corrected lines are published in
execution order so nested replays supersede earlier corrections. Public parser
regressions live in `syntect/tests/cross_line_replay.rs`; the application QML
corpus and document-edit regressions live in `ward-highlighting/tests/editor_files.rs`.
