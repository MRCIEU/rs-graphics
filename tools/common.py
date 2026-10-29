# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

"""Helpers shared by the scripts in tools/."""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Everything the tools write goes under target/ (ignored by git).
WORK = ROOT / "target" / "tools"
BASE = WORK / "base"
BASE_TARGET = WORK / "base-target"


def run(cmd, cwd=ROOT, env=None, log=None, check=False):
    """Runs `cmd`, returning the CompletedProcess with stdout and stderr
    merged into `.stdout`. With `log`, also writes the output there."""
    full_env = dict(os.environ, **(env or {}))
    p = subprocess.run(
        cmd, cwd=cwd, env=full_env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    if log is not None:
        log = Path(log)
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text(f"$ {' '.join(map(str, cmd))}\n{p.stdout}")
    if check and p.returncode != 0:
        sys.exit(f"{' '.join(map(str, cmd))} failed:\n{p.stdout}")
    return p


def git(*args, cwd=ROOT):
    return run(["git", *args], cwd=cwd, check=True).stdout.strip()


def base_worktree(rev):
    """A clean checkout of `rev` at target/tools/base (a git worktree,
    reused between runs). Build it with `base_env()` so its target
    directory persists too."""
    sha = git("rev-parse", "--verify", f"{rev}^{{commit}}")
    if not (BASE / ".git").exists():
        git("worktree", "prune")
        BASE.parent.mkdir(parents=True, exist_ok=True)
        git("worktree", "add", "--detach", str(BASE), sha)
    else:
        git("checkout", "--detach", "--force", sha, cwd=BASE)
        git("clean", "-fdq", cwd=BASE)
    return BASE


def base_env():
    return {"CARGO_TARGET_DIR": str(BASE_TARGET)}


def crate_version(root=ROOT):
    text = (Path(root) / "Cargo.toml").read_text()
    m = re.search(r'^\[package\][^\[]*?^version\s*=\s*"([^"]+)"', text, re.M | re.S)
    return m.group(1)
