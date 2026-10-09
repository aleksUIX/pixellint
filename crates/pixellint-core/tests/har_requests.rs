//! Independent capture oracles: omitted HAR fields cannot prove missing request data.
use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, HarBodyAvailability, HarHeaderPolicy,
    HarImport, HarImportOptions, HarReport, ValidationOptions, ValidationRequest, import_har,
};
use serde_json::{Value, json};

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/har-oracle", "vendor":"har-oracle", "display_name":"HAR oracle",
        "description":"Synthetic contract independent from a HAR exporter",
        "docs":"https://example.org/har-oracle", "match":{"hosts":["har.example"],"paths":["/events"],"json_paths":["time"]},
        "params":[{"name":"token","requirement":"required"}],
        "http_url_presence_overrides":["token"],
        "body":{"params":[{"name":"time","requirement":"required","json_type":"integer"}],
          "rules":[{"code":"vendor.har-oracle.body.clock", "kind":"time_window","param":"time",
            "unit":"seconds","max_age_seconds":10,"max_future_seconds":1,"severity":"error","message":"Outside the window."},
            {"code":"vendor.har-oracle.body.bytes","kind":"max_body_bytes","max_bytes":64,
              "severity":"error","message":"Over the wire byte limit."}]},
        "http":{"params":[
          {"name":"method","requirement":"required","format":{"kind":"enum","values":["POST"]}},
          {"name":"content_type","requirement":"required","format":{"kind":"enum","values":["application/json"]}},
          {"name":"body_encoding","requirement":"required","format":{"kind":"enum","values":["json"]}},
          {"name":"headers.x-key","requirement":"required","format":{"kind":"enum","values":["good"]}},
          {"name":"headers.authorization","format":{"kind":"regex","pattern":"^Bearer [a-z]+$"}},
          {"name":"query.token"}, {"name":"body.token"}, {"name":"body.time"}
        ],"rules":[
          {"code":"vendor.har-oracle.http.auth","kind":"require_any_of","groups":[["query.token"],["body.token"],["headers.authorization"]],
           "severity":"error","message":"A credential carrier is required."},
          {"code":"vendor.har-oracle.http.clock","kind":"time_window","param":"body.time","unit":"seconds",
           "max_age_seconds":10,"max_future_seconds":1,"severity":"error","message":"Outside the HTTP window."}
        ]}
    }).to_string()).unwrap();
    engine
}

fn entry() -> Value {
    json!({"startedDateTime":"2026-10-09T12:00:00.125Z", "pageref":"page_1",
        "request":{"method":"POST", "url":"https://har.example/events?a=1&a=%32&x=+&x=%20",
         "headers":[{"name":"Content-Type","value":"application/json"},{"name":"X-Key","value":"good"}],
         "bodySize":19,"postData":{"mimeType":"application/json", "text":"{\"time\":1791547200}"}},
        "response":{"content":{"encoding":"base64","text":"AP8="}}})
}

fn import(entries: Vec<Value>, policy: HarHeaderPolicy) -> HarImport {
    import_har(
        &json!({"log":{"version":"1.2","entries":entries}}).to_string(),
        &HarImportOptions {
            header_policy: policy,
        },
    )
    .unwrap()
}

fn codes(report: &HarReport) -> Vec<String> {
    report
        .document
        .artifacts
        .iter()
        .flat_map(|artifact| &artifact.reports)
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.clone())
        .collect()
}

fn replay(entries: Vec<Value>, policy: HarHeaderPolicy) -> HarReport {
    engine()
        .validate_har(&import(entries, policy), &ValidationOptions::default())
        .unwrap()
}

