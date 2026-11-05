// @Time    : 2026/11/4 19:44
// @Author  : fzf
// @FileName: line.rs
// @Software: Siskin

//! Terminal cells. Rendering, wrapping and the command engine all work on the
//! same span list, so nothing has to re-parse styled text later.

use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Prompt,
    Command,
    Output,
    System,
    Path,
    Ok,
    Warn,
    Error,
    Link,
    Faint,
}

impl Role {
    pub fn color(self, palette: &Palette) -> u32 {
        match self {
            Role::Prompt => palette.prompt,
            Role::Command => palette.command,
            Role::Output => palette.output,
            Role::System => palette.system,
            Role::Path => palette.path,
            Role::Ok => palette.ok,
            Role::Warn => palette.warn,
            Role::Error => palette.error,
            Role::Link => palette.link,
            Role::Faint => palette.faint_text,
        }
    }

    pub fn is_bold(self) -> bool {
        matches!(self, Role::Prompt | Role::Command)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub role: Role,
}

impl Span {
    pub fn new(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
        }
    }
}

/// Builds a span list from alternating text and style segments.
#[macro_export]
macro_rules! spans {
    ($($text:expr => $role:expr),* $(,)?) => {
        vec![$( $crate::line::Span::new($text, $role) ),*]
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn new(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    pub fn text(text: impl Into<String>, role: Role) -> Self {
        Self::new(vec![Span::new(text, role)])
    }

    pub fn plain(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    pub fn is_blank(&self) -> bool {
        self.spans.iter().all(|span| span.text.trim().is_empty())
    }

    pub fn push(&mut self, text: impl Into<String>, role: Role) {
        self.spans.push(Span::new(text, role));
    }

    /// Soft-wraps a logical line to `columns`, keeping each span's role.
    /// Wide characters count as one column, which is enough for this demo UI.
    pub fn wrap(&self, columns: usize) -> Vec<Line> {
        let columns = columns.max(8);
        let mut rows = vec![Line::default()];
        let mut used = 0;
        let mut chunk = String::new();
        let mut chunk_role = Role::Output;

        // Flushing appends to the row being built, so styles stay per span.
        macro_rules! flush {
            () => {
                if !chunk.is_empty() {
                    rows.last_mut()
                        .unwrap()
                        .push(std::mem::take(&mut chunk), chunk_role);
                }
            };
        }
        macro_rules! new_row {
            () => {{
                flush!();
                rows.push(Line::default());
                used = 0;
            }};
        }

        for span in &self.spans {
            if !chunk.is_empty() && chunk_role != span.role {
                flush!();
            }
            chunk_role = span.role;
            for ch in span.text.chars() {
                if ch == '\n' {
                    new_row!();
                    continue;
                }
                if used == columns {
                    new_row!();
                }
                chunk.push(ch);
                used += 1;
            }
        }
        flush!();
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_keeps_roles_and_never_loses_text() {
        let line = Line::new(spans![
            "abcdefgh" => Role::Command,
            "ijklmnop" => Role::Ok,
        ]);
        let rows = line.wrap(8);
        // Sixteen columns at width eight is two rows; the span boundary in the
        // middle must not add a row or leak styles across it.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain(), "abcdefgh");
        assert_eq!(rows[1].plain(), "ijklmnop");
        assert_eq!(rows[0].spans.first().unwrap().role, Role::Command);
        assert_eq!(rows[1].spans.first().unwrap().role, Role::Ok);
        assert_eq!(
            rows.iter().map(Line::plain).collect::<String>(),
            "abcdefghijklmnop"
        );
    }

    #[test]
    fn wrapping_at_an_exact_boundary_adds_no_trailing_row() {
        let rows = Line::text("abcdefgh", Role::Output).wrap(8);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].plain(), "abcdefgh");
    }

    #[test]
    fn embedded_newlines_start_new_rows() {
        let line = Line::text("one\ntwo", Role::Output);
        let rows = line.wrap(40);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain(), "one");
        assert_eq!(rows[1].plain(), "two");
    }

    #[test]
    fn wide_columns_are_clamped_to_a_usable_width() {
        let rows = Line::text("abc", Role::Output).wrap(0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].plain(), "abc");
    }
}
