#!/usr/bin/env python3
"""A System One server that answers like an imperfect model, for testing the
tuning loop with no key and no network.

    mock_system_one.py --port 8765 --cases cases.jud [--cases more.jud]
                       [--profile profile.json] [--log requests.jsonl]

It serves `POST /v1/systemone` and `GET /v1/models`. Each request's state is
looked up among the cases given (a conversation also by every prefix of its
turns, as `jud record` sends one turn at a time); the label of the matching
case is taken as the truth, and the answer is right with the profile's
accuracy, deterministically for a given seed, case, question and turn. A
right answer comes with a high probability and a wrong one with a middling
one, so a confidence bar has something to separate. A state that matches no
case, or a question the case does not label, is answered without a truth.

The profile (JSON, every key optional):

    {"model": "mock-jev-1", "seed": "a",
     "accuracy": 0.85,                       # every question
     "per_question": {"desk": 0.6},          # one question
     "answer": {"receipt": {"desk": "billing"}},  # force an answer: a case id,
                                             # a question, an option key, a
                                             # level's text, true or false
     "confident_wrong": ["desk"],            # wrong answers on these questions
                                             # come with high confidence
     "fail_at": 3}                           # answer the 3rd request with 400

This is a test double, not a model: it knows the labels. It exists so that
`jud record`, `jud eval` and `jud tune` (and the plugin commands that drive
them) can be exercised end to end; nothing it produces says anything about a
real model.
"""

import argparse
import hashlib
import json
import random
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import yaml


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def load_truths(paths):
    """Map each state (and each prefix of a conversation) to its case."""
    truths = {}
    for path in paths:
        with open(path, encoding="utf-8") as f:
            doc = yaml.safe_load(f)
        for i, case in enumerate(doc.get("spec", {}).get("cases", [])):
            name = case.get("id") or f"case-{i + 1}"
            state = case.get("state")
            expect = case.get("expect") or {}
            truths[canonical(state)] = (name, expect, None)
            if isinstance(state, list):
                for k in range(1, len(state) + 1):
                    truths.setdefault(canonical(state[:k]), (name, expect, k - 1))
                    # The full conversation keeps its own entry, read as the last turn.
                truths[canonical(state)] = (name, expect, len(state) - 1)
    return truths


def truth_of(question, label, turn):
    """The label as an answer at this turn: a bool, an option key or a level index."""
    if label is None:
        return None
    kind = question["type"]
    if kind == "noul":
        if isinstance(label, dict) and "from_turn" in label:
            at = label["from_turn"]
            return at is not None and turn is not None and turn >= at
        return bool(label)
    if kind == "choice":
        return str(label)
    levels = question["criteria"]
    if isinstance(label, int):
        return label
    return levels.index(label) if label in levels else None


def answer(question, truth, accuracy, rng, confident_wrong, forced):
    kind = question["type"]
    right = rng.random() < accuracy
    if kind == "noul":
        target = forced if forced is not None else truth
        if target is None:
            return {"type": "noul", "noul": round(rng.uniform(0.2, 0.8), 3)}
        if forced is not None or right:
            p = rng.uniform(0.62, 0.98)
        else:
            p = rng.uniform(0.8, 0.95) if confident_wrong else rng.uniform(0.3, 0.49)
            target = not target
        return {"type": "noul", "noul": round(p if target else 1 - p, 3)}

    if kind == "choice":
        keys = list(question["criteria"].keys())
        if forced is not None and str(forced) in keys:
            chosen, conf = str(forced), rng.uniform(0.55, 0.9)
        elif truth in keys and right:
            chosen, conf = truth, rng.uniform(0.6, 0.97)
        else:
            others = [k for k in keys if k != truth] or keys
            chosen = rng.choice(others)
            conf = rng.uniform(0.8, 0.93) if confident_wrong else rng.uniform(0.3, 0.55)
        probs = spread(keys, chosen, conf, truth, rng)
        return {"type": "choice", "choice": chosen, "probabilities": probs, "confidence": probs[chosen]}

    levels = question["criteria"]
    n = len(levels)
    if forced is not None:
        idx = levels.index(forced) if forced in levels else int(forced)
        conf = rng.uniform(0.55, 0.9)
    elif truth is not None and right:
        idx, conf = truth, rng.uniform(0.6, 0.95)
    else:
        base = truth if truth is not None else n // 2
        idx = min(n - 1, base + 1) if base == 0 or rng.random() < 0.5 else base - 1
        conf = rng.uniform(0.8, 0.9) if confident_wrong and truth is not None else rng.uniform(0.5, 0.6)
    keys = [str(i) for i in range(n)]
    probs = spread(keys, str(idx), conf, str(truth) if truth is not None else None, rng, neighbours=True)
    score = round(sum(int(k) * p for k, p in probs.items()), 4)
    legend = {str(i): level for i, level in enumerate(levels)}
    return {"type": "score", "score": score, "legend": legend, "probabilities": probs, "confidence": probs[str(idx)]}