#[test]
fn importer_keeps_wire_values_duplicate_order_and_original_provenance() {
    let mut row = entry();
    row["request"]["queryString"] = json!([{"name":"invented","value":"do-not-merge"}]);
    row["request"]["headers"].as_array_mut().unwrap().extend([
        json!({"name":"X-Repeat","value":"first"}),
        json!({"name":"x-repeat","value":"second"}),
    ]);
    let imported = import(vec![row], HarHeaderPolicy::Unknown);
    let capture: Value = serde_json::from_str(&imported.document.artifacts[0].artifact).unwrap();
    assert_eq!(
        capture["url"],
        "https://har.example/events?a=1&a=%32&x=+&x=%20"
    );
    assert_eq!(capture["body"], "{\"time\":1791547200}");
    assert_eq!(capture["headers"][2]["value"], "first");
    assert_eq!(capture["headers"][3]["value"], "second");
    assert!(!capture.to_string().contains("invented"));
    assert_eq!(
        imported.entries[0].reference_time_unix_seconds,
        Some(1_791_547_200)
    );
    assert_eq!(
        imported.entries[0].started_date_time.as_deref(),
        Some("2026-10-09T12:00:00.125Z")
    );
    assert_eq!(imported.entries[0].page_ref.as_deref(), Some("page_1"));
    assert_eq!(
        imported.document.artifacts[0].occurrences[0]
            .path
            .as_deref(),
        Some("/log/entries/0/request")
    );
    assert_eq!(
        imported.entries[0].body_availability,
        HarBodyAvailability::Available
    );
}

#[test]
fn unknown_and_sanitized_credentials_are_not_established_missing() {
    for policy in [HarHeaderPolicy::Unknown, HarHeaderPolicy::ChromeSanitized] {
        let report = replay(vec![entry()], policy);
        assert_eq!(report.document.summary.errors, 0, "{report:?}");
        assert_eq!(codes(&report), ["core.request.capture_incomplete"]);
        let context = &report.captures[0].capture;
        if policy == HarHeaderPolicy::Unknown {
            assert_eq!(context.unavailable_headers, ["authorization", "cookie"]);
            assert!(context.redacted_headers.is_empty());
        } else {
            assert_eq!(context.redacted_headers, ["authorization", "cookie"]);
            assert!(context.unavailable_headers.is_empty());
        }
    }
    let complete = replay(vec![entry()], HarHeaderPolicy::Complete);
    assert_eq!(codes(&complete), ["vendor.har-oracle.http.auth"]);
    assert_eq!(complete.document.summary.errors, 1);
}

