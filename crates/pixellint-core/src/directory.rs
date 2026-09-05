//! Vendor endpoint directory.
//!
//! The directory answers one question a rulepack cannot: whose pixel is this?
//! It maps hosts to vendors and nothing else. Entries carry no parameter
//! contracts and no rule text, so a directory hit never asserts that an
//! artifact is right or wrong; it says which vendor owns the endpoint and
//! whether Pixellint has a rulepack for it.
//!
//! That split is deliberate. Rules require a citable contract, and most vendors
//! never publish one. Attribution is a weaker claim that can be made honestly
//! for the long tail.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Pseudo-rulepack id used to include or exclude the directory in
/// [`ValidationOptions`](crate::ValidationOptions).
pub const DIRECTORY_ID: &str = "directory";

/// One vendor and the endpoints it serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorEntry {
    /// Stable vendor slug, reported as `detected_vendor`.
    pub vendor: String,
    pub display_name: String,
    /// Broad function of the endpoint: social, programmatic, analytics, and so on.
    pub category: String,
    /// Hosts the vendor serves. A host also matches its subdomains.
    pub hosts: Vec<String>,
    /// First-party rulepack that covers some of this vendor's endpoints, if one
    /// exists.
    #[serde(default)]
    pub rulepack: Option<String>,
}

/// A set of vendor entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorDirectory {
    entries: Vec<VendorEntry>,
}

/// Why a directory could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryError {
    Parse(String),
    Io(String),
    EmptyField {
        vendor: String,
        field: &'static str,
    },
    DuplicateHost {
        host: String,
        vendors: (String, String),
    },
}

impl fmt::Display for DirectoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "invalid vendor directory JSON: {error}"),
            Self::Io(error) => write!(f, "could not read vendor directory: {error}"),
            Self::EmptyField { vendor, field } => {
                write!(
                    f,
                    "vendor directory entry `{vendor}` has an empty `{field}`"
                )
            }
            Self::DuplicateHost {
                host,
                vendors: (first, second),
            } => write!(
                f,
                "host `{host}` is claimed by both `{first}` and `{second}` in the vendor directory"
            ),
        }
    }
}

impl Error for DirectoryError {}

impl VendorDirectory {
    /// The directory compiled into the crate.
    ///
    /// Parsing is proven by tests, so a malformed built-in directory is a build
    /// failure rather than a runtime surprise.
    pub fn builtin() -> Self {
        Self::from_json(crate::BUILTIN_VENDOR_DIRECTORY)
            .expect("built-in vendor directory should be valid")
    }

