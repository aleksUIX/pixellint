//! Raw semicolon-separated query groups with original artifact spans.

use crate::manifest::{ParamStyle, RawParam, extract_params};

/// One query group. Empty groups retain their original index.
pub(crate) struct QuerySegment<'a> {
    pub(crate) index: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) params: Vec<RawParam<'a>>,
}

/// Splits only literal semicolons, excluding the fragment. Percent-encoded
/// delimiters remain in their parameter values and use the normal decoding.
/// `first_index` can skip a transport's global and targeting groups without
/// changing the absolute indices or spans of its later slot groups.
pub(crate) fn query_segments(artifact: &str, first_index: usize) -> Vec<QuerySegment<'_>> {
    let query_end = artifact.find('#').unwrap_or(artifact.len());
    let Some(query_start) = artifact[..query_end].find('?').map(|start| start + 1) else {
        return Vec::new();
    };
    let mut params = extract_params(artifact, ParamStyle::QuerySemicolon)
        .into_iter()
        .peekable();
    let mut groups = Vec::new();
    let mut start = query_start;

    for (index, text) in artifact[query_start..query_end].split(';').enumerate() {
        let end = start + text.len();
        let mut group_params = Vec::new();
        while params.peek().is_some_and(|param| param.start < end) {
            let param = params.next().expect("peeked parameter exists");
            if index >= first_index && param.start >= start && param.end <= end {
                group_params.push(param);
            }
        }
        if index >= first_index {
            groups.push(QuerySegment {
                index,
                start,
                end,
                params: group_params,
            });
        }
        start = end + 1;
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_groups_skip_global_and_targeting_without_merging_slots() {
        let input = "https://demo.v.fwmrm.net/ad/g/1?nw=1;slid=targeting;slid=pre&ptgt=a&tpos=0;slid=mid&ptgt=a;tpos=tail";
        let groups = query_segments(input, 2);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].index, 2);
        assert_eq!(groups[0].params.len(), 3);
        assert_eq!(groups[0].params[0].value, "pre");
        assert_eq!(groups[1].params.len(), 2);
        assert!(!groups[1].params.iter().any(|param| param.name == "tpos"));
        assert_eq!(groups[2].params[0].name, "tpos");
        for group in groups {
            for param in group.params {
                assert!(param.start >= group.start && param.end <= group.end);
                assert!(input[param.start..param.end].contains('='));
            }
        }
    }

    #[test]
    fn encoded_semicolons_and_ampersands_remain_value_content() {
        let input = "https://example.test/?nw=1;;slid=pre%3Bpart%26x&sl%61u=preroll";
        let groups = query_segments(input, 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].params[0].value, "pre;part&x");
        assert_eq!(groups[0].params[1].name, "slau");
        assert_eq!(groups[0].params[1].value, "preroll");
    }

    #[test]
    fn empty_groups_retain_indices_and_fragment_is_excluded() {
        let input = "https://example.test/?nw=1;;;slid=pre;#slid=fragment;ptgt=a";
        let groups = query_segments(input, 2);
        assert_eq!(
            groups.iter().map(|group| group.index).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        assert_eq!(groups[0].start, groups[0].end);
        assert!(groups[0].params.is_empty());
        assert_eq!(groups[1].params[0].value, "pre");
        assert!(groups[2].params.is_empty());
        assert_eq!(groups[2].end, input.find('#').unwrap());
    }

    #[test]
    fn macros_repeated_fields_and_unicode_keep_normal_decoding_and_byte_spans() {
        let input = "https://example.test/?global=é;;slid=é&slid=%%ADID%%&slau=a+b";
        let groups = query_segments(input, 2);
        let params = &groups[0].params;
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].value, "é");
        assert_eq!(&input[params[0].start..params[0].end], "slid=é");
        assert_eq!(params[1].value, "%%ADID%%");
        assert_eq!(params[2].value, "a b");
    }

    #[test]
    fn absent_query_and_query_before_fragment_only() {
        assert!(query_segments("https://example.test/path;slid=x", 0).is_empty());
        assert!(query_segments("https://example.test/#?slid=x", 0).is_empty());
        assert!(query_segments("https://example.test/?nw=1", 2).is_empty());
        let groups = query_segments("https://example.test/?nw=1;", 0);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].params[0].name, "nw");
        assert!(groups[1].params.is_empty());
    }
}
