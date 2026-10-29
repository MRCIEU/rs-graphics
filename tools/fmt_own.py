#!/usr/bin/env python3
# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

"""Applies only the rustfmt changes that overlap lines changed since REV.

Many files aren't rustfmt-clean, so running rustfmt on them would reformat
code nobody touched. This formats a copy of each file with
`rustfmt --edition 2024` and keeps only the hunks that overlap lines that
differ from REV (a new file is formatted whole).

Out-of-line module declarations (`mod x;`) are stubbed as `mod x {}` in
the copy, so rustfmt never follows them into other files: it is safe on
src/lib.rs and tests/all/main.rs, unlike plain rustfmt.

rustfmt reorders `use` and `mod` lines, so a hunk can reach beyond an edit
and drop or duplicate an import. The script warns when the `use`/`mod`
tokens of a file change; check those files.

Usage:
  tools/fmt_own.py [--rev REV] [--dry-run] [FILE...]

With no FILE, every .rs file changed since REV (default HEAD), plus
untracked ones, is formatted.
"""

import argparse
import collections
import difflib
import os
import re
import subprocess
import sys
import tempfile

from common import ROOT, git, run

MOD_DECL = re.compile(r"^(\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+))\s*;\s*$")
STUB = re.compile(r"^(\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)) \{\}\s*$")
STATEMENT = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:use|mod)\b[^;{]*(?:\{[^;]*?\})?[^;]*;", re.M
)
TOKEN = re.compile(r"\w+|::|\*|\{|\}")


def changed_lines(old, cur):
    mine = set()
    sm = difflib.SequenceMatcher(None, old, cur, autojunk=False)
    for tag, _i1, _i2, j1, j2 in sm.get_opcodes():
        if tag != "equal":
            mine.update(range(j1, max(j2, j1 + 1)))
    return mine


def rustfmt(lines):
    """`lines` formatted, with out-of-line `mod` declarations stubbed."""
    stubbed = set()
    src = []
    for line in lines:
        m = MOD_DECL.match(line.rstrip("\n"))
        if m:
            stubbed.add(m.group(2))
            line = m.group(1) + " {}\n"
        src.append(line)
    with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as t:
        t.write("".join(src))
        tmp = t.name
    try:
        p = run(["rustfmt", "--edition", "2024", tmp])
        if p.returncode != 0:
            sys.exit(f"rustfmt failed:\n{p.stdout}")
        out = open(tmp).read().splitlines(keepends=True)
    finally:
        os.unlink(tmp)
    unstubbed = []
    for line in out:
        m = STUB.match(line.rstrip("\n"))
        if m and m.group(2) in stubbed:
            line = m.group(1) + ";\n"
        unstubbed.append(line)
    # Lines that were stubbed but unchanged otherwise map back exactly.
    return unstubbed


def import_tokens(text):
    c = collections.Counter()
    for stmt in STATEMENT.findall(text):
        c.update(TOKEN.findall(stmt))
    return c


def format_file(path, rev, dry_run):
    cur = open(path).read().splitlines(keepends=True)
    rel = os.path.relpath(os.path.abspath(path), ROOT)
    p = run(["git", "show", f"{rev}:{rel}"])
    old = p.stdout.splitlines(keepends=True) if p.returncode == 0 else []
    mine = changed_lines(old, cur)
    fmt = rustfmt(cur)
    out, applied = [], 0
    sm = difflib.SequenceMatcher(None, cur, fmt, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal" or not (set(range(i1, max(i2, i1 + 1))) & mine):
            out.extend(cur[i1:i2])
        else:
            out.extend(fmt[j1:j2])
            applied += 1
    before, after = "".join(cur), "".join(out)
    warn = ""
    if import_tokens(before) != import_tokens(after):
        lost = import_tokens(before) - import_tokens(after)
        gained = import_tokens(after) - import_tokens(before)
        warn = f"  WARNING: use/mod lines changed (lost {dict(lost)}, gained {dict(gained)}); check them"
    if not dry_run and after != before:
        open(path, "w").write(after)
    verb = "would apply" if dry_run else "applied"
    print(f"{rel}: {verb} {applied} hunk(s){warn}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rev", default="HEAD")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("files", nargs="*")
    a = ap.parse_args()
    files = a.files
    if not files:
        changed = git("diff", "--name-only", a.rev, "--", "*.rs").split()
        untracked = git("ls-files", "--others", "--exclude-standard", "--", "*.rs").split()
        files = [str(ROOT / f) for f in changed + untracked if (ROOT / f).exists()]
    for f in files:
        format_file(f, a.rev, a.dry_run)


if __name__ == "__main__":
    main()
