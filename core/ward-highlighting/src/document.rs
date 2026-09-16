// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::BTreeMap,
    ops::Range,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use syntect::highlighting::Highlighter as ThemeHighlighter;

use crate::{
    Error, Highlighter, Span, Theme,
    parser::{Checkpoint, Parser},
    push_span,
};

/// A replacement style range, including gaps that must return to plain text.
#[derive(Clone, Debug)]
pub struct HighlightBatch {
    pub revision: u64,
    pub configuration: u64,
    pub sequence: u64,
    pub range: Range<usize>,
    pub spans: Vec<Span>,
    pub syntax_name: String,
    pub language_recognized: bool,
    pub complete: bool,
    pub parsed_lines: usize,
}

struct PendingBatch {
    revision: u64,
    configuration: u64,
    sequence: u64,
    range: Range<usize>,
}

/// A worker-affine mirror of an editor document. Style changes remain pending
/// until acknowledged, even when an intervening edit makes a batch obsolete.
pub struct Document {
    engine: Arc<Highlighter>,
    source: String,
    starts: Vec<usize>,
    styles: Vec<Vec<Span>>,
    language: String,
    file_name: String,
    theme: Theme,
    syntax: Option<usize>,
    recognized: bool,
    revision: u64,
    configuration: u64,
    sequence: u64,
    parser: Option<Parser>,
    checkpoints: BTreeMap<usize, Checkpoint>,
    convergence_after: usize,
    pending: Option<Range<usize>>,
    published: Option<PendingBatch>,
    metadata_dirty: bool,
    parsed_lines: usize,
}

impl Document {
    pub fn new(
        engine: Arc<Highlighter>,
        source: &str,
        language: &str,
        file_name: &str,
        theme: Theme,
    ) -> Self {
        let starts = line_starts(source);
        let styles = vec![Vec::new(); starts.len()];
        let mut document = Self {
            engine,
            source: source.to_owned(),
            starts,
            styles,
            language: language.to_owned(),
            file_name: file_name.to_owned(),
            theme,
            syntax: None,
            recognized: false,
            revision: 0,
            configuration: 0,
            sequence: 0,
            parser: None,
            checkpoints: BTreeMap::new(),
            convergence_after: usize::MAX,
            pending: None,
            published: None,
            metadata_dirty: true,
            parsed_lines: 0,
        };
        document.restart();
        document
    }

    /// Applies exactly the next edit, using positions in the previous revision.
    pub fn edit(
        &mut self,
        from_revision: u64,
        start: usize,
        deleted: usize,
        inserted: &str,
    ) -> Result<(), Error> {
        let end = start.checked_add(deleted).ok_or(Error::InvalidEdit)?;
        if from_revision != self.revision
            || !self.source.is_char_boundary(start)
            || !self.source.is_char_boundary(end)
        {
            return Err(Error::InvalidEdit);
        }
        let next_revision = self.revision.checked_add(1).ok_or(Error::InvalidEdit)?;
        let first = self.line_at(start);
        let last = self.line_at(end);
        let old_count = last - first + 1;
        let start_offset = self.starts[first];
        let suffix_offset = self.line_end(last);
        let new_suffix = suffix_offset - deleted + inserted.len();
        let restart_line = self
            .parser
            .as_ref()
            .map_or(first, |parser| first.min(parser.next_line()));
        self.source.replace_range(start..end, inserted);
        if let Some(pending) = &mut self.pending {
            pending.start = map_offset(pending.start, start, end, inserted.len());
            pending.end = map_offset(pending.end, start, end, inserted.len());
        }
        let mut replacement = line_starts(&self.source[start_offset..new_suffix]);
        // A following cached line already represents this trailing boundary.
        if last + 1 < self.starts.len() && replacement.last() == Some(&(new_suffix - start_offset))
        {
            replacement.pop();
        }
        for offset in &mut replacement {
            *offset += start_offset;
        }
        for offset in self.starts.iter_mut().skip(last + 1) {
            *offset = *offset - deleted + inserted.len();
        }
        let new_count = replacement.len();
        self.starts.splice(first..=last, replacement);
        self.styles
            .splice(first..=last, vec![Vec::new(); new_count]);
        self.revision = next_revision;
        self.dirty(start_offset..new_suffix);
        self.metadata_dirty = true;
        let resolution = self.resolve();
        if resolution != (self.syntax, self.recognized) {
            self.restart();
        } else if self.syntax.is_some() {
            if old_count != new_count {
                self.checkpoints = std::mem::take(&mut self.checkpoints)
                    .into_iter()
                    .filter_map(|(line, state)| {
                        if line <= first {
                            Some((line, state))
                        } else if line > last {
                            let relocated = line - old_count + new_count;
                            state.relocate(relocated).map(|state| (relocated, state))
                        } else {
                            None
                        }
                    })
                    .collect();
                self.convergence_after = if self.convergence_after > last {
                    self.convergence_after - old_count + new_count
                } else {
                    self.convergence_after.min(first)
                };
            }
            let checkpoint = self
                .checkpoints
                .range(..=restart_line)
                .next_back()
                .map(|(_, state)| state.clone());
            self.parser = checkpoint.map(Parser::restore);
            if self.parser.is_none() {
                self.restart();
            }
            self.convergence_after =
                (first + new_count).max(self.convergence_after.min(self.starts.len()));
        }
        Ok(())
    }

