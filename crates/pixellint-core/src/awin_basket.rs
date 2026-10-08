//! Published Awin basket relationships within one sale request.
//!
//! https://help.awin.com/developers/docs/product-level-tracking-2
//! Read 2026-10-07. Every product is a bd[n] query entry, sequential from zero,
//! and tuple field eight must name a commission group declared by Sale.parts.
//! The field format separately checks tuple syntax and monetary values.

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BasketParam<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BasketIssueKind {
    IndexGap,
    DuplicateIndex,
    GroupMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BasketIssue {
    pub kind: BasketIssueKind,
    pub name: String,
    pub value: String,
    pub index: String,
    pub start: usize,
    pub end: usize,
    pub message: String,
}

fn index(name: &str) -> Option<String> {
    let digits = name.strip_prefix("bd[")?.strip_suffix(']')?;
    if digits.is_empty() || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let trimmed = digits.trim_start_matches('0');
    Some(if trimmed.is_empty() { "0" } else { trimmed }.to_string())
}

fn increment(digits: &str) -> String {
    let mut digits = digits.as_bytes().to_vec();
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            return String::from_utf8(digits).expect("decimal digits are UTF-8");
        }
        *digit = b'0';
    }
    digits.insert(0, b'1');
    String::from_utf8(digits).expect("decimal digits are UTF-8")
}

fn issue(
    param: BasketParam<'_>,
    index: &str,
    kind: BasketIssueKind,
    message: String,
) -> BasketIssue {
    BasketIssue {
        kind,
        name: param.name.to_string(),
        value: param.value.to_string(),
        index: index.to_string(),
        start: param.start,
        end: param.end,
        message,
    }
}

