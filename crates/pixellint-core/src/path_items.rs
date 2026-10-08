//! Independent bracketed baskets in colon-delimited conversion paths.

use crate::manifest::{ParamStyle, extract_params, path_span};
use crate::query_segments::QuerySegment;

pub(crate) fn path_items(
    artifact: &str,
    first_index: usize,
) -> Result<Vec<QuerySegment<'_>>, (usize, usize)> {
    let (start, end) = path_span(artifact);
    let bytes = artifact.as_bytes();
    let mut groups = Vec::new();
    let mut index = 0;
    let mut cursor = start;
    let all_params = extract_params(artifact, ParamStyle::ColonPath);
    while cursor < end {
        if bytes[cursor] != b'[' || (cursor > start && bytes[cursor - 1] != b'/') {
            cursor += 1;
            continue;
        }
        let group_start = cursor;
        let mut depth = 1;
        cursor += 1;
        while cursor < end && depth > 0 {
            match bytes[cursor] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            cursor += 1;
        }
        if depth != 0 || (cursor < end && bytes[cursor] != b'/') {
            return Err((group_start, cursor));
        }
        let text = &artifact[group_start..cursor];
        // A whole basket template has unknown structure. Field-level macros
        // still belong to a concrete basket and cannot hide its other fields.
        if crate::detect_macro_spans(text)
            .iter()
            .any(|span| span.start == 0 && span.end == text.len())
        {
            index += 1;
            continue;
        }
        if index >= first_index {
            groups.push(QuerySegment {
                index,
                start: group_start,
                end: cursor,
                params: all_params
                    .iter()
                    .filter(|param| param.start >= group_start && param.end <= cursor)
                    .cloned()
                    .collect(),
            });
        }
        index += 1;
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_macros_and_encoded_brackets_preserve_basket_boundaries() {
        let url = "https://example.test/conversion/clickref:[CLICKREF]/[category:ONE/sku:[SKU]/quantity:1]/[sku:%5BX%5D/quantity:2]";
        let groups = path_items(url, 0).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].params.len(), 3);
        assert_eq!(groups[0].params[1].value, "[SKU]");
        assert_eq!(groups[1].params.len(), 2);
        assert_eq!(groups[1].params[0].value, "[X]");
        assert!(
            groups[1]
                .params
                .iter()
                .all(|param| param.name != "category")
        );
    }

    #[test]
    fn whole_basket_macros_are_unknown_but_empty_and_malformed_baskets_are_checked() {
        let url = "https://example.test/conversion/[BASKET]/[]/[sku:a]";
        let groups = path_items(url, 0).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].index, 1);
        assert!(groups[0].params.is_empty());
        assert!(path_items("https://example.test/conversion/[sku:a", 0).is_err());
        assert!(path_items("https://example.test/conversion/[sku:a]bad", 0).is_err());
    }
}
