#!/usr/bin/env python3
"""A System One server over open-weight decision models, for the crate's live tests.

Serves the two endpoints the judgment crate calls, ``POST /v1/systemone``
and ``GET /v1/models``, over one of two backends, so the crate's live tests
and examples, and any System One client pointed here, run unchanged against
a model that does not serve the wire at that path itself.

``laya``
    Laya (https://huggingface.co/convaiinnovations/laya, Apache 2.0) in this
    process: one or more checkpoints, the request's ``model`` choosing
    which. Laya ships its own server, ``laya-serve`` (``pip install
    "laya[serve]"``), which is what docs/verification/laya-typed-decisions.md
    runs the crate against; prefer it. This backend stays for the one path
    laya-serve does not serve, ``GET /v1/models``, and as the smallest
    reference of the wire.

``cloudflare``
    Cloudflare's Clef (27B) and Clef-flash (9B) on Workers AI
    (https://developers.cloudflare.com/workers-ai/models/clef/, weights
    Apache 2.0). Workers AI takes the System One body at one exact URL per
    model, ``.../accounts/{account}/ai/run/@cf/cloudflare/{model}``, with no
    ``/v1/systemone`` path and no model list, and answers inside
    Cloudflare's ``{"result": ..., "success": ..., "errors": [...]}``
    envelope. This backend maps the path, picks the URL from the request's
    ``model``, unwraps the envelope, turns an envelope error into the
    ``{"error": {"message", "type"}}`` body the crate reads, and forwards
    ``Retry-After`` and the ``cf-ray`` id (as ``x-typesafe-request-id``).
    The Cloudflare token comes from the environment, never from the
    client's bearer.

From the crate's root directory::

    # Laya, in a virtualenv with torch and laya
    python -m venv .venv && .venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu
    .venv/bin/pip install laya
    USE_TF=0 .venv/bin/python tools/systemone/serve.py laya --models typed-decisions,english

    # Clef on Workers AI: the standard library is enough
    CLOUDFLARE_ACCOUNT_ID=... CLOUDFLARE_API_TOKEN=... python3 tools/systemone/serve.py cloudflare

    # then, against either (mise run live:clef does this for the second)
    JUDGMENT_LIVE_BASE_URL=http://127.0.0.1:8099 JUDGMENT_LIVE_MODEL=clef \\
      cargo test -p judgment --test live -- --ignored --nocapture --test-threads=1

A request for a model the backend does not list is answered by its first
model, as laya-serve routes an unknown name, so a client built for the
hosted API (``jev-latest``) is not refused; the response names the model
that answered. The bearer a client sends is accepted and ignored unless
``--api-key`` is set, in which case it must match (a 401 otherwise), which
is what the crate's bearer test needs. A request Laya raises on is a 400,
and one Workers AI refuses keeps its status, both with ``{"error":
{"message", "type"}}``, which the crate reads as ``Error::InvalidRequest``
and does not retry; an unreachable upstream is a 502 and a slow one a 504,
which it does retry. Not a product: no TLS, one process, no batching across
requests.
"""
import argparse
import json
import os
import socket
import sys
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CLOUDFLARE_API_BASE = "https://api.cloudflare.com/client/v4"
CLOUDFLARE_MODEL_PREFIX = "@cf/cloudflare/"


def log(message):
    print(message, file=sys.stderr, flush=True)


def error_body(message, kind):
    """The error shape the crate reads a message and a kind from."""
    return {"error": {"message": message, "type": kind}}


class Backend:
    """A list of models, the first the default, and a decision per request."""

    models = ()

    def resolve(self, requested):
        return requested if requested in self.models else self.models[0]

    def model_list(self):
        return {
            "models": [
                {"name": name, "description": self.describe(name), "release_date": self.released(name)}
                for name in self.models
            ]
        }

    def describe(self, name):
        raise NotImplementedError

    def released(self, name):
        raise NotImplementedError

    def decide(self, request):
        """The status, body and extra headers to answer `request` with."""
        raise NotImplementedError


class Laya(Backend):
    """Laya checkpoints in this process, loaded on first use."""

    def __init__(self, repo, models):
        os.environ.setdefault("USE_TF", "0")  # transformers may deadlock probing TensorFlow
        import laya  # noqa: PLC0415  (only this backend needs torch)

        self.laya = laya
        self.repo = repo
        self.models = tuple(models)
        self.agents = {}

    def describe(self, name):
        return f"Laya {name} checkpoint ({self.repo}), in this process"

    def released(self, name):
        return "2026-09-20"

    def agent(self, name):
        if name not in self.agents:
            started = time.time()
            if name == "english":
                self.agents[name] = self.laya.load(self.repo)
            else:
                self.agents[name] = self.laya.load(self.repo, subfolder=name)
            log(f"loaded laya:{name} in {time.time() - started:.1f}s")
        return self.agents[name]

    def decide(self, request):
        name = self.resolve(request.get("model"))
        try:
            result = self.agent(name).predict(request["state"], request["questions"])
        except Exception as e:  # noqa: BLE001  (a 400 is not retried by the client)
            return 400, error_body(str(e), "invalid_request"), {}
        result["model"] = f"laya:{name}"
        result.setdefault("usage", {"input_tokens": 0, "output_tokens": 0})
        return 200, result, {}


