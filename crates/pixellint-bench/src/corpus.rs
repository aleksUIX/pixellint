use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::case::{Case, parse_expansion, parse_kind};

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    kind: String,
    fixture: String,
    #[serde(default)]
    expansion_state: Option<String>,
    #[serde(default)]
    claimed_vendor: Option<String>,
}

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

pub fn load_corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut directories: Vec<PathBuf> = fs::read_dir(fixture_root())
        .expect("read fixtures directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("manifest.json").is_file())
        .collect();
    directories.sort();

    for directory in directories {
        let manifest_path = directory.join("manifest.json");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display()));
        let parsed: Vec<FixtureCase> = serde_json::from_str(&manifest)
            .unwrap_or_else(|error| panic!("parse {}: {error}", manifest_path.display()));
        let dir_name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture directory name");

        for case in parsed {
            let Some(kind) = parse_kind(&case.kind) else {
                continue;
            };
            let artifact_path = directory.join(&case.fixture);
            let artifact = fs::read_to_string(&artifact_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", artifact_path.display()));
            cases.push(Case {
                name: format!("{dir_name}/{}", case.id),
                kind,
                artifact,
                expansion_state: parse_expansion(case.expansion_state.as_deref()),
                claimed_vendor: case.claimed_vendor,
            });
        }
    }

    cases
}

pub fn url_cases(corpus: &[Case]) -> Vec<&Case> {
    corpus
        .iter()
        .filter(|case| crate::case::kind_label(case.kind) == "url")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_corpus_is_nonempty_and_skips_snippet_kinds() {
        let corpus = load_corpus();
        assert!(
            corpus.len() > 100,
            "expected a large golden corpus, got {}",
            corpus.len()
        );
        assert!(
            corpus
                .iter()
                .any(|case| case.name.starts_with("vendor-meta/"))
        );
        assert!(corpus.iter().any(|case| case.kind_label() == "json"));
        assert!(fixture_root().is_dir());
    }
}
