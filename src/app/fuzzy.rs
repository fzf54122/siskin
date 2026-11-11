// @Time    : 2026/11/11 14:40
// @Author  : fzf
// @FileName: fuzzy.rs
// @Software: Siskin

//! 模糊匹配：子序列匹配 + 打分，供命令面板用。纯函数，不碰 IO。

/// 一次模糊匹配的结果。`ranges` 是原串里被命中字符的字节区间，
/// 供 UI 高亮用；分数越大越相关。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FuzzyMatch {
    pub score: i32,
    pub ranges: Vec<(usize, usize)>,
}

/// 子序列匹配（不区分大小写）。查询为空返回 `None`，
/// 调用方把空查询当“全部展示”处理。
pub(crate) fn fuzzy_match(query: &str, text: &str) -> Option<FuzzyMatch> {
    if query.is_empty() || text.is_empty() {
        return None;
    }
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    // 原串每个字符：字节区间 + 小写后的首字符（个别字符小写会变长，
    // 只取首字符比较，标签与关键词里不会出现这种情况）。
    let text_chars: Vec<(usize, usize, char)> = text
        .char_indices()
        .map(|(start, ch)| {
            let end = start + ch.len_utf8();
            (start, end, ch.to_lowercase().next().unwrap_or(ch))
        })
        .collect();
    let mut ranges = Vec::with_capacity(query.len());
    let mut score = 0i32;
    let mut cursor = 0usize;
    let mut prev_matched: Option<usize> = None;
    for &want in &query {
        let found = text_chars
            .iter()
            .enumerate()
            .skip(cursor)
            .find(|(_, (_, _, have))| *have == want)
            .map(|(i, _)| i)?;
        let (start, end, _) = text_chars[found];
        ranges.push((start, end));
        score += 10;
        if found == 0 {
            score += 12;
        } else if !text_chars[found - 1].2.is_alphanumeric() {
            // 词首命中（空格或标点之后）比词中命中更相关。
            score += 8;
        }
        match prev_matched {
            Some(prev) if prev + 1 == found => score += 5,
            Some(prev) => score -= 3 * (found - prev - 1) as i32,
            None => {}
        }
        prev_matched = Some(found);
        cursor = found + 1;
    }
    Some(FuzzyMatch { score, ranges })
}
