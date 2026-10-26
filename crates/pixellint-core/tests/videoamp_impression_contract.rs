use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest, VendorDirectory,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

const PACK_ID: &str = "vendor/videoamp-impression";
const PACK: &str = include_str!("../rulepacks/vendor/videoamp-impression.json");

#[derive(Deserialize)]
struct SourceCases {
    pack_id: String,
    engine_derived: bool,
    collector_calls: usize,
    cases: Vec<SourceCase>,
}
#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    category: String,
    expect_pack: bool,
    expected_vendor_codes: Vec<String>,
    source_url: String,
    source_requirement: String,
    #[serde(default)]
    expansion_state: Option<String>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-videoamp-impression")
}
fn engine() -> Engine {
    let mut engine = Engine::default();
    if !engine.list_rulepacks().iter().any(|p| p.id == PACK_ID) {
        engine.register_manifest_json(PACK).unwrap();
    }
    if engine.directory().lookup_host("b.videoamp.com").is_none() {
        engine.merge_directory(VendorDirectory::from_json(&json!({"entries":[{"vendor":"videoamp","display_name":"VideoAmp","category":"analytics","hosts":["b.videoamp.com"],"rulepack":PACK_ID}]}).to_string()).unwrap()).unwrap();
    }
    engine
}
fn request(kind: &str, fixture: &str, expansion: Option<&str>) -> ValidationRequest {
    ValidationRequest {
        artifact: fs::read_to_string(root().join(fixture)).unwrap(),
        artifact_kind: match kind {
            "url" => ArtifactKind::Url,
            "vast" => ArtifactKind::VastTracker,
            "postback" => ArtifactKind::ServerPostback,
            "request" => ArtifactKind::NetworkRequest,
            "json" => ArtifactKind::JsonPayload,
            _ => panic!("unknown kind {kind}"),
        },
        claimed_vendor: None,
        expansion_state: if expansion == Some("template") {
            ExpansionState::Template
        } else {
            ExpansionState::Unknown
        },
    }
}
fn source_cases(categories: &[&str]) {
    let source: SourceCases =
        serde_json::from_str(&fs::read_to_string(root().join("source-cases.json")).unwrap())
            .unwrap();
    assert_eq!(source.pack_id, PACK_ID);
    assert!(!source.engine_derived);
    assert_eq!(source.collector_calls, 0);
    let engine = engine();
    let mut checked = 0;
    for case in source
        .cases
        .into_iter()
        .filter(|c| categories.contains(&c.category.as_str()))
    {
        assert!(case.source_url.starts_with("https://"));
        assert!(!case.source_requirement.is_empty());
        let summary = engine
            .validate_at(
                &request(&case.kind, &case.fixture, case.expansion_state.as_deref()),
                &ValidationOptions::default(),
                1792886400,
            )
            .unwrap();
        let report = summary.reports.iter().find(|r| r.plugin_id == PACK_ID);
        assert_eq!(report.is_some(), case.expect_pack, "{} selection", case.id);
        let mut actual: Vec<_> = report
            .iter()
            .flat_map(|r| r.violations.iter().map(|v| v.code.clone()))
            .collect();
        actual.sort();
        assert_eq!(
            actual, case.expected_vendor_codes,
            "{} source contract",
            case.id
        );
        if let Some(report) = report {
            for finding in &report.violations {
                assert_ne!(
                    finding.severity,
                    Severity::Error,
                    "{} remains advisory",
                    case.id
                );
                let info_names = [
                    "kv27",
                    "avid",
                    "caid",
                    "plid",
                    "publisherId",
                    "siteId",
                    "lineItemId",
                    "kv7",
                    "kv10",
                    "kv26",
                    "cb",
                    "priceBid",
                    "pricePaid",
                    "kv1",
                    "kv15",
                    "kv25",
                    "kv28",
                    "kv23",
                    "kv9",
                    "kv13",
                    "kv44",
                ];
                let info = PACK_ID == "vendor/pixalate-analytics"
                    && info_names
                        .iter()
                        .any(|name| finding.code.ends_with(&format!(".param.{name}.missing")));
                assert_eq!(
                    finding.severity,
                    if info {
                        Severity::Info
                    } else {
                        Severity::Warning
                    },
                    "{} source severity",
                    case.id
                );

                assert!(
                    finding
                        .source
                        .reference
                        .as_deref()
                        .is_some_and(|s| s.starts_with("https://"))
                );
                let documented_type = finding.code.ends_with(".kv4.invalid")
                    || finding.code.ends_with(".kv11.invalid")
                    || finding.code.ends_with(".kv55.invalid")
                    || finding.code.ends_with(".kv4.missing")
                    || finding.code.ends_with(".kv11.missing")
                    || finding.code.contains(".http.method.");
                if documented_type {
                    assert_eq!(finding.source.level, RuleSourceLevel::OfficialVendor);
                }
            }
        }
        let findings:Vec<_>=summary.reports.iter().flat_map(|r|r.violations.iter().map(move|v|json!({"plugin":r.plugin_id,"code":v.code,"severity":match v.severity {Severity::Error=>"error",Severity::Warning=>"warning",Severity::Info=>"info"}}))).collect();
        println!(
            "SOURCE_OBSERVATION\t{}",
            json!({"pack":PACK_ID,"id":case.id,"plugins":summary.reports.iter().map(|r|r.plugin_id.clone()).collect::<Vec<_>>(),"findings":findings,"ok":summary.is_ok()})
        );
        checked += 1;
    }
    assert!(checked > 0);
}
#[test]
fn source_and_field_boundaries() {
    source_cases(&["source", "field"]);
}
#[test]
fn carriers_duplicates_and_native_shapes() {
    source_cases(&["carrier"]);
}
#[test]
fn endpoint_scope_controls() {
    source_cases(&["selector"]);
}
#[test]
fn owned_golden_corpus() {
    let cases: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(root().join("manifest.json")).unwrap()).unwrap();
    assert!(!cases.is_empty());
    let engine = engine();
    for case in cases {
        let summary = engine
            .validate_at(
                &request(
                    case["kind"].as_str().unwrap(),
                    case["fixture"].as_str().unwrap(),
                    case["expansion_state"].as_str(),
                ),
                &ValidationOptions::default(),
                1792886400,
            )
            .unwrap();
        let codes: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| r.violations.iter().map(|v| v.code.clone()))
            .collect();
        let expected: Vec<String> = serde_json::from_value(case["expected_codes"].clone()).unwrap();
        assert_eq!(codes, expected, "{}", case["id"]);
        for (key, severity) in [
            ("expected_errors", Severity::Error),
            ("expected_warnings", Severity::Warning),
            ("expected_infos", Severity::Info),
        ] {
            let n = summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .filter(|v| v.severity == severity)
                .count();
            assert_eq!(
                n,
                case[key].as_u64().unwrap() as usize,
                "{} {key}",
                case["id"]
            );
        }
        assert_eq!(summary.is_ok(), case["expected_ok"].as_bool().unwrap());
        assert_eq!(
            summary
                .reports
                .iter()
                .map(|r| r.plugin_id.clone())
                .collect::<Vec<_>>(),
            serde_json::from_value::<Vec<String>>(case["expected_plugins"].clone()).unwrap()
        );
    }
}
