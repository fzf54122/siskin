# -*- coding: utf-8 -*-
# @Time    : 2026/11/5 13:50
# @Author  : fzf
# @FileName: check_structure.py
# @Software: Siskin

"""代码结构门禁。

检查四件事：

1. 单个 `.rs` 文件行数（默认 400）；
2. 单个函数行数（默认 60）；
3. 跨层 import：`domain/` 不许 use `gpui` / `zbus` / `ui` / `app` / `io`，
   `io/` 不许 use `gpui` / `ui` / `app`，`ui/` 不许 use `std::process`；
4. `#[cfg(test)]` 之外的 `unwrap()` / `expect()` / `panic!` / `unreachable!` /
   `todo!`，白名单按「文件:行号」逐条登记。

用法：

    python3 scripts/check_structure.py --max-lines 400 --max-fn-lines 60

缩进深度默认只告警不拦截：P1 会整体重写视图树，现在拦住会让 CI 一直红。
需要严格模式时加 `--strict-indent`。

退出码：发现违规为 1，否则为 0。
"""

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE_ROOT = REPO_ROOT / "src"

# 每个层允许出现的 crate 前缀。
LAYER_RULES = {
    "domain": {
        "forbidden": ("gpui", "pulse_ui", "zbus", "zvariant", "crate::app",
                      "crate::ui", "crate::io"),
        "message": "domain 是纯数据层，不能依赖 GUI、D-Bus 或上层模块",
    },
    "io": {
        "forbidden": ("gpui", "pulse_ui", "crate::app", "crate::ui"),
        "message": "io 是系统边界，不能依赖 GUI 或上层模块",
    },
    "ui": {
        "forbidden": ("crate::app::",),
        "message": "ui 只做渲染，动作必须通过 Action 交给 app",
    },
}

UI_EXTRA_FORBIDDEN = ("std::process",)

# plan 6.3：`render` / 顶层视图函数单独放宽到 80 行，其余函数 60 行。
# 「顶层」指定义在 `src/ui/` 下、缩进不超过一个 impl 块（4 空格）的函数。
VIEW_FN_LIMIT = 80

# 生产路径允许的兜底调用。每一项都必须能说明「为什么这里不可能失败」，
# 因此按文件:行号登记，而不是按文件整体放行。
UNWRAP_ALLOWLIST = {
    # 示例：("src/main.rs", 42),
}

# 这些文件整体是测试，`unwrap` 与行数上限都不适用。
TEST_FILE_NAMES = {"tests.rs"}

# plan 第 6.2 章明确保留、P2 直接删除的文件，P0 不动它们。
LEGACY_EXEMPT = {
    "src/shell.rs": "plan 6.2：P2 直接删除，删除前不再加功能",
    "src/fs.rs": "plan 6.2：P2 直接删除，删除前不再加功能",
}

UNWRAP_PATTERN = re.compile(
    r"\.(unwrap|expect)\s*\(|(?<![A-Za-z_])(panic!|unreachable!|todo!)"
)
IMPORT_PATTERN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([^;]+);")
CFG_TEST_PATTERN = re.compile(r"#\[cfg\(test\)\]")
FN_PATTERN = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)"
)


@dataclass
class Violation:
    path: Path
    line: int
    message: str

    def render(self) -> str:
        try:
            relative = self.path.relative_to(REPO_ROOT)
        except ValueError:
            relative = self.path
        return f"{relative}:{self.line}: {self.message}"


@dataclass
class SourceFile:
    path: Path
    lines: List[str]

    @property
    def layer(self) -> Optional[str]:
        relative = self.path.relative_to(SOURCE_ROOT)
        return relative.parts[0] if len(relative.parts) > 1 else None

    @property
    def relative(self) -> str:
        return self.path.relative_to(REPO_ROOT).as_posix()

    @property
    def is_test_file(self) -> bool:
        # `src/tests/` 整个目录与各层里的 `tests.rs` 都只放测试。
        return self.path.parent == SOURCE_ROOT / "tests" or self.path.name in TEST_FILE_NAMES

    @property
    def is_legacy_exempt(self) -> bool:
        return self.relative in LEGACY_EXEMPT

    def test_line_ranges(self) -> List[Tuple[int, int]]:
        """返回所有 `#[cfg(test)]` 模块覆盖的行号区间（1 基，闭区间）。"""
        ranges: List[Tuple[int, int]] = []
        for index, line in enumerate(self.lines):
            if not CFG_TEST_PATTERN.search(line):
                continue
            start = index
            end = self.block_end(index)
            ranges.append((start + 1, end + 1))
        return ranges

    def block_end(self, start: int) -> int:
        """从 `start` 行起找与首个 `{` 配对的 `}`，找不到就认为到文件末尾。"""
        depth = 0
        opened = False
        for index in range(start, len(self.lines)):
            text = strip_line_comment(self.lines[index])
            for char in text:
                if char == "{":
                    depth += 1
                    opened = True
                elif char == "}":
                    depth -= 1
                    if opened and depth == 0:
                        return index
        return len(self.lines) - 1


def strip_line_comment(line: str) -> str:
    """去掉行注释，避免注释里的 `{` / `}` 干扰括号配平。"""
    in_string = False
    escaped = False
    for index, char in enumerate(line):
        if escaped:
            escaped = False
            continue
        if char == "\\":
            escaped = True
            continue
        if char == '"':
            in_string = not in_string
            continue
        if not in_string and line.startswith("//", index):
            return line[:index]
    return line


def line_is_in(ranges: List[Tuple[int, int]], number: int) -> bool:
    return any(start <= number <= end for start, end in ranges)


def collect_files() -> List[SourceFile]:
    return [
        SourceFile(path, path.read_text(encoding="utf-8").splitlines())
        for path in sorted(SOURCE_ROOT.rglob("*.rs"))
    ]


