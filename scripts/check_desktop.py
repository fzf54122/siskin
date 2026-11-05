# -*- coding: utf-8 -*-
# @Time    : 2026/11/4 23:15
# @Author  : fzf
# @FileName: check_desktop.py
# @Software: Siskin

"""X11 acceptance check: STRING menu properties, idle D-Bus actions and restart.

Run after cargo build. Requires xdotool, gdbus, qdbus and busctl, plus an X11
desktop session. Uses an isolated temporary settings directory. Only its own
application processes are stopped, and desktop services are never restarted.
"""

import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target/debug/siskin"
MENU_PATH = "/org/siskin/Terminal/Menu"
MENU_INTERFACE = "com.canonical.dbusmenu"


def command(*args):
    return subprocess.check_output(args, text=True, timeout=12)


def check():
    if not os.environ.get("DISPLAY"):
        raise RuntimeError("This check requires an X11 display")
    with tempfile.TemporaryDirectory(prefix="siskin-desktop-check-") as directory:
        settings = Path(directory) / "siskin/settings.json"
        settings.parent.mkdir()
        settings.write_text(json.dumps({
            "version": 1, "theme_mode": "dark", "font_size": 13, "shell": "bash",
        }), encoding="utf-8")
        for run in range(2):
            process = subprocess.Popen(
                [str(BINARY)],
                env={**os.environ, "XDG_CONFIG_HOME": directory},
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            try:
                window = command(
                    "xdotool", "search", "--sync", "--pid", str(process.pid),
                ).splitlines()[0]
                service = "org.siskin.Terminal.p{}".format(process.pid)
                command("gdbus", "wait", "--session", "--timeout=5", service)
                props = command(
                    "xprop", "-id", window,
                    "_KDE_NET_WM_APPMENU_SERVICE_NAME",
                    "_KDE_NET_WM_APPMENU_OBJECT_PATH",
                    "_GTK_MENUBAR_OBJECT_PATH",
                )
                assert "_KDE_NET_WM_APPMENU_SERVICE_NAME(STRING)" in props, props
                assert "_KDE_NET_WM_APPMENU_OBJECT_PATH(STRING)" in props, props
                assert "_GTK_MENUBAR_OBJECT_PATH:  not found" in props, props
                label = command(
                    "qdbus", service, MENU_PATH, MENU_INTERFACE + ".GetProperty",
                    "21", "label",
                )
                assert "深色" in label, label
                for item in (12, 5):
                    # Actions are sent with no mouse, keyboard or forced draw.
                    command(
                        "busctl", "--user", "--timeout=3", "call", service,
                        MENU_PATH, MENU_INTERFACE, "Event", "isvu",
                        str(item), "clicked", "i", "0", "0",
                    )
                process.wait(timeout=5)
                assert process.returncode == 0, process.returncode
                assert json.loads(settings.read_text())["theme_mode"] == "dark"
                print("Pass {}: menu properties, idle quit and dark restore".format(run + 1))
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=5)
                errors = process.stderr.read().decode("utf-8", errors="replace")
                if errors:
                    print(errors)
                process.stderr.close()


if __name__ == "__main__":
    check()