#[test]
fn known_bad_method_header_and_body_survive_sanitized_policy() {
    let mut row = entry();
    row["request"]["method"] = json!("GET");
    row["request"]["headers"][1]["value"] = json!("bad");
    row["request"]["headers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"Authorization","value":"Basic SECRET"}));
    row["request"]["postData"]["text"] = json!("{}");
    row["request"]["bodySize"] = json!(2);
    let report = replay(vec![row], HarHeaderPolicy::ChromeSanitized);
    let found = codes(&report);
    for code in [
        "vendor.har-oracle.http.method.invalid",
        "vendor.har-oracle.http.headers.x-key.invalid",
        "vendor.har-oracle.http.headers.authorization.invalid",
        "vendor.har-oracle.body.time.missing",
    ] {
        assert!(found.contains(&code.to_string()), "{found:?}");
    }
    // Sanitized policy does not erase credentials that are actually present.
    let findings = serde_json::to_string(&report.document.artifacts[0].reports).unwrap();
    assert!(!findings.contains("SECRET"));
}

#[test]
fn omitted_body_remains_unknown_but_zero_size_establishes_absence() {
    let mut row = entry();
    row["request"].as_object_mut().unwrap().remove("postData");
    for size in [json!(19), json!(-1), Value::Null] {
        row["request"]["bodySize"] = size;
        let report = replay(vec![row.clone()], HarHeaderPolicy::Complete);
        assert_eq!(
            report.captures[0].body_availability,
            HarBodyAvailability::Unavailable
        );
        assert_eq!(codes(&report), ["core.request.capture_incomplete"]);
    }
    row["request"]["bodySize"] = json!(0);
    let report = replay(vec![row], HarHeaderPolicy::Complete);
    assert_eq!(
        report.captures[0].body_availability,
        HarBodyAvailability::Absent
    );
    assert!(codes(&report).contains(&"vendor.har-oracle.http.auth".into()));
    assert!(codes(&report).contains(&"vendor.har-oracle.http.body_encoding.invalid".into()));
}

#[test]
fn encoded_params_only_redacted_and_mismatched_byte_bodies_are_unavailable() {
    for (post, size, state) in [
        (
            json!({"mimeType":"application/json","text":"AP8=","encoding":"base64"}),
            2,
            HarBodyAvailability::Unavailable,
        ),
        (
            json!({"mimeType":"application/json","text":"AP8=","_encoding":"base64"}),
            2,
            HarBodyAvailability::Unavailable,
        ),
        (
            json!({"mimeType":"application/json","params":[{"name":"time","value":"1791547200"}]}),
            19,
            HarBodyAvailability::Unavailable,
        ),
        (
            json!({"text":"{\"time\":1791547200}","_redacted":true}),
            19,
            HarBodyAvailability::Redacted,
        ),
        (
            json!({"text":"{\"time\":1791547200}"}),
            1,
            HarBodyAvailability::Unavailable,
        ),
        (
            json!({"text":"{}","params":[]}),
            2,
            HarBodyAvailability::Unavailable,
        ),
    ] {
        let mut row = entry();
        row["request"]["postData"] = post.clone();
        row["request"]["bodySize"] = json!(size);
        let report = replay(vec![row], HarHeaderPolicy::Complete);
        assert_eq!(report.captures[0].body_availability, state);
        assert_eq!(report.captures[0].post_data.as_ref().unwrap(), &post);
        assert_eq!(codes(&report), ["core.request.capture_incomplete"]);
    }
}

#[test]
fn missing_headers_defer_missing_fields_but_do_not_hide_method() {
    let mut row = entry();
    row["request"].as_object_mut().unwrap().remove("headers");
    row["request"]["method"] = json!("GET");
    let report = replay(vec![row], HarHeaderPolicy::Complete);
    assert_eq!(
        codes(&report),
        [
            "core.request.capture_incomplete",
            "vendor.har-oracle.http.method.invalid"
        ]
    );
    assert!(report.captures[0].capture.headers_unavailable);
}

#[test]
fn historical_clocks_keep_per_entry_findings_and_at_override_wins() {
    let first = entry();
    let mut later = first.clone();
    later["startedDateTime"] = json!("2026-10-09T12:00:20.125Z");
    let imported = import(vec![first.clone(), first, later], HarHeaderPolicy::Unknown);
    let report = engine()
        .validate_har(&imported, &ValidationOptions::default())
        .unwrap();
    assert_eq!(report.document.summary.artifacts_total, Some(3));
    assert_eq!(report.document.summary.unique_artifacts, Some(2));
    assert_eq!(report.document.artifacts[0].occurrences.len(), 2);
    assert_eq!(report.document.artifacts[1].occurrences.len(), 1);
    assert_ne!(
        report.document.artifacts[0].dedupe_key,
        report.document.artifacts[1].dedupe_key
    );
    assert_eq!(report.document.artifacts[0].summary.errors, 0);
    assert_eq!(report.document.artifacts[1].summary.errors, 2);
    let overridden = engine()
        .validate_har_at(&imported, &ValidationOptions::default(), 1_791_547_200)
        .unwrap();
    assert_eq!(overridden.document.summary.unique_artifacts, Some(1));
    assert_eq!(overridden.document.summary.errors, 0);
}

#[test]
fn missing_invalid_local_and_offset_clocks_are_explicit() {
    let mut row = entry();
    row["startedDateTime"] = json!("2026-10-09T14:00:00.125+02:00");
    assert_eq!(
        import(vec![row.clone()], HarHeaderPolicy::Unknown).entries[0].reference_time_unix_seconds,
        Some(1_791_547_200)
    );
    row["startedDateTime"] = json!("1969-12-31T23:59:59.999Z");
    assert_eq!(
        import(vec![row.clone()], HarHeaderPolicy::Unknown).entries[0].reference_time_unix_seconds,
        Some(-1)
    );
    for timestamp in [
        json!("2026-10-09T12:00:00"),
        json!("2026-02-30T12:00:00Z"),
        Value::Null,
    ] {
        row["startedDateTime"] = timestamp;
        let imported = import(vec![row.clone()], HarHeaderPolicy::Unknown);
        assert_eq!(imported.entries[0].reference_time_unix_seconds, None);
        assert!(
            engine()
                .validate_har(&imported, &ValidationOptions::default())
                .unwrap_err()
                .contains("explicit reference-time override")
        );
        assert!(
            engine()
                .validate_har_at(&imported, &ValidationOptions::default(), 1_791_547_200)
                .unwrap()
                .is_ok()
        );
    }
}

#[test]
fn byte_limit_uses_original_unicode_and_whitespace_not_compacted_json() {
    let mut row = entry();
    let text = format!("{{\"time\":1791547200,\"note\":\"{}🦊\"}}", " ".repeat(35));
    assert!(text.len() > 64);
    row["request"]["bodySize"] = json!(text.len());
    row["request"]["postData"]["text"] = json!(text);
    let report = replay(vec![row], HarHeaderPolicy::Unknown);
    assert_eq!(
        codes(&report),
        [
            "core.request.capture_incomplete",
            "vendor.har-oracle.body.bytes"
        ]
    );
}

#[test]
fn existing_complete_envelopes_keep_their_missing_auth_contract() {
    let imported = import(vec![entry()], HarHeaderPolicy::Complete);
    let mut envelope: Value =
        serde_json::from_str(&imported.document.artifacts[0].artifact).unwrap();
    envelope.as_object_mut().unwrap().remove("capture");
    let summary = engine()
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            1_791_547_200,
        )
        .unwrap();
    let found: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect();
    assert_eq!(found, ["vendor.har-oracle.http.auth"]);
}

