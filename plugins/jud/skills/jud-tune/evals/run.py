#!/usr/bin/env python3
"""Battle-test the jud plugin's tuning commands: run each eval in evals.json
through `claude -p --plugin-dir plugins/jud` in a fresh workspace, then check
what it left behind.

    plugins/jud/skills/jud-tune/evals/run.py --out DIR [--only ID ...] [--jobs N]

Needs `claude` (logged in), `jud` on PATH (or JUD=path), python3 with PyYAML.
Nothing reaches a real API: every run's TYPESAFE_BASE_URL is the mock server
(mock_system_one.py) or a closed port, and its TYPESAFE_API_KEY is a dummy
unless the eval unsets it. Writes, per eval, DIR/ID/ with the workspace
(ws/), the transcript (transcript.jsonl), the reply (reply.md) and the checks
(checks.json); DIR/summary.json gathers the checks. Grading the expectations
(the prose ones) is left to a reader of reply.md and the transcript.
"""

import argparse
import concurrent.futures
import fnmatch
import glob
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time

import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
PLUGIN = os.path.join(REPO, "plugins", "jud")
MOCK = os.path.join(HERE, "mock_system_one.py")
JUD_SH = os.path.join(PLUGIN, "skills", "jud", "scripts", "jud.sh")

# What a user who installed the plugin would allow: the plugin's script, the
# `jud` command, reading and editing files in the project, and a few read-only
# shell commands. No python, no network tools, and no MCP server
# (--strict-mcp-config with none given), so the run sees only the plugin.
ALLOWED = [
    "Read", "Glob", "Grep", "Write", "Edit", "Skill",
    f"Bash({JUD_SH}:*)", "Bash(jud:*)",
    "Bash(ls:*)", "Bash(cp:*)", "Bash(mkdir:*)", "Bash(diff:*)", "Bash(wc:*)",
    "Bash(cat:*)", "Bash(head:*)", "Bash(grep:*)", "Bash(sort:*)", "Bash(uniq:*)",
    "Bash(git diff:*)", "Bash(git log:*)", "Bash(git status:*)", "Bash(git show:*)",
]


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def jud_bin():
    return os.environ.get("JUD") or shutil.which("jud") or sys.exit("run.py: no jud on PATH; set JUD")


def start_mock(ws, cases, profile, log, extra=None):
    port = free_port()
    args = [sys.executable, MOCK, "--port", str(port), "--log", log]
    for c in cases:
        args += ["--cases", os.path.join(ws, c)]
    if profile or extra:
        merged = {}
        if profile:
            with open(os.path.join(REPO, profile), encoding="utf-8") as f:
                merged = json.load(f)
        merged.update(extra or {})
        path = log + ".profile.json"
        with open(path, "w", encoding="utf-8") as f:
            json.dump(merged, f)
        args += ["--profile", path]
    proc = subprocess.Popen(args, stderr=subprocess.DEVNULL)
    for _ in range(100):
        try:
            socket.create_connection(("127.0.0.1", port), timeout=0.1).close()
            return proc, f"http://127.0.0.1:{port}"
        except OSError:
            time.sleep(0.05)
    proc.kill()
    raise RuntimeError("mock did not start")


def snapshot(ws):
    out = {}
    for root, _, files in os.walk(ws):
        if "/.config" in root or "/.claude" in root or "/.git" in root:
            continue
        for name in files:
            path = os.path.join(root, name)
            with open(path, "rb") as f:
                out[os.path.relpath(path, ws)] = hashlib.sha256(f.read()).hexdigest()
    return out


def count_lines(path):
    try:
        with open(path, encoding="utf-8") as f:
            return sum(1 for _ in f)
    except FileNotFoundError:
        return 0


def jud_check(ws, files, env):
    proc = subprocess.run([jud_bin(), "check", *files], cwd=ws, env=env, capture_output=True, text=True)
    return proc.returncode, proc.stdout + proc.stderr


def fingerprints(ws, files, env):
    """`jud check`'s fingerprints: questions per rubric, cases per cases document."""
    _, out = jud_check(ws, files, env)
    fps, current = {}, None
    for line in out.splitlines():
        m = re.match(r"(rubric|cases)\s+(\S+): ", line)
        if m:
            current = m.group(2)
            continue
        m = re.match(r"\s+(questions|cases)\s+(sha256:\w+)", line)
        if m and current:
            fps.setdefault(current, {})[m.group(1)] = m.group(2)
    return fps


def get_path(doc, dotted):
    cur = doc
    for part in dotted.split("."):
        if not isinstance(cur, dict) or part not in cur:
            return KeyError
        cur = cur[part]
    return cur


def comment_lines(path):
    try:
        with open(path, encoding="utf-8") as f:
            return [l.strip() for l in f if l.strip().startswith("#")]
    except FileNotFoundError:
        return []


