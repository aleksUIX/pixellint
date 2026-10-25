//! The Top Mail.ru counter profile is bound to the reviewed producer and route.

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};

fn request(kind: ArtifactKind, artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.to_owned(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Template,
    }
}

#[test]
fn reviewed_counter_options_apply_consistently_across_url_like_inputs() {
    let engine = Engine::default();
    let url = "https://top-fwz1.mail.ru/counter?id=12345;js=13;age=128;gender=3";
    let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
    for (kind, artifact) in [
        (ArtifactKind::Url, url),
        (ArtifactKind::VastTracker, url),
        (ArtifactKind::NetworkRequest, capture.as_str()),
    ] {
        let summary = engine
            .validate(&request(kind, artifact), &ValidationOptions::default())
            .unwrap();
        assert_eq!(
            summary.coverage.vendor_rulepacks,
            ["vendor/top-mail-counter"]
        );
        for code in [
            "vendor.top-mail-counter.param.age.invalid",
            "vendor.top-mail-counter.param.gender.invalid",
        ] {
            assert!(
                summary
                    .reports
                    .iter()
                    .flat_map(|r| &r.violations)
                    .any(|v| v.code == code),
                "{kind:?}: {summary:?}"
            );
        }
    }
}

#[test]
fn demographic_template_checks_do_not_apply_to_other_producer_versions() {
    let engine = Engine::default();
    for query in [
        "id=12345;age=999;gender=other",
        "id=12345;js=na;age=999;gender=other",
        "id=12345;js=14;age=999;gender=other",
    ] {
        let summary = engine
            .validate(
                &request(
                    ArtifactKind::Url,
                    &format!("https://top-fwz1.mail.ru/counter?{query}"),
                ),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(summary.reports.iter().flat_map(|r| &r.violations).all(|v| {
            !v.code.starts_with("vendor.top-mail-counter.param.age.")
                && !v.code.starts_with("vendor.top-mail-counter.param.gender.")
        }));
    }
}

#[test]
fn loaders_goal_trackers_mail_siblings_and_lookalikes_stay_outside_the_pack() {
    let engine = Engine::default();
    for url in [
        "https://top-fwz1.mail.ru/tracker?id=12345",
        "https://top-fwz1.mail.ru/js/code.js",
        "https://top-fwz1.mail.ru/counter/?id=12345",
        "https://top-fwz1.mail.ru/counter/extra?id=12345",
        "https://rs.mail.ru/counter?id=12345",
        "https://sub.top-fwz1.mail.ru/counter?id=12345",
        "https://top-fwz1.mail.ru.example/counter?id=12345",
    ] {
        let summary = engine
            .validate(
                &request(ArtifactKind::Url, url),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(
            !summary
                .coverage
                .vendor_rulepacks
                .iter()
                .any(|p| p == "vendor/top-mail-counter"),
            "{url}: {summary:?}"
        );
    }
}