    /// Configuration changes invalidate all styles but preserve the editor text.
    pub fn configure(&mut self, language: &str, file_name: &str, theme: Theme) {
        self.language = language.to_owned();
        self.file_name = file_name.to_owned();
        self.theme = theme;
        self.configuration += 1;
        self.restart();
    }

    /// Performs a bounded parse batch and publishes at most one style range.
    /// A caller must acknowledge a published batch before requesting another.
    pub fn step(
        &mut self,
        line_budget: usize,
        byte_budget: usize,
    ) -> Result<Option<HighlightBatch>, Error> {
        if self.published.is_some() {
            return Ok(None);
        }
        if self.parser.is_none() && self.pending.is_none() && !self.metadata_dirty {
            return Ok(None);
        }
        let started = Instant::now();
        let engine = self.engine.clone();
        let highlighter = ThemeHighlighter::new(&engine.themes.themes[self.theme.name()]);
        for _ in 0..line_budget.clamp(1, 1024) {
            let Some(mut parser) = self.parser.take() else {
                break;
            };
            let line = parser.next_line();
            if line >= self.starts.len() {
                break;
            }
            let text = &self.source[self.starts[line]..self.line_end(line)];
            let changes = parser.line(text, &engine.syntaxes, &highlighter)?;
            self.parsed_lines += 1;
            for (index, styles) in changes {
                if self.styles[index] != styles {
                    self.styles[index] = styles;
                    self.dirty(self.starts[index]..self.line_end(index));
                }
            }
            let next = parser.next_line();
            let mut converged = false;
            if next % 32 == 0 || next == self.starts.len() || self.checkpoints.contains_key(&next) {
                if let Some(state) = parser.checkpoint() {
                    converged = next >= self.convergence_after
                        && self.checkpoints.get(&next) == Some(&state);
                    self.checkpoints.insert(next, state);
                } else {
                    // A previously settled boundary can become speculative after
                    // an edit. It is no longer a valid restart point for later edits.
                    self.checkpoints.remove(&next);
                }
            }
            if !converged && next < self.starts.len() {
                self.parser = Some(parser);
            } else {
                self.convergence_after = 0;
            }
            if started.elapsed() >= Duration::from_millis(2) {
                break;
            }
        }
        let valid_end = self
            .parser
            .as_ref()
            .map_or(self.source.len(), |parser| self.starts[parser.next_line()]);
        let range = self.pending.as_ref().map_or(0..0, |pending| {
            if pending.start >= valid_end {
                return 0..0;
            }
            let mut end = pending
                .end
                .min(valid_end)
                .min(pending.start.saturating_add(byte_budget.max(4)));
            while !self.source.is_char_boundary(end) {
                end -= 1;
            }
            pending.start..end
        });
        let mut spans = Vec::new();
        if self.syntax.is_some() && !range.is_empty() {
            for line in self.line_at(range.start)..=self.line_at(range.end.saturating_sub(1)) {
                for span in &self.styles[line] {
                    let start = (self.starts[line] + span.range.start).max(range.start);
                    let end = (self.starts[line] + span.range.end).min(range.end);
                    if start < end {
                        push_span(&mut spans, start..end, span.style);
                    }
                }
            }
        }
        self.sequence += 1;
        self.published = Some(PendingBatch {
            revision: self.revision,
            configuration: self.configuration,
            sequence: self.sequence,
            range: range.clone(),
        });
        Ok(Some(HighlightBatch {
            revision: self.revision,
            configuration: self.configuration,
            sequence: self.sequence,
            range,
            spans,
            syntax_name: self.syntax.map_or_else(
                || "Plain Text".into(),
                |index| engine.syntaxes.syntaxes()[index].name.clone(),
            ),
            language_recognized: self.recognized,
            complete: self.parser.is_none(),
            parsed_lines: self.parsed_lines,
        }))
    }

