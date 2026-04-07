"""
macOS Terminal Launcher — opens a new terminal window and runs a command.

Supports Ghostty, iTerm2, and Terminal.app.
Detection priority: env vars (current terminal) → installed apps → fallback.
"""

from __future__ import annotations

import os
import subprocess
import shlex
from pathlib import Path


def detect_terminal() -> str | None:
    """Detect which supported terminal emulator is available.

    Checks the current terminal first (env vars), then falls back to
    checking installed apps.

    Returns 'ghostty', 'iterm2', 'terminal', or None.
    """
    # Current terminal — env var detection
    if os.environ.get("GHOSTTY_RESOURCES_DIR"):
        return "ghostty"
    if os.environ.get("ITERM_SESSION_ID"):
        return "iterm2"
    if os.environ.get("TERM_PROGRAM") == "Apple_Terminal":
        return "terminal"

    # Fallback — check installed apps
    for app, name in [
        ("/Applications/Ghostty.app", "ghostty"),
        ("/Applications/iTerm.app", "iterm2"),
    ]:
        if Path(app).exists():
            return name

    # Terminal.app is always installed on macOS
    if Path("/System/Applications/Utilities/Terminal.app").exists():
        return "terminal"

    return None


def launch_in_terminal(script: str, args: list[str], cwd: Path) -> bool:
    """Open a new terminal window running `script` with `args` from `cwd`.

    Returns True if the terminal was launched, False if no supported terminal
    was detected (caller should fall back to manual instructions).
    """
    terminal = detect_terminal()
    if terminal is None:
        return False

    # Build the shell command that runs inside the new terminal
    script_with_args = f"{script} {' '.join(shlex.quote(a) for a in args)}"
    inner_cmd = f"cd {shlex.quote(str(cwd))} && {script_with_args}; echo ''; echo '✅ Script finished. Press Enter to close.'; read"

    try:
        if terminal == "ghostty":
            return _launch_ghostty(inner_cmd)
        elif terminal == "iterm2":
            return _launch_iterm2(inner_cmd)
        elif terminal == "terminal":
            return _launch_terminal_app(inner_cmd)
    except (subprocess.CalledProcessError, FileNotFoundError, OSError) as exc:
        print(f"  ⚠️  Failed to launch terminal ({terminal}): {exc}")

    return False


def _launch_ghostty(cmd: str) -> bool:
    subprocess.Popen(
        ["open", "-na", "Ghostty", "--args", "-e", f"bash -c {shlex.quote(cmd)}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return True


def _launch_iterm2(cmd: str) -> bool:
    # Single quotes inside AppleScript need escaping
    escaped_cmd = cmd.replace("\\", "\\\\").replace('"', '\\"')
    applescript = (
        'tell application "iTerm2"\n'
        f'  create window with default profile command "bash -c \\"{escaped_cmd}\\""\n'
        "  activate\n"
        "end tell"
    )
    subprocess.Popen(
        ["osascript", "-e", applescript],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return True


def _launch_terminal_app(cmd: str) -> bool:
    escaped_cmd = cmd.replace("\\", "\\\\").replace('"', '\\"')
    applescript = (
        'tell application "Terminal"\n'
        f'  do script "{escaped_cmd}"\n'
        "  activate\n"
        "end tell"
    )
    subprocess.Popen(
        ["osascript", "-e", applescript],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return True
