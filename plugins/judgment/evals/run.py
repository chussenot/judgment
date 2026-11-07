#!/usr/bin/env python3
"""Battle-test the judgment plugin: run each eval in evals.json through
`claude -p --plugin-dir plugins/judgment` in a fresh copy of the fixture
crate (files/app/), then build, test and lint what the run left and hold it
to the scenario.

    plugins/judgment/evals/run.py --out DIR [--only ID ...] [--jobs N] [--model M]

Needs `claude` (logged in) and cargo. Nothing reaches a model API: every
run has no TYPESAFE_API_KEY and a TYPESAFE_BASE_URL on a closed port, so a
test that calls a real API fails. Writes, per eval, DIR/ID/ with the
workspace (ws/), the transcript, the reply and the checks; DIR/summary.json
gathers the checks. Grading the expectations is left to a reader.
"""

import argparse
import concurrent.futures
import glob
import hashlib
import json
import os
import re
import shutil
import subprocess
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
PLUGIN = os.path.join(REPO, "plugins", "judgment")
APP = os.path.join(HERE, "files", "app")

# What a user who installed the plugin would allow: cargo's checks, reading
# and editing files, a few read-only commands. No network tool.
ALLOWED = [
    "Read", "Glob", "Grep", "Write", "Edit", "Skill",
    "Bash(cargo check:*)", "Bash(cargo test:*)", "Bash(cargo clippy:*)", "Bash(cargo build:*)",
    "Bash(cargo fmt:*)", "Bash(rustfmt:*)", "Bash(cargo tree:*)",
    "Bash(ls:*)", "Bash(mkdir:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(grep:*)", "Bash(diff:*)",
    "Bash(git diff:*)", "Bash(git status:*)",
]


def snapshot(ws):
    out = {}
    for root, dirs, files in os.walk(ws):
        dirs[:] = [d for d in dirs if d not in (".git", "target", ".claude")]
        for name in files:
            if name == "Cargo.lock":
                continue
            path = os.path.join(root, name)
            with open(path, "rb") as f:
                out[os.path.relpath(path, ws)] = hashlib.sha256(f.read()).hexdigest()
    return out


def setup_workspace(ws, spec):
    shutil.copytree(APP, ws)
    manifest = os.path.join(ws, "Cargo.toml")
    with open(manifest, encoding="utf-8") as f:
        # A package name of its own: scenarios share one target directory,
        # and two concurrent builds of one name overwrite each other's binaries.
        name = "app_" + re.sub(r"[^a-z0-9]", "_", os.path.basename(os.path.dirname(ws)))
        text = f.read().replace("JUDGMENT_PATH", REPO).replace('name = "app"', f'name = "{name}"')
    with open(manifest, "w", encoding="utf-8") as f:
        f.write(text)
    for edit in spec.get("edit", []):
        path = os.path.join(ws, edit["file"])
        with open(path, encoding="utf-8") as f:
            t = f.read()
        if edit["old"] not in t:
            raise RuntimeError(f"edit: {edit['old']!r} not in {edit['file']}")
        with open(path, "w", encoding="utf-8") as f:
            f.write(t.replace(edit["old"], edit["new"]))
    git = ["git", "-c", "user.name=eval", "-c", "user.email=eval@example.com"]
    subprocess.run(["git", "init", "-q"], cwd=ws, check=True)
    with open(os.path.join(ws, ".gitignore"), "w", encoding="utf-8") as f:
        f.write("target/\nCargo.lock\n")
    subprocess.run(git + ["add", "-A"], cwd=ws, check=True)
    subprocess.run(git + ["commit", "-q", "-m", "before the session"], cwd=ws, check=True)