    /// Rejected or obsolete results leave their changes pending for a later batch.
    pub fn acknowledge(&mut self, sequence: u64, applied: bool) {
        if self
            .published
            .as_ref()
            .is_none_or(|batch| batch.sequence != sequence)
        {
            return;
        }
        let batch = self.published.take().unwrap();
        if !applied || batch.revision != self.revision || batch.configuration != self.configuration
        {
            return;
        }
        if let Some(pending) = &mut self.pending {
            if batch.range.start == pending.start && batch.range.end > pending.start {
                pending.start = batch.range.end;
            }
            if pending.start >= pending.end {
                self.pending = None;
            }
        }
        self.metadata_dirty = false;
    }

    fn resolve(&self) -> (Option<usize>, bool) {
        let (syntax, recognized) = if !self.language.trim().is_empty() {
            self.engine.syntax_for(Some(&self.language))
        } else if !self.file_name.is_empty() {
            let path = Path::new(&self.file_name);
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let syntax = std::iter::once(name)
                .chain(name.match_indices('.').map(|(index, _)| &name[index + 1..]))
                .find_map(|suffix| self.engine.syntaxes.find_syntax_by_extension(suffix))
                .or_else(|| {
                    self.engine
                        .syntaxes
                        .find_syntax_by_first_line(self.source.lines().next().unwrap_or(""))
                });
            (syntax, syntax.is_some())
        } else {
            (None, false)
        };
        let index = syntax
            .filter(|syntax| syntax.name != "Plain Text")
            .and_then(|syntax| {
                self.engine
                    .syntaxes
                    .syntaxes()
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, syntax))
            });
        (index, recognized)
    }

    fn restart(&mut self) {
        (self.syntax, self.recognized) = self.resolve();
        self.checkpoints.clear();
        self.parser = self.syntax.map(|index| {
            let highlighter = ThemeHighlighter::new(&self.engine.themes.themes[self.theme.name()]);
            Parser::new(&self.engine.syntaxes.syntaxes()[index], &highlighter)
        });
        if let Some(parser) = &self.parser {
            self.checkpoints.insert(0, parser.checkpoint().unwrap());
        }
        self.convergence_after = self.starts.len();
        self.dirty(0..self.source.len());
        self.metadata_dirty = true;
    }

    fn line_at(&self, position: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= position)
            .saturating_sub(1)
    }
    fn line_end(&self, line: usize) -> usize {
        self.starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.source.len())
    }
    fn dirty(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        self.pending = Some(self.pending.take().map_or(range.clone(), |pending| {
            pending.start.min(range.start)..pending.end.max(range.end)
        }));
    }
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

fn map_offset(position: usize, start: usize, end: usize, inserted: usize) -> usize {
    if position <= start {
        position
    } else if position >= end {
        position - (end - start) + inserted
    } else {
        start + inserted
    }
}

#[cfg(test)]
mod tests {
    use crate::{Document, Highlighter, Theme};
    use std::sync::Arc;

