use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest,
};
use serde_json::json;

fn validate(artifact: &str, kind: ArtifactKind, clock: i64) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate_at(
            &ValidationRequest {
                artifact_kind: kind,
                artifact: artifact.into(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
            clock,
        )
        .unwrap()
}
fn codes(result: &pixellint_core::ValidationSummary) -> Vec<&str> {
    result
        .reports
        .iter()
        .flat_map(|r| &r.violations)
        .map(|v| v.code.as_str())
        .collect()
}
fn cj(extra: &str) -> String {
    format!(
        "https://www.emjcd.com/u?CID=123&TYPE=456&OID=synthetic&METHOD=S2S&CJEVENT=abc123&{extra}"
    )
}

#[test]
fn cj_item_identity_checks_survive_vast_and_complete_request_envelopes() {
    let url = cj("ITEM1=sku&AMT2=1&QTY1=1");
    for kind in [
        ArtifactKind::Url,
        ArtifactKind::VastTracker,
        ArtifactKind::ServerPostback,
        ArtifactKind::NetworkRequest,
    ] {
        let artifact = if kind == ArtifactKind::NetworkRequest {
            json!({"url":url,"method":"GET","headers":{}}).to_string()
        } else {
            url.clone()
        };
        let result = validate(&artifact, kind, 1_792_584_000);
        assert_eq!(result.coverage().vendor_rulepacks, ["vendor/cj"]);
        assert_eq!(codes(&result), ["vendor.cj.items_incomplete"]);
        let finding = result
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .next()
            .unwrap();
        if kind == ArtifactKind::NetworkRequest {
            assert!(finding.targets.is_empty());
            continue;
        }
        assert_eq!(finding.targets.len(), 3);
        assert!(
            finding
                .targets
                .iter()
                .any(|t| t.name.as_deref() == Some("AMT2"))
        );
    }
}

#[test]
fn cj_future_timestamp_uses_the_supplied_clock_and_receipt_time_defaults_remain_valid() {
    for extra in ["", "eventTime=", "EVENTTIME=2026-10-22T12%3A00%3A00Z"] {
        assert!(codes(&validate(&cj(extra), ArtifactKind::Url, 1_792_584_000)).is_empty());
    }
    let result = validate(
        &cj("eventTime=2999-01-01T00%3A00%3A00Z"),
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert_eq!(codes(&result), ["vendor.cj.event_time_future"]);
}

#[test]
fn cj_legacy_pixels_do_not_inherit_s2s_requiredness_or_item_grammar() {
    let result = validate(
        "https://www.emjcd.com/u?CID=123&TYPE=456&OID=legacy%20order&ITEM3=legacy",
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert!(codes(&result).is_empty());
}

#[test]
fn indexed_rule_rejects_invalid_configuration_and_uncontracted_families() {
    let manifest = json!({"id":"custom/indexed","display_name":"Indexed groups","description":"Checks item families.","source_level":"heuristic","match":{"hosts":["example.test"]},"params":[{"name":"ITEM","name_pattern":"^ITEM[0-9]+$"},{"name":"AMT","name_pattern":"^AMT[0-9]+$"}],"rules":[{"code":"custom.indexed.groups","kind":"indexed_param_groups","params":["ITEM","AMT"],"max_index":100,"contiguous":true,"severity":"error","message":"Complete groups."}]});
    assert!(ManifestRulePack::from_json(&manifest.to_string()).is_ok());
    for patch in [
        json!({"max_index":0}),
        json!({"params":["ITEM"]}),
        json!({"optional_params":["ITEM"]}),
        json!({"params":["ITEM","UNKNOWN"]}),
    ] {
        let mut value = manifest.clone();
        for (key, v) in patch.as_object().unwrap() {
            value["rules"][0][key] = v.clone();
        }
        assert!(ManifestRulePack::from_json(&value.to_string()).is_err());
    }
}

#[test]
fn rakuten_adjustment_rows_and_encoded_sku_separators_do_not_invent_list_mismatches() {
    let result = validate(
        "https://track.linksynergy.com/ep?mid=123&ord=synthetic&skulist=sku%7Cembedded%7CDISCOUNT&qlist=1%7C0&amtlist=100%7C-20&cur=USD",
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert!(codes(&result).is_empty());
    let result = validate(
        "https://track.linksynergy.com/ep?mid=123&ord=synthetic&skulist=one&qlist=1%7C0&amtlist=100",
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert_eq!(codes(&result), ["vendor.rakuten.numeric_lists_misaligned"]);
}

#[test]
fn mntn_unmapped_product_fields_and_nonrevenue_conversions_keep_their_scope() {
    let result = validate(
        "https://px.mountain.com/sti?shaid=123&cb=opaque&conv=1&shpp=null&shpi=null",
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert!(codes(&result).is_empty());
    let result = validate(
        "https://px.mountain.com/sti?shaid=123&cb=opaque&shpp=money&gdpr=bad",
        ArtifactKind::Url,
        1_792_584_000,
    );
    assert_eq!(
        codes(&result),
        [
            "core.privacy.gdpr_invalid",
            "vendor.mntn-image.param.shpp.invalid"
        ]
    );
}
