#!/usr/bin/env python3
"""Run one task of the jud authoring skill's eval set (evals.json) through
`claude -p`, with the skill (`--plugin-dir plugins/jud`) or without it, in a
fresh workspace holding the task's files; then run `jud check` on every
document the run wrote and, for the image task (6), check its assertions
against the parsed documents. The other tasks' expectations are graded by a
reader from the reply and the documents, as the guide describes.

    plugins/jud/skills/jud/evals/run.py --task 6 --mode with --out DIR [--model M]
    plugins/jud/skills/jud/evals/run.py --task 6 --mode without --out DIR

`--mode without` is the baseline: an agent with the specification, the
examples and the `jud` command, and no skill. It runs claude in a
configuration directory of its own under DIR (the account's
`.credentials.json` copied in from `$CLAUDE_CONFIG_DIR` or `~/.claude`), so
a plugin installed at user scope does not load; the workspace is outside
any project, so a project's plugins do not either. The checkout itself
stays readable, and an agent that searches the disk finds the skill's page
in it (the 2026-10-15 run did): the baseline is the same text found as a
file, not the absence of it. Needs `claude` (logged
in) and `jud` on PATH (or JUD=path). Nothing calls a model API but claude
itself: `jud check` reads documents and asks nothing. Writes DIR/ws/ (the
workspace, with the documents), transcript.jsonl, reply.md and checks.json.
"""

import argparse
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
    "Bash(ls:*)", "Bash(mkdir:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(grep:*)", "Bash(find:*)", "Bash(python3:*)",
]


def load_yaml(path):
    import yaml  # PyYAML, for the checks only; claude and jud need none

    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f)


def run_claude(prompt, ws, mode, model, config_dir, transcript):
    env = dict(os.environ)
    jud = os.environ.get("JUD") or shutil.which("jud") or sys.exit("run.py: no jud on PATH; set JUD")
    env["PATH"] = os.path.dirname(os.path.abspath(jud)) + os.pathsep + env["PATH"]
    env.pop("JUD_REPLAY", None)
    if mode == "without":
        env["CLAUDE_CONFIG_DIR"] = config_dir
    cmd = ["claude", "-p", prompt, "--permission-mode", "acceptEdits", "--output-format", "stream-json",
           "--verbose", "--strict-mcp-config", "--add-dir", "/tmp", "--allowedTools", *ALLOWED]
    if mode == "with":
        cmd[3:3] = ["--plugin-dir", PLUGIN]
    if model:
        cmd += ["--model", model]
    started = time.time()
    with open(transcript, "w", encoding="utf-8") as t:
        subprocess.run(cmd, cwd=ws, env=env, stdout=t, stderr=subprocess.PIPE, text=True, timeout=3600)
    return time.time() - started


def read_transcript(path):
    reply, cost, turns, tools, skills = "", None, None, [], []
    with open(path, encoding="utf-8") as t:
        for line in t:
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("type") == "result":
                reply, cost, turns = msg.get("result", ""), msg.get("total_cost_usd"), msg.get("num_turns")
            if msg.get("type") == "assistant":
                for block in msg.get("message", {}).get("content", []):
                    if block.get("type") == "tool_use":
                        tools.append(block["name"])
                        inp = block.get("input", {})
                        if block["name"] == "Skill":
                            skills.append(inp.get("skill"))
                        if block["name"] == "Read" and str(inp.get("file_path", "")).endswith("SKILL.md"):
                            skills.append(os.path.basename(os.path.dirname(inp["file_path"])))
    return reply, cost, turns, tools, skills


