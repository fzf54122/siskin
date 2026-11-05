// @Time    : 2026/11/4 19:56
// @Author  : fzf
// @FileName: shell.rs
// @Software: Siskin

//! A deterministic, read-only shell. It never spawns processes and never reads
//! the disk, so `ls`, `cat` and `cargo check` look the same on every machine.

use crate::fs;
use crate::line::{Line, Role};
use crate::spans;

pub const COMMANDS: [&str; 12] = [
    "cargo", "cat", "cd", "clear", "date", "echo", "git", "help", "ls", "pwd", "siskin", "whoami",
];

fn shared_prefix(words: &[String]) -> String {
    let mut prefix = words.first().cloned().unwrap_or_default();
    for word in words.iter().skip(1) {
        let keep = prefix
            .chars()
            .zip(word.chars())
            .take_while(|(left, right)| left == right)
            .count();
        let cut = prefix
            .char_indices()
            .nth(keep)
            .map_or(prefix.len(), |(at, _)| at);
        prefix.truncate(cut);
    }
    prefix
}

#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub lines: Vec<Line>,
    pub clear: bool,
}

impl Reply {
    fn output(lines: Vec<Line>) -> Self {
        Self {
            lines,
            clear: false,
        }
    }

    fn quiet() -> Self {
        Self::default()
    }

    fn clear() -> Self {
        Self {
            lines: Vec::new(),
            clear: true,
        }
    }
}

