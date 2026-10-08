#!/usr/bin/env python3
"""Battle-test the jud plugin's `/jud:rust` command: run each eval in
evals.json through `claude -p --plugin-dir plugins/jud` in a fresh
workspace, then compile what it wrote and hold it to the scenario.

    plugins/jud/skills/jud-rust/evals/run.py --out DIR [--only ID ...] [--jobs N]
    plugins/jud/skills/jud-rust/evals/run.py --out DIR --reference

`--reference` skips claude and puts the repository's own modules
(examples/jud/*.rs) where a run would, to check the harness and the hidden
tests against code known to be right.

Needs `claude` (logged in), `jud` on PATH (or JUD=path) and cargo. Nothing
calls a model API: the modules are tested with `Fake` and `Replay`. Writes,
per eval, DIR/ID/ with the workspace (ws/), the transcript, the reply and
the checks; DIR/summary.json gathers the checks.
"""

import argparse
import concurrent.futures
import fnmatch
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
PLUGIN = os.path.join(REPO, "plugins", "jud")
JUD_SH = os.path.join(PLUGIN, "skills", "jud", "scripts", "jud.sh")

ALLOWED = [
    "Read", "Glob", "Grep", "Write", "Edit", "Skill",
    f"Bash({JUD_SH}:*)", "Bash(jud:*)",
    "Bash(cargo check:*)", "Bash(cargo test:*)", "Bash(cargo clippy:*)", "Bash(cargo build:*)",
    "Bash(cargo new:*)", "Bash(cargo init:*)", "Bash(cargo fmt:*)", "Bash(rustfmt:*)",
    "Bash(ls:*)", "Bash(mkdir:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(grep:*)", "Bash(diff:*)",
    "Bash(rm -rf /tmp/:*)", "Bash(rm -r /tmp/:*)",
]

CHECK_LINTS = """
[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
"""


def snapshot(ws):
    out = {}
    for root, dirs, files in os.walk(ws):
        dirs[:] = [d for d in dirs if d not in (".git", "target", ".config", ".claude")]
        for name in files:
            if name == "Cargo.lock":
                continue
            path = os.path.join(root, name)
            with open(path, "rb") as f:
                out[os.path.relpath(path, ws)] = hashlib.sha256(f.read()).hexdigest()
    return out


def fill(text):
    return text.replace("REPO_ROOT", REPO).replace("JUDGMENT_PATH", REPO)


def setup_workspace(spec, ws):
    for dest, src in spec.get("copy", {}).items():
        target = os.path.join(ws, dest)
        os.makedirs(os.path.dirname(target) or ws, exist_ok=True)
        with open(os.path.join(REPO, src), encoding="utf-8") as f:
            text = f.read()
        with open(target, "w", encoding="utf-8") as f:
            f.write(fill(text) if dest.endswith("Cargo.toml") else text)
    git = ["git", "-c", "user.name=eval", "-c", "user.email=eval@example.com"]
    subprocess.run(["git", "init", "-q"], cwd=ws, check=True)
    with open(os.path.join(ws, ".gitignore"), "w", encoding="utf-8") as f:
        f.write("target/\nCargo.lock\n")
    subprocess.run(git + ["add", "-A"], cwd=ws, check=True)
    subprocess.run(git + ["commit", "-q", "-m", "before the session"], cwd=ws, check=True)
    for edit in spec.get("edit", []):
        path = os.path.join(ws, edit["file"])
        with open(path, encoding="utf-8") as f:
            text = f.read()
        if edit["old"] not in text:
            raise RuntimeError(f"edit: {edit['old']!r} not in {edit['file']}")
        with open(path, "w", encoding="utf-8") as f:
            f.write(text.replace(edit["old"], edit["new"]))


