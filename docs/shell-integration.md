# Shell Integration（OSC 133）

Siskin 解析 shell 发出的 OSC 133 标记，得到命令边界与退出状态：
`A` 提示符开始、`B` 命令输入开始、`C` 命令执行开始、`D` 命令结束
（`;` 后第一个参数是退出码，如 `133;D;1`）。

终端侧的行为：

- 每会话保留最近 100 条命令（超了丢最老的），含起始行、退出码与耗时。
- 命令间跳转：`Ctrl+Shift+Up` 上一个命令、`Ctrl+Shift+Down` 下一个命令，
  目标命令滚到可见区顶部；命令面板里也可找到这两项。
- 失败命令（退出码存在且非 0）只在命令记录里标失败，暂不做新面板。
- 退出码只认 `D` 标记的参数，绝不从普通文本里猜。
- shell 没发标记时终端完全正常工作，命令记录为空是合法状态。
- 标记字节照常进 VT 解析器，不剥离、不改写任何输出。

行号是单调稳定的行计数（从会话第一行起算）：历史行号会随新输出
整体漂移，存下来不可靠，所以读线程记的是稳定行，跳转时再换算。

## Bash（已验证：bash 5.2.21，2026-11-11）

加进 `~/.bashrc`：

```bash
__siskin_preexec() { printf '\033]133;C\033\\'; }
trap '__siskin_preexec' DEBUG
__siskin_prompt_command() {
  local code=$?
  printf '\033]133;D;%s\033\\' "$code"
  printf '\033]133;A\033\\'
}
PROMPT_COMMAND=__siskin_prompt_command
PS1='\u@\h:\w\$ \[\e]133;B\e\\\]'
```

已有 `PROMPT_COMMAND` 时不要直接覆盖，改成追加：

```bash
PROMPT_COMMAND="__siskin_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

已知局限：Bash 没有 preexec 钩子，`C` 靠 `DEBUG` trap 实现；
trap 会在 `PROMPT_COMMAND` 内部命令执行前再触发一次 `C`，
所以每条命令会收到两次 `C`，命令耗时（`C` 到 `D` 的时长）不准。
`A` / `B` / `D` 与退出码不受影响，实测 `false` 正确报 `D;1`。

## Zsh（已验证：zsh 5.9，2026-11-11）

加进 `~/.zshrc`：

```zsh
__siskin_precmd() {
  printf '\033]133;D;%s\033\\' "$?"
  printf '\033]133;A\033\\'
}
__siskin_preexec() { printf '\033]133;C\033\\'; }
autoload -Uz add-zsh-hook
add-zsh-hook precmd __siskin_precmd
add-zsh-hook preexec __siskin_preexec
PS1=$'%n@%m %~ %# \x1b]133;B\x1b\\'
```

Zsh 有原生的 `precmd` / `preexec`，每条命令恰好收到一次 `C`，
实测 `false` 正确报 `D;1`，耗时也准确。

## Fish（未验证：本机未装 fish）

以下片段没有在本机跑过，不要当成已验证的配置照抄：

```fish
function __siskin_prompt --on-event fish_prompt
    printf '\033]133;D;%s\033\\' $status
    printf '\033]133;A\033\\'
end
function __siskin_preexec --on-event fish_preexec
    printf '\033]133;C\033\\'
end
functions -c fish_prompt __siskin_orig_fish_prompt
function fish_prompt
    __siskin_orig_fish_prompt
    printf '\033]133;B\033\\'
end
```

`D` / `A` 挂在 `fish_prompt` 事件上（提示符绘制前跑），
`C` 挂在 `fish_preexec` 事件上，`B` 跟在原提示符输出之后。

## 验证方式

`tests/pty.rs` 里有真实 PTY 测试：交互式 bash / zsh 按上面片段
配置后跑 `false`，断言收到 `D;1` 与 `C → D;1 → A → B` 子序列；
另有 `sh -c` 直接 `printf` 标记的测试，断言行号锚定与跳转。

## 行内自动补全（ghost text）

输入时，Siskin 按本会话已执行命令联想：与当前输入前缀匹配的最近一条
命令，剩余后缀以弱化色画在光标之后。只画出来——接受前不向 shell 发
任何字节，不改光标位置与终端内容。

- `Tab`：接受整条建议（以后缀字节写进 PTY，与自己敲出这些字等价）。
- `→`：接受下一个词（前导空白 + 一个词，可连续按）。
- `Esc`：取消当前建议；输入变化后恢复联想。
- 换行、新输出、失去焦点、滚进历史、行中光标、IME 预编辑时不画。
- 没有匹配、输入为空、命令完全相等时不画。

联想只来自本会话输入记录 + OSC 133 命令记录：`C` 标记到达时，
把待定命令的输入行从网格读进建议来源（最近 200 条，去重）。
不读 `~/.bash_history`、不联网、不调外部 AI。

前提是 shell 发 `A`/`B`/`C`：`B` 定输入起点（提示符末尾），
`C` 触发记录。没有 `B` 时不猜提示符位置、不联想；没有 `C` 的
shell 不进建议来源。上面 Bash / Zsh / Fish 的片段都发这三个标记。

已知局限：

- 只支持单行输入：命令折行到下一行时不联想。
- 只记第一行：多行命令（`\` 续行）只记第一行。
- 只读当前会话：不读其它会话的历史，也不读 shell 历史文件。
- 云端 VM 无显示器，桌面弱化色的实际观感待大王在 Linux 桌面上确认。
