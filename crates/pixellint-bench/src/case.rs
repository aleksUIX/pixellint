use pixellint_core::{ArtifactKind, ExpansionState, ValidationRequest};

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub kind: ArtifactKind,
    pub artifact: String,
    pub expansion_state: ExpansionState,
    pub claimed_vendor: Option<String>,
}

impl Case {
    pub fn kind_label(&self) -> &'static str {
        kind_label(self.kind)
    }

    pub fn bytes(&self) -> usize {
        self.artifact.len()
    }

    pub fn request(&self) -> ValidationRequest {
        ValidationRequest {
            artifact_kind: self.kind,
            artifact: self.artifact.clone(),
            claimed_vendor: self.claimed_vendor.clone(),
            expansion_state: self.expansion_state,
        }
    }
}

pub fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Url => "url",
        ArtifactKind::HtmlSnippet => "html",
        ArtifactKind::JavaScriptSnippet => "js",
        ArtifactKind::GtmTemplate => "gtm",
        ArtifactKind::NetworkRequest => "request",
        ArtifactKind::VastTracker => "vast",
        ArtifactKind::ServerPostback => "postback",
        ArtifactKind::JsonPayload => "json",
        ArtifactKind::Unknown => "unknown",
    }
}

pub fn parse_kind(value: &str) -> Option<ArtifactKind> {
    Some(match value {
        "url" => ArtifactKind::Url,
        "html" => ArtifactKind::HtmlSnippet,
        "js" => ArtifactKind::JavaScriptSnippet,
        "gtm" => ArtifactKind::GtmTemplate,
        "request" => ArtifactKind::NetworkRequest,
        "vast" => ArtifactKind::VastTracker,
        "postback" => ArtifactKind::ServerPostback,
        "json" => ArtifactKind::JsonPayload,
        "unknown" => ArtifactKind::Unknown,
        _ => return None,
    })
}

pub fn expansion_label(state: ExpansionState) -> &'static str {
    match state {
        ExpansionState::Unknown => "unknown",
        ExpansionState::Template => "template",
        ExpansionState::Fired => "fired",
    }
}

pub fn parse_expansion(value: Option<&str>) -> ExpansionState {
    match value.unwrap_or("unknown") {
        "template" => ExpansionState::Template,
        "fired" => ExpansionState::Fired,
        _ => ExpansionState::Unknown,
    }
}

pub fn tier_of(size: usize) -> &'static str {
    if size < 200 {
        "tiny"
    } else if size < 1_000 {
        "small"
    } else if size < 10_000 {
        "medium"
    } else if size < 100_000 {
        "large"
    } else {
        "xlarge"
    }
}

pub fn human_bytes(n: usize) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / 1024.0 / 1024.0)
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

pub fn uniquify(kind: ArtifactKind, artifact: &str, n: u64) -> String {
    match kind {
        ArtifactKind::JsonPayload => uniquify_json(artifact, n),
        _ => uniquify_url(artifact, n),
    }
}

fn uniquify_url(artifact: &str, n: u64) -> String {
    let trimmed = artifact.trim();
    if trimmed.contains('?') {
        format!("{trimmed}&_b={n}")
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        format!("{trimmed}?_b={n}")
    } else {
        format!("{trimmed}#{n}")
    }
}

fn uniquify_json(artifact: &str, n: u64) -> String {
    let mut value: serde_json::Value = match serde_json::from_str(artifact.trim()) {
        Ok(value) => value,
        Err(_) => return artifact.to_string(),
    };
    stamp_json(&mut value, n);
    serde_json::to_string(&value).unwrap_or_else(|_| artifact.to_string())
}

fn stamp_json(value: &mut serde_json::Value, n: u64) {
    let Some(map) = value.as_object_mut() else {
        return;
    };
    if let Some(serde_json::Value::Array(events)) = map.get_mut("data") {
        for (index, event) in events.iter_mut().enumerate() {
            if let Some(event) = event.as_object_mut() {
                event.insert(
                    "event_id".to_string(),
                    serde_json::Value::String(format!("bench-{n}-{index}")),
                );
            }
        }
        return;
    }
    map.insert(
        "event_id".to_string(),
        serde_json::Value::String(format!("bench-{n}")),
    );
}