class Cloudflare(Backend):
    """Clef on Workers AI: one URL per model, Cloudflare's envelope around the wire."""

    def __init__(self, account, token, models, base, timeout):
        self.account = account
        self.token = token
        self.models = tuple(models)
        self.base = base.rstrip("/")
        self.timeout = timeout

    def describe(self, name):
        return f"Cloudflare {name} on Workers AI ({CLOUDFLARE_MODEL_PREFIX}{name})"

    def released(self, name):
        return "2026-10-01"

    def url(self, name):
        return f"{self.base}/accounts/{self.account}/ai/run/{CLOUDFLARE_MODEL_PREFIX}{name}"

    def decide(self, request):
        name = self.resolve(request.get("model"))
        body = dict(request)
        body["model"] = name
        upstream = urllib.request.Request(
            self.url(name),
            data=json.dumps(body).encode(),
            method="POST",
            headers={
                "Authorization": f"Bearer {self.token}",
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
        )
        try:
            with urllib.request.urlopen(upstream, timeout=self.timeout) as response:
                status, text, headers = response.status, response.read().decode("utf-8", "replace"), response.headers
        except urllib.error.HTTPError as e:
            status, text, headers = e.code, e.read().decode("utf-8", "replace"), e.headers
        except (TimeoutError, socket.timeout):
            return 504, error_body(f"Workers AI did not answer within {self.timeout:g} s", "upstream_timeout"), {}
        except urllib.error.URLError as e:
            return 502, error_body(f"Workers AI unreachable: {e.reason}", "upstream_unreachable"), {}

        extra = {}
        if headers.get("Retry-After"):
            extra["Retry-After"] = headers["Retry-After"]
        if headers.get("cf-ray"):
            extra["x-typesafe-request-id"] = headers["cf-ray"]

        try:
            parsed = json.loads(text)
        except ValueError:
            kind = "upstream_not_json"
            return (status if status >= 400 else 502), error_body(text[:500], kind), extra

        enveloped = isinstance(parsed, dict) and "success" in parsed and ("result" in parsed or "errors" in parsed)
        if not enveloped:
            return status, parsed, extra
        if parsed.get("success") and isinstance(parsed.get("result"), dict):
            return status, parsed["result"], extra
        errors = [e for e in parsed.get("errors") or [] if isinstance(e, dict)]
        message = "; ".join(str(e.get("message")) for e in errors if e.get("message")) or text[:500]
        code = errors[0].get("code") if errors else None
        kind = f"cloudflare_{code}" if code is not None else "cloudflare_error"
        return (status if status >= 400 else 502), error_body(message, kind), extra


class Handler(BaseHTTPRequestHandler):
    backend = None
    api_key = None

    def _send(self, code, body, headers=None):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(data)

    def _authorized(self):
        if self.api_key is None:
            return True
        return self.headers.get("Authorization") == f"Bearer {self.api_key}"

    def _route(self):
        return self.path.split("?", 1)[0].rstrip("/")

    def do_GET(self):
        if not self._authorized():
            return self._send(401, error_body("invalid bearer token", "unauthorized"))
        if self._route() == "/v1/models":
            return self._send(200, self.backend.model_list())
        return self._send(404, error_body("not found", "not_found"))

    def do_POST(self):
        if not self._authorized():
            return self._send(401, error_body("invalid bearer token", "unauthorized"))
        if self._route() != "/v1/systemone":
            return self._send(404, error_body("not found", "not_found"))
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b""
        try:
            request = json.loads(raw or b"{}")
        except ValueError as e:
            return self._send(400, error_body(f"body is not JSON: {e}", "invalid_request"))
        if not isinstance(request, dict):
            return self._send(400, error_body("body is not a JSON object", "invalid_request"))
        started = time.time()
        status, body, headers = self.backend.decide(request)
        questions = request.get("questions")
        count = len(questions) if isinstance(questions, dict) else 0
        log(f"systemone {count} questions for {request.get('model')!r}: {status} in {(time.time() - started) * 1000:.0f} ms")
        return self._send(status, body, headers)

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("backend", choices=["laya", "cloudflare"])
    parser.add_argument(
        "--models",
        help="comma-separated names the server lists, the first the default: Laya checkpoint subfolders "
        "with `english` for the base (default english), or clef,clef-flash (the default)",
    )
    parser.add_argument("--host", default=os.environ.get("SYSTEMONE_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("SYSTEMONE_PORT", "8099")))
    parser.add_argument(
        "--api-key",
        default=os.environ.get("SYSTEMONE_API_KEY"),
        help="require this bearer token from clients (default: accept any)",
    )
    parser.add_argument("--timeout", type=float, default=120.0, help="seconds per Workers AI call (default 120)")
    args = parser.parse_args()

    if args.backend == "laya":
        models = (args.models or "english").split(",")
        backend = Laya(os.environ.get("LAYA_REPO", "convaiinnovations/laya"), models)
    else:
        account = os.environ.get("CLOUDFLARE_ACCOUNT_ID")
        token = os.environ.get("CLOUDFLARE_API_TOKEN")
        if not account or not token:
            parser.error("the cloudflare backend needs CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN in the environment")
        models = (args.models or "clef,clef-flash").split(",")
        backend = Cloudflare(account, token, models, os.environ.get("CLOUDFLARE_API_BASE", CLOUDFLARE_API_BASE), args.timeout)

    Handler.backend = backend
    Handler.api_key = args.api_key
    log(f"serving {args.backend} ({', '.join(backend.models)}) on http://{args.host}:{args.port}")
    ThreadingHTTPServer((args.host, args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
