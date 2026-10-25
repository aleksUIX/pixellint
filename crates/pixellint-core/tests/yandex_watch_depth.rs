use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationReport, ValidationRequest,
};
use serde_json::json;

fn report(artifact: String, artifact_kind: ArtifactKind) -> ValidationReport {
    let summary = Engine::default()
        .validate_at(
            &ValidationRequest {
                artifact,
                artifact_kind,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
            1792886400,
        )
        .unwrap();
    summary
        .reports
        .into_iter()
        .find(|r| r.plugin_id == "vendor/yandex-watch")
        .unwrap()
}

#[test]
fn pinned_source_outputs_preserve_image_form_beacon_and_native_json_branches() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-yandex-watch/source-cases-depth.json"
    ))
    .unwrap();
    assert_eq!(source["engine_derived"], false);
    assert_eq!(source["network_calls"], 0);
    for case in source["cases"].as_array().unwrap() {
        let result = report(case["capture"].to_string(), ArtifactKind::NetworkRequest);
        assert!(result.violations.is_empty(), "{}: {result:?}", case["id"]);
    }
}

#[test]
fn runtime_url_limit_counts_utf16_units_and_does_not_apply_to_noscript() {
    for (count, warnings) in [(1024, 0), (1025, 1)] {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("browser-info", "pv:1:st:1792886400")
            .append_pair("page-url", &"😀".repeat(count))
            .finish();
        let result = report(
            format!("https://mc.yandex.ru/watch/123456?{query}"),
            ArtifactKind::Url,
        );
        assert_eq!(result.violations.len(), warnings);
        if warnings > 0 {
            assert_eq!(
                result.violations[0].code,
                "vendor.yandex-watch.param.page-url.invalid"
            );
            assert_eq!(result.violations[0].severity, Severity::Warning);
        }
    }
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("page-url", &"a".repeat(3000))
        .append_pair("charset", "legacy")
        .append_pair("site-info", "not JSON")
        .finish();
    let result = report(
        format!("https://mc.yandex.ru/watch/123456?{query}"),
        ArtifactKind::Url,
    );
    assert!(result.violations.is_empty());
}

#[test]
fn selected_form_duplicates_and_existing_query_precedence_remain_explicit() {
    let base = "https://mc.yandex.ru/watch/123456?browser-info=pv%3A0%3Ast%3A1792886400";
    for (query, body, warnings) in [
        ("", "site-info=%7B%7D&site-info=bad", 1),
        ("&site-info=%7B%7D", "site-info=bad", 0),
        ("&site-info=bad", "site-info=%7B%7D", 1),
        ("&site-info=", "site-info=bad", 0),
    ] {
        let capture = json!({"url": format!("{base}{query}"), "method":"POST", "headers":{"Content-Type":"application/x-www-form-urlencoded"}, "body":body});
        let result = report(capture.to_string(), ArtifactKind::NetworkRequest);
        assert_eq!(result.violations.len(), warnings, "{capture}");
        for finding in result.violations {
            assert_eq!(finding.code, "vendor.yandex-watch.body.site-info.invalid");
            assert_eq!(finding.severity, Severity::Warning);
            assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
            assert!(
                finding
                    .source
                    .reference
                    .unwrap()
                    .contains("47a80d0f0f8278fbb32309b5444b801eda99556e")
            );
        }
    }
}

#[test]
fn encoded_json_accepts_lone_surrogates_null_scalars_and_large_numbers() {
    for value in [
        r#"{"custom":"\ud800"}"#,
        "null",
        "false",
        "7",
        "1e99999",
        r#""""#,
        "[]",
    ] {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("browser-info", "pv:1")
            .append_pair("site-info", value)
            .finish();
        assert!(
            report(
                format!("https://mc.yandex.ru/watch/123456?{query}"),
                ArtifactKind::Url
            )
            .violations
            .is_empty(),
            "{value}"
        );
    }
}