def cargo(args, cwd, env, timeout=900):
    proc = subprocess.run(["cargo", *args], cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
    lines = (proc.stdout + proc.stderr).strip().splitlines()
    keep = [l for l in lines if re.search(r"^error|^warning|panicked|FAILED|test result|-->", l)]
    return proc.returncode, "\n".join((keep or lines)[-30:]), proc.stdout + proc.stderr


def run_one(ev, out_dir, model):
    eid = ev["id"]
    base = os.path.join(out_dir, eid)
    shutil.rmtree(base, ignore_errors=True)
    os.makedirs(base)
    ws = os.path.join(base, "ws")
    setup_workspace(ws, ev.get("setup", {}))
    env = dict(os.environ)
    env.pop("TYPESAFE_API_KEY", None)
    env["TYPESAFE_BASE_URL"] = "http://127.0.0.1:9"  # closed: nothing reaches a real API
    # One target directory for every scenario, or the builds fill the disk.
    env["CARGO_TARGET_DIR"] = os.path.join(out_dir, "target")
    tmp = os.path.join(base, "tmp")
    os.makedirs(tmp)
    env["TMPDIR"] = tmp

    before = snapshot(ws)
    cmd = ["claude", "-p", ev["prompt"], "--plugin-dir", PLUGIN, "--permission-mode", "acceptEdits",
           "--output-format", "stream-json", "--verbose", "--strict-mcp-config",
           "--add-dir", tmp, "--add-dir", "/tmp", "--allowedTools", *ALLOWED]
    if model:
        cmd += ["--model", model]
    started = time.time()
    with open(os.path.join(base, "transcript.jsonl"), "w", encoding="utf-8") as t:
        result = subprocess.run(cmd, cwd=ws, env=env, stdout=t, stderr=subprocess.PIPE, text=True, timeout=2400)
    elapsed = time.time() - started

    reply, cost, denials, skills, tools = "", None, [], [], []
    with open(os.path.join(base, "transcript.jsonl"), encoding="utf-8") as t:
        for line in t:
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("type") == "result":
                reply, cost = msg.get("result", ""), msg.get("total_cost_usd")
                denials = msg.get("permission_denials", [])
            if msg.get("type") == "assistant":
                for block in msg.get("message", {}).get("content", []):
                    if block.get("type") == "tool_use":
                        inp = block.get("input", {})
                        tools.append(block["name"] if block["name"] != "Bash" else "Bash: " + str(inp.get("command", ""))[:160])
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
    for path in spec.get("changed", []):
        check(f"changed {path}", before.get(path) != after.get(path))
    for path in spec.get("unchanged", []):
        check(f"unchanged {path}", before.get(path) == after.get(path))
    for item in spec.get("exists", []):
        n = len(glob.glob(os.path.join(ws, item["glob"])))
        check(f"exists {item['glob']}", n >= 1, f"found {n}")
    for item in spec.get("file_matches", []):
        paths = sorted(glob.glob(os.path.join(ws, item["glob"]), recursive=True))
        text = "".join(open(p, encoding="utf-8").read() for p in paths)
        for r in item.get("regex", []):
            check(f"{item['glob']} matches /{r}/", re.search(r, text), f"{len(paths)} files")
        for r in item.get("absent", []):
            check(f"{item['glob']} lacks /{r}/", paths and not re.search(r, text), "")
    # The crate builds, its tests pass with no key and no network, and it
    # is lint-clean under the fixture's pedantic lints.
    # A module the user asked not to wire in is compiled in a copy of the
    # workspace that mounts it, so its tests are counted and linted too.
    crate = ws
    if spec.get("mount"):
        crate = os.path.join(base, "mounted")
        shutil.rmtree(crate, ignore_errors=True)
        shutil.copytree(ws, crate, ignore=shutil.ignore_patterns(".git", "target"))
        main_rs = os.path.join(crate, "src", "main.rs")
        with open(main_rs, encoding="utf-8") as f:
            text = f.read()
        # At the end, after the file's inner doc comment and its items; a
        # module the run already declared is not declared twice.
        mounts = "".join(f"\n#[allow(dead_code)]\nmod {m};\n" for m in spec["mount"]
                         if not re.search(rf"^\s*(pub\s+)?mod\s+{m}\s*;", text, re.M))
        with open(main_rs, "w", encoding="utf-8") as f:
            f.write(text + mounts)
    if spec.get("cargo", True):
        code, tail, full = cargo(["test", "--quiet"], crate, env)
        passed = sum(int(m) for m in re.findall(r"test result: \w+\. (\d+) passed", full))
        check("cargo test passes with no key and no network", code == 0, tail)
        if "min_tests" in spec:
            check(f"at least {spec['min_tests']} tests ran", passed >= spec["min_tests"], f"{passed} passed")
        code, tail, _ = cargo(["clippy", "--quiet", "--all-targets", "--", "-D", "warnings"], crate, env)
        check("cargo clippy -D warnings", code == 0, tail)
    if "skill_loaded" in spec:
        check(f"skill {spec['skill_loaded']} loaded", any(s and spec["skill_loaded"] in s for s in skills), ", ".join(map(str, skills)))
    stray = [p for p in set(after) - set(before) if not any(re.fullmatch(g, p) for g in spec.get("new_files", []))]
    check("no stray files", not stray, ", ".join(sorted(stray)[:8]))

    summary = {"id": eid, "passed": sum(c["ok"] for c in checks), "total": len(checks), "cost_usd": cost,
               "seconds": round(elapsed), "exit": result.returncode, "stderr": result.stderr[-500:],
               "skills": skills, "denials": [json.dumps(d.get("tool_input", {}))[:200] for d in denials],
               "checks": checks, "tools": tools}
    with open(os.path.join(base, "checks.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", required=True)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--jobs", type=int, default=3)
    parser.add_argument("--model")
    args = parser.parse_args()
    with open(os.path.join(HERE, "evals.json"), encoding="utf-8") as f:
        evals = json.load(f)["evals"]
    if args.only:
        evals = [e for e in evals if e["id"] in args.only]
    os.makedirs(args.out, exist_ok=True)
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(run_one, e, args.out, args.model): e["id"] for e in evals}
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
                    print(f"  FAIL {c['check']} ({c['detail'][:300]})", flush=True)
    results.sort(key=lambda r: r["id"])
    with open(os.path.join(args.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
