// @Time    : 2026/11/5 22:15
// @Author  : fzf
// @FileName: links.rs
// @Software: Siskin

//! 链接识别。只认白名单内的协议，其它一律不打开：
//! 终端里的输出来自不可信程序，`javascript:` 之类的协议不能被直接交给系统。

use std::path::{Path, PathBuf};

/// 允许打开的协议。`file://` 只在路径存在时才算有效。
const WEB_SCHEMES: [&str; 3] = ["http://", "https://", "file://"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Web(String),
    /// 本地路径，已经相对于会话 cwd 解析过。
    File(PathBuf),
}

/// 从一行文本里取出所有可点击片段及其列区间。
pub(crate) fn find(text: &str, cwd: Option<&Path>) -> Vec<(std::ops::Range<usize>, Target)> {
    const LEADING: [char; 6] = ['(', '[', '{', '<', '"', '\''];
    const TRAILING: [char; 9] = ['.', ',', ';', ':', ')', ']', '}', '>', '"'];
    let mut found = Vec::new();
    for (start, word) in words(text) {
        let trimmed = word.trim_start_matches(LEADING).trim_end_matches(TRAILING);
        let leading = word.chars().count() - word.trim_start_matches(LEADING).chars().count();
        if trimmed.is_empty() {
            continue;
        }
        let begin = start + leading;
        let end = begin + trimmed.chars().count();
        if let Some(target) = resolve(trimmed, cwd) {
            found.push((begin..end, target));
        }
    }
    found
}

/// 按空白切词，返回每个词的起始列与内容。列号按字符计。
fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<(usize, usize)> = None;
    for (column, (byte, ch)) in text.char_indices().enumerate() {
        if ch.is_whitespace() {
            if let Some((begin, byte_start)) = start.take() {
                out.push((begin, &text[byte_start..byte]));
            }
        } else if start.is_none() {
            start = Some((column, byte));
        }
    }
    if let Some((begin, byte_start)) = start {
        out.push((begin, &text[byte_start..]));
    }
    out
}

fn resolve(word: &str, cwd: Option<&Path>) -> Option<Target> {
    let lower = word.to_lowercase();
    if WEB_SCHEMES.iter().any(|scheme| lower.starts_with(scheme)) {
        return Some(Target::Web(word.to_string()));
    }
    // `path:line:col` 或 `path:line`
    let (path, _) = split_location(word);
    if path.is_empty() {
        return None;
    }
    let candidate = if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        cwd?.join(path)
    };
    if candidate.exists() {
        return Some(Target::File(candidate));
    }
    None
}

/// 剥掉末尾的 `:行号[:列号]`，返回路径与可选位置。
///
/// 从左往右数「最后两段」：`path:12:7` 取行 12、列 7；`path:12` 取行 12；
/// `main.rs` 里没有可解析的尾部数字，原样返回。
pub(crate) fn split_location(word: &str) -> (&str, Option<(u32, Option<u32>)>) {
    let parts: Vec<&str> = word.split(':').collect();
    if parts.len() < 2 {
        return (word, None);
    }
    let last = parts[parts.len() - 1];
    let previous = parts[parts.len() - 2];

    // `path:line:col`
    if let (Ok(line), Ok(column)) = (previous.parse::<u32>(), last.parse::<u32>()) {
        let end = word.len() - last.len() - 1 - previous.len() - 1;
        return (&word[..end], Some((line, Some(column))));
    }
    // `path:line`
    if let Ok(line) = last.parse::<u32>() {
        let end = word.len() - last.len() - 1;
        return (&word[..end], Some((line, None)));
    }
    (word, None)
}

/// 用系统默认程序打开目标。`Web` 与 `File` 走同一条路径，
/// 不用 shell 拼接，避免把输出里的内容当命令执行。
pub(crate) fn open(target: &Target) -> Result<(), String> {
    let (program, argument) = match target {
        Target::Web(url) => ("xdg-open", url.clone()),
        Target::File(path) => ("xdg-open", path.display().to_string()),
    };
    match std::process::Command::new(program).arg(&argument).spawn() {
        Ok(_) => Ok(()),
        Err(error) => Err(format!("无法打开 {argument}：{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_and_https_are_accepted() {
        let found = find("see https://example.com/a and http://b.test", None);
        assert_eq!(found.len(), 2);
        assert!(matches!(&found[0].1, Target::Web(url) if url.starts_with("https://")));
    }

    #[test]
    fn other_schemes_are_rejected() {
        for text in ["javascript:alert(1)", "data:text/html,hi", "ftp://x.test"] {
            assert!(find(text, None).is_empty(), "{text}");
        }
    }

    #[test]
    fn relative_paths_resolve_against_the_session_cwd() {
        let cwd = std::env::temp_dir();
        let file = cwd.join("siskin-link-test");
        std::fs::write(&file, b"x").expect("测试需要可写临时目录");
        let found = find("open siskin-link-test:12:3 now", Some(&cwd));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 5..26);
        assert!(matches!(&found[0].1, Target::File(path) if path.ends_with("siskin-link-test")));
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_the_link() {
        // 词是 `(https://example.com).`：两端标点都被剥掉，
        // 区间正好覆盖 `https://example.com`（1..20）。
        let found = find("(https://example.com).", None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1..20);
    }

    #[test]
    fn line_and_column_suffixes_are_split_off() {
        let (path, location) = split_location("src/main.rs:42:7");
        assert_eq!(path, "src/main.rs");
        assert_eq!(location, Some((42, Some(7))));

        let (path, location) = split_location("src/main.rs:42");
        assert_eq!(path, "src/main.rs");
        assert_eq!(location, Some((42, None)));

        let (path, location) = split_location("src/main.rs");
        assert_eq!(path, "src/main.rs");
        assert_eq!(location, None);
    }
}