def check_image_task(ws, reply, check):
    """Task 6's assertions, scripted: the two documents, their envelope, the
    Noul and the Box, the gate, the assets, the labels against the
    annotator's pixels, and what the reply says."""
    rubric_path, cases_path = os.path.join(ws, "labels.jud"), os.path.join(ws, "labels-cases.jud")
    check("wrote labels.jud and labels-cases.jud", os.path.exists(rubric_path) and os.path.exists(cases_path))
    if not (os.path.exists(rubric_path) and os.path.exists(cases_path)):
        return
    rubric, cases_doc = load_yaml(rubric_path), load_yaml(cases_path)
    for name, doc in (("rubric", rubric), ("cases", cases_doc)):
        meta = doc.get("metadata", {}) if isinstance(doc, dict) else {}
        check(f"{name}: apiVersion jud/v1, metadata.minor 4, a PascalCase kind",
              isinstance(doc, dict) and doc.get("apiVersion") == "jud/v1" and meta.get("minor") == 4 and doc.get("kind") in ("Rubric", "Cases"))
    questions = rubric.get("spec", {}).get("questions", {}) or {}
    nouls = [k for k, q in questions.items() if isinstance(q, dict) and q.get("type") == "noul"]
    boxes = [k for k, q in questions.items() if isinstance(q, dict) and q.get("type") == "box"]
    check("a noul and a box question; the box has instructions and no criteria",
          len(nouls) >= 1 and len(boxes) == 1 and questions[boxes[0]].get("instructions") and "criteria" not in questions[boxes[0]],
          f"nouls {nouls}, boxes {boxes}")
    if not boxes:
        return
    box_id = boxes[0]
    check("the box's instructions name the reference `photo`", "photo" in json.dumps(questions[box_id].get("instructions")))
    gate = (rubric.get("spec", {}).get("policy", {}) or {}).get(box_id, {}) or {}
    check("the box is gated by confidence or bands, never threshold or fallback",
          ("confidence" in gate or "bands" in gate) and "threshold" not in gate and "fallback" not in gate, json.dumps(gate))
    cases = cases_doc.get("spec", {}).get("cases", []) or []
    with open(os.path.join(ws, "images", "annotations.json"), encoding="utf-8") as f:
        annotated = json.load(f)["boxes"]

    def asset_path(case):
        state = case.get("state", {}) if isinstance(case.get("state"), dict) else {}
        asset = (case.get("assets", {}) or {}).get(state.get("photo", ""), {}) or {}
        return str(asset.get("path", ""))

    check("five cases, each state carrying `photo` and declaring its image under assets by a relative path",
          len(cases) == 5 and all(asset_path(c).startswith("images/") for c in cases), f"{len(cases)} cases")
    labelled = right = 0
    for case in cases:
        file_name = os.path.basename(asset_path(case))
        expect = case.get("expect", {}) or {}
        label = expect.get(box_id)
        if file_name in annotated:
            labelled += 1
            box = label.get("box") if isinstance(label, dict) else None
            pixels = annotated[file_name]
            want = {k: pixels[k] / 320 for k in ("top", "bottom", "left", "right")}
            if box and all(abs(float(box.get(k, 9)) - want[k]) <= 0.01 for k in want):
                right += 1
        else:
            check(f"{file_name}: the noul labelled false and the box unlabelled",
                  nouls and expect.get(nouls[0]) is False and label is None, json.dumps(expect))
    check("the three label cases label the box in fractions, within 0.01 of the annotator's pixels / 320",
          labelled == 3 and right == 3, f"{right} of {labelled}")
    check("the reply says a Box always answers with a box and that the Noul decides whether to use it",
          re.search(r"(always (answers|returns|gives|produces)|even when there is no|whether or not)[^.]*box|box[^.]*(always|even when there is no|whether or not)", reply, re.I)
          and re.search(r"\bnoul\b|yes/no question|whether", reply, re.I))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--task", type=int, required=True)
    parser.add_argument("--mode", choices=["with", "without"], required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--model")
    args = parser.parse_args()
    with open(os.path.join(HERE, "evals.json"), encoding="utf-8") as f:
        tasks = json.load(f)["evals"]
    task = next(t for t in tasks if t["id"] == args.task)
    out = os.path.abspath(args.out)
    shutil.rmtree(out, ignore_errors=True)
    ws = os.path.join(out, "ws")
    os.makedirs(ws)
    # The task's files go where the prompt says: FILES/ is a directory of
    # their own; a file the prompt places under OUT/ (the images) is copied
    # under the workspace at that path.
    files = task.get("files", [])
    for rel in files:
        src = os.path.join(HERE, "..", rel)
        dest_dir = os.path.join(ws, "images") if "/images/" in rel else os.path.join(ws, "files")
        os.makedirs(dest_dir, exist_ok=True)
        shutil.copy(src, dest_dir)
    prompt = task["prompt"].replace("OUT/", "").replace("FILES/", "files/")
    config_dir = os.path.join(out, "claude-config")
    if args.mode == "without":
        os.makedirs(config_dir)
        source = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
        shutil.copy(os.path.join(source, ".credentials.json"), config_dir)
    transcript = os.path.join(out, "transcript.jsonl")
    elapsed = run_claude(prompt, ws, args.mode, args.model, config_dir, transcript)
    reply, cost, turns, tools, skills = read_transcript(transcript)
    with open(os.path.join(out, "reply.md"), "w", encoding="utf-8") as f:
        f.write(reply)

    checks = []

    def check(name, ok, detail=""):
        checks.append({"check": name, "ok": bool(ok), "detail": str(detail)[:300]})

    written = sorted(p for p in os.listdir(ws) if p.endswith(".jud"))
    check("wrote .jud documents", bool(written), ", ".join(written))
    if written:
        jud = os.environ.get("JUD") or shutil.which("jud")
        proc = subprocess.run([jud, "check", *(os.path.join(ws, p) for p in written)], capture_output=True, text=True)
        check("jud check accepts every document written, 0 refused", proc.returncode == 0 and "0 refused" in proc.stdout,
              (proc.stdout[-500:] + proc.stderr[-200:]).strip())
    if args.task == 6:
        check_image_task(ws, reply, check)
    if args.mode == "with":
        check("the jud skill was loaded", any(s and "jud" in str(s) for s in skills), ", ".join(map(str, skills)))

    summary = {"task": args.task, "name": task["name"], "mode": args.mode, "passed": sum(c["ok"] for c in checks),
               "total": len(checks), "cost_usd": cost, "turns": turns, "seconds": round(elapsed),
               "tool_calls": len(tools), "skills": skills, "checks": checks}
    with open(os.path.join(out, "checks.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)
    print(f"task {args.task} {task['name']} {args.mode} the skill: {summary['passed']}/{summary['total']} checks"
          + (f", ${cost:.2f}, {turns} turns, {len(tools)} tool calls, {round(elapsed)}s" if cost is not None else ""))
    for c in checks:
        print(("  ok   " if c["ok"] else "  FAIL ") + c["check"] + (f"  [{c['detail']}]" if c["detail"] and not c["ok"] else ""))


if __name__ == "__main__":
    main()
