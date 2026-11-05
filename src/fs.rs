// @Time    : 2026/11/4 19:54
// @Author  : fzf
// @FileName: fs.rs
// @Software: Siskin

//! A tiny read-only file tree for the simulated shell. Nothing here touches the
//! real disk: `ls`, `cat` and `cd` must behave the same on every machine.

pub struct File {
    pub name: &'static str,
    pub body: &'static str,
}

pub struct Dir {
    pub name: &'static str,
    pub dirs: &'static [Dir],
    pub files: &'static [File],
}

pub const ROOT: Dir = Dir {
    name: "~",
    dirs: &[SRC, ASSETS],
    files: &[
        File {
            name: "README.md",
            body: "# Siskin\n\nA simulated terminal built with pulse-ui.\n\
                   The shell is layered: `line` styles text, `terminal` owns\n\
                   scrollback, `shell` answers commands, and `views` renders.\n",
        },
        File {
            name: "Cargo.toml",
            body: "[package]\nname = \"siskin\"\nversion = \"0.1.0\"\n\
                   edition = \"2024\"\n\n[dependencies]\npulse-ui = { git = \"https://github.com/fzf54122/resource-pulse.git\", tag = \"v0.1.1\" }\n",
        },
        File {
            name: "Cargo.lock",
            body: "Locked to one GPUI commit so the app and pulse-ui never\nlink two incompatible copies of the framework.\n",
        },
    ],
};

pub const SRC: Dir = Dir {
    name: "src",
    dirs: &[],
    files: &[
        File {
            name: "main.rs",
            body: "fn main() {\n    application().run(|cx| {\n        cx.open_window(options, |_, cx| cx.new(views::root))\n    });\n}\n",
        },
        File {
            name: "shell.rs",
            body: "pub fn run(&mut self, command: &str) -> Outcome {\n    match command {\n        \"help\" => Outcome::output(help()),\n        _ => Outcome::output(unknown(command)),\n    }\n}\n",
        },
        File {
            name: "terminal.rs",
            body: "pub struct Terminal {\n    lines: Vec<Line>,\n    input: String,\n    columns: usize,\n}\n",
        },
        File {
            name: "line.rs",
            body: "pub struct Span {\n    pub text: String,\n    pub role: Role,\n}\n",
        },
        File {
            name: "theme.rs",
            body: "pub const ACCENT: u32 = 0x3c6e53;\npub const WARM: u32 = 0xb9822f;\n",
        },
    ],
};

pub const ASSETS: Dir = Dir {
    name: "assets",
    dirs: &[],
    files: &[
        File {
            name: "siskin.svg",
            body: "Matte charcoal sphere, a `>` prompt, a bird-foot star and a\nwarm underscore. One shape family, one accent color.\n",
        },
        File {
            name: "siskin-256.png",
            body: "Raster fallback for the window icon.\n",
        },
    ],
};

/// Resolves a path relative to the current directory. Returns `None` when a
/// segment is neither a directory nor a file. Names are looked up in the static
/// tree, so callers can keep them past the input string.
pub fn walk(
    cwd: &[&'static str],
    path: &str,
) -> Option<(Vec<&'static str>, Option<&'static File>)> {
    let mut parts: Vec<&'static str> = if path.starts_with('~') {
        Vec::new()
    } else {
        cwd.to_vec()
    };
    let mut dir = &ROOT;
    for name in &parts {
        dir = dir.dirs.iter().find(|child| child.name == *name)?;
    }

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for (index, segment) in segments.iter().enumerate() {
        let last = index + 1 == segments.len();
        match *segment {
            "." | "~" => continue,
            ".." => {
                parts.pop();
                dir = &ROOT;
                for name in &parts {
                    dir = dir.dirs.iter().find(|child| child.name == *name)?;
                }
            }
            name => {
                if let Some(child) = dir.dirs.iter().find(|child| child.name == name) {
                    parts.push(child.name);
                    dir = child;
                } else if last {
                    let file = dir.files.iter().find(|file| file.name == name)?;
                    parts.push(file.name);
                    return Some((parts, Some(file)));
                } else {
                    return None;
                }
            }
        }
    }
    Some((parts, None))
}

/// Directory entries at the given path, names sorted for stable output.
pub fn entries(path: &[&str]) -> Option<(Vec<&'static str>, Vec<&'static str>)> {
    let mut dir = &ROOT;
    for name in path {
        dir = dir.dirs.iter().find(|child| child.name == *name)?;
    }
    let mut dirs: Vec<_> = dir.dirs.iter().map(|child| child.name).collect();
    let mut files: Vec<_> = dir.files.iter().map(|file| file.name).collect();
    dirs.sort_unstable();
    files.sort_unstable();
    Some((dirs, files))
}

pub fn read(path: &[&str]) -> Option<&'static str> {
    if path.is_empty() {
        return None;
    }
    let (parents, file_name) = path.split_at(path.len() - 1);
    let mut dir = &ROOT;
    for name in parents {
        dir = dir.dirs.iter().find(|child| child.name == *name)?;
    }
    dir.files
        .iter()
        .find(|file| file.name == file_name[0])
        .map(|file| file.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_nested_directories_and_files() {
        let (path, file) = walk(&[], "src/shell.rs").expect("known file");
        assert_eq!(path, vec!["src", "shell.rs"]);
        assert_eq!(file.map(|file| file.name), Some("shell.rs"));

        let (path, file) = walk(&["src"], "../assets").expect("parent traversal");
        assert_eq!(path, vec!["assets"]);
        assert!(file.is_none());
    }

    #[test]
    fn unknown_paths_are_rejected_instead_of_invented() {
        assert!(walk(&[], "src/nope.rs").is_none());
        assert!(walk(&[], "docs/readme.md").is_none());
    }

    #[test]
    fn directory_listing_is_sorted_and_complete() {
        let (dirs, files) = entries(&[]).expect("root");
        assert_eq!(dirs, vec!["assets", "src"]);
        assert_eq!(files, vec!["Cargo.lock", "Cargo.toml", "README.md"]);
    }

    #[test]
    fn files_are_readable_from_any_directory() {
        assert!(read(&["src", "theme.rs"]).unwrap().contains("ACCENT"));
        assert_eq!(read(&["src"]), None);
    }
}