#[test]
fn invalid_har_data_is_an_input_error_and_empty_archives_are_valid() {
    for raw in [
        "{}",
        r#"{"log":{"version":"1.1","entries":[]}}"#,
        r#"{"log":{"version":"1.2","entries":[{"request":{"url":"https://example.org","method":"GET","bodySize":-2}}]}}"#,
        r#"{"log":{"version":"1.2","entries":[{"request":{"url":"https://example.org","method":"GET","method":"POST"}}]}}"#,
        r#"{"log":{"version":"1.2","entries":[{"request":{"url":"https://example.org","method":"GET","headers":{}}}]}}"#,
    ] {
        assert!(
            import_har(raw, &HarImportOptions::default()).is_err(),
            "{raw}"
        );
    }
    let report = replay(vec![], HarHeaderPolicy::Unknown);
    assert_eq!(report.document.summary.artifacts_total, Some(0));
    assert!(report.is_ok());
}

#[test]
fn observed_headers_in_a_partial_collection_keep_scope_and_condition_checks() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"custom/har-scopes", "display_name":"HAR scopes", "description":"Independent availability scope oracle", "docs":"https://example.org/har-scopes",
        "match":{"hosts":["har.example"]},
        "http":[
          {"scope":"headers","params":[
            {"name":"x-key","requirement":"required","format":{"kind":"enum","values":["good"]}},
            {"name":"x-omitted","requirement":"required"}]},
          {"condition":{"kind":"present","param":"auth"}, "params":[
            {"name":"auth","root_path":"headers.authorization"},
            {"name":"method","format":{"kind":"enum","values":["POST"]}}]}
        ]
    }).to_string()).unwrap();
    let envelope = json!({"url":"https://har.example/events","method":"GET", "headers":{"X-Key":"bad"},
        "capture":{"headers_unavailable":true}});
    let summary = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            0,
        )
        .unwrap();
    let found: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect();
    assert_eq!(
        found,
        [
            "core.request.capture_incomplete",
            "custom.har-scopes.http.x-key.invalid"
        ]
    );
}

#[test]
fn explicitly_redacted_header_values_and_body_are_preserved_but_not_inspected() {
    let envelope = json!({"url":"https://har.example/events","method":"POST",
        "headers":{"Content-Type":"application/json","X-Key":"good","Authorization":"[removed]"},
        "body":"[removed]", "capture":{"redacted_headers":["Authorization"],"body":"redacted"}});
    let summary = engine()
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            1_791_547_200,
        )
        .unwrap();
    let found: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect();
    assert_eq!(found, ["core.request.capture_incomplete"]);
    assert_eq!(envelope["body"], "[removed]");
}

