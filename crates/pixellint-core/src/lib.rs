use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use url::Url;

mod adobe_events;
mod adobe_products;
mod awin_basket;
mod braze_time;
mod currency;
mod decimal_sum;
pub mod directory;
pub mod document;
mod google_additional_consent;
mod gpp_structure;
mod javascript_date;
mod json;
pub mod manifest;
mod path_items;
mod prepare;
mod privacy;
mod query_segments;
mod tcf_sections;
mod timestamp;

pub use directory::{DIRECTORY_ID, DirectoryError, VendorDirectory, VendorEntry};
pub use document::{
    AggregatedArtifact, ArtifactOccurrence, DocumentArtifactInput, DocumentError,
    DocumentExtractor, DocumentReport, DocumentRequest, FindingCounts, document_request_from_json,
    document_request_from_value,
};
pub use manifest::{
    Assertion, IpVersion, JsonType, JsonTypes, ManifestError, ManifestRulePack, MatchSpec,
    PackRule, ParamContract, ParamStyle, Requirement, RuleCondition, RulePackManifest,
    StringNormalization, ValueFormat,
};
pub use prepare::PreparedArtifact;
pub use timestamp::TimestampUnit;

/// The vendor endpoint directory compiled into the crate.
pub const BUILTIN_VENDOR_DIRECTORY: &str = include_str!("../rulepacks/directory.json");

