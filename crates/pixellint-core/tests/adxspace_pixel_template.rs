//! Published counting-pixel checks remain advisory and endpoint-specific.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, ValidatorPlugin,
};

const PACK: &str = include_str!("../rulepacks/vendor/adxspace-pixel.json");
const DOC: &str = "https://wiki.00px.com.br/en/technologies/pixel";

fn request(artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::VastTracker,
        artifact: artifact.to_owned(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn installation_stubs_warn_with_template_evidence_without_invented_cachebuster_types() {
    let pack = ManifestRulePack::from_json(PACK).unwrap();
    for stub in [
        "INSERIR+CACHEBUSTER",
        "INSERIR%20CACHEBUSTER",
        "INSERIR%2BCACHEBUSTER",
    ] {
        let report = pack.validate(&request(&format!(
            "https://00px.net/pixel/opaque-token/e.gif?t={stub}"
        )));
        assert_eq!(report.violations.len(), 1);
        let finding = &report.violations[0];
        assert_eq!(
            finding.code,
            "vendor.adxspace-pixel.cachebuster.installation_stub"
        );
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
        assert_eq!(finding.source.reference.as_deref(), Some(DOC));
    }
    for query in [
        "",
        "?t=",
        "?t=0",
        "?t=nonce-with-letters",
        "?t=[CACHEBUSTING]",
        "?t=prefix-INSERIR+CACHEBUSTER",
        "?t=INSERIR+CACHEBUSTER-suffix",
        "?t=inserir+cachebuster",
        "?other=INSERIR+CACHEBUSTER",
    ] {
        let report = pack.validate(&request(&format!("https://00px.net/pixel/x/e.gif{query}")));
        assert!(report.violations.is_empty(), "{query}: {report:?}");
    }
}

#[test]
fn template_token_is_opaque_and_query_fields_cannot_fill_an_empty_path_slot() {
    let pack = ManifestRulePack::from_json(PACK).unwrap();
    for token in ["x", "opaque-token", "not-base64!", "123", "a%2Bb%3D"] {
        assert!(
            pack.validate(&request(&format!("https://00px.net/pixel/{token}/e.gif")))
                .violations
                .is_empty(),
            "{token}: the guide does not declare an encoding or length contract"
        );
    }
    for query in ["", "?creative_token=supplied-in-the-wrong-place"] {
        let report = pack.validate(&request(&format!("https://00px.net/pixel//e.gif{query}")));
        assert_eq!(report.violations.len(), 1, "{query}: {report:?}");
        assert_eq!(
            report.violations[0].code,
            "vendor.adxspace-pixel.param.creative_token.empty"
        );
        assert_eq!(report.violations[0].severity, Severity::Warning);
        assert_eq!(
            report.violations[0].source.level,
            RuleSourceLevel::OfficialTemplate
        );
    }
}

#[test]
fn video_trackers_and_unreviewed_pixel_routes_keep_core_checks_without_pack_selection() {
    let pack = ManifestRulePack::from_json(PACK).unwrap();
    let mut engine = Engine::default();
    if !engine
        .list_rulepacks()
        .iter()
        .any(|metadata| metadata.id == "vendor/adxspace-pixel")
    {
        engine.register_manifest_json(PACK).unwrap();
    }
    for url in [
        "https://00px.net/tracking/sample/starts?gdpr=NaN",
        "https://00px.net/vast/pixel/sample?gdpr=NaN",
        "https://00px.net/pixel/sample/other.gif?gdpr=NaN",
        "https://00px.net/pixel/sample/e.gif/extra?gdpr=NaN",
        "https://cdn.00px.net/pixel/sample/e.gif?gdpr=NaN",
        "https://00px.net.example.test/pixel/sample/e.gif?gdpr=NaN",
    ] {
        let submitted = request(url);
        assert!(!pack.supports(&submitted), "{url}: endpoint scope");
        let summary = engine
            .validate(&submitted, &ValidationOptions::default())
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .all(|report| report.plugin_id != "vendor/adxspace-pixel"),
            "{url}: video tracker scope is still uncovered"
        );
        assert!(
            summary.reports.iter().any(|report| {
                report.plugin_id == "core"
                    && report.violations.iter().any(|finding| {
                        finding.code == "core.privacy.gdpr_invalid"
                            && finding.severity == Severity::Error
                    })
            }),
            "{url}: ordinary privacy validation continues"
        );
        assert!(!summary.is_ok());
    }
}