#[test]
fn capture_declarations_reject_contradictory_states_and_ambiguous_keys() {
    for capture in [
        json!({"body":"absent"}),
        json!({"unavailable_headers":["Authorization","authorization"]}),
        json!({"unavailable_headers":["Cookie"],"redacted_headers":["cookie"]}),
        json!({"headers_unavailable":"yes"}),
        json!({"body":"invented"}),
    ] {
        let mut envelope =
            json!({"url":"https://har.example/events","method":"POST","headers":{},"body":"{}"});
        envelope["capture"] = capture;
        let summary = engine()
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: envelope.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        assert_eq!(
            summary.reports[0].violations[0].code,
            "core.request.invalid_envelope"
        );
    }
    let envelope = r#"{"url":"https://har.example/events","method":"POST","headers":{},"capture":{"headers_unavailable":true,"headers_unavailable":false}}"#;
    let summary = engine()
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.into(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            0,
        )
        .unwrap();
    assert_eq!(
        summary.reports[0].violations[0].code,
        "core.request.invalid_envelope"
    );
}

#[test]
fn actual_meta_alternative_authentication_stays_unknown_when_har_omits_authorization() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-meta-conversions-api/round-034-bearer-json.txt"
    ))
    .unwrap();
    let mut headers: Vec<_> = capture["headers"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect();
    let body = capture["body"].as_str().unwrap();
    let row = json!({"startedDateTime":"2026-02-02T02:41:00Z", "request":{
        "url":capture["url"],"method":capture["method"],"headers":headers,"bodySize":body.len(),
        "postData":{"mimeType":"application/json","text":body}}});
    let imported = import(vec![row.clone()], HarHeaderPolicy::ChromeSanitized);
    let report = Engine::default()
        .validate_har_at(&imported, &ValidationOptions::default(), 1_770_000_060)
        .unwrap();
    assert!(
        !codes(&report)
            .contains(&"vendor.meta-conversions-api.http.authentication_required".into())
    );
    assert!(
        !codes(&report).contains(&"vendor.meta-conversions-api.param.access_token.missing".into())
    );
    let complete = Engine::default()
        .validate_har_at(
            &import(vec![row.clone()], HarHeaderPolicy::Complete),
            &ValidationOptions::default(),
            1_770_000_060,
        )
        .unwrap();
    assert!(
        codes(&complete)
            .contains(&"vendor.meta-conversions-api.http.authentication_required".into())
    );
    headers.push(json!({"name":"Authorization","value":"Bad REDACTME"}));
    let mut invalid = row;
    invalid["request"]["headers"] = json!(headers);
    let observed = Engine::default()
        .validate_har_at(
            &import(vec![invalid], HarHeaderPolicy::ChromeSanitized),
            &ValidationOptions::default(),
            1_770_000_060,
        )
        .unwrap();
    assert!(
        codes(&observed)
            .contains(&"vendor.meta-conversions-api.http.headers.authorization.invalid".into())
    );
}

#[test]
fn independently_authored_archive_fixtures_keep_expected_destination_findings() {
    for (raw, policy, errors, code) in [
        (
            include_str!("data/har/known-clean.har"),
            HarHeaderPolicy::Complete,
            0,
            None,
        ),
        (
            include_str!("data/har/credential-omitted.har"),
            HarHeaderPolicy::Unknown,
            0,
            Some("core.request.capture_incomplete"),
        ),
        (
            include_str!("data/har/body-unavailable.har"),
            HarHeaderPolicy::Unknown,
            0,
            Some("core.request.capture_incomplete"),
        ),
        (
            include_str!("data/har/params-only.har"),
            HarHeaderPolicy::Unknown,
            0,
            Some("core.request.capture_incomplete"),
        ),
        (
            include_str!("data/har/credential-omitted.har"),
            HarHeaderPolicy::Complete,
            1,
            Some("vendor.meta-conversions-api.http.authentication_required"),
        ),
    ] {
        let imported = import_har(
            raw,
            &HarImportOptions {
                header_policy: policy,
            },
        )
        .unwrap();
        let report = Engine::default()
            .validate_har(&imported, &ValidationOptions::default())
            .unwrap();
        assert_eq!(report.document.summary.errors, errors, "{report:?}");
        let found = codes(&report);
        assert_eq!(found, code.into_iter().collect::<Vec<_>>());
    }
}