    #[test]
    #[ignore = "opt-in 8 MiB highlighting and incremental-edit resource check"]
    fn large_document_keeps_edits_incremental() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let line = "fn example() { let value = 42; /* a comment */ }\n";
        let source = line.repeat(8 * 1024 * 1024 / line.len());
        let mut document = Document::new(engine, &source, "rust", "", Theme::Light);
        let started = std::time::Instant::now();
        let mut parsed = 0;
        while let Some(batch) = document.step(32, 16384).unwrap() {
            assert!(batch.range.len() <= 16384);
            parsed = batch.parsed_lines;
            document.acknowledge(batch.sequence, true);
        }
        let initial = started.elapsed();
        let started = std::time::Instant::now();
        document.edit(0, 0, 0, "\n").unwrap();
        let mut edited = 0;
        while let Some(batch) = document.step(32, 16384).unwrap() {
            edited = batch.parsed_lines - parsed;
            document.acknowledge(batch.sequence, true);
        }
        assert!(edited < 80);
        eprintln!(
            "{} bytes: initial {:?}, first-line insertion {:?}, {} parsed lines",
            source.len(),
            initial,
            started.elapsed(),
            edited
        );
    }

    #[test]
    fn incremental_edits_match_a_complete_highlight() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut text = "let value = 42;\n".repeat(200);
        let mut document = Document::new(engine.clone(), &text, "rust", "", Theme::Light);
        let mut displayed = vec![None; text.len()];
        let mut parsed_before = 0;
        while let Some(batch) = document.step(32, 4096).unwrap() {
            parsed_before = batch.parsed_lines;
            displayed[batch.range.clone()].fill(None);
            for span in &batch.spans {
                displayed[span.range.clone()].fill(Some(span.style));
            }
            document.acknowledge(batch.sequence, true);
        }
        text.replace_range(0..3, "// ");
        document.edit(0, 0, 3, "// ").unwrap();
        let mut parsed_after = 0;
        while let Some(batch) = document.step(32, 4096).unwrap() {
            parsed_after = batch.parsed_lines;
            displayed[batch.range.clone()].fill(None);
            for span in &batch.spans {
                displayed[span.range.clone()].fill(Some(span.style));
            }
            document.acknowledge(batch.sequence, true);
        }
        let reference = engine.highlight(&text, Some("rust"), Theme::Light).unwrap();
        assert!(
            parsed_after - parsed_before < 80,
            "unchanged suffixes should be reused"
        );
        for span in reference.spans {
            assert!(
                displayed[span.range]
                    .iter()
                    .all(|style| *style == Some(span.style))
            );
        }
    }

    #[test]
    fn later_edits_do_not_restore_checkpoints_that_became_speculative() {
        let syntax = syntect::parsing::SyntaxDefinition::load_from_str(
            r#"
name: Checkpoint Replay
file_extensions: [checkpoint-replay]
scope: source.checkpoint-replay
contexts:
  main:
    - match: 'BEGIN'
      branch_point: choice
      branch: [attempt, fallback]
  attempt:
    - meta_scope: string.quoted
    - match: 'FAIL'
      fail: choice
    - match: 'END'
      pop: true
  fallback:
    - meta_scope: comment.block
    - match: 'END'
      pop: true
"#,
            true,
            None,
        )
        .unwrap();
        let mut syntaxes = syntect::parsing::SyntaxSetBuilder::new();
        syntaxes.add(syntax);
        let mut engine = Highlighter::new().unwrap();
        engine.syntaxes = syntaxes.build();
        let engine = Arc::new(engine);
        // Exercise both existing checkpoint positions and positions relocated
        // by a line insertion or deletion before the speculative region.
        for (deleted, inserted) in [(5, "BEGIN"), (5, "BEGIN\n"), (11, "BEGIN")] {
            let mut source = format!(
                "{}TARGET\n{}END\n",
                "value\n".repeat(48),
                "value\n".repeat(48)
            );
            let mut document = Document::new(
                engine.clone(),
                &source,
                "checkpoint-replay",
                "",
                Theme::Light,
            );
            let mut displayed = vec![None; source.len()];
            for revision in 0..3 {
                if revision > 0 {
                    let (start, deleted, inserted) = if revision == 1 {
                        (0, deleted, inserted)
                    } else {
                        (source.find("TARGET").unwrap(), "TARGET".len(), "FAIL")
                    };
                    source.replace_range(start..start + deleted, inserted);
                    displayed.splice(start..start + deleted, vec![None; inserted.len()]);
                    document
                        .edit(revision - 1, start, deleted, inserted)
                        .unwrap();
                }
                while let Some(batch) = document.step(7, 79).unwrap() {
                    displayed[batch.range.clone()].fill(None);
                    for span in batch.spans {
                        displayed[span.range].fill(Some(span.style));
                    }
                    document.acknowledge(batch.sequence, true);
                }
                let mut expected = vec![None; source.len()];
                for span in engine
                    .highlight(&source, Some("checkpoint-replay"), Theme::Light)
                    .unwrap()
                    .spans
                {
                    expected[span.range].fill(Some(span.style));
                }
                let mismatch = displayed
                    .iter()
                    .zip(&expected)
                    .position(|(actual, expected)| actual != expected);
                assert!(
                    mismatch.is_none(),
                    "revision {revision}, replacement {deleted} -> {inserted:?}: mismatch at {mismatch:?}"
                );
            }
        }
    }

    #[test]
    fn rejected_batches_do_not_lose_earlier_changes() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut document = Document::new(
            engine.clone(),
            "let a = 1;\nlet b = 2;\n",
            "rust",
            "",
            Theme::Light,
        );
        while let Some(batch) = document.step(32, 4096).unwrap() {
            document.acknowledge(batch.sequence, true);
        }
        document.edit(0, 0, 3, "// ").unwrap();
        let obsolete = document.step(32, 4096).unwrap().unwrap();
        document.edit(1, 19, 1, "3").unwrap();
        document.acknowledge(obsolete.sequence, false);
        let current = document.step(32, 4096).unwrap().unwrap();
        assert_eq!(current.revision, 2);
        assert_eq!(current.range.start, 0);
        let reference = engine
            .highlight("//  a = 1;\nlet b = 3;\n", Some("rust"), Theme::Light)
            .unwrap();
        assert_eq!(
            current.spans.first().unwrap().style,
            reference.spans.first().unwrap().style
        );
    }

    #[test]
    fn identifies_full_file_names_and_clears_styles_for_plain_text() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut document = Document::new(
            engine,
            "version = 4\n",
            "",
            "/project/Cargo.lock",
            Theme::Light,
        );
        let batch = document.step(32, 4096).unwrap().unwrap();
        assert_eq!(batch.syntax_name, "TOML");
        assert!(batch.language_recognized);
        document.acknowledge(batch.sequence, true);
        document.configure("text", "/project/Cargo.lock", Theme::Dark);
        let plain = document.step(32, 4096).unwrap().unwrap();
        assert_eq!(plain.range, 0..12);
        assert!(plain.spans.is_empty());
        assert_eq!(plain.configuration, 1);
    }

    #[test]
    fn file_detection_prefers_the_longest_maintained_suffix() {
        let engine = Arc::new(Highlighter::new().unwrap());
        for (path, expected) in [
            ("/project/config.h.in", "CMake C Header"),
            ("/project/config.hpp.in", "CMake C++ Header"),
            ("/project/config.HPP.IN", "CMake C++ Header"),
            ("/project/deploy.go.yaml", "YAML (Go)"),
            ("/project/deploy.go.yml", "YAML (Go)"),
            ("/project/deploy.unrecognized.yaml", "YAML"),
            ("/project/配置.go.yaml", "YAML (Go)"),
            ("/project/.config.go.yaml", "YAML (Go)"),
            ("/project/Cargo.lock", "TOML"),
            ("/project/CMakeLists.txt", "CMake"),
        ] {
            let mut document = Document::new(engine.clone(), "", "", path, Theme::Light);
            let batch = document.step(32, 4096).unwrap().unwrap();
            assert_eq!(batch.syntax_name, expected, "{path}");
            assert!(batch.language_recognized, "{path}");
            document.acknowledge(batch.sequence, true);
            document.configure("text", path, Theme::Light);
            assert_eq!(
                document.step(32, 4096).unwrap().unwrap().syntax_name,
                "Plain Text"
            );
        }
    }

    #[test]
    fn detects_file_hints_and_rechecks_an_edited_shebang() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let source = "#!/usr/bin/env python3\nprint(42)\n";
        for (language, file_name, expected, recognized) in [
            ("", "/project/CMakeLists.txt", "CMake", true),
            ("", "/project/main.cpp", "C++", true),
            ("", "/project/main.js", "JavaScript", true),
            ("", "/project/script", "Python", true),
            ("javascript", "/project/main.rs", "JavaScript", true),
            ("unknown-language", "/project/main.rs", "Plain Text", false),
            ("", "", "Plain Text", false),
        ] {
            let mut document =
                Document::new(engine.clone(), source, language, file_name, Theme::Light);
            let batch = document.step(32, 4096).unwrap().unwrap();
            assert_eq!(batch.syntax_name, expected, "{language}: {file_name}");
            assert_eq!(batch.language_recognized, recognized);
        }
        let mut document = Document::new(engine, source, "", "/project/script", Theme::Light);
        while let Some(batch) = document.step(32, 4096).unwrap() {
            document.acknowledge(batch.sequence, true);
        }
        document
            .edit(0, 0, "#!/usr/bin/env python3\n".len(), "")
            .unwrap();
        let plain = document.step(32, 4096).unwrap().unwrap();
        assert_eq!(plain.syntax_name, "Plain Text");
        assert_eq!(plain.range, 0.."print(42)\n".len());
        assert!(plain.spans.is_empty());
    }

    #[test]
    fn chunks_a_long_unicode_line_and_recovers_from_an_empty_document() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let source = format!("// {}", "中文😀e\u{301}".repeat(6000));
        let mut document = Document::new(engine.clone(), &source, "rust", "", Theme::Light);
        let mut displayed = vec![None; source.len()];
        let mut batches = 0;
        while let Some(batch) = document.step(32, 16384).unwrap() {
            assert!(batch.range.len() <= 16384);
            assert!(source.is_char_boundary(batch.range.start));
            assert!(source.is_char_boundary(batch.range.end));
            for span in &batch.spans {
                displayed[span.range.clone()].fill(Some(span.style));
            }
            document.acknowledge(batch.sequence, true);
            batches += 1;
        }
        assert!(batches > 1);
        for span in engine
            .highlight(&source, Some("rust"), Theme::Light)
            .unwrap()
            .spans
        {
            assert!(
                displayed[span.range]
                    .iter()
                    .all(|style| *style == Some(span.style))
            );
        }
        assert!(document.edit(0, 4, 0, "x").is_err());
        document.edit(0, 0, source.len(), "").unwrap();
        let empty = document.step(32, 16384).unwrap().unwrap();
        assert!(empty.range.is_empty());
        assert!(empty.spans.is_empty());
        assert!(empty.complete);
        document.acknowledge(empty.sequence, true);
        assert!(document.step(32, 16384).unwrap().is_none());
        document.edit(1, 0, 0, "let x = 1;").unwrap();
        let restored = document.step(32, 16384).unwrap().unwrap();
        assert_eq!(restored.range, 0..10);
        assert_eq!(
            restored.spans,
            engine
                .highlight("let x = 1;", Some("rust"), Theme::Light)
                .unwrap()
                .spans
        );
    }

    #[test]
    fn unicode_line_edits_and_cancellation_match_full_parsing() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut text = "let value = 1;\r\n".repeat(80);
        let mut document = Document::new(engine.clone(), &text, "rust", "", Theme::Light);
        let mut displayed = vec![None; text.len()];
        let edits = [
            (0, 0, ""),
            (0, 0, "/* 中文 😀\n"),
            (20, 0, "\n"),
            (0, 17, ""),
            (5, 2, "42"),
            (0, 0, "// "),
        ];
        for (revision, (start, deleted, inserted)) in edits.into_iter().enumerate() {
            if revision > 0 {
                text.replace_range(start..start + deleted, inserted);
                displayed.splice(start..start + deleted, vec![None; inserted.len()]);
                document
                    .edit((revision - 1) as u64, start, deleted, inserted)
                    .unwrap();
            }
            while let Some(batch) = document.step(7, 127).unwrap() {
                displayed[batch.range.clone()].fill(None);
                for span in &batch.spans {
                    displayed[span.range.clone()].fill(Some(span.style));
                }
                document.acknowledge(batch.sequence, true);
            }
            let result = engine.highlight(&text, Some("rust"), Theme::Light).unwrap();
            for span in result.spans {
                assert!(
                    displayed[span.range]
                        .iter()
                        .all(|style| *style == Some(span.style)),
                    "revision {revision}"
                );
            }
        }
    }

    #[test]
    fn settled_checkpoints_converge_after_inserting_and_deleting_lines() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut text = "let value = 42;\n".repeat(400);
        let mut document = Document::new(engine.clone(), &text, "rust", "", Theme::Light);
        let mut parsed = 0;
        let mut displayed = vec![None; text.len()];
        for (revision, (deleted, inserted)) in [(0, ""), (0, "\n"), (1, "")].into_iter().enumerate()
        {
            if revision > 0 {
                text.replace_range(0..deleted, inserted);
                displayed.splice(0..deleted, vec![None; inserted.len()]);
                document
                    .edit((revision - 1) as u64, 0, deleted, inserted)
                    .unwrap();
            }
            let before = parsed;
            while let Some(batch) = document.step(32, 4096).unwrap() {
                parsed = batch.parsed_lines;
                displayed[batch.range.clone()].fill(None);
                for span in &batch.spans {
                    displayed[span.range.clone()].fill(Some(span.style));
                }
                document.acknowledge(batch.sequence, true);
            }
            if revision > 0 {
                assert!(
                    parsed - before < 80,
                    "line edits should reuse settled suffixes"
                );
            }
            for span in engine
                .highlight(&text, Some("rust"), Theme::Light)
                .unwrap()
                .spans
            {
                assert!(
                    displayed[span.range]
                        .iter()
                        .all(|style| *style == Some(span.style))
                );
            }
        }
    }

    #[test]
    fn mixed_edit_sequences_preserve_utf8_styles_with_obsolete_results() {
        let engine = Arc::new(Highlighter::new().unwrap());
        let mut text = "fn main() {\n    let value = 42; // comment\n}\n".repeat(20);
        let mut document = Document::new(engine.clone(), &text, "rust", "", Theme::Light);
        let mut displayed = vec![None; text.len()];
        let mut seed = 17u64;
        for revision in 0..60 {
            let obsolete = document.step(3, 57).unwrap();
            let boundaries: Vec<_> = text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(text.len()))
                .collect();
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let first = (seed as usize) % boundaries.len();
            let last = (first + (seed >> 32) as usize % 4).min(boundaries.len() - 1);
            let start = boundaries[first];
            let end = boundaries[last];
            let inserted = [
                "\n", "/*", "*/", "\"", "let ", "😀", "中文", "\r\n", "", "//",
            ][revision as usize % 10];
            text.replace_range(start..end, inserted);
            displayed.splice(start..end, vec![None; inserted.len()]);
            document
                .edit(revision, start, end - start, inserted)
                .unwrap();
            if let Some(batch) = obsolete {
                document.acknowledge(batch.sequence, false);
            }
            if revision % 5 == 0 || revision == 59 {
                while let Some(batch) = document.step(11, 137).unwrap() {
                    displayed[batch.range.clone()].fill(None);
                    for span in &batch.spans {
                        displayed[span.range.clone()].fill(Some(span.style));
                    }
                    document.acknowledge(batch.sequence, true);
                }
                for span in engine
                    .highlight(&text, Some("rust"), Theme::Light)
                    .unwrap()
                    .spans
                {
                    assert!(
                        displayed[span.range]
                            .iter()
                            .all(|style| *style == Some(span.style)),
                        "revision {revision}"
                    );
                }
            }
        }
    }
}