/// Consumes decoded query pairs and preserves the caller's original spans.
/// Query order does not change array indices. Huge indices yield one gap,
/// rather than overflow or an allocation for every missing product.
pub(crate) fn inspect_basket(pairs: &[BasketParam<'_>], parts: Option<&str>) -> Vec<BasketIssue> {
    let mut rows = pairs
        .iter()
        .copied()
        .filter_map(|param| index(param.name).map(|index| (index, param)))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.0
            .len()
            .cmp(&right.0.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut issues = Vec::new();
    let unresolved_index = pairs.iter().any(|param| {
        param.name.starts_with("bd[") && !crate::detect_macro_spans(param.name).is_empty()
    });
    if !unresolved_index {
        let mut expected = "0".to_string();
        let mut previous: Option<&str> = None;
        for (index, param) in &rows {
            if previous == Some(index.as_str()) {
                issues.push(issue(*param, index, BasketIssueKind::DuplicateIndex,
                    format!("Product index {index} is repeated; each product needs its own sequential bd[n] entry.")));
                continue;
            }
            if *index != expected {
                issues.push(issue(*param, index, BasketIssueKind::IndexGap,
                    format!("Product index {index} skips expected index {expected}; bd[n] indices must be sequential from zero.")));
            }
            previous = Some(index);
            expected = increment(index);
        }
    }
    // An unknown group template might resolve to the row's group. Preserve
    // that uncertainty, rather than declaring it absent from a partial list.
    let groups = parts
        .filter(|parts| crate::detect_macro_spans(parts).is_empty())
        .and_then(|parts| {
            parts
                .split('|')
                .map(|part| {
                    let (group, amount) = part.split_once(':')?;
                    let (integer, fraction) = amount.split_once('.').unwrap_or((amount, ""));
                    if group.is_empty()
                        || integer.is_empty()
                        || !integer.bytes().all(|digit| digit.is_ascii_digit())
                        || !fraction.bytes().all(|digit| digit.is_ascii_digit())
                        || (amount.contains('.') && fraction.is_empty())
                    {
                        None
                    } else {
                        Some(group)
                    }
                })
                .collect::<Option<BTreeSet<_>>>()
        });
    if let Some(groups) = groups {
        for (index, param) in rows {
            if !crate::detect_macro_spans(param.value).is_empty() {
                continue;
            }
            let tuple = param.value.split('|').collect::<Vec<_>>();
            if tuple.len() != 10 || tuple[0] != "AW:P" || tuple[8].is_empty() {
                continue;
            }
            if !groups.contains(tuple[8]) {
                issues.push(issue(
                    param,
                    &index,
                    BasketIssueKind::GroupMissing,
                    format!(
                        "Product commission group `{}` is absent from the sale's parts breakdown.",
                        tuple[8]
                    ),
                ));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    const CD: &str = "AW:P|1001|AA000006|B000EMSUQA|Product|5.55|2|SKU|CD|Music";
    const DVD: &str = "AW:P|1001|AA000006|B001N2Z41Y|Product|14.99|1|SKU|DVD|DVD";

    fn row<'a>(name: &'a str, value: &'a str, start: usize) -> BasketParam<'a> {
        BasketParam {
            name,
            value,
            start,
            end: start + value.len(),
        }
    }

    #[test]
    fn official_group_breakdown_matches_each_product_and_query_order_is_independent() {
        let parts = Some("CD:11.10|DVD:14.99");
        assert!(inspect_basket(&[row("bd[0]", CD, 10), row("bd[1]", DVD, 100)], parts).is_empty());
        assert!(inspect_basket(&[row("bd[1]", DVD, 100), row("bd[0]", CD, 10)], parts).is_empty());
        let issues = inspect_basket(
            &[row("bd[0]", CD, 10), row("bd[1]", DVD, 100)],
            Some("CD:26.09"),
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, BasketIssueKind::GroupMissing);
        assert_eq!(issues[0].name, "bd[1]");
        assert_eq!((issues[0].start, issues[0].end), (100, 100 + DVD.len()));
        assert_eq!(issues[0].value, DVD);
        assert_eq!(
            inspect_basket(&[row("bd[0]", CD, 0)], Some("cd:11.10"))[0].kind,
            BasketIssueKind::GroupMissing
        );
    }

    #[test]
    fn later_missing_and_duplicate_indices_target_the_offending_product() {
        let issues = inspect_basket(&[row("bd[0]", CD, 5), row("bd[2]", CD, 30)], None);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, BasketIssueKind::IndexGap);
        assert_eq!(issues[0].start, 30);
        assert!(issues[0].message.contains("expected index 1"));
        let issues = inspect_basket(&[row("bd[1]", CD, 5)], None);
        assert!(issues[0].message.contains("expected index 0"));
        let issues = inspect_basket(&[row("bd[0]", CD, 5), row("bd[00]", CD, 30)], None);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, BasketIssueKind::DuplicateIndex);
        assert_eq!(issues[0].start, 30);
    }

    #[test]
    fn arbitrary_precision_indices_do_not_overflow_or_enumerate_missing_rows() {
        let name = format!("bd[{}]", "9".repeat(1000));
        let issues = inspect_basket(&[row(&name, CD, 25)], None);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, BasketIssueKind::IndexGap);
        assert_eq!(issues[0].index.len(), 1000);
        assert_eq!(
            increment(&"9".repeat(1000)),
            format!("1{}", "0".repeat(1000))
        );
    }

    #[test]
    fn templates_and_malformed_rows_retain_their_separate_syntax_contract() {
        assert!(inspect_basket(&[row("bd[0]", CD, 0)], Some("[PARTS]")).is_empty());
        assert!(inspect_basket(&[row("bd[0]", "[PRODUCT]", 0)], Some("CD:11.10")).is_empty());
        assert!(inspect_basket(&[row("bd[0]", "AW:P|bad", 0)], Some("CD:11.10")).is_empty());
        assert!(inspect_basket(&[row("bd[0]", DVD, 0)], Some("CD:bad")).is_empty());
        assert!(
            inspect_basket(
                &[
                    row("bd[0]", CD, 0),
                    row("bd[2]", CD, 20),
                    row("bd[INDEX]", CD, 40)
                ],
                None
            )
            .is_empty()
        );
        assert!(inspect_basket(&[row("p1", "custom", 0)], Some("CD:11.10")).is_empty());
    }
}