#[test]
fn local_resource_limits_make_unavailable_body_context_visible() {
    let mut row = entry();
    let text = " ".repeat(pixellint_core::har::MAX_HAR_BODY_BYTES + 1);
    row["request"]["bodySize"] = json!(text.len());
    row["request"]["postData"]["text"] = json!(text);
    let report = replay(vec![row], HarHeaderPolicy::Complete);
    assert_eq!(
        report.captures[0].body_availability,
        HarBodyAvailability::Unavailable
    );
    assert!(
        report.captures[0]
            .body_reason
            .as_deref()
            .unwrap()
            .contains("4 MiB")
    );
    assert_eq!(codes(&report), ["core.request.capture_incomplete"]);
    let input = " ".repeat(pixellint_core::har::MAX_HAR_BYTES + 1);
    assert!(
        import_har(&input, &HarImportOptions::default())
            .unwrap_err()
            .contains("64 MiB")
    );
    let entries = vec![
        json!({"request":{"url":"https://example.org/","method":"GET"}});
        pixellint_core::har::MAX_HAR_ENTRIES + 1
    ];
    let raw = json!({"log":{"version":"1.2","entries":entries}}).to_string();
    assert!(
        import_har(&raw, &HarImportOptions::default())
            .unwrap_err()
            .contains("50000-entry")
    );
}

#[test]
fn post_data_mime_never_fabricates_a_missing_request_header() {
    let mut row = entry();
    row["request"]["headers"].as_array_mut().unwrap().remove(0);
    let report = replay(vec![row], HarHeaderPolicy::Unknown);
    assert!(codes(&report).contains(&"vendor.har-oracle.http.content_type.missing".into()));
    assert!(codes(&report).contains(&"vendor.har-oracle.http.body_encoding.invalid".into()));
}

#[test]
fn unknown_aliases_and_repeated_members_do_not_hide_observed_header_formats() {
    for (headers, http, expected) in [
        (
            json!({"X-Key":"bad"}),
            json!({"scope":"headers","params":[
          {"name":"x-key","aliases":["x-alt"],"format":{"kind":"enum","values":["good"]}}]}),
            "custom.har-peer.http.x-key.invalid",
        ),
        (
            json!([{"name":"X-Key","value":"good"},{"name":"X-Key","value":"bad"}]),
            json!({"params":[{"name":"headers.x-key","json_type":["string","array"]},
           {"name":"headers.x-key[]","format":{"kind":"enum","values":["good"]}}]}),
            "custom.har-peer.http.headers.x-key[].invalid",
        ),
    ] {
        let mut engine = Engine::new();
        engine.register(CoreRulePack::default());
        engine.register_manifest_json(&json!({"id":"custom/har-peer","display_name":"HAR peer oracle",
            "description":"Availability cannot hide observed header values","docs":"https://example.org/har-peer",
            "match":{"hosts":["har.example"]},"http":http}).to_string()).unwrap();
        let envelope = json!({"url":"https://har.example/events","method":"GET","headers":headers,
            "capture":{"headers_unavailable":true}});
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: envelope.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        let found: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .map(|finding| finding.code.as_str())
            .collect();
        assert_eq!(found, ["core.request.capture_incomplete", expected]);
    }
}

