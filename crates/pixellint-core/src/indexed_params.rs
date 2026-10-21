use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn matching_prefix<'a>(name: &str, prefixes: &'a [String]) -> Option<(&'a str, usize)> {
    prefixes.iter().find_map(|prefix| {
        let suffix = name.strip_prefix(prefix.as_str())?;
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some((prefix.as_str(), suffix.parse().unwrap_or(usize::MAX)))
    })
}

pub(crate) fn valid<'a>(
    names: impl Iterator<Item = &'a str>,
    required: &[String],
    optional: &[String],
    max_index: usize,
    contiguous: bool,
) -> bool {
    let prefixes: Vec<_> = required.iter().chain(optional).cloned().collect();
    let mut groups = BTreeMap::<usize, BTreeSet<&str>>::new();
    for name in names {
        let Some((prefix, index)) = matching_prefix(name, &prefixes) else {
            continue;
        };
        if index == 0 || index > max_index {
            return false;
        }
        groups.entry(index).or_default().insert(prefix);
    }
    groups.values().all(|group| {
        required
            .iter()
            .all(|prefix| group.contains(prefix.as_str()))
    }) && (!contiguous
        || groups
            .last_key_value()
            .is_none_or(|(&last, _)| last == groups.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_index_identity_instead_of_only_family_counts() {
        let required = ["ITEM", "AMT", "QTY"].map(String::from);
        let optional = [String::from("DCNT")];
        for (names, expected) in [
            (vec![], true),
            (vec!["ITEM1", "AMT1", "QTY1", "DCNT1"], true),
            (vec!["ITEM1", "AMT2", "QTY1"], false),
            (vec!["ITEM2", "AMT2", "QTY2"], false),
            (vec!["DCNT1"], false),
            (vec!["ITEM0", "AMT0", "QTY0"], false),
            (vec!["ITEM101", "AMT101", "QTY101"], false),
            (vec!["ITEM1", "AMT1", "QTY1", "QTY1"], true),
            (vec!["ITEM9999999999999999999999999999999999999"], false),
            (vec!["ITEM_future", "custom"], true),
        ] {
            assert_eq!(
                valid(names.into_iter(), &required, &optional, 100, true),
                expected
            );
        }
        let names: Vec<_> = (1..=100)
            .flat_map(|i| required.iter().map(move |p| format!("{p}{i}")))
            .collect();
        assert!(valid(
            names.iter().map(String::as_str),
            &required,
            &optional,
            100,
            true
        ));
    }
}