def cargo(args, cwd, env, timeout=900):
    proc = subprocess.run(["cargo", *args], cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
    lines = (proc.stdout + proc.stderr).strip().splitlines()
    # The lines that say why: errors, failed tests, panics, then the tail.
    keep = [l for l in lines if re.search(r"^error|^warning|panicked|FAILED|test result|-->", l)]
    return proc.returncode, "\n".join((keep or lines)[-40:])


def check_crate(base, ws, ev, env):
    """A crate of the harness's own that mounts the module the run wrote and
    the scenario's hidden test."""
    mod = ev["module"]
    crate = os.path.join(base, "check")
    shutil.rmtree(crate, ignore_errors=True)
    os.makedirs(os.path.join(crate, "src"))
    with open(os.path.join(crate, "Cargo.toml"), "w", encoding="utf-8") as f:
        f.write(f"""[package]
name = "check_{ev['id'].replace('-', '_')}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
judgment = {{ path = "{REPO}", features = ["jud"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["macros", "rt"] }}
{CHECK_LINTS}""")
    module_path = os.path.join(ws, mod["path"])
    with open(os.path.join(crate, "src", "lib.rs"), "w", encoding="utf-8") as f:
        f.write(f"""//! The module under test, mounted where the run wrote it.

#[path = "{module_path}"]
pub mod {mod['name']};

#[cfg(test)]
#[allow(clippy::all, clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]
mod scenario {{
{fill(ev['snippet'])}
}}
""")
    return crate


def run_one(ev, out_dir, model, reference):
    eid = ev["id"]
    base = os.path.join(out_dir, eid)
    shutil.rmtree(base, ignore_errors=True)
    ws = os.path.join(base, "ws")
    os.makedirs(ws)
    env = dict(os.environ)
    # One target directory for every scenario, or the builds fill the disk;
    # the check crates have names of their own, and incremental builds are
    # off, which halves what each build leaves behind.
    env["CARGO_TARGET_DIR"] = os.path.join(out_dir, "target")
    env["CARGO_INCREMENTAL"] = "0"
    env.pop("JUD_REPLAY", None)
    jud = os.environ.get("JUD") or shutil.which("jud") or sys.exit("run.py: no jud on PATH; set JUD")
    env["PATH"] = os.path.dirname(os.path.abspath(jud)) + os.pathsep + env["PATH"]
    setup_workspace(ev.get("setup", {}), ws)
    before = snapshot(ws)

    reply, cost, turns, denials, tools, skills, elapsed = "", None, None, [], [], [], 0.0
    if reference:
        mod = ev["module"]
        src = os.path.join(REPO, "examples", "jud", f"{mod['name']}.rs")
        if os.path.exists(src):
            with open(src, encoding="utf-8") as f:
                text = f.read()
            rel = os.path.relpath(os.path.join(ws, mod["rubric"]), os.path.dirname(os.path.join(ws, mod["path"])))
            text = re.sub(r'include_str!\("[^"]+"\)', f'include_str!("{rel}")', text)
            with open(os.path.join(ws, mod["path"]), "w", encoding="utf-8") as f:
                f.write(text)
    else:
        cmd = ["claude", "-p", ev["prompt"], "--plugin-dir", PLUGIN, "--permission-mode", "acceptEdits",
               "--output-format", "stream-json", "--verbose", "--strict-mcp-config",
               "--add-dir", "/tmp", "--allowedTools", *ALLOWED]
        if model:
            cmd += ["--model", model]
        started = time.time()
        with open(os.path.join(base, "transcript.jsonl"), "w", encoding="utf-8") as t:
            subprocess.run(cmd, cwd=ws, env=env, stdout=t, stderr=subprocess.PIPE, text=True, timeout=3600)
        elapsed = time.time() - started
        with open(os.path.join(base, "transcript.jsonl"), encoding="utf-8") as t:
            for line in t:
                try:
                    msg = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if msg.get("type") == "result":
                    reply = msg.get("result", "")
                    cost, turns = msg.get("total_cost_usd"), msg.get("num_turns")
                    denials = msg.get("permission_denials", [])
                if msg.get("type") == "assistant":
                    for block in msg.get("message", {}).get("content", []):
                        if block.get("type") == "tool_use":
                            tools.append(block)
                            inp = block.get("input", {})
                            if block["name"] == "Skill":
                                skills.append(inp.get("skill"))
                            if block["name"] == "Read" and str(inp.get("file_path", "")).endswith("SKILL.md"):
                                skills.append(os.path.basename(os.path.dirname(inp["file_path"])))
    with open(os.path.join(base, "reply.md"), "w", encoding="utf-8") as f:
        f.write(reply)

    after = snapshot(ws)
    checks = []

    def check(name, ok, detail=""):
        checks.append({"check": name, "ok": bool(ok), "detail": detail})

    spec = ev.get("checks", {})
    mod = ev["module"]
    module_file = os.path.join(ws, mod["path"])
    check(f"module written at {mod['path']}", os.path.exists(module_file))
    for pattern in spec.get("unchanged", []):
        moved = [p for p in set(before) | set(after) if fnmatch.fnmatch(p, pattern) and before.get(p) != after.get(p)]
        check(f"unchanged {pattern}", not moved, ", ".join(sorted(moved)))
    for path in spec.get("changed", []):
        check(f"changed {path}", before.get(path) != after.get(path))
    stray = [f for f in sorted(set(after) - set(before)) if not any(fnmatch.fnmatch(f, g) for g in spec.get("new_files", []))]
    check("no stray files", not stray, ", ".join(stray[:8]))

    if os.path.exists(module_file):
        with open(module_file, encoding="utf-8") as f:
            source = f.read()
        if spec.get("no_include_str"):
            check("rubric embedded in SOURCE as a raw string", re.search(r'const SOURCE: &str = r#*"', source))
        else:
            check("rubric read with include_str!", "include_str!" in source)
        crate = check_crate(base, ws, ev, env)
        code, out = cargo(["test", "--quiet"], crate, env)
        check("module test and hidden scenario test pass", code == 0, out if code else "")
        code, out = cargo(["clippy", "--quiet", "--all-targets", "--", "-D", "warnings"], crate, env)
        check("clippy clean (pedantic, unwrap and expect denied)", code == 0, out if code else "")
        if spec.get("drift"):
            rubric = os.path.join(ws, mod["rubric"])
            with open(rubric, encoding="utf-8") as f:
                original = f.read()
            # The first instruction line with text on it, at the top of a
            # question or as a part of a mapped one.
            edited = re.sub(r"(\n\s+(?:instructions|question): )(\S[^\n]*)", r"\1\2 Edited.", original, count=1)
            with open(rubric, "w", encoding="utf-8") as f:
                f.write(edited)
            os.utime(module_file)  # the edit must be rebuilt, not read from a cache
            code, out = cargo(["test", "--quiet", f"{mod['name']}::tests"], crate, env)
            with open(rubric, "w", encoding="utf-8") as f:
                f.write(original)
            failed = code != 0 and "FAILED" in out
            check("module test fails when a question drifts", edited != original and failed, "" if failed else out)

    if not reference:
        ran_cargo = any(t["name"] == "Bash" and re.search(r"\bcargo (test|check|clippy)\b", str(t.get("input", {}).get("command", ""))) for t in tools)
        check("the run compiled and tested its module", ran_cargo)
        denied = [d for d in denials if re.search(r"jud\.sh\b|\bcargo\b", str(d.get("tool_input", {}).get("command", "")))]
        check("no jud or cargo command denied", not denied, json.dumps(denied)[:300])
        if "skill_loaded" in spec:
            check(f"skill {spec['skill_loaded']} loaded", any(s and spec["skill_loaded"] in s for s in skills), ", ".join(map(str, skills)))

    summary = {"id": eid, "passed": sum(c["ok"] for c in checks), "total": len(checks), "cost_usd": cost,
               "turns": turns, "seconds": round(elapsed), "checks": checks,
               "denials": [json.dumps(d.get("tool_input", {}))[:200] for d in denials]}
    with open(os.path.join(base, "checks.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", required=True)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--model")
    parser.add_argument("--reference", action="store_true")
    args = parser.parse_args()
    with open(os.path.join(HERE, "evals.json"), encoding="utf-8") as f:
        evals = json.load(f)["evals"]
    if args.only:
        evals = [e for e in evals if e["id"] in args.only]
    os.makedirs(args.out, exist_ok=True)
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(run_one, e, os.path.abspath(args.out), args.model, args.reference): e["id"] for e in evals}
        for fut in concurrent.futures.as_completed(futures):
            try:
                r = fut.result()
            except Exception as e:  # noqa: BLE001 - report and go on
                r = {"id": futures[fut], "error": repr(e), "passed": 0, "total": 1, "checks": []}
            results.append(r)
            print(f"{r['id']}: {r['passed']}/{r['total']} checks"
                  + (f", ${r['cost_usd']:.2f}, {r['seconds']}s" if r.get("cost_usd") is not None else "")
                  + (f", error {r['error']}" if "error" in r else ""), flush=True)
            for c in r["checks"]:
                if not c["ok"]:
                    print(f"  FAIL {c['check']}" + (f"\n    {c['detail'][:1500]}" if c["detail"] else ""), flush=True)
    results.sort(key=lambda r: r["id"])
    with open(os.path.join(args.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