pub struct Shell {
    cwd: Vec<&'static str>,
    history: Vec<String>,
    cursor: Option<usize>,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Shell {
    pub fn new(cwd: Vec<&'static str>) -> Self {
        Self {
            cwd,
            history: Vec::new(),
            cursor: None,
        }
    }

    pub fn cwd_label(&self) -> String {
        if self.cwd.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", self.cwd.join("/"))
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn run(&mut self, command: &str) -> Reply {
        let command = command.trim();
        if command.is_empty() {
            return Reply::quiet();
        }
        if self.history.last().map(String::as_str) != Some(command) {
            self.history.push(command.to_string());
            if self.history.len() > 200 {
                self.history.remove(0);
            }
        }
        self.cursor = None;

        let mut words = command.split_whitespace();
        let name = words.next().unwrap_or_default();
        let args: Vec<&str> = words.collect();

        match (name, args.as_slice()) {
            ("help", []) => Reply::output(help()),
            ("clear", []) => Reply::clear(),
            ("pwd", []) => Reply::output(vec![Line::text(self.cwd_label(), Role::Path)]),
            ("ls", rest) => self.ls(rest),
            ("cd", rest) => self.cd(rest),
            ("cat", rest) => self.cat(rest),
            ("echo", rest) => Reply::output(vec![Line::text(rest.join(" "), Role::Output)]),
            ("git", rest) => git(rest),
            ("cargo", rest) => cargo(rest),
            ("whoami", []) => Reply::output(vec![
                Line::text("fzf", Role::Command),
                Line::text("Siskin demo identity, not your login shell.", Role::Faint),
            ]),
            ("date", []) => Reply::output(vec![
                Line::text("Thu Nov  4 19:56:00 CST 2026", Role::Output),
                Line::text(
                    "Fixed timestamp so screenshots stay reproducible.",
                    Role::Faint,
                ),
            ]),
            ("uname", rest) => {
                let flags = rest.join(" ");
                let text = if flags.contains('a') {
                    "Linux siskin 5.15 simulated x86_64 GNU/Linux"
                } else {
                    "Linux"
                };
                Reply::output(vec![Line::text(text, Role::Output)])
            }
            ("siskin", rest) => about(rest),
            ("exit", []) => Reply::output(vec![Line::text(
                "The demo keeps this session open. Use the sidebar to switch sessions.",
                Role::Faint,
            )]),
            _ => Reply::output(vec![
                Line::new(spans![
                    format!("siskin: command not found: {name}") => Role::Error,
                ]),
                Line::new(spans![
                    "Try " => Role::Faint,
                    "help" => Role::Path,
                    " to see the four supported groups." => Role::Faint,
                ]),
            ]),
        }
    }

    /// Previous entry relative to the current draft. `None` when exhausted.
    pub fn history_back(&mut self) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        let next = match self.cursor {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(index) => index - 1,
        };
        self.cursor = Some(next);
        self.history.get(next).cloned()
    }

    pub fn history_forward(&mut self) -> Option<String> {
        let index = self.cursor?;
        if index + 1 >= self.history.len() {
            self.cursor = None;
            return Some(String::new());
        }
        self.cursor = Some(index + 1);
        self.history.get(index + 1).cloned()
    }

    /// Prefix completion for the first word, then for paths under the cwd.
    pub fn complete(&self, input: &str) -> Option<String> {
        let trimmed = input.trim_end();
        // Keep whatever the user typed before the word being completed, including
        // the separating space, so `cd ` stays `cd ` instead of collapsing.
        let (head, tail) = match trimmed.rsplit_once(char::is_whitespace) {
            Some((head, tail)) => (&input[..head.len() + 1], tail),
            None => ("", trimmed),
        };
        if tail.is_empty() {
            return None;
        }
        // A trailing slash never matches an entry name, so treat it as a path
        // prefix and search inside that directory instead.
        let (prefix, needle) = match tail.rsplit_once('/') {
            Some((dir, name)) => (&tail[..dir.len() + 1], name),
            None => ("", tail),
        };

        let candidates: Vec<String> = if head.is_empty() {
            if !prefix.is_empty() {
                return None;
            }
            COMMANDS
                .iter()
                .filter(|name| name.starts_with(needle))
                .map(|name| name.to_string())
                .collect()
        } else {
            let (resolved, file) = fs::walk(&self.cwd, prefix)?;
            if file.is_some() {
                return None;
            }
            let (dirs, files) = fs::entries(&resolved)?;
            dirs.into_iter()
                .filter(|name| name.starts_with(needle))
                .map(|name| format!("{prefix}{name}/"))
                .chain(
                    files
                        .into_iter()
                        .filter(|name| name.starts_with(needle))
                        .map(|name| format!("{prefix}{name}")),
                )
                .collect()
        };

        match candidates.as_slice() {
            [] => None,
            [only] => {
                let suffix = if only.ends_with('/') { "" } else { " " };
                Some(format!("{head}{only}{suffix}"))
            }
            many => {
                // Only extend up to the shared prefix; ambiguous matches must
                // not silently pick the first entry.
                let shared = shared_prefix(many);
                (shared.len() > tail.len()).then(|| format!("{head}{shared}"))
            }
        }
    }

    fn ls(&self, args: &[&str]) -> Reply {
        let target = args.iter().copied().find(|arg| !arg.starts_with('-'));
        let long = args
            .iter()
            .any(|arg| arg.starts_with('-') && arg.contains('l'));
        let path = target.unwrap_or(".");
        let Some((resolved, file)) = fs::walk(&self.cwd, path) else {
            return Reply::output(vec![Line::text(
                format!("ls: cannot access '{path}': no such file or directory"),
                Role::Error,
            )]);
        };
        if let Some(file) = file {
            return Reply::output(vec![Line::text(file.name, Role::Output)]);
        }
        let Some((dirs, files)) = fs::entries(&resolved) else {
            return Reply::output(vec![Line::text(
                format!("ls: cannot access '{path}'"),
                Role::Error,
            )]);
        };
        let mut lines = Vec::new();
        lines.push(Line::text(path.to_string(), Role::Faint));
        if long {
            for name in dirs {
                lines.push(Line::new(spans![
                    "drwxr-xr-x  " => Role::Faint,
                    format!("{name}/") => Role::Path,
                ]));
            }
            for name in files {
                lines.push(Line::new(spans![
                    "-rw-r--r--  " => Role::Faint,
                    name.to_string() => Role::Output,
                ]));
            }
        } else {
            let mut line = Line::default();
            let mut first = true;
            for (names, role) in [(dirs, Role::Path), (files, Role::Output)] {
                for name in names {
                    if !first {
                        line.push("   ", Role::Output);
                    }
                    first = false;
                    line.push(name, role);
                }
            }
            lines.push(line);
        }
        Reply::output(lines)
    }

    fn cd(&mut self, args: &[&str]) -> Reply {
        let target = args.first().copied().unwrap_or("~");
        let Some((resolved, file)) = fs::walk(&self.cwd, target) else {
            return Reply::output(vec![Line::text(
                format!("cd: no such file or directory: {target}"),
                Role::Error,
            )]);
        };
        if file.is_some() {
            return Reply::output(vec![Line::text(
                format!("cd: not a directory: {target}"),
                Role::Error,
            )]);
        }
        self.cwd = resolved;
        Reply::quiet()
    }

    fn cat(&self, args: &[&str]) -> Reply {
        let Some(target) = args.first() else {
            return Reply::output(vec![Line::text("usage: cat <file>", Role::Faint)]);
        };
        let Some((resolved, _)) = fs::walk(&self.cwd, target) else {
            return Reply::output(vec![Line::text(
                format!("cat: {target}: no such file or directory"),
                Role::Error,
            )]);
        };
        let Some(body) = fs::read(&resolved) else {
            return Reply::output(vec![Line::text(
                format!("cat: {target}: is a directory"),
                Role::Error,
            )]);
        };
        Reply::output(
            body.lines()
                .map(|line| Line::text(line, Role::Output))
                .collect(),
        )
    }
}

fn help() -> Vec<Line> {
    vec![
        Line::text("Siskin 模拟终端 · 所有输出都是本地固定数据", Role::Command),
        Line::text("", Role::Output),
        Line::new(spans![
            "文件  " => Role::Path,
            "ls [-l] [path]   cd [path]   pwd   cat <file>" => Role::Output,
        ]),
        Line::new(spans![
            "仓库  " => Role::Path,
            "git status        git log      cargo check   cargo test" => Role::Output,
        ]),
        Line::new(spans![
            "环境  " => Role::Path,
            "whoami  date  uname -a  echo <text>" => Role::Output,
        ]),
        Line::new(spans![
            "界面  " => Role::Path,
            "clear   siskin   help" => Role::Output,
        ]),
        Line::text("", Role::Output),
        Line::new(spans![
            "↑ ↓ 翻历史，Tab 补全命令，" => Role::Faint,
            "⌘/Ctrl + K" => Role::Faint,
            " 打开命令面板。" => Role::Faint,
        ]),
    ]
}

fn about(args: &[&str]) -> Reply {
    let topic = args.first().copied().unwrap_or("about");
    match topic {
        "about" => Reply::output(vec![
            Line::new(spans![
                "Siskin" => Role::Command,
                " 0.1.0" => Role::Faint,
            ]),
            Line::new(spans![
                "一个用 " => Role::Output,
                "pulse-ui" => Role::Path,
                " + GPUI 画的模拟终端。" => Role::Output,
            ]),
            Line::text("No PTY, no network, no disk writes.", Role::Faint),
        ]),
        "icon" => Reply::output(vec![
            Line::text("  >_*    matte sphere, warm underscore", Role::Output),
            Line::text("  assets/siskin.svg", Role::Path),
        ]),
        other => Reply::output(vec![Line::text(
            format!("siskin: unknown topic: {other}"),
            Role::Error,
        )]),
    }
}

fn git(args: &[&str]) -> Reply {
    match args.first().copied() {
        Some("status") => Reply::output(vec![
            Line::new(spans!["On branch " => Role::Output, "main" => Role::Ok]),
            Line::text("Your branch is up to date with 'origin/main'.", Role::Faint),
            Line::text("", Role::Output),
            Line::text("Changes not staged for commit:", Role::Output),
            Line::new(spans![
                "  modified:   " => Role::Output,
                "src/views/terminal.rs" => Role::Warn,
            ]),
            Line::text("", Role::Output),
            Line::new(spans![
                "no changes added to commit" => Role::Faint,
            ]),
        ]),
        Some("log") => Reply::output(vec![
            Line::new(spans![
                "e41c9aa " => Role::Faint,
                "feat: add simulated terminal shell" => Role::Output,
            ]),
            Line::new(spans![
                "8d2b10f " => Role::Faint,
                "feat: compose window chrome with pulse-ui" => Role::Output,
            ]),
            Line::new(spans![
                "1a77c04 " => Role::Faint,
                "chore: pin gpui together with pulse-ui" => Role::Output,
            ]),
        ]),
        Some(other) => Reply::output(vec![Line::text(
            format!("git: '{other}' is read-only in this demo"),
            Role::Faint,
        )]),
        None => Reply::output(vec![Line::text("usage: git status | git log", Role::Faint)]),
    }
}

fn cargo(args: &[&str]) -> Reply {
    match args.first().copied() {
        Some("check") => Reply::output(vec![
            Line::new(spans![
                "    Checking" => Role::Ok,
                " siskin v0.1.0 (/home/fzf/code/rust/siskin)" => Role::Output,
            ]),
            Line::new(spans![
                "    Finished" => Role::Ok,
                " `dev` profile in 0.42s" => Role::Faint,
            ]),
        ]),
        Some("test") => Reply::output(vec![
            Line::new(spans![
                "running 12 tests" => Role::Output,
            ]),
            Line::new(spans![
                "test result: " => Role::Output,
                "ok" => Role::Ok,
                ". 12 passed; 0 failed; 0 ignored" => Role::Output,
            ]),
        ]),
        Some("build") => Reply::output(vec![
            Line::new(spans![
                "   Compiling" => Role::Ok,
                " siskin v0.1.0" => Role::Output,
            ]),
            Line::new(spans![
                "    Finished" => Role::Ok,
                " `release` profile [optimized] target(s)" => Role::Faint,
            ]),
        ]),
        Some(other) => Reply::output(vec![Line::text(
            format!("cargo: `{other}` is not simulated here"),
            Role::Faint,
        )]),
        None => Reply::output(vec![Line::text(
            "usage: cargo check | cargo test | cargo build",
            Role::Faint,
        )]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(reply: &Reply) -> String {
        reply
            .lines
            .iter()
            .map(Line::plain)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn navigation_changes_the_prompt_and_rejects_files() {
        let mut shell = Shell::default();
        assert!(shell.run("cd src").lines.is_empty());
        assert_eq!(shell.cwd_label(), "~/src");
        let reply = shell.run("cd shell.rs");
        assert!(text(&reply).contains("not a directory"));
        assert_eq!(shell.cwd_label(), "~/src");
        shell.run("cd ..");
        assert_eq!(shell.cwd_label(), "~");
    }

    #[test]
    fn cat_and_ls_stay_inside_the_simulated_tree() {
        let mut shell = Shell::default();
        let reply = shell.run("cat src/shell.rs");
        assert!(text(&reply).contains("pub fn run"));
        let missing = shell.run("cat /etc/passwd");
        assert!(text(&missing).contains("no such file"));
    }

    #[test]
    fn clear_is_an_effect_and_does_not_print() {
        let mut shell = Shell::default();
        let reply = shell.run("clear");
        assert!(reply.clear);
        assert!(reply.lines.is_empty());
    }

    #[test]
    fn unknown_commands_suggest_help() {
        let mut shell = Shell::default();
        let reply = shell.run("ruby");
        assert!(text(&reply).contains("command not found"));
        assert!(text(&reply).contains("help"));
    }

    #[test]
    fn history_walks_backwards_then_returns_to_the_draft() {
        let mut shell = Shell::default();
        shell.run("help");
        shell.run("ls");
        assert_eq!(shell.history_back().as_deref(), Some("ls"));
        assert_eq!(shell.history_back().as_deref(), Some("help"));
        assert_eq!(shell.history_back().as_deref(), Some("help"));
        assert_eq!(shell.history_forward().as_deref(), Some("ls"));
        assert_eq!(shell.history_forward().as_deref(), Some(""));
        assert!(shell.history_forward().is_none());
    }

    #[test]
    fn commands_are_trimmed_and_duplicates_are_not_stored_twice() {
        let mut shell = Shell::default();
        shell.run("  help  ");
        shell.run("help");
        assert_eq!(shell.history(), &["help".to_string()]);
        let reply = shell.run("   ");
        assert!(reply.lines.is_empty() && !reply.clear);
    }

    #[test]
    fn completion_extends_only_unambiguous_prefixes() {
        let shell = Shell::default();
        assert_eq!(shell.complete("wh").as_deref(), Some("whoami "));
        assert_eq!(shell.complete("c"), None);
        assert_eq!(shell.complete("cd s").as_deref(), Some("cd src/"));
        assert_eq!(shell.complete("ls src").as_deref(), Some("ls src/"));
        assert_eq!(
            shell.complete("cat src/sh").as_deref(),
            Some("cat src/shell.rs ")
        );
        assert_eq!(shell.complete("cat nope"), None);
    }
}
