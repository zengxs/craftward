// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{path::Path, sync::Arc};
use ward_highlighting::{Document, Highlighter, Style, Theme};

fn drain(document: &mut Document, displayed: &mut [Option<Style>]) {
    while let Some(batch) = document.step(7, 4096).unwrap() {
        assert!(batch.language_recognized);
        displayed[batch.range.clone()].fill(None);
        for span in batch.spans {
            displayed[span.range].fill(Some(span.style));
        }
        document.acknowledge(batch.sequence, true);
    }
}

#[test]
fn application_qml_files_highlight_as_snapshots_and_documents() {
    fn collect(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in directory.read_dir().unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "qml") {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/qml"),
        &mut files,
    );
    assert!(!files.is_empty());
    files.sort();
    let engine = Arc::new(Highlighter::new().unwrap());
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        let expected = engine
            .highlight(&source, Some("qml"), Theme::Light)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(expected.language_recognized, "{}", path.display());
        let mut document = Document::new(
            engine.clone(),
            &source,
            "",
            path.to_str().unwrap(),
            Theme::Light,
        );
        let mut displayed = vec![None; source.len()];
        drain(&mut document, &mut displayed);
        for span in expected.spans {
            assert!(
                displayed[span.range]
                    .iter()
                    .all(|style| *style == Some(span.style)),
                "{}",
                path.display()
            );
        }
    }
    eprintln!("Highlighted {} application QML files", files.len());
}

#[test]
fn qml_replay_remains_correct_after_line_and_unicode_edits() {
    let engine = Arc::new(Highlighter::new().unwrap());
    let mut source = "Item {\n    focusPolicy: Qt.StrongFocus\n}\n".to_owned();
    let mut document = Document::new(engine.clone(), &source, "qml", "", Theme::Dark);
    let mut displayed = vec![None; source.len()];
    for (revision, (deleted, inserted)) in [(0, ""), (0, "// 中文😀\n"), (14, ""), (0, "\n")]
        .into_iter()
        .enumerate()
    {
        if revision > 0 {
            source.replace_range(..deleted, inserted);
            displayed.splice(..deleted, vec![None; inserted.len()]);
            document
                .edit((revision - 1) as u64, 0, deleted, inserted)
                .unwrap();
        }
        drain(&mut document, &mut displayed);
        for span in engine
            .highlight(&source, Some("qml"), Theme::Dark)
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
fn representative_source_and_configuration_files_highlight() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Arc::new(Highlighter::new().unwrap());
    for (path, language) in [
        ("core/Cargo.lock", "toml"),
        ("app/CMakeLists.txt", "cmake"),
        ("app/src/editor/scintillaeditorbackend.cpp", "cpp"),
        ("core/ward-highlighting/src/lib.rs", "rust"),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let expected = engine
            .highlight(&source, Some(language), Theme::Dark)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(expected.language_recognized);
        let mut document = Document::new(engine.clone(), &source, "", path, Theme::Dark);
        let mut displayed = vec![None; source.len()];
        drain(&mut document, &mut displayed);
        for span in expected.spans {
            assert!(
                displayed[span.range]
                    .iter()
                    .all(|style| *style == Some(span.style)),
                "{path}"
            );
        }
    }
}