    pub fn from_json(json: &str) -> Result<Self, DirectoryError> {
        let directory: Self =
            serde_json::from_str(json).map_err(|error| DirectoryError::Parse(error.to_string()))?;
        directory.validate()?;
        Ok(directory)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, DirectoryError> {
        let path = path.as_ref();
        let json = fs::read_to_string(path)
            .map_err(|error| DirectoryError::Io(format!("{}: {error}", path.display())))?;
        Self::from_json(&json)
    }

    fn validate(&self) -> Result<(), DirectoryError> {
        let mut claimed: Vec<(String, &str)> = Vec::new();

        for entry in &self.entries {
            for (field, value) in [
                ("vendor", &entry.vendor),
                ("display_name", &entry.display_name),
                ("category", &entry.category),
            ] {
                if value.trim().is_empty() {
                    return Err(DirectoryError::EmptyField {
                        vendor: entry.vendor.clone(),
                        field,
                    });
                }
            }

            if entry.hosts.is_empty() {
                return Err(DirectoryError::EmptyField {
                    vendor: entry.vendor.clone(),
                    field: "hosts",
                });
            }

            for host in &entry.hosts {
                let host = host.to_ascii_lowercase();

                if let Some((_, owner)) = claimed.iter().find(|(claimed, _)| claimed == &host) {
                    return Err(DirectoryError::DuplicateHost {
                        host,
                        vendors: ((*owner).to_string(), entry.vendor.clone()),
                    });
                }

                claimed.push((host, entry.vendor.as_str()));
            }
        }

        Ok(())
    }

    pub fn entries(&self) -> &[VendorEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total hosts across every entry.
    pub fn host_count(&self) -> usize {
        self.entries.iter().map(|entry| entry.hosts.len()).sum()
    }

    /// Finds the vendor that serves a host. A directory host also matches its
    /// subdomains, so `sc-static.net` covers `cdn.sc-static.net`.
    pub fn lookup_host(&self, host: &str) -> Option<&VendorEntry> {
        let host = host.to_ascii_lowercase();

        self.entries.iter().find(|entry| {
            entry.hosts.iter().any(|candidate| {
                let candidate = candidate.to_ascii_lowercase();
                host == candidate || host.ends_with(&format!(".{candidate}"))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DIRECTORY: &str = r#"{
        "entries": [
            {
                "vendor": "acme",
                "display_name": "Acme",
                "category": "programmatic",
                "hosts": ["px.acme.example", "acme-static.example"],
                "rulepack": "vendor/acme"
            },
            {
                "vendor": "globex",
                "display_name": "Globex",
                "category": "analytics",
                "hosts": ["globex.example"]
            }
        ]
    }"#;

    #[test]
    fn lookup_matches_exact_hosts_and_subdomains() {
        let directory = VendorDirectory::from_json(TEST_DIRECTORY).expect("parse");

        assert_eq!(
            directory.lookup_host("px.acme.example").map(|e| &e.vendor),
            Some(&"acme".to_string())
        );
        assert_eq!(
            directory
                .lookup_host("cdn.acme-static.example")
                .map(|e| &e.vendor),
            Some(&"acme".to_string())
        );
        assert_eq!(
            directory.lookup_host("GLOBEX.EXAMPLE").map(|e| &e.vendor),
            Some(&"globex".to_string())
        );
        assert_eq!(directory.lookup_host("notglobex.example"), None);
        assert_eq!(directory.lookup_host("example.com"), None);
    }

    #[test]
    fn duplicate_hosts_are_rejected() {
        let json = TEST_DIRECTORY.replace(r#""globex.example""#, r#""px.acme.example""#);
        let error = VendorDirectory::from_json(&json).expect_err("duplicates are caught");
        assert!(
            matches!(error, DirectoryError::DuplicateHost { .. }),
            "{error}"
        );
    }

    #[test]
    fn entries_need_a_vendor_and_hosts() {
        let json = TEST_DIRECTORY.replace(r#""vendor": "globex""#, r#""vendor": """#);
        let error = VendorDirectory::from_json(&json).expect_err("empty vendor is caught");
        assert!(
            matches!(error, DirectoryError::EmptyField { .. }),
            "{error}"
        );

        let json = TEST_DIRECTORY.replace(r#""hosts": ["globex.example"]"#, r#""hosts": []"#);
        let error = VendorDirectory::from_json(&json).expect_err("empty hosts are caught");
        assert!(
            matches!(error, DirectoryError::EmptyField { .. }),
            "{error}"
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json =
            TEST_DIRECTORY.replace(r#""vendor": "acme","#, r#""vendor": "acme", "oops": 1,"#);
        let error = VendorDirectory::from_json(&json).expect_err("unknown fields are caught");
        assert!(matches!(error, DirectoryError::Parse(_)), "{error}");
    }

    #[test]
    fn the_builtin_directory_loads_and_covers_the_shipped_packs() {
        let directory = VendorDirectory::builtin();

        assert!(
            directory.len() >= 80,
            "directory shrank to {}",
            directory.len()
        );
        assert!(directory.host_count() >= 200);

        for host in [
            "www.facebook.com",
            "analytics.tiktok.com",
            "ct.pinterest.com",
            "trc.taboola.com",
            "pixel.quantserve.com",
            "track.celtra.com",
            "tpsc-video-as.doubleverify.com",
            "tpsc-video-eu.doubleverify.com",
            "d9.flashtalking.com",
            "qa-xre.flashtalking.net",
            "s.update.3lift.com",
            "rtb-us-west.linkedin.com",
            "beacons.extremereach.io",
            "analytics.adcanvas.com",
            "stats.sxp.smartclip.net",
            "log.xpln.tech",
            "cs10.connected-stories.com",
            "postback.iqm.com",
            "tr.blismedia.com",
        ] {
            assert!(
                directory.lookup_host(host).is_some(),
                "{host} is not in the directory"
            );
        }

        // Every rulepack an entry points at has to exist, and every first-party
        // pack's vendor has to point at some pack. Directory hits on a covered
        // vendor should say coverage exists elsewhere, not that the vendor is
        // unknown to the rulepacks.
        let pack_ids: Vec<&str> = crate::BUILTIN_VENDOR_MANIFESTS
            .iter()
            .map(|(id, _)| *id)
            .collect();
        for entry in directory.entries() {
            if let Some(rulepack) = &entry.rulepack {
                assert!(
                    pack_ids.contains(&rulepack.as_str()),
                    "directory entry `{}` points at unknown rulepack `{rulepack}`",
                    entry.vendor
                );
            }
        }

        for (id, json) in crate::BUILTIN_VENDOR_MANIFESTS {
            let manifest: serde_json::Value =
                serde_json::from_str(json).unwrap_or_else(|error| panic!("{id}: {error}"));
            let vendor = manifest["vendor"]
                .as_str()
                .unwrap_or_else(|| panic!("{id} is missing vendor"));
            let entry = directory
                .entries()
                .iter()
                .find(|entry| entry.vendor == vendor);
            let Some(entry) = entry else {
                panic!("no directory entry for pack `{id}` vendor `{vendor}`");
            };
            assert!(
                entry.rulepack.is_some(),
                "directory entry `{vendor}` has pack `{id}` but no rulepack pointer"
            );
        }
    }
}
