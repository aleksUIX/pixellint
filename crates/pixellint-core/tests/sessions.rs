use pixellint_core::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const AT: i64 = 1_791_590_400;
const IAS: &str = "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=";
const PROFILE: &str = include_str!("../rulepacks/session/ias-video-pixel.json");
fn request(values: &[&str], groups: Vec<SessionGroup>) -> SessionRequest {
    session_request_from_json(&json!({"document":{"artifacts":values.iter().map(|v| json!({"artifact":format!("{IAS}{v}")})).collect::<Vec<_>>()},"sessions":groups}).to_string()).unwrap()
}
fn group(id: &str, indexes: &[usize]) -> SessionGroup {
    SessionGroup {
        session_id: id.into(),
        artifact_indexes: indexes.into(),
    }
}

#[test]
fn authored_static_relationship_controls() {
    let engine = Engine::default();
    for raw in [
        include_str!("../../../tests/fixtures/ias-session-depth/static-controls.json"),
        include_str!("../../../tests/fixtures/ias-session-depth/independent-static-controls.json"),
    ] {
        let fixture: Value = serde_json::from_str(raw).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["id"].as_str().unwrap();
            let input: SessionRequest = serde_json::from_value(case["request"].clone()).unwrap();
            let options = ValidationOptions {
                only_rulepacks: serde_json::from_value(case["options"]["rulepacks"].clone())
                    .unwrap_or_default(),
                except_rulepacks: serde_json::from_value(
                    case["options"]["exceptRulepacks"].clone(),
                )
                .unwrap_or_default(),
            };
            let report = engine
                .validate_sessions_at(&input, &options, AT)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let actual = serde_json::to_value(&report).unwrap();
            let codes: Vec<_> = report.findings.iter().map(|f| f.code.as_str()).collect();
            let expected: Vec<_> = case["expected"]["relationship_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(codes, expected, "{name}");
            assert_eq!(
                report.relationship_summary.errors,
                case["expected"]["relationship_errors"].as_u64().unwrap() as usize,
                "{name}"
            );
            assert_eq!(
                report.summary.errors,
                report.document.summary.errors + report.relationship_summary.errors,
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(
                    engine
                        .validate_many_at(&input.document, &options, AT)
                        .unwrap()
                )
                .unwrap(),
                actual["document"],
                "legacy document changed: {name}"
            );
            let reasons: Vec<_> = actual["checks"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|c| {
                    c["skipped"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s["reason"].as_str().unwrap())
                })
                .collect();
            for reason in case["expected"]["skip_reasons_present"]
                .as_array()
                .into_iter()
                .flatten()
            {
                assert!(
                    reasons.contains(&reason.as_str().unwrap()),
                    "{name}: missing skip {reason}"
                );
            }
            if let Some(exact) = case["expected"]["skip_reasons_exact"].as_array() {
                let mut actual_reasons = reasons.clone();
                actual_reasons.sort_unstable();
                actual_reasons.dedup();
                let mut wanted: Vec<_> = exact.iter().map(|s| s.as_str().unwrap()).collect();
                wanted.sort_unstable();
                assert_eq!(actual_reasons, wanted, "{name}");
            }
            if let Some(n) = case["expected_unique_artifacts"].as_u64() {
                assert_eq!(report.document.artifacts.len(), n as usize, "{name}");
            }
            if let Some(indexes) = case["expected_target_artifact_indexes"].as_array() {
                assert_eq!(
                    report.findings[0]
                        .targets
                        .iter()
                        .map(|t| t.artifact_index)
                        .collect::<Vec<_>>(),
                    indexes
                        .iter()
                        .map(|i| i.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>(),
                    "{name}"
                );
            }
            if let Some(span) = case.get("expected_first_target_span") {
                let target = &report.findings[0].targets[0].query_spans[0];
                assert_eq!(
                    target.start,
                    span["start"].as_u64().unwrap() as usize,
                    "{name}"
                );
                assert_eq!(target.end, span["end"].as_u64().unwrap() as usize, "{name}");
            }
            if let Some(ids) = case["expected_original_occurrence_ids"].as_array() {
                assert_eq!(
                    report.findings[0]
                        .targets
                        .iter()
                        .flat_map(|t| t
                            .occurrences
                            .iter()
                            .filter_map(|o| o.occurrence_id.as_deref()))
                        .collect::<Vec<_>>(),
                    ids.iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>(),
                    "{name}"
                );
            }
            if let Some(indexes) = case["expected_ungrouped_artifact_indexes"].as_array() {
                assert_eq!(
                    report.coverage.ungrouped_artifact_indexes,
                    indexes
                        .iter()
                        .map(|i| i.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>(),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn grouping_and_parse_error_controls() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ias-session-depth/error-and-registry-controls.json"
    ))
    .unwrap();
    for c in fixture["input_errors"].as_array().unwrap() {
        let name = c["id"].as_str().unwrap();
        let mut value = fixture["grouping_base_request"].clone();
        let mut options = ValidationOptions::default();
        for (key, replacement) in c["request_mutation"].as_object().unwrap() {
            if key == "options" {
                options.only_rulepacks =
                    serde_json::from_value(replacement["rulepacks"].clone()).unwrap_or_default();
                options.except_rulepacks =
                    serde_json::from_value(replacement["exceptRulepacks"].clone())
                        .unwrap_or_default();
            } else {
                value[key] = replacement.clone();
            }
        }
        let raw = c["request_mutation"]["raw_json"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        let result = session_request_from_json(&raw)
            .and_then(|r| Engine::default().validate_sessions_at(&r, &options, AT));
        let error = result.unwrap_err().to_string();
        match c["expected_error"].as_str().unwrap() {
            "invalid_json" => assert!(error.starts_with("invalid_json"), "{name}: {error}"),
            "engine_error" => assert!(error.contains("rulepack not found"), "{name}: {error}"),
            "document_error" => assert!(error.contains("not a validation kind"), "{name}: {error}"),
            code => assert!(error.starts_with(code), "{name}: {error}"),
        }
    }
}

fn custom_profile(rules: usize, kind: SessionRuleKind) -> SessionRulesManifest {
    SessionRulesManifest {
        owner_plugin_id: "vendor/test".into(),
        source_level: RuleSourceLevel::OfficialVendor,
        docs: "https://example.com/vendor-spec".into(),
        rules: (0..rules)
            .map(|i| SessionRule {
                code: format!("vendor.test.session.rule{i}"),
                kind,
                param: "xsId".into(),
                severity: Severity::Error,
                message: "Known identity relationship".into(),
                fix_hint: None,
                placeholder_values: vec![],
            })
            .collect(),
    }
}
struct CountingOwner {
    metadata: RulePackMetadata,
    validated: Arc<AtomicUsize>,
    matched: Arc<AtomicUsize>,
}
impl ValidatorPlugin for CountingOwner {
    fn metadata(&self) -> &RulePackMetadata {
        &self.metadata
    }
    fn supports(&self, _: &ValidationRequest) -> bool {
        self.matched.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn validate(&self, _: &ValidationRequest) -> ValidationReport {
        self.validated.fetch_add(1, Ordering::Relaxed);
        ValidationReport {
            plugin_id: self.metadata.id.clone(),
            detected_vendor: Some("test".into()),
            violations: vec![],
        }
    }
}
fn custom_engine() -> (Engine, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let mut engine = Engine::new();
    let validated = Arc::new(AtomicUsize::new(0));
    let matched = Arc::new(AtomicUsize::new(0));
    engine.register(CountingOwner {
        metadata: RulePackMetadata {
            id: "vendor/test".into(),
            display_name: "Test owner".into(),
            version: "1".into(),
            description: "Synthetic test".into(),
            source_level: RuleSourceLevel::OfficialVendor,
            vendor: Some("test".into()),
        },
        validated: validated.clone(),
        matched: matched.clone(),
    });
    (engine, validated, matched)
}

#[test]
fn registry_custom_profiles_and_replacement_controls() {
    let (mut e, _, _) = custom_engine();
    let r = request(&["a", "b"], vec![group("ad-1", &[0, 1])]);
    assert!(
        e.validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap()
            .findings
            .is_empty()
    );
    assert!(
        Engine::new()
            .register_session_manifest(custom_profile(1, SessionRuleKind::ConsistentParameter))
            .is_err()
    );
    e.register_session_manifest(custom_profile(1, SessionRuleKind::ConsistentParameter))
        .unwrap();
    assert_eq!(
        e.validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap()
            .findings
            .len(),
        1
    );
    e.register_session_manifest(custom_profile(
        1,
        SessionRuleKind::UniqueParameterAcrossSessions,
    ))
    .unwrap();
    let reuse = request(&["a", "a"], vec![group("ad-1", &[0]), group("ad-2", &[1])]);
    assert_eq!(
        e.validate_sessions_at(&reuse, &ValidationOptions::default(), AT)
            .unwrap()
            .findings
            .len(),
        1
    );
    let mut profile: Value =
        serde_json::to_value(custom_profile(1, SessionRuleKind::ConsistentParameter)).unwrap();
    for (field, value) in [
        ("kind", json!("complete_all_events")),
        ("code", json!("vendor.other.session.code")),
        ("param", json!("")),
        ("message", json!("")),
    ] {
        let mut invalid = profile.clone();
        invalid["rules"][0][field] = value;
        assert!(
            e.register_session_manifest_json(&invalid.to_string())
                .is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("source_level", json!("guessed")),
        ("docs", json!("")),
        ("rules", json!([])),
        ("inferred", json!(true)),
    ] {
        let mut invalid = profile.clone();
        invalid[field] = value;
        assert!(
            e.register_session_manifest_json(&invalid.to_string())
                .is_err(),
            "{field}"
        );
    }
    let duplicate = profile["rules"][0].clone();
    profile["rules"].as_array_mut().unwrap().push(duplicate);
    assert!(
        e.register_session_manifest_json(&profile.to_string())
            .is_err()
    );
    let (replacement, _, _) = custom_engine();
    e.register(CountingOwner {
        metadata: replacement.list_rulepacks()[0].clone(),
        validated: Arc::new(AtomicUsize::new(0)),
        matched: Arc::new(AtomicUsize::new(0)),
    });
    assert_eq!(
        e.validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap()
            .coverage
            .profiles_total,
        0
    );
    e.register_session_manifest(custom_profile(1, SessionRuleKind::ConsistentParameter))
        .unwrap();
    assert_eq!(
        e.validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap()
            .findings
            .len(),
        1
    );
    assert!(SessionRulesManifest::from_json(PROFILE).is_ok());
}

#[test]
fn output_projection_rejects_before_legacy_validation() {
    for case in 0..4 {
        let (mut e, validated, _) = custom_engine();
        let mut p = custom_profile(
            if case == 3 { 1 } else { 1000 },
            if case == 3 {
                SessionRuleKind::ConsistentParameter
            } else {
                SessionRuleKind::UniqueParameterAcrossSessions
            },
        );
        let mut r = if case == 3 {
            let values = ["a", "b"].repeat(1000);
            request(
                &values,
                (0..1000)
                    .map(|g| group(&format!("ad-{g}"), &[g * 2, g * 2 + 1]))
                    .collect(),
            )
        } else {
            request(&["a", "a"], vec![group("ad-1", &[0]), group("ad-2", &[1])])
        };
        if case == 3 {
            p.rules[0].message = "m".repeat(1024 * 1024);
        } else {
            for row in &mut r.document.artifacts {
                row.occurrences = if case == 1 {
                    vec![ArtifactOccurrence {
                        context_label: Some("x".repeat(1024 * 1024)),
                        occurrence_id: None,
                        source_kind: None,
                        path: None,
                        line: None,
                        column: None,
                    }]
                } else {
                    vec![
                        ArtifactOccurrence {
                            context_label: None,
                            occurrence_id: None,
                            source_kind: None,
                            path: None,
                            line: None,
                            column: None
                        };
                        50_000
                    ]
                };
            }
        }
        e.register_session_manifest(p).unwrap();
        let opts = ValidationOptions {
            only_rulepacks: vec![],
            except_rulepacks: if case == 2 {
                vec!["vendor/test".into()]
            } else {
                vec![]
            },
        };
        assert!(
            matches!(
                e.validate_sessions_at(&r, &opts, AT),
                Err(SessionError::ResourceLimit {
                    resource: "projected_output_items" | "projected_output_text",
                    ..
                })
            ),
            "case {case}"
        );
        assert_eq!(validated.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn ungrouped_rows_do_not_enter_relationship_matchers() {
    let (mut e, _, matched) = custom_engine();
    let r = request(&vec!["a"; 50_000], vec![]);
    let baseline = e
        .validate_many_at(&r.document, &ValidationOptions::default(), AT)
        .unwrap();
    let count = matched.swap(0, Ordering::Relaxed);
    e.register_session_manifest(custom_profile(
        1000,
        SessionRuleKind::UniqueParameterAcrossSessions,
    ))
    .unwrap();
    let report = e
        .validate_sessions_at(&r, &ValidationOptions::default(), AT)
        .unwrap();
    assert_eq!(matched.load(Ordering::Relaxed), count);
    assert_eq!(report.document, baseline);
    assert_eq!(report.coverage.ungrouped_artifact_indexes.len(), 50_000);
}

#[test]
fn large_placeholder_lookup_and_linear_relationship_workloads() {
    let (mut e, _, _) = custom_engine();
    let mut p = custom_profile(1, SessionRuleKind::ConsistentParameter);
    p.rules[0].placeholder_values = (0..100_000).map(|i| format!("placeholder-{i}")).collect();
    e.register_session_manifest(p).unwrap();
    let r = request(
        &vec!["observed"; 50_000],
        vec![group("ad-1", &(0..50_000).collect::<Vec<_>>())],
    );
    assert!(
        e.validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap()
            .findings
            .is_empty()
    );
    for n in [1000, 10_000] {
        let (mut e, _, _) = custom_engine();
        e.register_session_manifest(custom_profile(
            1,
            SessionRuleKind::UniqueParameterAcrossSessions,
        ))
        .unwrap();
        let r = request(
            &vec!["observed"; n],
            (0..n).map(|i| group(&format!("ad-{i}"), &[i])).collect(),
        );
        let report = e
            .validate_sessions_at(&r, &ValidationOptions::default(), AT)
            .unwrap();
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].targets.len(), n);
        assert_eq!(report.findings[0].session_ids.len(), n);
    }
}

#[test]
fn strict_binding_options_controls() {
    for raw in [
        "{\"at\":null}",
        "{\"at\":-0}",
        "{\"at\":1.5}",
        "{\"at\":1.0}",
        "{\"at\":1e0}",
        "{\"at\":9007199254740991.1}",
        "{\"at\":-9007199254740991.1}",
        "{\"at\":9007199254740992}",
        "{\"at\":1,\"at\":2}",
        "{\"inferSessions\":true}",
    ] {
        assert!(session_options_from_json(raw).is_err(), "{raw}");
    }
    for raw in [
        "{}",
        "{\"at\":9007199254740991}",
        "{\"at\":-9007199254740991}",
        "{\"at\":0,\"rulepacks\":[\"core\"]}",
    ] {
        assert!(session_options_from_json(raw).is_ok(), "{raw}");
    }
}

#[test]
fn raw_http_wrappers_and_raw_request_urls_preserve_document_contract() {
    let e = Engine::default();
    let mut r = request(&["a", "b"], vec![group("ad-1", &[0, 1])]);
    r.document.artifacts[0].artifact_kind = ArtifactKind::NetworkRequest;
    r.document.artifacts[0].artifact =
        format!("GET {IAS}a HTTP/1.1\r\nHost: unified.adsafeprotected.com\r\n\r\n");
    let report = e
        .validate_sessions_at(&r, &ValidationOptions::default(), AT)
        .unwrap();
    assert!(report.findings.is_empty());
    assert_eq!(
        report.checks[0].skipped[0].reason,
        SessionSkipReason::UnsupportedArtifact
    );
    assert_eq!(
        report.document,
        e.validate_many_at(&r.document, &ValidationOptions::default(), AT)
            .unwrap()
    );
    r.document.artifacts[0].artifact = format!("{IAS}a");
    let report = e
        .validate_sessions_at(&r, &ValidationOptions::default(), AT)
        .unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.document,
        e.validate_many_at(&r.document, &ValidationOptions::default(), AT)
            .unwrap()
    );
}