def spread(keys, chosen, conf, truth, rng, neighbours=False):
    """`conf` on the chosen key; the rest mostly on the truth (or neighbours)."""
    probs = {k: 0.0 for k in keys}
    probs[chosen] = conf
    rest = 1.0 - conf
    if neighbours:
        i = int(chosen)
        near = [str(j) for j in (i - 1, i + 1) if 0 <= j < len(keys)]
        targets = near or [k for k in keys if k != chosen]
    else:
        targets = [k for k in keys if k != chosen]
    if truth is not None and truth != chosen and truth in targets:
        probs[truth] += rest * 0.8
        rest *= 0.2
        targets = [k for k in targets if k != truth] or [truth]
    for k in targets:
        probs[k] += rest / len(targets)
    # Round to three places and put the rounding error on the chosen key.
    probs = {k: round(v, 3) for k, v in probs.items()}
    probs[chosen] = round(probs[chosen] + 1.0 - sum(probs.values()), 3)
    return probs


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--cases", action="append", default=[], help="a Cases document; repeatable")
    parser.add_argument("--profile", help="a JSON profile (see the module docstring)")
    parser.add_argument("--log", help="append each request to this JSONL file")
    args = parser.parse_args()

    profile = {}
    if args.profile:
        with open(args.profile, encoding="utf-8") as f:
            profile = json.load(f)
    truths = load_truths(args.cases)
    model = profile.get("model", "mock-jev-1")
    seed = str(profile.get("seed", "a"))
    count = {"n": 0}

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def send(self, status, body):
            data = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path.rstrip("/").endswith("/v1/models"):
                self.send(200, {"data": [{"id": model, "object": "model"}]})
            else:
                self.send(404, {"detail": "Not Found"})

        def do_POST(self):
            if not self.path.rstrip("/").endswith("/v1/systemone"):
                return self.send(404, {"detail": "Not Found"})
            body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))))
            count["n"] += 1
            state = body.get("state")
            name, expect, turn = truths.get(canonical(state), (None, {}, None))
            if args.log:
                with open(args.log, "a", encoding="utf-8") as f:
                    f.write(json.dumps({"n": count["n"], "case": name, "turn": turn, "questions": list(body["questions"])}) + "\n")
            if profile.get("fail_at") == count["n"]:
                return self.send(400, {"detail": [{"loc": ["body", "state"], "msg": "mock refusal", "type": "value_error"}]})
            answers = {}
            for qid, question in body["questions"].items():
                rng = random.Random(hashlib.sha256(f"{seed}|{name}|{qid}|{turn}".encode()).hexdigest())
                accuracy = profile.get("per_question", {}).get(qid, profile.get("accuracy", 0.85))
                forced = profile.get("answer", {}).get(name or "", {}).get(qid)
                truth = truth_of(question, expect.get(qid), turn)
                wrong_confident = qid in profile.get("confident_wrong", [])
                answers[qid] = answer(question, truth, accuracy, rng, wrong_confident, forced)
            self.send(200, {"model": model, "answers": answers, "usage": {"input_tokens": 50, "output_tokens": 5}})

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"mock System One on http://127.0.0.1:{args.port}, {len(truths)} states, model {model}", file=sys.stderr, flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
