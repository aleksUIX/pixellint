use pixellint_core::{
    ArtifactKind, DocumentArtifactInput, DocumentExtractor, DocumentRequest, ExpansionState,
};

use crate::case::{Case, expansion_label, kind_label, uniquify};
use crate::corpus::url_cases;

#[derive(Debug, Clone)]
pub struct LoadCase {
    pub name: String,
    pub request: DocumentRequest,
    pub artifacts: usize,
    pub unique: usize,
    pub bytes: usize,
}

#[derive(Debug, Clone)]
pub struct LoadOptions {
    pub wrapper: usize,
    pub pod: usize,
    pub playground: usize,
    pub unique: usize,
    pub dup_total: usize,
    pub dup_unique: usize,
    pub heavy: bool,
}

impl LoadOptions {
    pub fn full(batch_size: usize, heavy: bool) -> Self {
        Self {
            wrapper: 80,
            pod: 500,
            playground: 1_000,
            unique: batch_size,
            dup_total: 10_000,
            dup_unique: 200,
            heavy,
        }
    }

    pub fn quick(batch_size: usize) -> Self {
        Self {
            wrapper: 80,
            pod: 0,
            playground: 200,
            unique: batch_size,
            dup_total: 400,
            dup_unique: 40,
            heavy: false,
        }
    }

    #[cfg(test)]
    pub fn smoke() -> Self {
        Self {
            wrapper: 16,
            pod: 0,
            playground: 12,
            unique: 12,
            dup_total: 20,
            dup_unique: 4,
            heavy: false,
        }
    }
}

impl LoadCase {
    fn from_items(
        name: &str,
        document_kind: &str,
        extractor: &str,
        items: Vec<DocumentArtifactInput>,
    ) -> Self {
        let artifacts = items.len();
        let unique = unique_count(&items);
        let bytes = items.iter().map(|item| item.artifact.len()).sum();
        let request = DocumentRequest {
            document_kind: document_kind.to_string(),
            extractor: Some(DocumentExtractor {
                id: extractor.to_string(),
                version: Some("bench".to_string()),
            }),
            artifacts: items,
        };
        Self {
            name: name.to_string(),
            request,
            artifacts,
            unique,
            bytes,
        }
    }

    pub fn dump_json(&self) -> String {
        let artifacts: Vec<DumpItem> = self
            .request
            .artifacts
            .iter()
            .map(|item| DumpItem {
                artifact_kind: kind_label(item.artifact_kind),
                artifact: item.artifact.as_str(),
                expansion_state: expansion_label(item.expansion_state),
            })
            .collect();
        serde_json::to_string(&DumpDocument {
            document_kind: &self.request.document_kind,
            artifacts,
        })
        .expect("serialize load document")
    }
}

#[derive(serde::Serialize)]
struct DumpDocument<'a> {
    document_kind: &'a str,
    artifacts: Vec<DumpItem<'a>>,
}

#[derive(serde::Serialize)]
struct DumpItem<'a> {
    artifact_kind: &'a str,
    artifact: &'a str,
    expansion_state: &'a str,
}

pub fn load_cases(corpus: &[Case], options: &LoadOptions) -> Vec<LoadCase> {
    let urls = url_cases(corpus);
    assert!(!urls.is_empty(), "golden corpus has no url fixtures");

    let mut cases = Vec::new();
    if options.wrapper > 0 {
        cases.push(LoadCase::from_items(
            &format!("vast-wrapper-{}", options.wrapper),
            "vast",
            "vastlint",
            vast_shaped(&urls, options.wrapper, options.wrapper * 3 / 10),
        ));
    }
    if options.pod > 0 {
        cases.push(LoadCase::from_items(
            &format!("vast-pod-{}", options.pod),
            "vast",
            "vastlint",
            vast_shaped(&urls, options.pod, options.pod * 4 / 5),
        ));
    }
    if options.playground > 0 {
        cases.push(LoadCase::from_items(
            &format!("d1-playground-{}", options.playground),
            "list",
            "pixellint-playground",
            uniquified(corpus, options.playground),
        ));
    }
    if options.dup_total > 0 {
        cases.push(LoadCase::from_items(
            &format!("dup-{}-{}-unique", options.dup_total, options.dup_unique),
            "list",
            "vastlint",
            duplicated(corpus, options.dup_total, options.dup_unique),
        ));
    }
    if options.unique > 0 {
        cases.push(LoadCase::from_items(
            &format!("unique-{}", options.unique),
            "list",
            "pixellint-bench",
            uniquified(corpus, options.unique),
        ));
    }
    if options.heavy {
        cases.push(LoadCase::from_items(
            "unique-100000",
            "list",
            "pixellint-bench",
            uniquified(corpus, 100_000),
        ));
    }
    cases
}

fn vast_shaped(urls: &[&Case], total: usize, unique: usize) -> Vec<DocumentArtifactInput> {
    let unique = unique.max(1).min(total).min(urls.len());
    let mut items = Vec::with_capacity(total);
    for index in 0..total {
        let source = if index < unique {
            urls[index % urls.len()]
        } else {
            urls[index % unique]
        };
        items.push(item(
            source.kind,
            uniquify(source.kind, &source.artifact, (index % unique) as u64),
            source.expansion_state,
        ));
    }
    items
}

fn uniquified(corpus: &[Case], total: usize) -> Vec<DocumentArtifactInput> {
    let mut items = Vec::with_capacity(total);
    for index in 0..total {
        let source = &corpus[index % corpus.len()];
        items.push(item(
            source.kind,
            uniquify(source.kind, &source.artifact, index as u64),
            source.expansion_state,
        ));
    }
    items
}

fn duplicated(corpus: &[Case], total: usize, unique: usize) -> Vec<DocumentArtifactInput> {
    let unique = unique.max(1).min(total).min(corpus.len());
    let pool: Vec<DocumentArtifactInput> = (0..unique)
        .map(|index| {
            let source = &corpus[index];
            item(
                source.kind,
                uniquify(source.kind, &source.artifact, index as u64),
                source.expansion_state,
            )
        })
        .collect();
    (0..total)
        .map(|index| pool[index % unique].clone())
        .collect()
}

fn item(
    kind: ArtifactKind,
    artifact: String,
    expansion_state: ExpansionState,
) -> DocumentArtifactInput {
    DocumentArtifactInput {
        artifact_kind: kind,
        artifact,
        claimed_vendor: None,
        expansion_state,
        occurrences: Vec::new(),
    }
}

fn unique_count(items: &[DocumentArtifactInput]) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        seen.insert((
            kind_label(item.artifact_kind),
            item.artifact.trim().to_string(),
        ));
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::load_corpus;
    use pixellint_core::{Engine, ValidationOptions};

    #[test]
    fn small_load_documents_validate_many() {
        let corpus = load_corpus();
        let engine = Engine::default();
        let options = ValidationOptions::default();
        let cases = load_cases(&corpus, &LoadOptions::smoke());
        assert!(cases.len() >= 3);
        for case in &cases {
            let report = engine
                .validate_many(&case.request, &options)
                .unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(report.summary.artifacts_total, Some(case.artifacts));
            assert_eq!(report.summary.unique_artifacts, Some(case.unique));
        }
    }
}
