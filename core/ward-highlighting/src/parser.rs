// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::VecDeque;

use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter as ThemeHighlighter};
use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

use crate::{Error, Span, Style, color, push_span};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Checkpoint {
    pub next_line: usize,
    parse: ParseState,
    highlight: HighlightState,
}

impl Checkpoint {
    pub fn relocate(mut self, next_line: usize) -> Option<Self> {
        if !self.parse.relocate_line(next_line) {
            return None;
        }
        self.next_line = next_line;
        Some(self)
    }
}

struct RecentLine {
    index: usize,
    text: String,
    ops: Vec<(usize, ScopeStackOp)>,
    before: HighlightState,
}

/// Keeps speculative colors recoverable until the parser resolves its branches.
pub(crate) struct Parser {
    state: Checkpoint,
    recent: VecDeque<RecentLine>,
}

impl Parser {
    pub fn new(syntax: &SyntaxReference, highlighter: &ThemeHighlighter<'_>) -> Self {
        Self::restore(Checkpoint {
            next_line: 0,
            parse: ParseState::new(syntax),
            highlight: HighlightState::new(highlighter, ScopeStack::new()),
        })
    }

    pub fn restore(state: Checkpoint) -> Self {
        Self {
            state,
            recent: VecDeque::new(),
        }
    }

    pub fn next_line(&self) -> usize {
        self.state.next_line
    }

    pub fn checkpoint(&self) -> Option<Checkpoint> {
        (!self.state.parse.is_speculative()).then(|| self.state.clone())
    }

    pub fn line(
        &mut self,
        text: &str,
        syntaxes: &SyntaxSet,
        highlighter: &ThemeHighlighter<'_>,
    ) -> Result<Vec<(usize, Vec<Span>)>, Error> {
        let output = self
            .state
            .parse
            .parse_line(text, syntaxes)
            .map_err(|error| Error::Highlight(error.to_string()))?;
        let mut corrections = output.replayed.into_iter();
        let mut earliest = self.state.next_line;
        for range in output.replay_ranges {
            if range.end > self.state.next_line || range.start > range.end {
                return Err(Error::Highlight("invalid parser replay range".into()));
            }
            if range.is_empty() {
                continue;
            }
            earliest = earliest.min(range.start);
            for index in range {
                let previous = self
                    .recent
                    .iter_mut()
                    .find(|line| line.index == index)
                    .ok_or_else(|| {
                        Error::Highlight("parser replay exceeded its retained history".into())
                    })?;
                previous.ops = corrections
                    .next()
                    .ok_or_else(|| Error::Highlight("incomplete parser replay".into()))?;
            }
        }
        if corrections.next().is_some() {
            return Err(Error::Highlight("unlocated parser replay".into()));
        }
        let mut changes = Vec::new();
        if earliest < self.state.next_line {
            let start = self
                .recent
                .iter()
                .position(|line| line.index == earliest)
                .ok_or_else(|| Error::Highlight("missing replay highlight state".into()))?;
            self.state.highlight = self.recent[start].before.clone();
            for line in self.recent.iter_mut().skip(start) {
                line.before = self.state.highlight.clone();
                changes.push((
                    line.index,
                    style_line(
                        &line.text,
                        &line.ops,
                        &mut self.state.highlight,
                        highlighter,
                    )?,
                ));
            }
        }
        let before = self.state.highlight.clone();
        let styles = style_line(text, &output.ops, &mut self.state.highlight, highlighter)?;
        changes.push((self.state.next_line, styles));
        if self.state.parse.is_speculative() {
            self.recent.push_back(RecentLine {
                index: self.state.next_line,
                text: text.to_owned(),
                ops: output.ops,
                before,
            });
            // The pinned parser permits at most 128 previous lines of replay.
            while self.recent.len() > 128 {
                self.recent.pop_front();
            }
        } else {
            self.recent.clear();
        }
        self.state.next_line += 1;
        Ok(changes)
    }
}

fn style_line(
    text: &str,
    ops: &[(usize, ScopeStackOp)],
    state: &mut HighlightState,
    highlighter: &ThemeHighlighter<'_>,
) -> Result<Vec<Span>, Error> {
    let mut previous = 0;
    for (offset, _) in ops {
        if *offset < previous || !text.is_char_boundary(*offset) {
            return Err(Error::Highlight("invalid parser scope offset".into()));
        }
        previous = *offset;
    }
    let mut spans = Vec::new();
    let mut offset = 0;
    for (style, region) in HighlightIterator::new(state, ops, text, highlighter) {
        let end = offset + region.len();
        if end > offset {
            push_span(
                &mut spans,
                offset..end,
                Style {
                    foreground: color(style.foreground),
                    background: color(style.background),
                    bold: style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::BOLD),
                    italic: style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::ITALIC),
                    underline: style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::UNDERLINE),
                },
            );
        }
        offset = end;
    }
    Ok(spans)
}
