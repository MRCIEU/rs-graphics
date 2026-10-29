#!/usr/bin/env python3
# Copyright (C) 2026 University of Bristol
# rs-graphics includes code translated from R, copyright (C) The R Core Team
# and others; see NOTICE.
#
# This file is part of rs-graphics, licensed under the GNU General Public
# License, version 3 or (at your option) any later version; see LICENSE.
# SPDX-License-Identifier: GPL-3.0-or-later

"""Runs the checks a change must pass, comparing with a base revision.

Steps, in order:
  bless    RS_GRAPHICS_BLESS=1 cargo test --test all generated_docs, which
           rewrites src/guide/r_reference.md, src/charts/*.svg and
           llms.txt; lists what changed, for review
  test     RS_GRAPHICS_REQUIRE_R=1 cargo test (needs Rscript)
  doc      RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
  clippy   warning counts for the library, its unit tests and tests/all,
           each against the base; new warnings are listed
  msrv     cargo +1.85 build --all-targets
  gallery  cargo run --release --example gallery, here and at the base;
           every file in gallery_output/ must be byte-identical

The base (default HEAD, so: the uncommitted changes) is checked out in a
reusable worktree at target/tools/base. After committing, compare with
the commit before: --base HEAD~1.

Usage:
  tools/verify.py [--base REV] [--only STEP,...] [--skip STEP,...]

Full logs are in target/tools/logs/. Exits 1 if any step failed.
"""

import argparse
import collections
import filecmp
import json
import sys
import time

from common import ROOT, WORK, base_env, base_worktree, run

LOGS = WORK / "logs"
STEPS = ["bless", "test", "doc", "clippy", "msrv", "gallery"]
MSRV = "1.85"
GENERATED = ["src/guide/r_reference.md", "src/charts", "llms.txt"]
# Each target's arguments, and the files whose warnings it counts (the
# test targets compile the library too).
CLIPPY_TARGETS = {
    "lib": (["--lib"], "src/"),
    "lib tests": (["--lib", "--tests"], "src/"),
    "tests/all": (["--test", "all"], "tests/"),
}


def step_bless(_base):
    p = run(["cargo", "test", "--test", "all", "generated_docs"],
            env={"RS_GRAPHICS_BLESS": "1"}, log=LOGS / "bless.log")
    if p.returncode != 0:
        return False, "generated_docs failed (see log)"
    changed = run(["git", "status", "--porcelain", "--", *GENERATED]).stdout.strip()
    if not changed:
        return True, "generated docs unchanged"
    files = ", ".join(line[3:] for line in changed.splitlines())
    return True, f"REVIEW the diff of: {files}"


def step_test(_base):
    p = run(["cargo", "test"], env={"RS_GRAPHICS_REQUIRE_R": "1"}, log=LOGS / "test.log")
    passed = failed = 0
    for line in p.stdout.splitlines():
        if line.startswith("test result:"):
            parts = line.replace(";", "").split()
            passed += int(parts[parts.index("passed") - 1])
            failed += int(parts[parts.index("failed") - 1])
    if p.returncode != 0:
        return False, f"{failed} failed, {passed} passed (see log)"
    return True, f"{passed} passed"


def step_doc(_base):
    p = run(["cargo", "doc", "--no-deps"], env={"RUSTDOCFLAGS": "-D warnings"}, log=LOGS / "doc.log")
    return p.returncode == 0, "clean" if p.returncode == 0 else "warnings or errors (see log)"


def clippy_warnings(cwd, env, args, prefix, log):
    """Distinct warnings in files under `prefix`, keyed without line
    numbers (so edits elsewhere don't make old warnings look new)."""
    p = run(["cargo", "clippy", "--message-format=json", *args], cwd=cwd, env=env, log=log)
    if p.returncode != 0:
        return None
    seen, found = set(), collections.Counter()
    for line in p.stdout.splitlines():
        if not line.startswith("{"):
            continue
        msg = json.loads(line)
        if msg.get("reason") != "compiler-message":
            continue
        m = msg["message"]
        if m["level"] != "warning" or not m["spans"]:
            continue
        span = next((s for s in m["spans"] if s["is_primary"]), m["spans"][0])
        code = (m.get("code") or {}).get("code", "")
        where = (span["file_name"], span["line_start"], span["column_start"], code, m["message"])
        if not span["file_name"].startswith(prefix) or where in seen:
            continue
        seen.add(where)
        found[(span["file_name"], code, m["message"])] += 1
    return found


def step_clippy(base):
    ok, parts, new = True, [], []
    for name, (args, prefix) in CLIPPY_TARGETS.items():
        slug = name.replace(" ", "_").replace("/", "_")
        now = clippy_warnings(ROOT, None, args, prefix, LOGS / f"clippy_{slug}.log")
        then = clippy_warnings(base, base_env(), args, prefix, LOGS / f"clippy_{slug}_base.log")
        if now is None or then is None:
            return False, f"clippy failed to build {name} (see logs)"
        n, b = sum(now.values()), sum(then.values())
        parts.append(f"{name} {b}->{n}" if n != b else f"{name} {n}")
        added = now - then
        if added:
            ok = False
            new += [f"    {f}: {code} {msg}" for (f, code, msg) in added]
    detail = ", ".join(parts)
    if new:
        detail += "\n  new warnings:\n" + "\n".join(sorted(set(new)))
    return ok, detail


def step_msrv(_base):
    p = run(["cargo", f"+{MSRV}", "build", "--all-targets"], log=LOGS / "msrv.log")
    return p.returncode == 0, f"builds with {MSRV}" if p.returncode == 0 else f"fails with {MSRV} (see log)"


def step_gallery(base):
    for where, env, name in [(ROOT, None, "gallery"), (base, base_env(), "gallery_base")]:
        out = where / "gallery_output"
        if out.exists():
            for f in out.iterdir():
                f.unlink()
        p = run(["cargo", "run", "--release", "--example", "gallery"], cwd=where, env=env,
                log=LOGS / f"{name}.log")
        if p.returncode != 0:
            return False, f"{name} failed (see log)"
    ours, theirs = ROOT / "gallery_output", base / "gallery_output"
    names = sorted({f.name for f in ours.iterdir()} | {f.name for f in theirs.iterdir()})
    differ = [n for n in names
              if not ((ours / n).exists() and (theirs / n).exists()
                      and filecmp.cmp(ours / n, theirs / n, shallow=False))]
    if differ:
        return False, f"{len(differ)} of {len(names)} files differ: {', '.join(differ)}"
    return True, f"{len(names)} files byte-identical"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="HEAD")
    ap.add_argument("--only")
    ap.add_argument("--skip")
    a = ap.parse_args()
    steps = a.only.split(",") if a.only else list(STEPS)
    if a.skip:
        steps = [s for s in steps if s not in a.skip.split(",")]
    for s in steps:
        if s not in STEPS:
            ap.error(f"unknown step {s}; steps are {', '.join(STEPS)}")
    base = base_worktree(a.base) if {"clippy", "gallery"} & set(steps) else None
    failed = False
    for s in steps:
        t = time.time()
        ok, detail = globals()[f"step_{s}"](base)
        failed |= not ok
        print(f"{'PASS' if ok else 'FAIL'}  {s:<8} {detail}  ({time.time() - t:.0f} s)", flush=True)
    print(f"logs: {LOGS}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
