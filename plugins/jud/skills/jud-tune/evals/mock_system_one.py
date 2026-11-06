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
     "collapse": {"desk": "technical"},      # give this answer on every case
                                             # (a case's own `answer` wins)
     "confidence": {"tone": [0.2, 0.35]},    # the confidence a Choice or Score
                                             # reports, whatever it answers (the
                                             # probabilities are left as they are)
     "fail_at": 3,                           # answer the 3rd request with 400
     "flip_on": {"swapped": 0.1, "toned": 0.3, "injected": 0.5}}
                                             # on a perturbed request, pick the
                                             # other option with this probability

`collapse` and `confidence` imitate what a small real model did on a real
rubric: one option for nearly every state, and a Score always below its bar.

A perturbed request is one `jud eval --swap`, `--tone-perturb` or
`--injection-perturb` makes: the state is a case's, and a Choice's options
differ from the ones the case supplies under `options`. The mock tells
them apart by comparing the request's criteria with the case's options
(reversed order: swapped; the labelled option's text changed: toned; a
losing option's text with the original as its prefix: injected), or by a
`.swapped` question id under `--swap=joint`. Only a Choice whose options
the case supplies is seen this way; a static Choice reads as unperturbed.
The pick is the one the original request gets, flipped to the other
option with `flip_on`'s probability for that kind, so a scenario can plant
a flip rate and an attack success and hold a reply to it.

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
            options = case.get("options") or {}
            truths[canonical(state)] = (name, expect, None, options)
            if isinstance(state, list):
                for k in range(1, len(state) + 1):
                    truths.setdefault(canonical(state[:k]), (name, expect, k - 1, options))
                    # The full conversation keeps its own entry, read as the last turn.
                truths[canonical(state)] = (name, expect, len(state) - 1, options)
    return truths


def perturbation(qid, question, options, expect):
    """How a request's question differs from the case's own: `swapped`,
    `toned`, `injected`, or None. Read off the options the case supplies."""
    if qid.endswith(".swapped"):
        return "swapped"
    if question.get("type") != "choice":
        return None
    base = options.get(qid) or {}
    criteria = question.get("criteria") or {}
    supplied = [k for k in criteria if k in base]
    if len(supplied) < 2:
        return None
    if supplied == list(reversed(list(base))) and supplied != list(base):
        return "swapped"
    label = str(expect.get(qid))
    for k in supplied:
        if criteria[k] == base[k]:
            continue
        if str(k) == label:
            return "toned"
        if isinstance(criteria[k], str) and isinstance(base[k], str) and criteria[k].startswith(base[k]):
            return "injected"
        return "toned"
    return None


def flipped(ans, question, rng):
    """The same Choice answer with its pick moved to another option."""
    keys = list(question["criteria"].keys())
    others = [k for k in keys if k != ans["choice"]]
    if not others:
        return ans
    chosen = rng.choice(others)
    conf = ans["probabilities"][ans["choice"]]
    probs = spread(keys, chosen, conf, None, rng)
    reported = ans["confidence"] if ans["confidence"] != ans["probabilities"][ans["choice"]] else probs[chosen]
    return {"type": "choice", "choice": chosen, "probabilities": probs, "confidence": reported}


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


def answer(question, truth, accuracy, rng, confident_wrong, forced, conf_range=None):
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
        reported = round(rng.uniform(*conf_range), 3) if conf_range else probs[chosen]
        return {"type": "choice", "choice": chosen, "probabilities": probs, "confidence": reported}

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
    reported = round(rng.uniform(*conf_range), 3) if conf_range else probs[str(idx)]
    return {"type": "score", "score": score, "legend": legend, "probabilities": probs, "confidence": reported}


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
            name, expect, turn, options = truths.get(canonical(state), (None, {}, None, {}))
            kinds = {qid: perturbation(qid, q, options, expect) for qid, q in body["questions"].items()}
            if args.log:
                with open(args.log, "a", encoding="utf-8") as f:
                    f.write(json.dumps({"n": count["n"], "case": name, "turn": turn, "questions": list(body["questions"]),
                                        "perturbed": {k: v for k, v in kinds.items() if v}}) + "\n")
            if profile.get("fail_at") == count["n"]:
                return self.send(400, {"detail": [{"loc": ["body", "state"], "msg": "mock refusal", "type": "value_error"}]})
            answers = {}
            for qid, question in body["questions"].items():
                base_qid = qid[: -len(".swapped")] if qid.endswith(".swapped") else qid
                # Seeded on the case's own question, so a perturbed request
                # gets the original's pick before any flip.
                rng = random.Random(hashlib.sha256(f"{seed}|{name}|{base_qid}|{turn}".encode()).hexdigest())
                accuracy = profile.get("per_question", {}).get(base_qid, profile.get("accuracy", 0.85))
                forced = profile.get("answer", {}).get(name or "", {}).get(base_qid)
                if forced is None:
                    forced = profile.get("collapse", {}).get(base_qid)
                truth = truth_of(question, expect.get(base_qid), turn)
                wrong_confident = base_qid in profile.get("confident_wrong", [])
                conf_range = profile.get("confidence", {}).get(base_qid)
                ans = answer(question, truth, accuracy, rng, wrong_confident, forced, conf_range)
                kind = kinds[qid]
                if kind and ans["type"] == "choice":
                    flip_rng = random.Random(hashlib.sha256(f"{seed}|{name}|{base_qid}|{kind}".encode()).hexdigest())
                    if flip_rng.random() < profile.get("flip_on", {}).get(kind, 0.0):
                        ans = flipped(ans, question, flip_rng)
                answers[qid] = ans
            self.send(200, {"model": model, "answers": answers, "usage": {"input_tokens": 50, "output_tokens": 5}})

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"mock System One on http://127.0.0.1:{args.port}, {len(truths)} states, model {model}", file=sys.stderr, flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