/// First-party vendor rulepacks compiled into the crate, as (id, manifest JSON).
///
/// Every pack here is a declarative manifest, the same format users write for
/// their own packs. Nothing about a first-party pack is privileged.
pub const BUILTIN_VENDOR_MANIFESTS: &[(&str, &str)] = &[
    (
        "vendor/adform",
        include_str!("../rulepacks/vendor/adform.json"),
    ),
    (
        "vendor/adjust",
        include_str!("../rulepacks/vendor/adjust.json"),
    ),
    (
        "vendor/adobe-analytics",
        include_str!("../rulepacks/vendor/adobe-analytics.json"),
    ),
    (
        "vendor/adobe-dcs-event",
        include_str!("../rulepacks/vendor/adobe-dcs-event.json"),
    ),
    (
        "vendor/adobe-dcs-id",
        include_str!("../rulepacks/vendor/adobe-dcs-id.json"),
    ),
    (
        "vendor/adobe-ecid",
        include_str!("../rulepacks/vendor/adobe-ecid.json"),
    ),
    (
        "vendor/adobe-web-sdk",
        include_str!("../rulepacks/vendor/adobe-web-sdk.json"),
    ),
    (
        "vendor/amazon-ads",
        include_str!("../rulepacks/vendor/amazon-ads.json"),
    ),
    (
        "vendor/amazon-vfw",
        include_str!("../rulepacks/vendor/amazon-vfw.json"),
    ),
    (
        "vendor/amplitude",
        include_str!("../rulepacks/vendor/amplitude.json"),
    ),
    (
        "vendor/amplitude-group-identify",
        include_str!("../rulepacks/vendor/amplitude-group-identify.json"),
    ),
    (
        "vendor/amplitude-identify",
        include_str!("../rulepacks/vendor/amplitude-identify.json"),
    ),
    (
        "vendor/appsflyer",
        include_str!("../rulepacks/vendor/appsflyer.json"),
    ),
    (
        "vendor/appsflyer-onelink-impression",
        include_str!("../rulepacks/vendor/appsflyer-onelink-impression.json"),
    ),
    ("vendor/awin", include_str!("../rulepacks/vendor/awin.json")),
    (
        "vendor/awin-basket",
        include_str!("../rulepacks/vendor/awin-basket.json"),
    ),
    (
        "vendor/awin-mastertag",
        include_str!("../rulepacks/vendor/awin-mastertag.json"),
    ),
    (
        "vendor/baidu",
        include_str!("../rulepacks/vendor/baidu.json"),
    ),
    (
        "vendor/branch",
        include_str!("../rulepacks/vendor/branch.json"),
    ),
    (
        "vendor/braze",
        include_str!("../rulepacks/vendor/braze.json"),
    ),
    (
        "vendor/brevo",
        include_str!("../rulepacks/vendor/brevo.json"),
    ),
    (
        "vendor/brevo-js",
        include_str!("../rulepacks/vendor/brevo-js.json"),
    ),
    (
        "vendor/chartbeat",
        include_str!("../rulepacks/vendor/chartbeat.json"),
    ),
    ("vendor/cj", include_str!("../rulepacks/vendor/cj.json")),
    (
        "vendor/cloudflare",
        include_str!("../rulepacks/vendor/cloudflare.json"),
    ),
    (
        "vendor/cm360-tracking-ad",
        include_str!("../rulepacks/vendor/cm360-tracking-ad.json"),
    ),
    (
        "vendor/cm360-vast-event",
        include_str!("../rulepacks/vendor/cm360-vast-event.json"),
    ),
    (
        "vendor/comscore",
        include_str!("../rulepacks/vendor/comscore.json"),
    ),
    (
        "vendor/cookiebot",
        include_str!("../rulepacks/vendor/cookiebot.json"),
    ),
    (
        "vendor/cookiebot-declaration",
        include_str!("../rulepacks/vendor/cookiebot-declaration.json"),
    ),
    (
        "vendor/crazyegg",
        include_str!("../rulepacks/vendor/crazyegg.json"),
    ),
    (
        "vendor/criteo",
        include_str!("../rulepacks/vendor/criteo.json"),
    ),
    (
        "vendor/criteo-retail-media",
        include_str!("../rulepacks/vendor/criteo-retail-media.json"),
    ),
    (
        "vendor/didomi",
        include_str!("../rulepacks/vendor/didomi.json"),
    ),
    (
        "vendor/doubleverify",
        include_str!("../rulepacks/vendor/doubleverify.json"),
    ),
    (
        "vendor/doubleverify-event",
        include_str!("../rulepacks/vendor/doubleverify-event.json"),
    ),
    (
        "vendor/drift",
        include_str!("../rulepacks/vendor/drift.json"),
    ),
    (
        "vendor/flashtalking",
        include_str!("../rulepacks/vendor/flashtalking.json"),
    ),
    (
        "vendor/floodlight",
        include_str!("../rulepacks/vendor/floodlight.json"),
    ),
    (
        "vendor/freewheel",
        include_str!("../rulepacks/vendor/freewheel.json"),
    ),
    (
        "vendor/google-ad-manager",
        include_str!("../rulepacks/vendor/google-ad-manager.json"),
    ),
    (
        "vendor/google-ads-conversion",
        include_str!("../rulepacks/vendor/google-ads-conversion.json"),
    ),
    (
        "vendor/google-ads-call-conversions",
        include_str!("../rulepacks/vendor/google-ads-call-conversions.json"),
    ),
    (
        "vendor/google-ads-click-conversions",
        include_str!("../rulepacks/vendor/google-ads-click-conversions.json"),
    ),
    (
        "vendor/google-ads-conversion-adjustments",
        include_str!("../rulepacks/vendor/google-ads-conversion-adjustments.json"),
    ),
    (
        "vendor/google-analytics",
        include_str!("../rulepacks/vendor/google-analytics.json"),
    ),
    (
        "vendor/google-analytics-collect",
        include_str!("../rulepacks/vendor/google-analytics-collect.json"),
    ),
    (
        "vendor/google-tag-manager",
        include_str!("../rulepacks/vendor/google-tag-manager.json"),
    ),
    ("vendor/heap", include_str!("../rulepacks/vendor/heap.json")),
    (
        "vendor/heap-classic",
        include_str!("../rulepacks/vendor/heap-classic.json"),
    ),
    (
        "vendor/heap-identify",
        include_str!("../rulepacks/vendor/heap-identify.json"),
    ),
    (
        "vendor/heap-track",
        include_str!("../rulepacks/vendor/heap-track.json"),
    ),
    (
        "vendor/heap-user-properties",
        include_str!("../rulepacks/vendor/heap-user-properties.json"),
    ),
    (
        "vendor/heap-account-properties",
        include_str!("../rulepacks/vendor/heap-account-properties.json"),
    ),
    (
        "vendor/hotjar",
        include_str!("../rulepacks/vendor/hotjar.json"),
    ),
    (
        "vendor/hubspot",
        include_str!("../rulepacks/vendor/hubspot.json"),
    ),
    (
        "vendor/hubspot-pixel",
        include_str!("../rulepacks/vendor/hubspot-pixel.json"),
    ),
    ("vendor/ias", include_str!("../rulepacks/vendor/ias.json")),
    (
        "vendor/ias-video",
        include_str!("../rulepacks/vendor/ias-video.json"),
    ),
    ("vendor/id5", include_str!("../rulepacks/vendor/id5.json")),
    (
        "vendor/id5-ctv",
        include_str!("../rulepacks/vendor/id5-ctv.json"),
    ),
    (
        "vendor/impact",
        include_str!("../rulepacks/vendor/impact.json"),
    ),
    (
        "vendor/impact-conversions",
        include_str!("../rulepacks/vendor/impact-conversions.json"),
    ),
    (
        "vendor/intercom",
        include_str!("../rulepacks/vendor/intercom.json"),
    ),
    (
        "vendor/intercom-events",
        include_str!("../rulepacks/vendor/intercom-events.json"),
    ),
    ("vendor/iqm", include_str!("../rulepacks/vendor/iqm.json")),
    (
        "vendor/ispot",
        include_str!("../rulepacks/vendor/ispot.json"),
    ),
    (
        "vendor/ispot-conversion",
        include_str!("../rulepacks/vendor/ispot-conversion.json"),
    ),
    (
        "vendor/kantar",
        include_str!("../rulepacks/vendor/kantar.json"),
    ),
    (
        "vendor/kevel",
        include_str!("../rulepacks/vendor/kevel.json"),
    ),
    (
        "vendor/klaviyo",
        include_str!("../rulepacks/vendor/klaviyo.json"),
    ),
    (
        "vendor/kochava",
        include_str!("../rulepacks/vendor/kochava.json"),
    ),
    ("vendor/kwai", include_str!("../rulepacks/vendor/kwai.json")),
    (
        "vendor/linkedin",
        include_str!("../rulepacks/vendor/linkedin.json"),
    ),
    (
        "vendor/linkedin-conversions-api",
        include_str!("../rulepacks/vendor/linkedin-conversions-api.json"),
    ),
    (
        "vendor/liveramp-envelope",
        include_str!("../rulepacks/vendor/liveramp-envelope.json"),
    ),
    (
        "vendor/liveramp-envelope-refresh",
        include_str!("../rulepacks/vendor/liveramp-envelope-refresh.json"),
    ),
    (
        "vendor/lotame",
        include_str!("../rulepacks/vendor/lotame.json"),
    ),
    (
        "vendor/mailchimp",
        include_str!("../rulepacks/vendor/mailchimp.json"),
    ),
    (
        "vendor/matomo",
        include_str!("../rulepacks/vendor/matomo.json"),
    ),
    (
        "vendor/mediamath",
        include_str!("../rulepacks/vendor/mediamath.json"),
    ),
    (
        "vendor/mediamath-mobile",
        include_str!("../rulepacks/vendor/mediamath-mobile.json"),
    ),
    ("vendor/meta", include_str!("../rulepacks/vendor/meta.json")),
    (
        "vendor/meta-conversions-api",
        include_str!("../rulepacks/vendor/meta-conversions-api.json"),
    ),
    (
        "vendor/microsoft-clarity",
        include_str!("../rulepacks/vendor/microsoft-clarity.json"),
    ),
    (
        "vendor/microsoft-conversions-api",
        include_str!("../rulepacks/vendor/microsoft-conversions-api.json"),
    ),
    (
        "vendor/microsoft-uet",
        include_str!("../rulepacks/vendor/microsoft-uet.json"),
    ),
    (
        "vendor/mixpanel",
        include_str!("../rulepacks/vendor/mixpanel.json"),
    ),
    (
        "vendor/mixpanel-import",
        include_str!("../rulepacks/vendor/mixpanel-import.json"),
    ),
    (
        "vendor/mixpanel-engage",
        include_str!("../rulepacks/vendor/mixpanel-engage.json"),
    ),
    (
        "vendor/mixpanel-groups",
        include_str!("../rulepacks/vendor/mixpanel-groups.json"),
    ),
    (
        "vendor/mouseflow",
        include_str!("../rulepacks/vendor/mouseflow.json"),
    ),
    (
        "vendor/nextdoor-conversions-api",
        include_str!("../rulepacks/vendor/nextdoor-conversions-api.json"),
    ),
    (
        "vendor/nielsen",
        include_str!("../rulepacks/vendor/nielsen.json"),
    ),
    (
        "vendor/nielsen-config",
        include_str!("../rulepacks/vendor/nielsen-config.json"),
    ),
    (
        "vendor/nielsen-audit",
        include_str!("../rulepacks/vendor/nielsen-audit.json"),
    ),
    (
        "vendor/onetrust",
        include_str!("../rulepacks/vendor/onetrust.json"),
    ),
    (
        "vendor/openai",
        include_str!("../rulepacks/vendor/openai.json"),
    ),
    (
        "vendor/openai-conversions-api",
        include_str!("../rulepacks/vendor/openai-conversions-api.json"),
    ),
    (
        "vendor/oracle-bluekai",
        include_str!("../rulepacks/vendor/oracle-bluekai.json"),
    ),
    (
        "vendor/outbrain",
        include_str!("../rulepacks/vendor/outbrain.json"),
    ),
    (
        "vendor/pardot",
        include_str!("../rulepacks/vendor/pardot.json"),
    ),
    (
        "vendor/parsely",
        include_str!("../rulepacks/vendor/parsely.json"),
    ),
    (
        "vendor/parsely-collect",
        include_str!("../rulepacks/vendor/parsely-collect.json"),
    ),
    (
        "vendor/partnerize",
        include_str!("../rulepacks/vendor/partnerize.json"),
    ),
    (
        "vendor/pinterest",
        include_str!("../rulepacks/vendor/pinterest.json"),
    ),
    (
        "vendor/pinterest-conversions-api",
        include_str!("../rulepacks/vendor/pinterest-conversions-api.json"),
    ),
    (
        "vendor/plausible",
        include_str!("../rulepacks/vendor/plausible.json"),
    ),
    (
        "vendor/posthog",
        include_str!("../rulepacks/vendor/posthog.json"),
    ),
    (
        "vendor/quantcast",
        include_str!("../rulepacks/vendor/quantcast.json"),
    ),
    (
        "vendor/quora-conversions-api",
        include_str!("../rulepacks/vendor/quora-conversions-api.json"),
    ),
    (
        "vendor/rakuten",
        include_str!("../rulepacks/vendor/rakuten.json"),
    ),
    (
        "vendor/reddit",
        include_str!("../rulepacks/vendor/reddit.json"),
    ),
    (
        "vendor/reddit-conversions-api",
        include_str!("../rulepacks/vendor/reddit-conversions-api.json"),
    ),
    (
        "vendor/rudderstack",
        include_str!("../rulepacks/vendor/rudderstack.json"),
    ),
    (
        "vendor/segment",
        include_str!("../rulepacks/vendor/segment.json"),
    ),
    (
        "vendor/singular",
        include_str!("../rulepacks/vendor/singular.json"),
    ),
    (
        "vendor/snapchat",
        include_str!("../rulepacks/vendor/snapchat.json"),
    ),
    (
        "vendor/taboola",
        include_str!("../rulepacks/vendor/taboola.json"),
    ),
    (
        "vendor/taboola-s2s",
        include_str!("../rulepacks/vendor/taboola-s2s.json"),
    ),
    (
        "vendor/taboola-s2s-bulk",
        include_str!("../rulepacks/vendor/taboola-s2s-bulk.json"),
    ),
    (
        "vendor/taboola-unip",
        include_str!("../rulepacks/vendor/taboola-unip.json"),
    ),
    (
        "vendor/the-trade-desk",
        include_str!("../rulepacks/vendor/the-trade-desk.json"),
    ),
    (
        "vendor/tiktok",
        include_str!("../rulepacks/vendor/tiktok.json"),
    ),
    (
        "vendor/tiktok-events-api",
        include_str!("../rulepacks/vendor/tiktok-events-api.json"),
    ),
    (
        "vendor/tiktok-events-2",
        include_str!("../rulepacks/vendor/tiktok-events-2.json"),
    ),
    (
        "vendor/trustarc",
        include_str!("../rulepacks/vendor/trustarc.json"),
    ),
    (
        "vendor/trustarc-notice",
        include_str!("../rulepacks/vendor/trustarc-notice.json"),
    ),
    ("vendor/x", include_str!("../rulepacks/vendor/x.json")),
    (
        "vendor/x-conversions-api",
        include_str!("../rulepacks/vendor/x-conversions-api.json"),
    ),
    (
        "vendor/xandr",
        include_str!("../rulepacks/vendor/xandr.json"),
    ),
    (
        "vendor/xandr-sspx",
        include_str!("../rulepacks/vendor/xandr-sspx.json"),
    ),
    (
        "vendor/yahoo-conversions-api",
        include_str!("../rulepacks/vendor/yahoo-conversions-api.json"),
    ),
    (
        "vendor/yahoo-dot",
        include_str!("../rulepacks/vendor/yahoo-dot.json"),
    ),
    (
        "vendor/yandex-metrica",
        include_str!("../rulepacks/vendor/yandex-metrica.json"),
    ),
    (
        "vendor/yandex-watch",
        include_str!("../rulepacks/vendor/yandex-watch.json"),
    ),
    (
        "vendor/zendesk",
        include_str!("../rulepacks/vendor/zendesk.json"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Url,
    #[serde(rename = "html")]
    HtmlSnippet,
    #[serde(rename = "js")]
    JavaScriptSnippet,
    #[serde(rename = "gtm")]
    GtmTemplate,
    #[serde(rename = "request")]
    NetworkRequest,
    #[serde(rename = "vast")]
    VastTracker,
    #[serde(rename = "postback")]
    ServerPostback,
    /// A JSON request body, as the conversion APIs carry their events.
    #[serde(rename = "json")]
    JsonPayload,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionState {
    #[default]
    Unknown,
    Template,
    Fired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSourceLevel {
    Normative,
    OfficialVendor,
    OfficialTemplate,
    EcosystemReference,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleSource {
    pub level: RuleSourceLevel,
    pub name: String,
    pub reference: Option<String>,
}

impl RuleSource {
    pub fn normative(name: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            level: RuleSourceLevel::Normative,
            name: name.into(),
            reference: Some(reference.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationTargetComponent {
    WholeUrl,
    Scheme,
    Authority,
    UserInfo,
    Host,
    Port,
    Path,
    QueryParam,
    Fragment,
    /// A field inside a JSON request body, named by its path.
    BodyField,
    /// The request body as a whole.
    WholeBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViolationTarget {
    pub component: ViolationTargetComponent,
    pub name: Option<String>,
    pub value: Option<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub code: String,
    pub message: String,
    pub severity: Severity,
    pub field: Option<String>,
    pub fix_hint: Option<String>,
    pub source: RuleSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<ViolationTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulePackMetadata {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub source_level: RuleSourceLevel,
    /// Vendor slug the pack covers, when it covers one vendor's endpoints.
    pub vendor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationRequest {
    pub artifact_kind: ArtifactKind,
    pub artifact: String,
    pub claimed_vendor: Option<String>,
    pub expansion_state: ExpansionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ValidationOptions {
    pub only_rulepacks: Vec<String>,
    pub except_rulepacks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub plugin_id: String,
    pub detected_vendor: Option<String>,
    pub violations: Vec<Violation>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.violations
            .iter()
            .all(|violation| violation.severity != Severity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationSummary {
    pub reports: Vec<ValidationReport>,
}

impl ValidationSummary {
    pub fn is_ok(&self) -> bool {
        self.reports.iter().all(ValidationReport::is_ok)
    }
}

/// How the engine can shortlist this pack before calling [`ValidatorPlugin::supports`].
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum PluginRouting {
    /// Always a candidate. Used by `core` and custom plugins that do not
    /// advertise hosts.
    Always,
    /// Candidate when the artifact host or JSON shape can match this pack.
    Indexed {
        hosts: Vec<String>,
        suffixes: Vec<String>,
        json: bool,
    },
}

pub trait ValidatorPlugin: Send + Sync {
    fn metadata(&self) -> &RulePackMetadata;
    fn supports(&self, request: &ValidationRequest) -> bool;
    fn validate(&self, request: &ValidationRequest) -> ValidationReport;

    #[doc(hidden)]
    fn routing(&self) -> PluginRouting {
        PluginRouting::Always
    }

    #[doc(hidden)]
    fn supports_prepared(&self, prepared: &PreparedArtifact<'_>) -> bool {
        self.supports(prepared.request())
    }

    #[doc(hidden)]
    fn validate_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
        self.validate(prepared.request())
    }

    /// Whether a documented browser loader consumes this URL's fragment keys.
    #[doc(hidden)]
    fn accepts_client_fragment(&self, _prepared: &PreparedArtifact<'_>) -> bool {
        false
    }

    /// Establish documented destination context before core evaluates signals.
    #[doc(hidden)]
    fn prepare_vendor_context(&self, prepared: &mut PreparedArtifact<'_>) {
        if self.accepts_client_fragment(prepared) {
            prepared.mark_client_fragment_configuration();
        }
    }

    /// An explicit pack selection also checks malformed body envelopes that
    /// cannot satisfy the pack's automatic shape matcher.
    #[doc(hidden)]
    fn validate_selected_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
        self.validate_prepared(prepared)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    PluginNotFound(String),
    NoMatchingPlugin,
    NoRulepacksSelected,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PluginNotFound(plugin_id) => {
                write!(f, "rulepack not found: {plugin_id}")
            }
            Self::NoMatchingPlugin => write!(f, "no matching rulepack found"),
            Self::NoRulepacksSelected => write!(f, "no rulepacks remain after applying toggles"),
        }
    }
}

impl Error for EngineError {}

struct PluginEntry {
    plugin: Arc<dyn ValidatorPlugin>,
}

#[derive(Default)]
struct HostIndex {
    /// Plugin ids in BTreeMap order. Host buckets store slots into this vec
    /// instead of cloning the id string per host.
    id_by_slot: Vec<String>,
    exact: HashMap<String, Vec<u16>>,
    suffixes: HashMap<String, Vec<u16>>,
    json_ids: Vec<u16>,
    always_ids: Vec<u16>,
}

pub struct Engine {
    plugins: BTreeMap<String, PluginEntry>,
    directory: VendorDirectory,
    index: HostIndex,
}

impl Default for Engine {
    /// An engine with the `core` pack and every first-party vendor pack
    /// registered. Built-in manifests are compiled in tests, so a malformed one
    /// is a build-time bug rather than a runtime surprise.
    fn default() -> Self {
        let mut engine = Self::new();
        engine.set_directory(VendorDirectory::builtin());
        engine.insert_plugin(CoreRulePack::default());

        for (id, json) in BUILTIN_VENDOR_MANIFESTS {
            let pack = ManifestRulePack::from_json(json).unwrap_or_else(|error| {
                panic!("built-in rulepack `{id}` failed to compile: {error}")
            });
            engine.insert_plugin(pack);
        }

        engine.rebuild_index();
        engine
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            plugins: BTreeMap::new(),
            directory: VendorDirectory::default(),
            index: HostIndex::default(),
        }
    }

    /// Replaces the vendor directory. An empty directory disables endpoint
    /// attribution entirely.
    pub fn set_directory(&mut self, directory: VendorDirectory) {
        self.directory = directory;
    }

    /// The vendor directory this engine attributes endpoints with.
    pub fn directory(&self) -> &VendorDirectory {
        &self.directory
    }

    /// Adds overlay entries to the current directory. Duplicate hosts fail
    /// the merge, so a community file cannot steal a first-party attribution.
    pub fn merge_directory(&mut self, extra: VendorDirectory) -> Result<(), crate::DirectoryError> {
        self.directory.merge(extra)
    }

    pub fn register<P>(&mut self, plugin: P)
    where
        P: ValidatorPlugin + 'static,
    {
        self.insert_plugin(plugin);
        self.rebuild_index();
    }

    fn insert_plugin<P>(&mut self, plugin: P)
    where
        P: ValidatorPlugin + 'static,
    {
        self.plugins.insert(
            plugin.metadata().id.clone(),
            PluginEntry {
                plugin: Arc::new(plugin),
            },
        );
    }

    /// Compiles a declarative rulepack manifest and registers it.
    pub fn register_manifest_json(&mut self, json: &str) -> Result<(), ManifestError> {
        self.register(ManifestRulePack::from_json(json)?);
        Ok(())
    }

    /// Compiles a declarative rulepack manifest from disk and registers it.
    pub fn register_manifest_path(&mut self, path: impl AsRef<Path>) -> Result<(), ManifestError> {
        self.register(ManifestRulePack::from_path(path)?);
        Ok(())
    }

    pub fn list_rulepacks(&self) -> Vec<RulePackMetadata> {
        self.plugins
            .values()
            .map(|entry| entry.plugin.metadata().clone())
            .collect()
    }

    pub fn validate(
        &self,
        request: &ValidationRequest,
        options: &ValidationOptions,
    ) -> Result<ValidationSummary, EngineError> {
        self.validate_at(request, options, timestamp::current_unix_seconds())
    }

    /// Validates with an explicit Unix-seconds clock, shared by every plugin.
    /// Call this method on platforms without a native wall clock, including
    /// wasm32-unknown-unknown, and for reproducible timestamp boundaries.
    pub fn validate_at(
        &self,
        request: &ValidationRequest,
        options: &ValidationOptions,
        reference_time_unix_seconds: i64,
    ) -> Result<ValidationSummary, EngineError> {
        let mut prepared = PreparedArtifact::from_request_at(request, reference_time_unix_seconds);
        let plugins = self.select_plugins(&prepared, options)?;
        for plugin in &plugins {
            plugin.prepare_vendor_context(&mut prepared);
        }
        let mut reports: Vec<ValidationReport> = plugins
            .into_iter()
            .map(|plugin| {
                if options.only_rulepacks.is_empty() {
                    plugin.validate_prepared(&prepared)
                } else {
                    plugin.validate_selected_prepared(&prepared)
                }
            })
            .collect();

        // Vendor contracts cannot inspect an unparsed document. Syntax remains
        // an input invariant even when the caller selects only vendor packs.
        if prepared.json().is_some_and(Result::is_err)
            && !reports.iter().any(|report| report.plugin_id == "core")
        {
            reports.insert(0, CoreRulePack::default().validate_prepared(&prepared));
        }

        if let Some(report) = self.directory_report(&prepared, options, &reports) {
            reports.push(report);
        }

        Ok(ValidationSummary { reports })
    }

    fn rebuild_index(&mut self) {
        let mut index = HostIndex {
            id_by_slot: self.plugins.keys().cloned().collect(),
            ..HostIndex::default()
        };
        for (slot, entry) in self.plugins.values().enumerate() {
            let slot = u16::try_from(slot).expect("at most 65535 rulepacks");
            match entry.plugin.routing() {
                PluginRouting::Always => index.always_ids.push(slot),
                PluginRouting::Indexed {
                    hosts,
                    suffixes,
                    json,
                } => {
                    for host in hosts {
                        index.exact.entry(host).or_default().push(slot);
                    }
                    for suffix in suffixes {
                        index.suffixes.entry(suffix).or_default().push(slot);
                    }
                    if json {
                        index.json_ids.push(slot);
                    }
                }
            }
        }
        self.index = index;
    }

    fn candidate_ids<'a>(&'a self, prepared: &PreparedArtifact<'_>) -> Vec<&'a str> {
        let mut ids: Vec<&str> = Vec::with_capacity(4);
        let mut seen = [0u64; 8];
        let mut push = |slot: u16| {
            let index = slot as usize;
            let word = index / 64;
            let bit = 1u64 << (index % 64);
            if word >= seen.len() {
                ids.push(self.index.id_by_slot[index].as_str());
                return;
            }
            if seen[word] & bit != 0 {
                return;
            }
            seen[word] |= bit;
            ids.push(self.index.id_by_slot[index].as_str());
        };
        for &slot in &self.index.always_ids {
            push(slot);
        }
        if prepared.wants_json() {
            for &slot in &self.index.json_ids {
                push(slot);
            }
        }
        if let Some(url) = prepared.url()
            && let Some(host) = url.host.as_deref()
        {
            if let Some(list) = self.index.exact.get(host) {
                for &slot in list {
                    push(slot);
                }
            }
            let mut label = host;
            loop {
                if let Some(list) = self.index.suffixes.get(label) {
                    for &slot in list {
                        push(slot);
                    }
                }
                match label.find('.') {
                    Some(index) => label = &label[index + 1..],
                    None => break,
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn select_plugins(
        &self,
        prepared: &PreparedArtifact<'_>,
        options: &ValidationOptions,
    ) -> Result<Vec<Arc<dyn ValidatorPlugin>>, EngineError> {
        self.ensure_known_rulepacks(&options.only_rulepacks)?;
        self.ensure_known_rulepacks(&options.except_rulepacks)?;

        if options.only_rulepacks.is_empty() && options.except_rulepacks.is_empty() {
            let selected = self
                .candidate_ids(prepared)
                .into_iter()
                .filter_map(|rulepack_id| {
                    self.plugins
                        .get(rulepack_id)
                        .filter(|entry| entry.plugin.supports_prepared(prepared))
                        .map(|entry| Arc::clone(&entry.plugin))
                })
                .collect::<Vec<_>>();

            return if selected.is_empty() {
                Err(EngineError::NoMatchingPlugin)
            } else {
                Ok(selected)
            };
        }

        let excluded: BTreeSet<&str> = options
            .except_rulepacks
            .iter()
            .map(String::as_str)
            .collect();

        // The directory is not a plugin, so it never takes part in plugin
        // selection. Asking for it alone is legitimate and yields no plugins.
        let only_packs: Vec<&String> = options
            .only_rulepacks
            .iter()
            .filter(|rulepack_id| rulepack_id.as_str() != DIRECTORY_ID)
            .collect();

        if !options.only_rulepacks.is_empty() {
            if only_packs.is_empty() {
                return Ok(Vec::new());
            }

            let selected = only_packs
                .into_iter()
                .filter(|rulepack_id| !excluded.contains(rulepack_id.as_str()))
                .filter_map(|rulepack_id| {
                    self.plugins
                        .get(rulepack_id)
                        .map(|entry| Arc::clone(&entry.plugin))
                })
                .collect::<Vec<_>>();

            if selected.is_empty() {
                return Err(EngineError::NoRulepacksSelected);
            }

            return Ok(selected);
        }

        let selected = self
            .candidate_ids(prepared)
            .into_iter()
            .filter(|rulepack_id| !excluded.contains(rulepack_id))
            .filter_map(|rulepack_id| {
                self.plugins
                    .get(rulepack_id)
                    .filter(|entry| entry.plugin.supports_prepared(prepared))
                    .map(|entry| Arc::clone(&entry.plugin))
            })
            .collect::<Vec<_>>();

        if selected.is_empty() {
            if excluded.is_empty() {
                Err(EngineError::NoMatchingPlugin)
            } else {
                Err(EngineError::NoRulepacksSelected)
            }
        } else {
            Ok(selected)
        }
    }

    /// Attributes an endpoint no rulepack claimed.
    ///
    /// This runs only when nothing else detected a vendor: a matching vendor
    /// pack is strictly better information than a directory hit.
    fn directory_report(
        &self,
        prepared: &PreparedArtifact<'_>,
        options: &ValidationOptions,
        reports: &[ValidationReport],
    ) -> Option<ValidationReport> {
        if options.except_rulepacks.iter().any(|id| id == DIRECTORY_ID) {
            return None;
        }

        if !options.only_rulepacks.is_empty()
            && !options.only_rulepacks.iter().any(|id| id == DIRECTORY_ID)
        {
            return None;
        }

        if reports
            .iter()
            .any(|report| report.detected_vendor.is_some())
        {
            return None;
        }

        let artifact = prepared.trimmed();
        let host = prepared.url().and_then(|url| url.host.as_deref())?;
        let entry = self.directory.lookup_host(host)?;

        let mut message = format!(
            "This endpoint belongs to {} ({}). No Pixellint rulepack covers it, so only the core checks ran.",
            entry.display_name, entry.category
        );

        if let Some(rulepack) = &entry.rulepack {
            message.push_str(&format!(
                " The `{rulepack}` rulepack covers other {} endpoints, not this one.",
                entry.display_name
            ));
        }

        let target = artifact
            .find(host)
            .map(|start| ViolationTarget {
                component: ViolationTargetComponent::Host,
                name: None,
                value: Some(host.to_string()),
                start,
                end: start + host.len(),
            })
            .unwrap_or(ViolationTarget {
                component: ViolationTargetComponent::WholeUrl,
                name: None,
                value: None,
                start: 0,
                end: artifact.len(),
            });

        Some(ValidationReport {
            plugin_id: DIRECTORY_ID.to_string(),
            detected_vendor: Some(entry.vendor.clone()),
            violations: vec![Violation {
                code: "directory.no_rulepack_coverage".to_string(),
                message,
                severity: Severity::Info,
                field: Some("url.host".to_string()),
                fix_hint: Some(
                    "Write a custom rulepack for this endpoint, or ask for first-party coverage."
                        .to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::EcosystemReference,
                    name: "Pixellint vendor directory".to_string(),
                    reference: None,
                },
                targets: vec![target],
            }],
        })
    }

    fn ensure_known_rulepacks(&self, rulepack_ids: &[String]) -> Result<(), EngineError> {
        for rulepack_id in rulepack_ids {
            if rulepack_id == DIRECTORY_ID {
                continue;
            }

            if !self.plugins.contains_key(rulepack_id) {
                return Err(EngineError::PluginNotFound(rulepack_id.clone()));
            }
        }

        Ok(())
    }
}

pub struct CoreRulePack {
    metadata: RulePackMetadata,
}

impl Default for CoreRulePack {
    fn default() -> Self {
        Self {
            metadata: RulePackMetadata {
                id: "core".to_string(),
                display_name: "Core Cardinal Rules".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description:
                    "Shared, spec-backed baseline checks for URL-like measurement artifacts."
                        .to_string(),
                source_level: RuleSourceLevel::Normative,
                vendor: None,
            },
        }
    }
}

impl ValidatorPlugin for CoreRulePack {
    fn metadata(&self) -> &RulePackMetadata {
        &self.metadata
    }

    fn supports(&self, _request: &ValidationRequest) -> bool {
        true
    }

    fn validate(&self, request: &ValidationRequest) -> ValidationReport {
        self.validate_prepared(&PreparedArtifact::from_request(request))
    }

    fn validate_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
        let mut violations = Vec::new();
        let artifact = prepared.trimmed();
        let request = prepared.request();

        if artifact.is_empty() {
            violations.push(Violation {
                code: "core.input.empty".to_string(),
                message: "Artifact is empty.".to_string(),
                severity: Severity::Error,
                field: None,
                fix_hint: Some(
                    "Provide a pixel URL, snippet, template, or request to validate.".to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: "Pixellint input baseline".to_string(),
                    reference: None,
                },
                targets: Vec::new(),
            });
        }

        if !artifact.is_empty() {
            match request.artifact_kind {
                ArtifactKind::Url | ArtifactKind::VastTracker | ArtifactKind::ServerPostback => {
                    validate_url_like_artifact(
                        artifact,
                        request.expansion_state,
                        prepared,
                        &mut violations,
                    );
                    privacy::apply_privacy_rules(prepared, &mut violations);
                }
                ArtifactKind::JsonPayload => {
                    validate_json_artifact(artifact, prepared.json(), &mut violations)
                }
                // An unstated kind that opens like a document is read as one, so
                // a pasted payload still gets its syntax checked.
                ArtifactKind::Unknown if json::JsonDocument::looks_like_json(artifact) => {
                    validate_json_artifact(artifact, prepared.json(), &mut violations)
                }
                _ => {}
            }
        }

        // `core` is vendor-neutral: it never claims to have detected a vendor,
        // even when the caller says which one they expect.
        ValidationReport {
            plugin_id: self.metadata.id.clone(),
            detected_vendor: None,
            violations,
        }
    }
}

/// A body that does not parse cannot be contracted by any vendor pack, so the
/// syntax error is the whole finding and it is worth saying precisely.
fn validate_json_artifact(
    artifact: &str,
    parsed: Option<&Result<json::JsonDocument<'_>, json::JsonError>>,
    violations: &mut Vec<Violation>,
) {
    let error = match parsed {
        Some(Ok(_)) => return,
        Some(Err(error)) => error,
        None => match json::JsonDocument::parse(artifact) {
            Ok(_) => return,
            Err(error) => {
                violations.push(json_parse_violation(artifact, &error));
                return;
            }
        },
    };

    violations.push(json_parse_violation(artifact, error));
}

fn json_parse_violation(artifact: &str, error: &json::JsonError) -> Violation {
    Violation {
        code: "core.json.parse_error".to_string(),
        message: format!("Request body is not valid JSON: {error}."),
        severity: Severity::Error,
        field: Some("body".to_string()),
        fix_hint: Some(
            "Fix the payload so it parses, then validate it again. Serializers that emit trailing commas or unquoted keys are the usual cause."
                .to_string(),
        ),
        source: RuleSource::normative(
            "RFC 8259: The JavaScript Object Notation (JSON) Data Interchange Format",
            "https://www.rfc-editor.org/rfc/rfc8259",
        ),
        targets: vec![ViolationTarget {
            component: ViolationTargetComponent::WholeBody,
            name: None,
            value: None,
            start: error.offset.min(artifact.len()),
            end: artifact.len(),
        }],
    }
}

fn validate_url_like_artifact(
    artifact: &str,
    expansion_state: ExpansionState,
    prepared: &PreparedArtifact<'_>,
    violations: &mut Vec<Violation>,
) {
    apply_broken_macro_delimiters(artifact, prepared, violations);

    let macro_spans = prepared.macro_spans();
    let has_unsafe_macro_positions = apply_macro_rules(
        artifact,
        prepared.request().artifact_kind,
        expansion_state,
        macro_spans,
        violations,
    );

    if has_unsafe_macro_positions {
        return;
    }

    let sanitized;
    let parse_artifact = if macro_spans.is_empty() {
        artifact
    } else {
        sanitized = sanitize_macro_spans(artifact, macro_spans);
        sanitized.as_str()
    };

    if has_missing_network_host(parse_artifact) {
        violations.push(Violation {
            code: "core.url.host_missing".to_string(),
            message: "Network-delivered tracking URLs must include a host component.".to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Provide a fully qualified endpoint such as https://example.com/pixel.".to_string(),
            ),
            source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
            targets: Vec::new(),
        });
        return;
    }

    if let Some(parsed) = prepared.url().filter(|parsed| parsed.core_ready) {
        emit_parsed_url_findings(
            &parsed.scheme,
            parsed.host.is_none(),
            parsed.has_userinfo,
            parsed.has_fragment && !prepared.has_client_fragment_configuration(),
            violations,
        );
        return;
    }

    match Url::parse(parse_artifact) {
        Ok(url) => emit_parsed_url_findings(
            url.scheme(),
            url.host_str().is_none(),
            !url.username().is_empty() || url.password().is_some(),
            url.fragment().is_some() && !prepared.has_client_fragment_configuration(),
            violations,
        ),
        Err(_) => violations.push(Violation {
            code: "core.url.invalid".to_string(),
            message: "Artifact is not a valid URL.".to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Provide a fully qualified URL such as https://example.com/pixel?x=1".to_string(),
            ),
            source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
            targets: Vec::new(),
        }),
    }
}

fn emit_parsed_url_findings(
    scheme: &str,
    host_missing: bool,
    has_userinfo: bool,
    has_fragment: bool,
    violations: &mut Vec<Violation>,
) {
    if scheme == "http" {
        violations.push(Violation {
            code: "core.url.insecure_transport".to_string(),
            message:
                "Plain http tracking endpoints are discouraged; use https for measurement artifacts."
                    .to_string(),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Upgrade the endpoint to https so trackers remain compatible with secure playback and delivery environments."
                    .to_string(),
            ),
            source: RuleSource {
                level: RuleSourceLevel::EcosystemReference,
                name: "Secure tracking transport baseline".to_string(),
                reference: None,
            },
            targets: Vec::new(),
        });
    }

    if !matches!(scheme, "http" | "https") {
        violations.push(Violation {
            code: "core.url.unsupported_scheme".to_string(),
            message: "Only http and https URL artifacts are supported by the core rulepack."
                .to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Use an http or https endpoint for network-delivered tracking artifacts."
                    .to_string(),
            ),
            source: RuleSource::normative(
                "W3C Beacon / URL transport baseline",
                "https://www.w3.org/TR/beacon/",
            ),
            targets: Vec::new(),
        });
    }

    if host_missing {
        violations.push(Violation {
            code: "core.url.host_missing".to_string(),
            message: "Network-delivered tracking URLs must include a host component.".to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Provide a fully qualified endpoint such as https://example.com/pixel.".to_string(),
            ),
            source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
            targets: Vec::new(),
        });
    }

    if has_userinfo {
        violations.push(Violation {
            code: "core.url.userinfo_deprecated".to_string(),
            message: "Credentials embedded in tracking URLs are deprecated and should not be used."
                .to_string(),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Move credentials to a safer transport or server-side configuration.".to_string(),
            ),
            source: RuleSource::normative(
                "RFC 3986 URI generic syntax",
                "https://www.rfc-editor.org/rfc/rfc3986",
            ),
            targets: Vec::new(),
        });
    }

    if has_fragment {
        violations.push(Violation {
            code: "core.url.fragment_ignored".to_string(),
            message:
                "URL fragments are not transmitted to the server and cannot carry server measurement parameters."
                    .to_string(),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Move server measurement data into the query string or request body. Keep documented browser configuration in its fragment.".to_string(),
            ),
            source: RuleSource::normative(
                "RFC 3986 URI generic syntax",
                "https://www.rfc-editor.org/rfc/rfc3986",
            ),
            targets: Vec::new(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MacroSyntax {
    Bracket,
    DollarBraces,
    DoubleBraces,
    Braces,
    Percent,
    Bang,
    FlashBracket,
}

impl MacroSyntax {
    fn example(self) -> &'static str {
        match self {
            Self::Bracket => "[NAME]",
            Self::DollarBraces => "${NAME}",
            Self::DoubleBraces => "{{NAME}}",
            Self::Braces => "{NAME}",
            Self::Percent => "%%NAME%%",
            Self::Bang => "!!NAME!!",
            Self::FlashBracket => "[%NAME%]",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MacroPosition {
    Scheme,
    Authority,
    UserInfo,
    Host,
    Port,
    Path,
    Query,
    Fragment,
    WholeUrl,
}

impl MacroPosition {
    fn field(self) -> &'static str {
        match self {
            Self::Scheme => "url.scheme",
            Self::Authority => "url.authority",
            Self::UserInfo => "url.userinfo",
            Self::Host => "url.host",
            Self::Port => "url.port",
            Self::Path => "url.path",
            Self::Query => "url.query",
            Self::Fragment => "url.fragment",
            Self::WholeUrl => "url",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Scheme => "scheme",
            Self::Authority => "authority",
            Self::UserInfo => "userinfo",
            Self::Host => "host",
            Self::Port => "port",
            Self::Path => "path",
            Self::Query => "query",
            Self::Fragment => "fragment",
            Self::WholeUrl => "url",
        }
    }

    fn target_component(self) -> ViolationTargetComponent {
        match self {
            Self::Scheme => ViolationTargetComponent::Scheme,
            Self::Authority => ViolationTargetComponent::Authority,
            Self::UserInfo => ViolationTargetComponent::UserInfo,
            Self::Host => ViolationTargetComponent::Host,
            Self::Port => ViolationTargetComponent::Port,
            Self::Path => ViolationTargetComponent::Path,
            Self::Query => ViolationTargetComponent::QueryParam,
            Self::Fragment => ViolationTargetComponent::Fragment,
            Self::WholeUrl => ViolationTargetComponent::WholeUrl,
        }
    }

    fn is_unsafe(self) -> bool {
        matches!(
            self,
            Self::Scheme | Self::Authority | Self::UserInfo | Self::Host | Self::Port
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacroSpan {
    start: usize,
    end: usize,
    syntax: MacroSyntax,
}

fn apply_macro_rules(
    artifact: &str,
    artifact_kind: ArtifactKind,
    expansion_state: ExpansionState,
    macro_spans: &[MacroSpan],
    violations: &mut Vec<Violation>,
) -> bool {
    if macro_spans.is_empty() {
        return false;
    }

    let syntaxes = macro_spans
        .iter()
        .map(|span| span.syntax)
        .collect::<BTreeSet<_>>();

    // A VAST tracker routinely carries IAB [MACROS] beside the ad server's ${MACROS}.
    if syntaxes.len() > 1 && !vast_bracket_and_dollar(artifact_kind, &syntaxes) {
        let targets = macro_spans
            .iter()
            .map(|span| target_for_macro_span(artifact, span))
            .collect();
        let syntax_list = syntaxes
            .iter()
            .map(|syntax| syntax.example())
            .collect::<Vec<_>>()
            .join(", ");
        violations.push(Violation {
            code: "core.macro.mixed_syntax".to_string(),
            message: format!(
                "Multiple macro syntaxes were detected in the same artifact: {syntax_list}."
            ),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Standardize on one macro syntax per artifact so trafficking and expansion behavior stays predictable."
                    .to_string(),
            ),
            source: macro_rule_source(),
            targets,
        });
    }

    if expansion_state == ExpansionState::Fired {
        let targets = macro_spans
            .iter()
            .map(|span| target_for_macro_span(artifact, span))
            .collect();
        violations.push(Violation {
            code: "core.macro.unexpanded_in_fired_url".to_string(),
            message: "Observed fired URLs should not contain unresolved macro tokens."
                .to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Expand macros before the request is fired, or validate the artifact in template mode instead."
                    .to_string(),
            ),
            source: macro_rule_source(),
            targets,
        });
    }

    if artifact_kind == ArtifactKind::VastTracker {
        apply_vast_macro_vocabulary(artifact, macro_spans, violations);
    }

    let unsafe_spans = macro_spans
        .iter()
        .map(|span| (*span, classify_macro_position(artifact, span)))
        .filter(|(_, position)| position.is_unsafe())
        .collect::<Vec<_>>();

    let unsafe_positions = unsafe_spans
        .iter()
        .map(|(_, position)| *position)
        .collect::<BTreeSet<_>>();

    if unsafe_positions.is_empty() {
        return false;
    }

    let position_list = unsafe_positions
        .iter()
        .map(|position| position.label())
        .collect::<Vec<_>>()
        .join(", ");
    let first_position = *unsafe_positions
        .iter()
        .next()
        .unwrap_or(&MacroPosition::WholeUrl);
    let targets = unsafe_spans
        .iter()
        .map(|(span, _)| target_for_macro_span(artifact, span))
        .collect();

    violations.push(Violation {
        code: "core.macro.unsafe_position".to_string(),
        message: format!(
            "Macros in URL {position_list} components are unsafe because they can prevent reliable endpoint resolution."
        ),
        severity: Severity::Error,
        field: Some(first_position.field().to_string()),
        fix_hint: Some(
            "Keep macros in path or query values, or expand scheme and authority components before validation."
                .to_string(),
        ),
        source: macro_rule_source(),
        targets,
    });

    true
}

fn macro_rule_source() -> RuleSource {
    RuleSource {
        level: RuleSourceLevel::EcosystemReference,
        name: "Ad-tech macro handling baseline".to_string(),
        reference: None,
    }
}

fn vast_macro_source() -> RuleSource {
    RuleSource::normative("IAB VAST macros", "https://iabtechlab.com/standards/vast/")
}

/// IAB `[MACRO]` plus `${MACRO}` on a vast tracker is one tag, two expanders.
fn vast_bracket_and_dollar(kind: ArtifactKind, syntaxes: &BTreeSet<MacroSyntax>) -> bool {
    kind == ArtifactKind::VastTracker
        && syntaxes.len() == 2
        && syntaxes.contains(&MacroSyntax::Bracket)
        && syntaxes.contains(&MacroSyntax::DollarBraces)
}

/// IAB VAST macro names. Same set vastlint checks. Bracket syntax only.
const VAST_MACROS: &[&str] = &[
    "ADCATEGORIES",
    "ADCOUNT",
    "ADPLAYHEAD",
    "ADSERVINGID",
    "ADTYPE",
    "APIFRAMEWORKS",
    "APPBUNDLE",
    "ASSETURI",
    "BLOCKEDADCATEGORIES",
    "BREAKMAXADLENGTH",
    "BREAKMAXADS",
    "BREAKMAXDURATION",
    "BREAKMINADLENGTH",
    "BREAKMINDURATION",
    "BREAKPOSITION",
    "CACHEBUSTING",
    "CLICKPOS",
    "CLIENTUA",
    "CONTENTID",
    "CONTENTPLAYHEAD",
    "CONTENTURI",
    "DEVICEIP",
    "DEVICEUA",
    "DOMAIN",
    "ERRORCODE",
    "EXTENSIONS",
    "GDPR",
    "GDPRCONSENT",
    "IFA",
    "IFATYPE",
    "INVENTORYSTATE",
    "LATLONG",
    "LIMITADTRACKING",
    "MEDIAMIME",
    "MEDIAPLAYHEAD",
    "OMIDPARTNER",
    "PAGEURL",
    "PLACEMENTTYPE",
    "PLAYERCAPABILITIES",
    "PLAYERSIZE",
    "PLAYERSTATE",
    "PODSEQUENCE",
    "REASON",
    "REGULATIONS",
    "SERVERSIDE",
    "SERVERUA",
    "TIMESTAMP",
    "TRANSACTIONID",
    "UNIVERSALADID",
    "VASTVERSIONS",
    "VERIFICATIONVENDORS",
];

fn apply_vast_macro_vocabulary(
    artifact: &str,
    macro_spans: &[MacroSpan],
    violations: &mut Vec<Violation>,
) {
    let mut unknown = Vec::new();
    let mut lowercase = Vec::new();
    let mut deprecated = Vec::new();

    for span in macro_spans {
        if span.syntax != MacroSyntax::Bracket {
            continue;
        }

        let token = &artifact[span.start + 1..span.end - 1];
        if !is_vast_bracket_token(token) {
            continue;
        }

        let upper = token.to_ascii_uppercase();
        if VAST_MACROS.binary_search(&upper.as_str()).is_err() {
            unknown.push(span);
        } else if token != upper {
            lowercase.push(span);
        } else if upper == "CONTENTPLAYHEAD" || upper == "MEDIAPLAYHEAD" {
            deprecated.push(span);
        }
    }

    if !unknown.is_empty() {
        let listed = join_macro_text(artifact, &unknown);
        let (noun, verb) = if unknown.len() == 1 {
            ("token", "is")
        } else {
            ("tokens", "are")
        };
        violations.push(Violation {
            code: "core.macro.vast_unknown".to_string(),
            message: format!(
                "Bracket {noun} {listed} {verb} outside the IAB VAST macro table. Players substitute the published names, and a vendor token stays in the request until that vendor expands it."
            ),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Use a published IAB macro, or keep the token only on an endpoint that expands it."
                    .to_string(),
            ),
            source: vast_macro_source(),
            targets: unknown
                .iter()
                .map(|span| target_for_macro_span(artifact, span))
                .collect(),
        });
    }

    if !lowercase.is_empty() {
        let listed = join_macro_text(artifact, &lowercase);
        let (noun, verb) = if lowercase.len() == 1 {
            ("macro", "is")
        } else {
            ("macros", "are")
        };
        violations.push(Violation {
            code: "core.macro.vast_lowercase".to_string(),
            message: format!(
                "IAB {noun} {listed} {verb} lowercase. Players match the uppercase spelling and leave the token in the request."
            ),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some("Uppercase the macro name.".to_string()),
            source: vast_macro_source(),
            targets: lowercase
                .iter()
                .map(|span| target_for_macro_span(artifact, span))
                .collect(),
        });
    }

    if !deprecated.is_empty() {
        let listed = join_macro_text(artifact, &deprecated);
        violations.push(Violation {
            code: "core.macro.vast_deprecated".to_string(),
            message: format!("{listed} is a pre-4.1 playhead macro. VAST 4.1 uses [ADPLAYHEAD]."),
            severity: Severity::Info,
            field: Some("url".to_string()),
            fix_hint: Some("Use [ADPLAYHEAD].".to_string()),
            source: vast_macro_source(),
            targets: deprecated
                .iter()
                .map(|span| target_for_macro_span(artifact, span))
                .collect(),
        });
    }
}

fn is_vast_bracket_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && token
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}

fn join_macro_text(artifact: &str, spans: &[&MacroSpan]) -> String {
    spans
        .iter()
        .map(|span| &artifact[span.start..span.end])
        .collect::<Vec<_>>()
        .join(", ")
}

/// `!!NAME!` is a bang short of `!!NAME!!`. `FT_NAME` is that name with no delimiter.
fn apply_broken_macro_delimiters(
    artifact: &str,
    prepared: &PreparedArtifact<'_>,
    violations: &mut Vec<Violation>,
) {
    let bytes = artifact.as_bytes();
    let mut index = 0;

    while index + 1 < bytes.len() {
        if bytes[index] != b'!' || bytes[index + 1] != b'!' {
            index += 1;
            continue;
        }

        let body_start = index + 2;
        let mut body_end = body_start;
        while body_end < bytes.len() && is_macro_body_byte(bytes[body_end]) {
            body_end += 1;
        }

        let body = &artifact[body_start..body_end];
        if !is_macro_body(body) {
            index += 1;
            continue;
        }

        let closed =
            body_end + 1 < bytes.len() && bytes[body_end] == b'!' && bytes[body_end + 1] == b'!';
        if closed {
            index = body_end + 2;
            continue;
        }

        let one_bang = body_end < bytes.len()
            && bytes[body_end] == b'!'
            && (body_end + 1 == bytes.len() || bytes[body_end + 1] != b'!');
        if one_bang {
            let end = body_end + 1;
            let token = &artifact[index..end];
            let span = MacroSpan {
                start: index,
                end,
                syntax: MacroSyntax::Bang,
            };
            let target = target_for_macro_span(artifact, &span);
            let field = target
                .name
                .as_ref()
                .map(|name| format!("param.{name}"))
                .unwrap_or_else(|| "url".to_string());
            violations.push(Violation {
                code: "core.macro.broken_delimiter".to_string(),
                message: format!(
                    "`{token}` is one bang short of a closed `!!{body}!!` macro, so the request sends the token literally."
                ),
                severity: Severity::Warning,
                field: Some(field),
                fix_hint: Some(format!("Close the macro as !!{body}!!.")),
                source: macro_rule_source(),
                targets: vec![target],
            });
            index = end;
            continue;
        }

        index += 1;
    }

    for param in prepared.params(ParamStyle::Query) {
        if !is_bare_ft_macro(&param.value) {
            continue;
        }

        violations.push(Violation {
            code: "core.macro.broken_delimiter".to_string(),
            message: format!(
                "`{}` is `{}`, a macro name with no delimiter, so the request sends the name literally.",
                param.name, param.value
            ),
            severity: Severity::Warning,
            field: Some(format!("param.{}", param.name)),
            fix_hint: Some(format!(
                "Wrap the name as [%{}%] or !!{}!!, or send the expanded value.",
                param.value, param.value
            )),
            source: macro_rule_source(),
            targets: vec![param.target()],
        });
    }
}

fn is_macro_body_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')
}

fn is_bare_ft_macro(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("FT_") else {
        return false;
    };

    !rest.is_empty()
        && rest.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
        && rest
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}

pub(crate) fn detect_macro_spans(artifact: &str) -> Vec<MacroSpan> {
    let mut spans = Vec::new();
    let mut index = 0;

    while index < artifact.len() {
        let remainder = &artifact[index..];
        let Some(relative) = remainder.find(['$', '[', '{', '%', '!']) else {
            break;
        };
        index += relative;
        let remainder = &artifact[index..];

        if let Some(span) = match_macro_span(artifact, index, remainder) {
            index = span.end;
            spans.push(span);
            continue;
        }

        // Step by Unicode scalar so a non-ASCII byte (a replacement char in a
        // D1 sample, a decoded UTF-8 query) cannot land `index` inside a char.
        index += remainder.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }

    spans
}

/// Macro delimiters recognized by the generic ad-tech macro scanner, in match order.
const MACRO_DELIMITERS: [(&str, &str, MacroSyntax); 7] = [
    ("${", "}", MacroSyntax::DollarBraces),
    ("{{", "}}", MacroSyntax::DoubleBraces),
    ("{", "}", MacroSyntax::Braces),
    ("[%", "%]", MacroSyntax::FlashBracket),
    ("!!", "!!", MacroSyntax::Bang),
    ("%%", "%%", MacroSyntax::Percent),
    ("[", "]", MacroSyntax::Bracket),
];

fn match_macro_span(artifact: &str, index: usize, remainder: &str) -> Option<MacroSpan> {
    for (open, close, syntax) in MACRO_DELIMITERS {
        let Some(after_open) = remainder.strip_prefix(open) else {
            continue;
        };
        let Some(close_offset) = after_open.find(close) else {
            continue;
        };

        let body_start = index + open.len();
        let body_end = body_start + close_offset;

        if is_macro_body(&artifact[body_start..body_end]) {
            return Some(MacroSpan {
                start: index,
                end: body_end + close.len(),
                syntax,
            });
        }
    }

    None
}

fn is_macro_body(body: &str) -> bool {
    if body.is_empty() || body.trim() != body {
        return false;
    }

    let mut has_identifier_character = false;

    for character in body.chars() {
        match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | '-' => {
                if character.is_ascii_alphabetic() || character == '_' {
                    has_identifier_character = true;
                }
            }
            _ => return false,
        }
    }

    has_identifier_character
}

pub(crate) fn sanitize_macro_spans(artifact: &str, macro_spans: &[MacroSpan]) -> String {
    let mut sanitized = String::with_capacity(artifact.len());
    let mut cursor = 0;

    for span in macro_spans {
        sanitized.push_str(&artifact[cursor..span.start]);
        sanitized.push_str("macro");
        cursor = span.end;
    }

    sanitized.push_str(&artifact[cursor..]);
    sanitized
}

fn target_for_macro_span(artifact: &str, span: &MacroSpan) -> ViolationTarget {
    let position = classify_macro_position(artifact, span);
    let value = &artifact[span.start..span.end];

    if position == MacroPosition::Query {
        let (name, param_value) = query_param_context(artifact, span);
        return ViolationTarget {
            component: ViolationTargetComponent::QueryParam,
            name,
            value: param_value,
            start: span.start,
            end: span.end,
        };
    }

    ViolationTarget {
        component: position.target_component(),
        name: None,
        value: Some(value.to_string()),
        start: span.start,
        end: span.end,
    }
}

fn query_param_context(artifact: &str, span: &MacroSpan) -> (Option<String>, Option<String>) {
    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let Some(query_start) = artifact[..fragment_start].find('?') else {
        return (None, Some(artifact[span.start..span.end].to_string()));
    };

    let query_value_start = query_start + 1;
    if !overlaps(span, query_value_start, fragment_start) {
        return (None, Some(artifact[span.start..span.end].to_string()));
    }

    let segment_start = artifact[..span.start]
        .rfind(['?', '&'])
        .map(|index| index + 1)
        .unwrap_or(query_value_start);
    let segment_end = artifact[span.end..fragment_start]
        .find('&')
        .map(|offset| span.end + offset)
        .unwrap_or(fragment_start);
    let segment = &artifact[segment_start..segment_end];

    if let Some(separator) = segment.find('=') {
        let name = &segment[..separator];
        let value = &segment[separator + 1..];
        (
            (!name.is_empty()).then(|| name.to_string()),
            Some(value.to_string()),
        )
    } else {
        (
            (!segment.is_empty()).then(|| segment.to_string()),
            Some(segment.to_string()),
        )
    }
}

fn classify_macro_position(artifact: &str, span: &MacroSpan) -> MacroPosition {
    let length = artifact.len();
    let Some(scheme_end) = artifact.find("://") else {
        return MacroPosition::WholeUrl;
    };

    if overlaps(span, 0, scheme_end) {
        return MacroPosition::Scheme;
    }

    let authority_start = scheme_end + 3;
    let authority_end = artifact[authority_start..]
        .find(['/', '?', '#'])
        .map(|offset| authority_start + offset)
        .unwrap_or(length);

    let fragment_start = artifact.find('#');
    let query_search_end = fragment_start.unwrap_or(length);
    let query_start = artifact[..query_search_end].find('?');
    let path_end = query_start.or(fragment_start).unwrap_or(length);

    if overlaps(span, authority_start, authority_end) {
        let authority = &artifact[authority_start..authority_end];
        let (userinfo_end, host_start) = if let Some(at_offset) = authority.find('@') {
            let userinfo_end = authority_start + at_offset;
            if overlaps(span, authority_start, userinfo_end) {
                return MacroPosition::UserInfo;
            }
            (Some(userinfo_end), userinfo_end + 1)
        } else {
            (None, authority_start)
        };

        if host_start < authority_end {
            let host_port = &artifact[host_start..authority_end];
            if host_port.starts_with('[') {
                if let Some(close_offset) = host_port.find(']') {
                    let host_end = host_start + close_offset + 1;
                    if overlaps(span, host_start, host_end) {
                        return MacroPosition::Host;
                    }

                    if host_end < authority_end
                        && artifact.as_bytes().get(host_end) == Some(&b':')
                        && overlaps(span, host_end + 1, authority_end)
                    {
                        return MacroPosition::Port;
                    }
                }
            } else if let Some(port_separator) = host_port.rfind(':') {
                let port_start = host_start + port_separator + 1;
                let host_end = host_start + port_separator;

                if overlaps(span, host_start, host_end) {
                    return MacroPosition::Host;
                }

                if overlaps(span, port_start, authority_end) {
                    return MacroPosition::Port;
                }
            } else if overlaps(span, host_start, authority_end) {
                return MacroPosition::Host;
            }
        }

        if userinfo_end.is_some() || overlaps(span, authority_start, authority_end) {
            return MacroPosition::Authority;
        }
    }

    if authority_end < path_end
        && artifact.as_bytes().get(authority_end) == Some(&b'/')
        && overlaps(span, authority_end, path_end)
    {
        return MacroPosition::Path;
    }

    if let Some(query_start) = query_start {
        let query_value_start = query_start + 1;
        let query_end = fragment_start.unwrap_or(length);
        if overlaps(span, query_value_start, query_end) {
            return MacroPosition::Query;
        }
    }

    if let Some(fragment_start) = fragment_start {
        let fragment_value_start = fragment_start + 1;
        if overlaps(span, fragment_value_start, length) {
            return MacroPosition::Fragment;
        }
    }

    MacroPosition::WholeUrl
}

fn overlaps(span: &MacroSpan, start: usize, end: usize) -> bool {
    start < end && span.start < end && span.end > start
}

fn http_url_remainder(artifact: &str) -> Option<&str> {
    let bytes = artifact.as_bytes();
    if bytes.len() >= 8 && bytes[..8].eq_ignore_ascii_case(b"https://") {
        Some(&artifact[8..])
    } else if bytes.len() >= 7 && bytes[..7].eq_ignore_ascii_case(b"http://") {
        Some(&artifact[7..])
    } else {
        None
    }
}

fn has_missing_network_host(artifact: &str) -> bool {
    let Some(remainder) = http_url_remainder(artifact) else {
        return false;
    };

    matches!(
        remainder.as_bytes().first().copied(),
        None | Some(b'/') | Some(b'?') | Some(b'#') | Some(b':') | Some(b'@')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_browser_fragments_are_approved_only_on_the_loader_endpoint() {
        let engine = Engine::default();
        let request = |artifact: &str, kind| ValidationRequest {
            artifact_kind: kind,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        for fragment in [
            "name=nlsnInstance&ns=NOLBUNDLE",
            "name=&ns=NOLBUNDLE",
            "name=x&%6Es=NOLBUNDLE",
        ] {
            let summary = engine
                .validate_at(
                    &request(
                        &format!("https://cdn-gl.imrworldwide.com/conf/APP.js#{fragment}"),
                        ArtifactKind::Url,
                    ),
                    &ValidationOptions::default(),
                    0,
                )
                .unwrap();
            assert!(violation_codes(&summary).is_empty(), "{fragment}");
        }
        for artifact in [
            "https://cdn-gl.imrworldwide.com/conf/APP.js#name=x&ns=NOLBUNDLE&event=purchase",
            "https://cdn-gl.imrworldwide.com/conf/APP.txt#name=x&ns=NOLBUNDLE",
            "https://example.test/conf/APP.js#name=x&ns=NOLBUNDLE",
        ] {
            let summary = engine
                .validate_at(
                    &request(artifact, ArtifactKind::Url),
                    &ValidationOptions::default(),
                    0,
                )
                .unwrap();
            assert!(
                violation_codes(&summary).contains(&"core.url.fragment_ignored".to_string()),
                "{artifact}"
            );
        }
        // A server postback does not consume browser-loader configuration.
        let summary = engine
            .validate_at(
                &request(
                    "https://cdn-gl.imrworldwide.com/conf/APP.js#name=x&ns=NOLBUNDLE",
                    ArtifactKind::ServerPostback,
                ),
                &ValidationOptions::default(),
                0,
            )
            .unwrap();
        assert!(violation_codes(&summary).contains(&"core.url.fragment_ignored".to_string()));
        let summary = engine
            .validate_at(
                &request(
                    "https://cdn-gl.imrworldwide.com/conf/APP.js#name=x&ns=NOLBUNDLE",
                    ArtifactKind::Url,
                ),
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string()],
                    except_rulepacks: Vec::new(),
                },
                0,
            )
            .unwrap();
        assert_eq!(violation_codes(&summary), vec!["core.url.fragment_ignored"]);
    }

    #[test]
    fn browser_fragment_declarations_preserve_other_url_checks() {
        let engine = Engine::default();
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: "http://user@cdn-gl.imrworldwide.com/conf/APP.js?cb=[CACHEBUSTER]#name=&ns=NOLBUNDLE".to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Fired,
        };
        let summary = engine
            .validate_at(&request, &ValidationOptions::default(), 0)
            .unwrap();
        let codes = violation_codes(&summary);
        assert!(codes.contains(&"core.url.insecure_transport".to_string()));
        assert!(codes.contains(&"core.url.userinfo_deprecated".to_string()));
        assert!(codes.contains(&"core.macro.unexpanded_in_fired_url".to_string()));
        assert!(!codes.contains(&"core.url.fragment_ignored".to_string()));
    }

    #[test]
    fn an_explicit_clock_is_shared_by_all_plugins_and_repeated_validation() {
        struct ClockProbe {
            metadata: RulePackMetadata,
            observed: Arc<std::sync::Mutex<Vec<i64>>>,
        }
        impl ValidatorPlugin for ClockProbe {
            fn metadata(&self) -> &RulePackMetadata {
                &self.metadata
            }
            fn supports(&self, _: &ValidationRequest) -> bool {
                true
            }
            fn validate(&self, _: &ValidationRequest) -> ValidationReport {
                unreachable!("the engine shares its prepared artifact")
            }
            fn validate_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
                self.observed
                    .lock()
                    .unwrap()
                    .push(prepared.reference_time_unix_seconds());
                ValidationReport {
                    plugin_id: self.metadata.id.clone(),
                    detected_vendor: None,
                    violations: Vec::new(),
                }
            }
        }
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut engine = Engine::new();
        for id in ["clock/first", "clock/second"] {
            let mut metadata = FixtureRulePack::default().metadata;
            metadata.id = id.to_string();
            engine.register(ClockProbe {
                metadata,
                observed: Arc::clone(&observed),
            });
        }
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: "https://example.test/".to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        for _ in 0..2 {
            assert!(
                engine
                    .validate_at(&request, &ValidationOptions::default(), 1234567890)
                    .unwrap()
                    .is_ok()
            );
        }
        assert_eq!(*observed.lock().unwrap(), vec![1234567890; 4]);
        assert_eq!(
            PreparedArtifact::from_request_at(&request, -1).reference_time_unix_seconds(),
            -1
        );
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        {
            observed.lock().unwrap().clear();
            engine
                .validate(&request, &ValidationOptions::default())
                .unwrap();
            let observed = observed.lock().unwrap();
            assert_eq!(observed.len(), 2);
            assert_eq!(observed[0], observed[1]);
        }
    }

    #[test]
    fn selected_vendor_packs_retain_json_syntax_errors_without_duplicate_core_reports() {
        let mut engine = Engine::new();
        engine.register(FixtureRulePack::default());
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
            artifact: r#"{"data":"\ué😀"}"#.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        for excluded in [Vec::new(), vec!["core".to_string()]] {
            engine.register(CoreRulePack::default());
            let summary = engine
                .validate_at(
                    &request,
                    &ValidationOptions {
                        only_rulepacks: vec!["fixture".to_string()],
                        except_rulepacks: excluded,
                    },
                    0,
                )
                .unwrap();
            assert!(!summary.is_ok());
            assert_eq!(
                summary
                    .reports
                    .iter()
                    .filter(|report| report.plugin_id == "core")
                    .count(),
                1
            );
            assert_eq!(
                summary.reports[0].violations[0].code,
                "core.json.parse_error"
            );
        }
        let summary = engine
            .validate_at(
                &request,
                &ValidationOptions {
                    only_rulepacks: vec!["fixture".to_string(), "core".to_string()],
                    except_rulepacks: Vec::new(),
                },
                0,
            )
            .unwrap();
        assert_eq!(
            summary
                .reports
                .iter()
                .filter(|report| report.plugin_id == "core")
                .count(),
            1
        );
    }

    struct FixtureRulePack {
        metadata: RulePackMetadata,
    }

    impl Default for FixtureRulePack {
        fn default() -> Self {
            Self {
                metadata: RulePackMetadata {
                    id: "fixture".to_string(),
                    display_name: "Fixture RulePack".to_string(),
                    version: "0.0.0".to_string(),
                    description: "Test helper rulepack".to_string(),
                    source_level: RuleSourceLevel::Heuristic,
                    vendor: None,
                },
            }
        }
    }

    impl ValidatorPlugin for FixtureRulePack {
        fn metadata(&self) -> &RulePackMetadata {
            &self.metadata
        }

        fn supports(&self, _request: &ValidationRequest) -> bool {
            true
        }

        fn validate(&self, request: &ValidationRequest) -> ValidationReport {
            ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: request.claimed_vendor.clone(),
                violations: vec![Violation {
                    code: "fixture.info".to_string(),
                    message: "fixture ran".to_string(),
                    severity: Severity::Info,
                    field: None,
                    fix_hint: None,
                    source: RuleSource {
                        level: RuleSourceLevel::Heuristic,
                        name: "Fixture".to_string(),
                        reference: None,
                    },
                    targets: Vec::new(),
                }],
            }
        }
    }

    fn sample_request(artifact: &str) -> ValidationRequest {
        sample_request_with_kind(ArtifactKind::Url, artifact)
    }

    fn sample_request_with_kind(artifact_kind: ArtifactKind, artifact: &str) -> ValidationRequest {
        sample_request_with_kind_and_state(artifact_kind, ExpansionState::Unknown, artifact)
    }

    fn sample_request_with_kind_and_state(
        artifact_kind: ArtifactKind,
        expansion_state: ExpansionState,
        artifact: &str,
    ) -> ValidationRequest {
        ValidationRequest {
            artifact_kind,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state,
        }
    }

    fn violation_codes(summary: &ValidationSummary) -> Vec<String> {
        summary
            .reports
            .iter()
            .flat_map(|report| report.violations.iter())
            .map(|violation| violation.code.clone())
            .collect()
    }

    #[test]
    fn core_rulepack_runs_in_auto_mode() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1#ignored"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, "core");
        assert_eq!(
            summary.reports[0].violations[0].code,
            "core.url.fragment_ignored"
        );
    }

    #[test]
    fn explicit_rulepack_selection_is_toggleable() {
        let mut engine = Engine::default();
        engine.register(FixtureRulePack::default());

        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string(), "fixture".to_string()],
                    except_rulepacks: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 2);
        assert_eq!(summary.reports[0].plugin_id, "core");
        assert_eq!(summary.reports[1].plugin_id, "fixture");
    }

    #[test]
    fn excluding_all_selected_rulepacks_returns_an_error() {
        let engine = Engine::default();
        let error = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string()],
                    except_rulepacks: vec!["core".to_string()],
                },
            )
            .unwrap_err();

        assert_eq!(error, EngineError::NoRulepacksSelected);
    }

    #[test]
    fn core_rulepack_warns_on_deprecated_userinfo() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://user:pass@example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            summary.reports[0].violations[0].code,
            "core.url.userinfo_deprecated"
        );
    }

    #[test]
    fn core_rulepack_reports_empty_input_without_cascading_url_noise() {
        let engine = Engine::default();
        let summary = engine
            .validate(&sample_request("   \n\t"), &ValidationOptions::default())
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.input.empty"]);
    }

    #[test]
    fn core_rulepack_errors_on_invalid_url() {
        let engine = Engine::default();
        let summary = engine
            .validate(&sample_request("not a url"), &ValidationOptions::default())
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.invalid"]);
    }

    #[test]
    fn core_rulepack_errors_on_unsupported_scheme() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("ftp://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.unsupported_scheme"]
        );
    }

    #[test]
    fn core_rulepack_warns_on_insecure_http_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("http://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport"]
        );
    }

    #[test]
    fn core_rulepack_errors_on_missing_host() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https:///pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.host_missing"]);
    }

    #[test]
    fn missing_network_host_reads_the_scheme_prefix_without_copying_the_url() {
        assert!(has_missing_network_host("https:///pixel?id=1"));
        assert!(has_missing_network_host("HTTPS://"));
        assert!(has_missing_network_host("http://?q=1"));
        assert!(has_missing_network_host("HTTP://#frag"));
        assert!(has_missing_network_host("https://:8443/pixel"));
        assert!(!has_missing_network_host("https://example.com/pixel"));
        assert!(!has_missing_network_host("HTTPS://example.com/pixel"));
        assert!(!has_missing_network_host("HTTP://example.com/pixel"));
        assert!(!has_missing_network_host("ftp://example.com/pixel"));
    }

    #[test]
    fn core_rulepack_accepts_clean_https_pixel() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_trimmed_https_pixel() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request(" \n https://example.com/pixel?id=1 \t "),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_localhost_with_port() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://localhost:8443/pixel?id=1&source=qa"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_ipv6_hosts() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://[2001:db8::1]/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_combines_insecure_transport_userinfo_and_fragment_warnings() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("http://user:pass@example.com/pixel?id=1#frag"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec![
                "core.url.insecure_transport",
                "core.url.userinfo_deprecated",
                "core.url.fragment_ignored"
            ]
        );
    }

    #[test]
    fn core_rulepack_validates_vast_tracker_urls() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::VastTracker,
                    "https://example.com/vast/track?event=start#ignored",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.fragment_ignored"]);
    }

    #[test]
    fn core_rulepack_warns_on_insecure_vast_tracker_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::VastTracker,
                    "http://tracker.example.com/vast/track?event=start#ignored",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport", "core.url.fragment_ignored"]
        );
    }

    #[test]
    fn core_rulepack_validates_server_postback_urls() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::ServerPostback,
                    "ftp://example.com/postback?tx=abc123",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.unsupported_scheme"]
        );
    }

    #[test]
    fn core_rulepack_warns_on_insecure_server_postback_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::ServerPostback,
                    "http://collector.example.com/postback?tx=abc123",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport"]
        );
    }

    #[test]
    fn core_rulepack_accepts_query_macros_in_unknown_state() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?cb=[CACHEBUSTING]&id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_errors_on_unexpanded_macro_in_fired_url() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Fired,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&id=1",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.macro.unexpanded_in_fired_url"]
        );

        let targets = &summary.reports[0].violations[0].targets;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].component, ViolationTargetComponent::QueryParam);
        assert_eq!(targets[0].name.as_deref(), Some("cb"));
        assert_eq!(targets[0].value.as_deref(), Some("[CACHEBUSTING]"));
    }

    #[test]
    fn core_rulepack_warns_on_mixed_macro_syntax() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&price=${AUCTION_PRICE}",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.macro.mixed_syntax"]);

        let targets = &summary.reports[0].violations[0].targets;
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].name.as_deref(), Some("cb"));
        assert_eq!(targets[1].name.as_deref(), Some("price"));
    }

    #[test]
    fn vast_tracker_accepts_iab_brackets_beside_dollar_macros() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::VastTracker,
                    ExpansionState::Template,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&price=${AUCTION_PRICE}",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(violation_codes(&summary).is_empty());
    }

    #[test]
    fn vast_tracker_still_warns_when_a_third_macro_syntax_appears() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::VastTracker,
                    ExpansionState::Template,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&x={{CLICK_URL}}",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.macro.mixed_syntax"]);
    }

    #[test]
    fn url_kind_does_not_apply_the_vast_macro_table() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://example.com/pixel?x=[VIEWABILITY]",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(violation_codes(&summary).is_empty());
    }

    #[test]
    fn closed_bang_percent_and_flash_macros_skip_the_gdpr_flag_check() {
        let engine = Engine::default();
        for artifact in [
            "https://example.com/pixel?gdpr=!!GDPR!!",
            "https://example.com/pixel?gdpr=%%GDPR%%",
            "https://example.com/pixel?gdpr=[%FT_GDPR%]",
            // `%` plus two hex digits is the start of CACHEBUSTER, ADID, DEVICEUA.
            "https://example.com/pixel?gdpr=%%CACHEBUSTER%%",
            "https://example.com/pixel?gdpr=[%ADID%]",
            "https://example.com/pixel?gdpr=%%DEVICEUA%%",
        ] {
            let summary = engine
                .validate(
                    &sample_request_with_kind_and_state(
                        ArtifactKind::Url,
                        ExpansionState::Template,
                        artifact,
                    ),
                    &ValidationOptions::default(),
                )
                .unwrap();
            assert!(
                violation_codes(&summary).is_empty(),
                "{artifact} -> {:?}",
                violation_codes(&summary)
            );
        }
    }

    #[test]
    fn named_braces_keep_nested_delimiters_and_json_objects_distinct() {
        // Affise documents {ip} as a tracking-link macro:
        // https://help-center.affise.com/en/articles/6474898-advertiser-tracking-url-macros
        let artifact = r#"https://example.com/a?ip={ip}&ad=${AD}&id={{ID}}&json={"x":1}&empty={}&number={123}"#;
        let spans = detect_macro_spans(artifact);
        assert_eq!(spans.len(), 3);
        assert_eq!(
            spans
                .iter()
                .map(|span| (&artifact[span.start..span.end], span.syntax))
                .collect::<Vec<_>>(),
            vec![
                ("{ip}", MacroSyntax::Braces),
                ("${AD}", MacroSyntax::DollarBraces),
                ("{{ID}}", MacroSyntax::DoubleBraces)
            ]
        );
        assert!(spans.windows(2).all(|pair| pair[0].end <= pair[1].start));
        assert!(detect_macro_spans(r#"{"ip":"192.0.2.1"}"#).is_empty());
    }

    #[test]
    fn percent_encoding_is_not_a_percent_macro() {
        assert!(detect_macro_spans("https://example.com/a?q=%20&x=1").is_empty());
        let encoded = Engine::default()
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://example.com/pixel?gdpr=%20",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert_eq!(violation_codes(&encoded), vec!["core.privacy.gdpr_invalid"]);
        let artifact = "https://example.com/a?e=%%ERRORCODE%%";
        let spans = detect_macro_spans(artifact);
        assert_eq!(spans.len(), 1);
        assert_eq!(&artifact[spans[0].start..spans[0].end], "%%ERRORCODE%%");
    }

    #[test]
    fn vast_macro_table_is_sorted() {
        let mut sorted = VAST_MACROS.to_vec();
        sorted.sort_unstable();
        assert_eq!(VAST_MACROS, sorted.as_slice());
    }

    #[test]
    fn core_rulepack_errors_on_macro_in_host_position() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://${HOST}/pixel?id=1",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.macro.unsafe_position"]
        );
        assert_eq!(
            summary.reports[0].violations[0].field.as_deref(),
            Some("url.host")
        );
        assert_eq!(summary.reports[0].violations[0].targets.len(), 1);
        assert_eq!(
            summary.reports[0].violations[0].targets[0].component,
            ViolationTargetComponent::Host
        );
        assert_eq!(
            summary.reports[0].violations[0].targets[0].value.as_deref(),
            Some("${HOST}")
        );
    }

    #[test]
    fn detect_macro_spans_walks_utf8_char_boundaries() {
        let artifact = "https://example.com/event.png?cb=abc\u{FFFD}def&id=1";
        let spans = detect_macro_spans(artifact);
        assert!(spans.is_empty());

        // Broken vendor macro `{` + replacement char + `name}` as seen on
        // Amazon VFW IAS wrappers after D1 storage.
        let broken = "https://example.com/ias.gif?campId={\u{FFFD}mpaign_cfid}&x=1";
        assert!(detect_macro_spans(broken).is_empty());

        let with_macro = "https://example.com/event.png?cb=abc\u{FFFD}def&price=${AUCTION_PRICE}";
        let spans = detect_macro_spans(with_macro);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            &with_macro[spans[0].start..spans[0].end],
            "${AUCTION_PRICE}"
        );
    }

    fn directory_request(artifact: &str) -> ValidationRequest {
        ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        }
    }

    #[test]
    fn the_directory_attributes_endpoints_no_rulepack_claims() {
        let summary = Engine::default()
            .validate(
                &directory_request("https://trc.taboola.com/actions?a=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        let report = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == DIRECTORY_ID)
            .expect("directory report");
        assert_eq!(report.detected_vendor.as_deref(), Some("taboola"));
        assert_eq!(report.violations[0].code, "directory.no_rulepack_coverage");
        assert_eq!(report.violations[0].severity, Severity::Info);
        assert!(report.is_ok(), "attribution must never fail an artifact");
    }

    #[test]
    fn a_matching_rulepack_suppresses_the_directory() {
        let summary = Engine::default()
            .validate(
                &directory_request("https://www.facebook.com/tr?id=1234567890123456&ev=PageView"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(
            summary
                .reports
                .iter()
                .all(|report| report.plugin_id != DIRECTORY_ID),
            "a vendor pack is better information than an attribution"
        );
    }

    #[test]
    fn a_known_vendor_on_an_uncovered_endpoint_still_gets_attributed() {
        let summary = Engine::default()
            .validate(
                &directory_request("https://www.facebook.com/some/other/path"),
                &ValidationOptions::default(),
            )
            .unwrap();

        let report = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == DIRECTORY_ID)
            .expect("directory report");
        assert_eq!(report.detected_vendor.as_deref(), Some("meta"));
        assert!(
            report.violations[0].message.contains("`vendor/meta`"),
            "{}",
            report.violations[0].message
        );
    }

    #[test]
    fn unknown_hosts_get_no_directory_report() {
        let summary = Engine::default()
            .validate(
                &directory_request("https://pixel.example.com/collect?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, "core");
    }

    #[test]
    fn a_directory_overlay_attributes_new_hosts() {
        let extra = VendorDirectory::from_json(
            r#"{
                "entries": [{
                    "vendor": "acme",
                    "display_name": "Acme",
                    "category": "analytics",
                    "hosts": ["px.acme.example"]
                }]
            }"#,
        )
        .expect("overlay");
        let mut engine = Engine::default();
        engine.merge_directory(extra).expect("merge");

        let summary = engine
            .validate(
                &directory_request("https://px.acme.example/collect?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == DIRECTORY_ID)
            .expect("directory report");
        assert_eq!(report.detected_vendor.as_deref(), Some("acme"));
        assert!(report.is_ok());
    }

    #[test]
    fn the_directory_honors_rulepack_toggles() {
        let engine = Engine::default();
        let request = directory_request("https://trc.taboola.com/actions?a=1");

        let summary = engine
            .validate(
                &request,
                &ValidationOptions {
                    except_rulepacks: vec![DIRECTORY_ID.to_string()],
                    ..ValidationOptions::default()
                },
            )
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .all(|report| report.plugin_id != DIRECTORY_ID)
        );

        let summary = engine
            .validate(
                &request,
                &ValidationOptions {
                    only_rulepacks: vec![DIRECTORY_ID.to_string()],
                    ..ValidationOptions::default()
                },
            )
            .unwrap();
        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, DIRECTORY_ID);

        let summary = engine
            .validate(
                &request,
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string()],
                    ..ValidationOptions::default()
                },
            )
            .unwrap();
        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, "core");
    }

    #[test]
    fn an_engine_without_a_directory_attributes_nothing() {
        let mut engine = Engine::default();
        engine.set_directory(VendorDirectory::default());

        let summary = engine
            .validate(
                &directory_request("https://trc.taboola.com/actions?a=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, "core");
    }
}