#[test]
fn decisive_guard_branches_keep_observed_rules_and_unknown_negation_stays_unknown() {
    for (condition, expected_errors) in [
        (
            json!({"kind":"any","conditions":[{"kind":"value_in","param":"query.mode","values":["server"]},{"kind":"present","param":"headers.authorization"}]}),
            1,
        ),
        (
            json!({"kind":"not","condition":{"kind":"all","conditions":[{"kind":"value_in","param":"query.mode","values":["browser"]},{"kind":"present","param":"headers.authorization"}]}}),
            1,
        ),
        (
            json!({"kind":"not","condition":{"kind":"present","param":"headers.authorization"}}),
            0,
        ),
        (
            json!({"kind":"all","conditions":[{"kind":"value_in","param":"query.mode","values":["server"]},{"kind":"present","param":"headers.authorization"}]}),
            0,
        ),
    ] {
        let mut engine = Engine::new();
        engine.register(CoreRulePack::default());
        engine.register_manifest_json(&json!({"id":"custom/har-peer","display_name":"HAR peer oracle",
          "description":"Decisive guard branches remain observable","docs":"https://example.org/har-peer",
          "match":{"hosts":["har.example"]},"http":{"condition":condition,"params":[
            {"name":"query.mode"},{"name":"headers.authorization"},
            {"name":"method","format":{"kind":"enum","values":["POST"]}}]}}).to_string()).unwrap();
        let envelope = json!({"url":"https://har.example/events?mode=server","method":"GET","headers":{},"capture":{"headers_unavailable":true}});
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: envelope.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        let errors = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .filter(|finding| finding.severity == pixellint_core::Severity::Error)
            .count();
        assert_eq!(errors, expected_errors, "{summary:?}");
    }
}

#[test]
fn redacted_placeholder_controls_are_unavailable_but_observed_controls_are_errors() {
    for (redacted, expected_errors) in [(true, 0), (false, 1)] {
        let mut envelope = json!({"url":"https://example.org/events","method":"GET",
          "headers":{"Authorization":"[removed]\r\n"}});
        if redacted {
            envelope["capture"] = json!({"redacted_headers":["authorization"]});
        }
        let summary = Engine::default()
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: envelope.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        let errors = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .filter(|finding| finding.severity == pixellint_core::Severity::Error)
            .count();
        assert_eq!(errors, expected_errors, "{summary:?}");
    }
}

#[test]
fn format_assertions_on_observed_aliases_keep_their_value_findings() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({"id":"custom/har-peer","display_name":"HAR peer oracle",
        "description":"Format assertions inspect observed values","docs":"https://example.org/har-peer",
        "match":{"hosts":["har.example"]},"http":{"params":[
            {"name":"headers.x-key","aliases":["headers.x-alt"]}],"rules":[
            {"code":"custom.har-peer.visible_format","kind":"format","param":"headers.x-key",
             "format":{"kind":"enum","values":["good"]},"severity":"error","message":"Invalid observed value."},
            {"code":"custom.har-peer.visible_forbidden","kind":"forbid_value_pattern","params":["headers.x-key"],
             "pattern":"^bad$","severity":"error","message":"Forbidden observed value."}]}}).to_string()).unwrap();
    let envelope = json!({"url":"https://har.example/events","method":"GET","headers":{"X-Key":"bad"},"capture":{"headers_unavailable":true}});
    let summary = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            0,
        )
        .unwrap();
    let found: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect();
    assert_eq!(
        found,
        [
            "core.request.capture_incomplete",
            "custom.har-peer.visible_format",
            "custom.har-peer.visible_forbidden"
        ]
    );
}

#[test]
fn dotted_header_names_keep_exact_redaction_and_observed_value_paths() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({"id":"custom/har-peer","display_name":"HAR peer oracle",
        "description":"Header names remain literal object keys","docs":"https://example.org/har-peer",
        "match":{"hosts":["har.example"]},"http":{"params":[
          {"name":"credential","root_path":"headers[\"x.key\"]","requirement":"required",
           "format":{"kind":"enum","values":["good"]}}]}}).to_string()).unwrap();
    for (headers, capture, expected_errors) in [
        (
            json!({"X.Key":"bad\r\n"}),
            json!({"redacted_headers":["x.key"]}),
            0,
        ),
        (json!({}), json!({"unavailable_headers":["x.key"]}), 0),
        (
            json!({"X.Key":"bad"}),
            json!({"headers_unavailable":true}),
            1,
        ),
    ] {
        let envelope = json!({"url":"https://har.example/events","method":"GET","headers":headers,"capture":capture});
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: envelope.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        let errors = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .filter(|finding| finding.severity == pixellint_core::Severity::Error)
            .count();
        assert_eq!(errors, expected_errors, "{summary:?}");
    }
}