def setup_workspace(spec, ws, env, logdir):
    for dest, src in spec.get("copy", {}).items():
        src = os.path.join(REPO, src)
        target = os.path.join(ws, dest)
        if dest.endswith("/"):
            shutil.copytree(src, target)
        else:
            os.makedirs(os.path.dirname(target) or ws, exist_ok=True)
            shutil.copy(src, target)
    rec = spec.get("record")
    if rec:
        extra = {"fail_at": rec["fail_at"]} if "fail_at" in rec else None
        proc, url = start_mock(ws, [rec["cases"]], rec.get("profile"), os.path.join(logdir, "setup-mock.jsonl"), extra)
        try:
            run_env = dict(env, TYPESAFE_BASE_URL=url, TYPESAFE_API_KEY="mock-key")
            subprocess.run([jud_bin(), "record", rec["rubric"], rec["cases"], "--out", rec["out"]],
                           cwd=ws, env=run_env, capture_output=True, text=True)
        finally:
            proc.kill()
    # A real project is a repository: commit what the user had before the
    # edits, so `git diff` shows what changed since.
    git = ["git", "-c", "user.name=eval", "-c", "user.email=eval@example.com"]
    subprocess.run(["git", "init", "-q"], cwd=ws, check=True)
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


def run_one(ev, out_dir, model):
    eid = ev["id"]
    base = os.path.join(out_dir, eid)
    shutil.rmtree(base, ignore_errors=True)
    ws = os.path.join(base, "ws")
    os.makedirs(ws)
    env = dict(os.environ)
    env["XDG_CONFIG_HOME"] = os.path.join(ws, ".config")
    env["TYPESAFE_API_KEY"] = "mock-key"
    env["TYPESAFE_BASE_URL"] = "http://127.0.0.1:9"  # closed: nothing reaches a real API
    env.pop("JUD_REPLAY", None)
    tmp = os.path.join(base, "tmp")
    os.makedirs(tmp)
    env["TMPDIR"] = tmp
    env["PATH"] = os.path.dirname(jud_bin()) + os.pathsep + env["PATH"]
    setup_workspace(ev.get("setup", {}), ws, env, base)

    mock_log = os.path.join(base, "mock.jsonl")
    open(mock_log, "w").close()
    proc = None
    if "mock" in ev:
        proc, url = start_mock(ws, ev["mock"].get("cases", []), ev["mock"].get("profile"), mock_log)
        env["TYPESAFE_BASE_URL"] = url
    for name in ev.get("env", {}).get("unset", []):
        env.pop(name, None)

    before = snapshot(ws)
    comments_before = {f: comment_lines(os.path.join(ws, f)) for f in [ev.get("checks", {}).get("comments_kept")] if f}
    q_before = {}
    if "questions_unchanged" in ev.get("checks", {}):
        f = ev["checks"]["questions_unchanged"]
        q_before = fingerprints(ws, [f], env)

    cmd = ["claude", "-p", ev["prompt"], "--plugin-dir", PLUGIN, "--permission-mode", "acceptEdits",
           "--output-format", "stream-json", "--verbose", "--strict-mcp-config", "--add-dir", tmp, "--allowedTools", *ALLOWED]
    if model:
        cmd += ["--model", model]
    started = time.time()
    with open(os.path.join(base, "transcript.jsonl"), "w", encoding="utf-8") as t:
        result = subprocess.run(cmd, cwd=ws, env=env, stdout=t, stderr=subprocess.PIPE, text=True, timeout=1800)
    elapsed = time.time() - started
    if proc:
        proc.kill()

    reply, cost, turns, denials, skills, tools = "", None, None, [], [], []
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
                        tools.append({"name": block["name"], "input": block.get("input")})
                        if block["name"] == "Skill":
                            skills.append(block.get("input", {}).get("skill"))
                        if block["name"] == "Read" and str(block.get("input", {}).get("file_path", "")).endswith("SKILL.md"):
                            skills.append(os.path.basename(os.path.dirname(block["input"]["file_path"])))
    with open(os.path.join(base, "reply.md"), "w", encoding="utf-8") as f:
        f.write(reply)

    after = snapshot(ws)
    checks = []

    def check(name, ok, detail=""):
        checks.append({"check": name, "ok": bool(ok), "detail": detail})

    spec = ev.get("checks", {})
    for pattern in spec.get("unchanged", []):
        files = [p for p in set(before) | set(after) if fnmatch.fnmatch(p, pattern)]
        moved = [p for p in files if before.get(p) != after.get(p)]
        check(f"unchanged {pattern}", not moved, ", ".join(sorted(moved)[:5]))
    new_files = sorted(set(after) - set(before))
    stray = [f for f in new_files if not any(fnmatch.fnmatch(f, g) for g in spec.get("new_files", []))]
    check("no stray files", not stray, ", ".join(stray[:8]))
    for path in spec.get("changed", []):
        check(f"changed {path}", before.get(path) != after.get(path))
    for item in spec.get("exists", []):
        n = len(glob.glob(os.path.join(ws, item["glob"])))
        check(f"exists {item['glob']} == {item['count']}", n == item["count"], f"found {n}")
    for files in spec.get("jud_check", []):
        code, out = jud_check(ws, files, env)
        check(f"jud check {' '.join(files)}", code == 0 and " 0 refused" in out, out.strip().splitlines()[-1] if out.strip() else "")
    if "questions_unchanged" in spec:
        f = spec["questions_unchanged"]
        q_after = fingerprints(ws, [f], env)
        b = [v.get("questions") for v in q_before.values()]
        a = [v.get("questions") for v in q_after.values()]
        check(f"questions fingerprint of {f} unchanged", b and b == a, f"{b} -> {a}")
    if "comments_kept" in spec:
        f = spec["comments_kept"]
        now = comment_lines(os.path.join(ws, f))
        lost = [c for c in comments_before.get(f, []) if c not in now]
        check(f"comments of {f} kept", not lost, "; ".join(lost[:3]))
    for item in spec.get("yaml", []):
        try:
            with open(os.path.join(ws, item["file"]), encoding="utf-8") as f:
                doc = yaml.safe_load(f)
            value = get_path(doc, item["path"])
        except Exception as e:  # noqa: BLE001 - a broken file is a failed check
            check(f"yaml {item['file']} {item['path']}", False, str(e))
            continue
        op, want = item["op"], item.get("value")
        if op == "exists":
            ok = value is not KeyError
        elif op == "eq":
            ok = value == want
        elif op == "ne":
            ok = value is not KeyError and value != want
        elif op == "cases_fingerprint":
            fps = fingerprints(ws, [want], env)
            fp = next((v.get("cases") for v in fps.values()), None)
            ok, want = value == fp, fp
        else:
            ok = False
        check(f"yaml {item['file']} {item['path']} {op} {want}", ok, f"is {value if value is not KeyError else 'absent'}")
    if "split_disjoint" in spec:
        tune_f, hold_f, full_f = spec["split_disjoint"]
        try:
            ids = []
            for f in (tune_f, hold_f, full_f):
                with open(os.path.join(ws, f), encoding="utf-8") as fh:
                    ids.append([c.get("id") for c in yaml.safe_load(fh)["spec"]["cases"]])
            t, h, full = ids
            ok = not (set(t) & set(h)) and sorted(t + h) == sorted(full)
            check("split is disjoint and complete", ok, f"tune {len(t)}, holdout {len(h)}, full {len(full)}")
        except Exception as e:  # noqa: BLE001
            check("split is disjoint and complete", False, str(e))
    if "mock_requests" in spec:
        n = count_lines(mock_log)
        check(f"mock requests == {spec['mock_requests']}", n == spec["mock_requests"], f"made {n}")
    if "skill_loaded" in spec:
        check(f"skill {spec['skill_loaded']} loaded", any(s and spec["skill_loaded"] in s for s in skills), ", ".join(map(str, skills)))
    # A denied `jud` command means a command page taught a form the user's
    # permission rules do not match (a shell variable, a `cd` first). Other
    # denials (a guessed path outside the workspace) are noted, not failed.
    runs_jud = re.compile(r"jud\.sh\b|(^|[;&|(]\s*)jud\s")
    jud_denied = [d for d in denials if runs_jud.search(str(d.get("tool_input", {}).get("command", "")))]
    check("no jud command denied", not jud_denied, json.dumps(jud_denied)[:300])

    summary = {"denials": [json.dumps(d.get("tool_input", {}))[:200] for d in denials], "id": eid, "passed": sum(c["ok"] for c in checks), "total": len(checks),
               "cost_usd": cost, "turns": turns, "seconds": round(elapsed), "exit": result.returncode,
               "stderr": result.stderr[-500:], "skills": skills, "checks": checks,
               "tools": [t["name"] if t["name"] != "Bash" else "Bash: " + str(t["input"].get("command", ""))[:160] for t in tools]}
    with open(os.path.join(base, "checks.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", required=True)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--model", help="passed to claude --model")
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
            failed = [c["check"] + (f" ({c['detail']})" if c["detail"] else "") for c in r["checks"] if not c["ok"]]
            print(f"{r['id']}: {r['passed']}/{r['total']} checks"
                  + (f", ${r['cost_usd']:.2f}, {r['seconds']}s" if r.get("cost_usd") is not None else "")
                  + (f", error {r['error']}" if "error" in r else ""), flush=True)
            for item in failed:
                print(f"  FAIL {item}", flush=True)
    results.sort(key=lambda r: r["id"])
    with open(os.path.join(args.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
