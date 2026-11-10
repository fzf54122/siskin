// @Time    : 2026/11/5 15:10
// @Author  : fzf
// @FileName: shell_spec.rs
// @Software: Siskin

//! Shell 选择。`ShellKind` 是设置页里的预设，`ShellSpec` 是要执行的命令。
//! 两者分开，界面只处理预设，PTY 派生只看 `ShellSpec`。

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ShellKind {
    Zsh,
    Bash,
    Fish,
    Sh,
}

impl ShellKind {
    pub(crate) const ALL: [ShellKind; 4] = [
        ShellKind::Zsh,
        ShellKind::Bash,
        ShellKind::Fish,
        ShellKind::Sh,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            ShellKind::Zsh => "zsh",
            ShellKind::Bash => "bash",
            ShellKind::Fish => "fish",
            ShellKind::Sh => "sh",
        }
    }

    /// 预设对应的可执行文件。找不到时退回 `$SHELL`，不在这里报错：
    /// 真正要启动时才知道路径是否有效，错误留给 PTY 派生统一处理。
    pub(crate) fn spec(self) -> ShellSpec {
        match self.locate() {
            Some(program) => ShellSpec::Program(program),
            None => ShellSpec::Default,
        }
    }

    fn locate(self) -> Option<PathBuf> {
        let name = self.name();
        ["/usr/bin", "/bin", "/usr/local/bin"]
            .into_iter()
            .map(|dir| PathBuf::from(dir).join(name))
            .find(|path| is_executable(path))
    }
}

/// 新会话要启动什么。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ShellSpec {
    /// `$SHELL`，未设置时 `/bin/sh`。
    Default,
    Program(PathBuf),
    WithArgs {
        program: PathBuf,
        args: Vec<String>,
    },
}

/// 交给 PTY 的最终命令。路径用 `String` 是因为底层 tty 接口要求 UTF-8；
/// 非 UTF-8 路径会在这里被拒绝，而不是在 fork 之后才发现。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedShell {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ShellError {
    MissingProgram(String),
    NonUtf8Path(PathBuf),
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellError::MissingProgram(program) => {
                write!(formatter, "找不到可执行文件：{program}")
            }
            ShellError::NonUtf8Path(path) => {
                write!(formatter, "路径不是有效的 UTF-8：{}", path.display())
            }
        }
    }
}

impl std::error::Error for ShellError {}

