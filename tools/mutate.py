#!/usr/bin/env python3
# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

"""Mutation checks: breaks the code on purpose and confirms a test fails.

Each mutation replaces one exact snippet of a file (it must occur exactly
once) and runs a test command, which should fail. The file is restored
afterwards, also on Ctrl-C, with a fresh mtime so cargo rebuilds it.

Results:
  caught          the tests failed, as they should
  SURVIVED        the tests passed: they don't check what was broken
  DOESN'T COMPILE the mutation proves nothing; write one that compiles

Usage:
  tools/mutate.py FILE OLD NEW [-- CMD...]
  tools/mutate.py --batch MUTATIONS.json

CMD defaults to `cargo test --test all`; give a test name filter, e.g.
`-- cargo test --test all legend_extras`. Tests run with
RS_GRAPHICS_REQUIRE_R=1, so R cross-checks can't skip silently.

A batch file is a JSON list of objects with "file", "old", "new" and
optionally "name" and "cmd" (a list of strings). Exits 1 if any mutation
survived or didn't compile.
"""

import argparse
import json
import os
import sys
from pathlib import Path

from common import ROOT, run

DEFAULT_CMD = ["cargo", "test", "--test", "all"]
ENV = {"RS_GRAPHICS_REQUIRE_R": "1"}


def build_only(cmd):
    """The same cargo test command with --no-run (before any `--`)."""
    if cmd[:2] != ["cargo", "test"]:
        return None
    end = cmd.index("--") if "--" in cmd else len(cmd)
    # A test name filter is a positional argument; --no-run ignores it.
    return cmd[:end] + ["--no-run"]


def mutate(m):
    path = (ROOT / m["file"]).resolve()
    original = path.read_text()
    n = original.count(m["old"])
    if n != 1:
        return f"ERROR: snippet found {n} times in {m['file']}"
    cmd = m.get("cmd") or DEFAULT_CMD
    try:
        path.write_text(original.replace(m["old"], m["new"]))
        build = build_only(cmd)
        if build and run(build, env=ENV).returncode != 0:
            return "DOESN'T COMPILE"
        p = run(cmd, env=ENV)
        return "caught" if p.returncode != 0 else "SURVIVED"
    finally:
        path.write_text(original)
        os.utime(path, None)


def main():
    argv = sys.argv[1:]
    cmd = None
    if "--" in argv:
        i = argv.index("--")
        argv, cmd = argv[:i], argv[i + 1:]
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--batch")
    ap.add_argument("file", nargs="?")
    ap.add_argument("old", nargs="?")
    ap.add_argument("new", nargs="?")
    a = ap.parse_args(argv)
    if a.batch:
        muts = json.loads(Path(a.batch).read_text())
    elif a.file and a.old is not None and a.new is not None:
        muts = [{"file": a.file, "old": a.old, "new": a.new, "cmd": cmd}]
    else:
        ap.error("give FILE OLD NEW, or --batch")
    bad = 0
    for i, m in enumerate(muts, 1):
        name = m.get("name") or f"{m['file']}: {m['old'][:40]!r} -> {m['new'][:40]!r}"
        result = mutate(m)
        bad += result != "caught"
        print(f"{i:>3}. {result:<15} {name}", flush=True)
    print(f"{len(muts) - bad} of {len(muts)} caught")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