def check_file_length(source: SourceFile, max_lines: int) -> List[Violation]:
    if len(source.lines) <= max_lines:
        return []
    return [
        Violation(
            source.path,
            1,
            f"文件 {len(source.lines)} 行，超过上限 {max_lines}，请按结构拆分",
        )
    ]


def function_limit(source: SourceFile, line: str, max_lines: int) -> int:
    """视图层顶层函数按 plan 6.3 放宽到 80 行。"""
    is_view = source.layer == "ui" and indentation_depth(line) <= 1
    return max(max_lines, VIEW_FN_LIMIT) if is_view else max_lines


def check_function_length(source: SourceFile, max_lines: int) -> List[Violation]:
    violations: List[Violation] = []
    for index, line in enumerate(source.lines):
        match = FN_PATTERN.match(line)
        if not match:
            continue
        end = source.block_end(index)
        length = end - index + 1
        limit = function_limit(source, line, max_lines)
        if length > limit:
            violations.append(
                Violation(
                    source.path,
                    index + 1,
                    f"函数 {match.group(1)} 有 {length} 行，超过上限 {limit}",
                )
            )
    return violations


def check_layers(source: SourceFile) -> List[Violation]:
    rule = LAYER_RULES.get(source.layer or "")
    if rule is None:
        return []
    forbidden = list(rule["forbidden"])
    if source.layer == "ui":
        forbidden.extend(UI_EXTRA_FORBIDDEN)
    violations: List[Violation] = []
    for index, line in enumerate(source.lines):
        match = IMPORT_PATTERN.match(line)
        if not match:
            continue
        target = match.group(1).strip()
        for name in forbidden:
            if target == name or target.startswith(f"{name}::"):
                violations.append(
                    Violation(source.path, index + 1, f"{rule['message']}：{target}")
                )
                break
    return violations


def check_unwraps(source: SourceFile) -> List[Violation]:
    if source.is_test_file:
        return []
    tests = source.test_line_ranges()
    violations: List[Violation] = []
    for index, line in enumerate(source.lines):
        number = index + 1
        if line_is_in(tests, number):
            continue
        stripped = strip_line_comment(line)
        match = UNWRAP_PATTERN.search(stripped)
        if not match:
            continue
        key = (source.relative, number)
        if key in UNWRAP_ALLOWLIST:
            continue
        violations.append(
            Violation(
                source.path,
                number,
                f"生产路径出现 {match.group(1)}，请改用 Result / 早返回；"
                "确属不可达请在 UNWRAP_ALLOWLIST 登记原因",
            )
        )
    return violations


def indentation_depth(line: str) -> int:
    width = len(line) - len(line.lstrip(" "))
    return width // 4


def check_indentation(source: SourceFile, max_depth: int) -> List[Violation]:
    """只统计「开启新块」的行。

    builder 链的续行缩进很深但只是排版，不算嵌套；真正要压平的是
    `if` / `match` / `for` / 闭包这类会开启新作用域的行。
    """
    violations: List[Violation] = []
    for index, line in enumerate(source.lines):
        stripped = strip_line_comment(line).rstrip()
        if not stripped.endswith("{"):
            continue
        depth = indentation_depth(line)
        if depth > max_depth:
            violations.append(
                Violation(
                    source.path,
                    index + 1,
                    f"缩进 {depth} 层，超过上限 {max_depth}",
                )
            )
    return violations


def report(title: str, violations: List[Violation]) -> None:
    print(f"{title}: {len(violations)} 项")
    for violation in violations:
        print(f"  {violation.render()}")


def filter_legacy(sources: List[SourceFile]) -> List[SourceFile]:
    kept: List[SourceFile] = []
    for source in sources:
        if source.is_legacy_exempt:
            print(f"跳过 {source.relative}：{LEGACY_EXEMPT[source.relative]}")
            continue
        kept.append(source)
    return kept


def main() -> int:
    parser = argparse.ArgumentParser(description="Siskin 代码结构门禁")
    parser.add_argument("--max-lines", type=int, default=400)
    parser.add_argument("--max-fn-lines", type=int, default=60)
    parser.add_argument("--max-indent", type=int, default=4)
    parser.add_argument(
        "--strict-indent",
        action="store_true",
        help="把缩进深度也作为失败条件（默认只告警）",
    )
    arguments = parser.parse_args()

    if not SOURCE_ROOT.is_dir():
        print(f"找不到源码目录 {SOURCE_ROOT}", file=sys.stderr)
        return 1

    sources = filter_legacy(collect_files())
    file_length = [v for s in sources for v in check_file_length(s, arguments.max_lines)]
    function_length = [
        v for s in sources for v in check_function_length(s, arguments.max_fn_lines)
    ]
    layers = [v for s in sources for v in check_layers(s)]
    unwraps = [v for s in sources for v in check_unwraps(s)]
    indentation = [
        v for s in sources for v in check_indentation(s, arguments.max_indent)
    ]

    report("文件行数", file_length)
    report("函数行数", function_length)
    report("跨层 import", layers)
    report("生产路径兜底调用", unwraps)
    if arguments.strict_indent:
        report("缩进深度", indentation)
    else:
        print(f"缩进深度（仅告警）: {len(indentation)} 项")
        for violation in indentation[:10]:
            print(f"  {violation.render()}")
        if len(indentation) > 10:
            print(f"  …另有 {len(indentation) - 10} 项，加 --strict-indent 查看全部")

    failures = len(file_length) + len(function_length) + len(layers) + len(unwraps)
    if arguments.strict_indent:
        failures += len(indentation)
    if failures:
        print(f"\n共 {failures} 项结构违规", file=sys.stderr)
        return 1
    print("\n结构检查通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