/// 交给 PTY 的终端环境。子 shell 默认裸继承父进程，于是「从桌面启动」
/// 和「从某个终端里启动」会拿到不同的 `TERM`，提示符与行编辑行为随之不同。
/// 这里显式给出终端应有的变量，让两种启动方式表现一致。
pub(crate) fn terminal_env() -> Vec<(String, String)> {
    vec![
        // `dumb` 会让 zsh 关掉部分行编辑、p10k 退化；256 色是这个网格的真实能力。
        ("TERM".to_string(), "xterm-256color".to_string()),
        // 支持 24 位真彩色。少了它，部分程序会退回 256 色索引。
        ("COLORTERM".to_string(), "truecolor".to_string()),
        // 让 shell 集成脚本能识别宿主，Ghostty / iTerm 都用这一招。
        ("TERM_PROGRAM".to_string(), "siskin".to_string()),
        // 程序直接用 terminfo 而不是自己探测 tty。
        (
            "TERM_PROGRAM_VERSION".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ]
}

impl ShellSpec {
    pub(crate) fn resolve(self) -> Result<ResolvedShell, ShellError> {
        match self {
            ShellSpec::Default => {
                let program = default_shell();
                Ok(ResolvedShell {
                    program: program.to_string_lossy().into_owned(),
                    args: Vec::new(),
                })
            }
            ShellSpec::Program(program) => Ok(ResolvedShell {
                program: resolve_program(&program)?,
                args: Vec::new(),
            }),
            ShellSpec::WithArgs { program, args } => Ok(ResolvedShell {
                program: resolve_program(&program)?,
                args,
            }),
        }
    }
}

/// 可执行文件判定。用 `exists` 而不是 `is_file`：`/bin/sh` 这类符号链接
/// 在多数发行版上指向 `dash`，`is_file` 会跟随链接失败并误判为「找不到」。
fn is_executable(program: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(program)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// 把用户给的程序名解析成可执行路径。
///
/// 两种情况分别处理：
/// - 已含路径（`/bin/sh`、`./run`、`../app/bin/x`）：直接判定存在。
/// - 裸名字（`sh`、`vim`、`htop`）：按 `PATH` 逐个目录找，找不到才算缺失。
///
/// 不这么分的话，`siskin -e vim` 会在启动目录里找一个不存在的 `vim`，
/// 报「找不到可执行文件」，尽管 `vim` 明明在 PATH 里。
fn resolve_program(program: &std::path::Path) -> Result<String, ShellError> {
    if program.as_os_str().as_encoded_bytes().contains(&b'/') {
        return program_to_string(program);
    }
    let missing = ShellError::MissingProgram(program.display().to_string());
    let Some(paths) = std::env::var_os("PATH") else {
        return Err(missing);
    };
    for directory in std::env::split_paths(&paths) {
        let candidate = if directory.as_os_str().is_empty() {
            program.to_path_buf()
        } else {
            directory.join(program)
        };
        if is_executable(&candidate) {
            return program_to_string(&candidate);
        }
    }
    Err(missing)
}

fn program_to_string(program: &std::path::Path) -> Result<String, ShellError> {
    if !is_executable(program) {
        return Err(ShellError::MissingProgram(program.display().to_string()));
    }
    program
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| ShellError::NonUtf8Path(program.to_path_buf()))
}

/// 用户配置的登录 shell，未设置时退回 POSIX shell。
pub(crate) fn default_shell() -> PathBuf {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| is_executable(path))
        .unwrap_or_else(|| PathBuf::from("/bin/sh"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_spec_resolves_to_an_existing_program() {
        let resolved = ShellSpec::Default.resolve().expect("默认 shell 必须可解析");
        assert!(is_executable(Path::new(&resolved.program)));
    }

    /// `/bin/sh` 是符号链接。这条守住「跟随链接而非只看普通文件」的判定，
    /// 否则每个发行版的 POSIX shell 都被误判为「找不到可执行文件」。
    #[test]
    fn a_symlinked_program_is_accepted() {
        let symlinked = ShellSpec::Program(PathBuf::from("/bin/sh"));
        assert!(symlinked.resolve().is_ok(), "/bin/sh 必须能解析");
    }

    /// 预设定位同样要跟随符号链接，否则 `sh` 预设永远选不中。
    #[test]
    fn a_preset_locates_symlinked_shells() {
        let located = ShellKind::Sh.spec();
        match located {
            ShellSpec::Program(path) => assert!(is_executable(&path), "{path:?} 必须可执行"),
            other => panic!("sh 预设应定位到具体路径，得到 {other:?}"),
        }
    }

    #[test]
    fn a_missing_program_is_reported_before_spawning() {
        let spec = ShellSpec::Program(PathBuf::from("/nonexistent/siskin-shell"));
        assert!(matches!(spec.resolve(), Err(ShellError::MissingProgram(_))));
    }

    /// `-e vim` 给的是裸名字。解析必须走 `PATH`，而不是在启动目录里找一个
    /// 同名的空文件，否则每个 PATH 里的程序都启动不了。
    #[test]
    fn a_bare_program_name_is_resolved_through_path() {
        let spec = ShellSpec::WithArgs {
            program: PathBuf::from("sh"),
            args: vec!["-c".into(), "printf hi".into()],
        };
        let resolved = spec.resolve().expect("sh 应在 PATH 里");
        assert!(
            std::path::Path::new(&resolved.program).is_absolute(),
            "裸名字应解析成绝对路径，得到 {}",
            resolved.program
        );
        assert_eq!(resolved.args, ["-c", "printf hi"]);
    }

    #[test]
    fn a_bare_name_absent_from_path_is_still_reported_missing() {
        let spec = ShellSpec::Program(PathBuf::from("siskin-definitely-not-installed"));
        assert!(matches!(spec.resolve(), Err(ShellError::MissingProgram(_))));
    }

    #[test]
    fn arguments_survive_resolution() {
        let spec = ShellSpec::WithArgs {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), "printf hi".into()],
        };
        let resolved = spec.resolve().expect("/bin/sh 必须存在");
        assert_eq!(resolved.args, ["-c", "printf hi"]);
    }
}
