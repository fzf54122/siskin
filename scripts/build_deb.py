# -*- coding: utf-8 -*-
# @Time    : 2026/11/5 19:42
# @Author  : fzf
# @FileName: build_deb.py
# @Software: Siskin

"""构建本地 Debian 包。只打包，不安装、不改桌面偏好。

图标直接用 `assets/icons/` 里已经导好的位图组，工程里不再维护 SVG 母版。
打包只做拷贝，不再二次缩放，避免引入额外的模糊。

用法：

    python3 scripts/build_deb.py                # 先 cargo build --release
    python3 scripts/build_deb.py --skip-build   # 打包已有二进制
    python3 scripts/build_deb.py --output /tmp  # 指定输出目录
"""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
APP_ID = "io.github.fzf.Siskin"
PACKAGE = "siskin"
BINARY_NAME = "siskin"

# `assets/icons/siskin-<n>.png` 里已有的尺寸。64 以下来自设计稿原图，
# 其余由 1024 母图等比缩小；打包时按同名直接安装。
ICON_SIZES = [16, 22, 24, 32, 48, 64, 96, 128, 180, 256, 512]
# 高分屏目录。hicolor 约定目录名就是 `NxN@2`，内容必须是 2N 像素。
SCALED_SIZES = [16, 22, 24, 32, 48]

# shlibdeps 会算出基础库依赖，这些是运行期还要额外保证的包。
RUNTIME_DEPENDENCIES = [
    "fonts-noto-cjk",
    "hicolor-icon-theme",
    "gtk-update-icon-cache",
    "librsvg2-common",
    "libvulkan1",
    "libfontconfig1",
    "libfreetype6",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libwayland-client0",
    "libwayland-cursor0",
    "libx11-xcb1",
    "libxcb-shape0",
    "libxcb-xfixes0",
]

REQUIRED_TOOLS = [
    "cargo",
    "dpkg",
    "dpkg-deb",
    "dpkg-shlibdeps",
    "desktop-file-validate",
    "ffmpeg",
]


def run(*args, cwd=ROOT):
    return subprocess.run(
        args, cwd=cwd, check=True, text=True, capture_output=True
    ).stdout.strip()


def copy(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(0o644)


def source_icon(size: int) -> Path:
    return ROOT / f"assets/icons/siskin-{size}.png"


def install_icons(stage: Path) -> None:
    theme = stage / "usr/share/icons/hicolor"

    for size in ICON_SIZES:
        source = source_icon(size)
        if not source.is_file():
            raise RuntimeError(f"缺少图标 {source.relative_to(ROOT)}")
        copy(source, theme / f"{size}x{size}/apps/{APP_ID}.png")
        if size in SCALED_SIZES:
            # 2x 目录仍然填同一张图，只是按目录名要求放大到 2N 像素。
            doubled = theme / f"{size}x{size}@2/apps/{APP_ID}.png"
            doubled.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(
                [
                    "ffmpeg", "-v", "error", "-y",
                    "-i", str(source),
                    "-vf", f"scale={size * 2}:{size * 2}:flags=lanczos",
                    str(doubled),
                ],
                check=True,
            )
            doubled.chmod(0o644)


def install_hooks(debian: Path) -> None:
    for name in ("postinst", "postrm"):
        script = debian / name
        copy(ROOT / "packaging" / name, script)
        script.chmod(0o755)


def dependencies(shlibs: str) -> str:
    lines = [line for line in shlibs.splitlines() if line.startswith("shlibs:Depends=")]
    if len(lines) != 1:
        raise ValueError("dpkg-shlibdeps 没有给出依赖列表")
    required = lines[0].split("=", 1)[1].split(", ")
    names = {value.split(" ", 1)[0] for value in required}
    required.extend(value for value in RUNTIME_DEPENDENCIES if value not in names)
    return ", ".join(required)


def cargo_version() -> str:
    # 不加 --offline：Cargo.lock 里包含其它平台的依赖，本地未缓存时
    # 离线模式会直接失败，而这里只需要读版本号。
    metadata = run("cargo", "metadata", "--format-version", "1", "--locked")
    import json

    for package in json.loads(metadata)["packages"]:
        if package["name"] == BINARY_NAME:
            return package["version"]
    raise ValueError("Cargo.toml 里找不到 siskin 包")


def build(skip_build: bool, output: Path) -> Path:
    missing = [tool for tool in REQUIRED_TOOLS if shutil.which(tool) is None]
    if missing:
        raise RuntimeError("缺少构建工具：" + ", ".join(missing))

    if not skip_build:
        subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)

    binary = ROOT / "target/release" / BINARY_NAME
    if not binary.is_file():
        raise RuntimeError("没有找到 release 二进制，先跑 cargo build --release --locked")

    version = cargo_version()
    architecture = run("dpkg", "--print-architecture")
    output.mkdir(parents=True, exist_ok=True)
    artifact = output / f"{BINARY_NAME}_{version}_{architecture}.deb"

    with tempfile.TemporaryDirectory(prefix="siskin-deb-", dir=ROOT / "target") as temporary:
        work = Path(temporary)
        stage = work / "root"
        debian = stage / "DEBIAN"
        debian.mkdir(parents=True)
        # 环境的 umask / 父目录 setgid 位会漏进新目录（本机 umask 0007
        # 时得到 2770），dpkg-deb 要求控制目录在 0755..0775 之间，
        # 这里显式固定，避免构建结果随环境变化。
        debian.chmod(0o755)

        target_binary = stage / f"usr/bin/{BINARY_NAME}"
        copy(binary, target_binary)
        target_binary.chmod(0o755)

        desktop = stage / f"usr/share/applications/{APP_ID}.desktop"
        copy(ROOT / f"packaging/{APP_ID}.desktop", desktop)
        run("desktop-file-validate", str(desktop))

        install_icons(stage)
        install_hooks(debian)
        for directory in stage.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)

        doc = stage / f"usr/share/doc/{PACKAGE}"
        copy(ROOT / "README.md", doc / "README.md")
        copy(ROOT / "LICENSE", doc / "copyright")

        description = (
            "Section: x11\nPriority: optional\n"
            "Maintainer: fzf <noreply@localhost>\n"
            "Description: Siskin terminal emulator\n"
            " A focused Linux terminal with real PTY, VT parsing and GPUI rendering.\n"
        )
        (work / "debian").mkdir()
        (work / "debian/control").write_text(
            f"Source: {PACKAGE}\n{description}\nPackage: {PACKAGE}\n"
            f"Architecture: any\n{description}",
            encoding="utf-8",
        )
        shlibs = run("dpkg-shlibdeps", "-O", "-e" + str(binary), cwd=work)
        installed_size = sum(p.stat().st_size for p in stage.rglob("*") if p.is_file()) // 1024
        (debian / "control").write_text(
            f"Package: {PACKAGE}\nVersion: {version}\nArchitecture: {architecture}\n"
            f"Installed-Size: {installed_size}\nDepends: {dependencies(shlibs)}\n"
            f"{description}",
            encoding="utf-8",
        )
        (debian / "control").chmod(0o644)

        # 与控制目录同理：数据目录也可能从环境继承 setgid/组写权限
        # （本机各级目录都是 2770），打包前统一收成 0755，避免装
        # 出来的目录带奇怪的权限位。位置必须在所有拷贝完成之后，
        # 否则后建的目录又会漏网。文件本身的权限前面已逐个定好，不动。
        stage.chmod(0o755)
        for directory in stage.rglob("*"):
            if directory.is_dir():
                directory.chmod(0o755)

        run("dpkg-deb", "--root-owner-group", "--build", str(stage), str(artifact))

    print(artifact)
    return artifact


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-build", action="store_true", help="打包已有二进制")
    parser.add_argument("--output", type=Path, default=ROOT / "target/debian")
    args = parser.parse_args()
    try:
        build(args.skip_build, args.output.resolve())
    except (RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or str(error)
        parser.exit(1, f"Deb 构建失败：{detail}\n")


if __name__ == "__main__":
    main()
