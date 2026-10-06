---
title: System One models on Google Cloud
description: A study, as of 2026-10-06, of how a System One model can be reached on Google Cloud (a compatible server on Cloud Run, an open-weights model on a Vertex AI endpoint, a partner model on Model Garden, a gateway in front of the hosted API), what the crate's client already does for each path and where it stops (the request path, a short-lived bearer, the model list), and how a caller attaches request labels in an HTTP header with the client as it is, with the vendor's rules for them and the changes to the crate that would make a Vertex AI endpoint a base URL and nothing more.
status: current
last_reviewed: 2026-10-06
tags: [research, judgment, typesafe, compatibility, google-cloud, vertex-ai, labels, headers]
---

# System One models on Google Cloud

The crate is a client to one wire, `POST /v1/systemone`, and its promise is that a server speaking it works unchanged behind a `base_url` and a `model`. [Compatible servers and models](compatible-servers-and-models.md) lists the servers that have been reached that way, hosted and local. None of them runs on Google Cloud. This page studies what it takes to reach a System One model that does: which shapes a Google Cloud deployment can have, what the client does with each one today, and how a caller sends request labels in an HTTP header on every call, which is the one thing a deployment on Vertex AI asks of a client that the hosted TypeSafe API does not.

It is a study, not a record of a run. Nothing here has been verified against a real endpoint; the last section says how it would be. The crate's documentation does not say what any one application should label or why; a label is a string the caller chooses, and this page covers only how it travels.

Sources, read on 2026-10-06: the TypeSafe [HTTP API](https://docs.typesafe.ai/api.md) and [models](https://docs.typesafe.ai/models.md) pages and the [documentation index](https://docs.typesafe.ai/llms.txt); the crate's own `src/client.rs` and `tools/systemone/serve.py`; and Google's pages on [partner models](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/partner-models/use-partner-models), [custom metadata labels on API calls](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/add-labels-to-api-calls), the [`rawPredict` method](https://docs.cloud.google.com/vertex-ai/docs/reference/rest/v1/projects.locations.endpoints/rawPredict) and [custom container requirements](https://docs.cloud.google.com/vertex-ai/docs/predictions/custom-container-requirements). Google's pages render their body in the browser and gave this environment their navigation and headings only, so the label header's name, encoding and limits below are what the vendor documents as relayed by a client that uses them, marked as such, and must be read off the pages themselves before anything is built on them.

## What "System One on Google Cloud" can mean

TypeSafe's documentation names one place its models are served: `POST /v1/systemone` at `https://api.typesafe.ai`, with "higher limits available on custom and enterprise plans". It names no cloud marketplace. Google's partner model catalogue lists Anthropic, xAI, Mistral AI, Meta, DeepSeek, OpenAI, Qwen, Z.ai, MiniMax and Moonshot; TypeSafe is not among them. So there is no managed Jev on Google Cloud today, and a System One model there is one of the following.

| Path | What runs | The URL a client posts to | The bearer | What the crate needs |
|---|---|---|---|---|
| **A** A compatible server on Cloud Run | Ollama 0.35 with `clef`, `clef-flash`, `nimble` or `tev1`; Decider; OpenDecider; `laya-serve`; or `tools/systemone/serve.py` in front of any of them. Each serves `/v1/systemone` itself. | `https://<service>.run.app/v1/systemone` | A Google ID token for the service's URL when the service requires IAM, any string otherwise | A `base_url`; a token that is refreshed |
| **B** An open-weights model on a Vertex AI endpoint | One of the same servers, packaged as a custom serving container and deployed to an endpoint (from Model Garden where the model is listed, or from the container registry). Vertex forwards a raw body to the container's predict route. | `https://<region>-aiplatform.googleapis.com/v1/projects/<project>/locations/<region>/endpoints/<id>:rawPredict` | An OAuth 2.0 access token with the `cloud-platform` scope, from Application Default Credentials, for a principal holding `aiplatform.endpoints.predict` | The path of the evaluate call; a token that is refreshed; no model list |
| **C** A partner model on Model Garden | A model Google serves on the publisher's behalf. None publishes a System One model. | `.../publishers/<publisher>/models/<model>:rawPredict` | As B | As B, when a publisher appears |
| **D** A gateway in Google Cloud in front of the hosted API | Cloud Run or Apigee forwarding `/v1/systemone` to `api.typesafe.ai`, holding the TypeSafe key itself | `https://<gateway>/v1/systemone` | What the gateway asks for | A `base_url`; nothing else |

A and D are the shapes the crate already serves: a base URL, and the client's bearer is whatever the server wants. B is the shape that needs work, and C is B with a publisher's path. The rest of this page is about B, with A as the way to verify most of it cheaply.

Two properties of `rawPredict` matter for the crate. The body is passed to the container as sent, so a container that speaks the wire answers in the wire's shape and the response needs no unwrapping; this is the difference from Cloudflare Workers AI, whose `{"result", "success", "errors"}` envelope is why `tools/systemone/serve.py cloudflare` exists. And the method is one URL per endpoint with a verb after a colon, so there is no `/v1/systemone` and no `/v1/models` under it: a request for the model list is a 404 or a 400 from Vertex, which the client already classifies as [`Error::Http`](../tour.md) and nothing else in the crate depends on.

## What the client does today, path by path

The client builds its URLs with `Url::join` on the base URL (`src/client.rs`, `Client::url`): `v1/systemone` for an evaluation and `v1/models` for the list. For A and D that is the whole configuration. For B it is the obstacle: the evaluate URL ends in `endpoints/<id>:rawPredict`, which is not a base to join `v1/systemone` onto, and a base URL that already names the method gets `v1/systemone` appended or substituted. Today B is reached the way Cloudflare is, through a shim that owns the mapping, or not at all.

The bearer is fixed when the client is built. [`ClientBuilder::build`](../tour.md) takes the key from the builder or from `TYPESAFE_API_KEY`, checks it with the Python SDK's rules (trimmed, non-blank, printable ASCII, no inner whitespace) and sends it as `Authorization: Bearer <key>` on every call. A Google access token or ID token passes those rules: it is ASCII without whitespace, and long keys are not refused. What does not fit is its life. Google's tokens expire after about an hour, and the client has no hook to replace one. The caller that holds a `Client` for longer either builds a new client when its token source rotates, which is cheap (a `reqwest::Client` and a parsed URL), or puts the credential in a sidecar that adds the header on the way out, as `serve.py` does for the Cloudflare token, "never from the client's bearer". A per-call `authorization` header is refused ([`Error::ReservedHeader`](../tour.md)) and should stay refused: a request that authenticates as another account is exactly what that rule exists to make impossible, and rotating a token is the builder's business, not a call's.

Everything above the URL and the bearer holds unchanged. [`Response::verify`](../design.md) checks a Vertex-served answer against the questions as it checks any other; an answer of a kind the crate does not know is tolerated and logged once; `Response::extra` keeps whatever the container adds at the top level, as it keeps Laya's `routing`. The request id is optional everywhere and absent from a Vertex response, which sends no `x-typesafe-request-id`; a caller that wants a handle on a failed call can read Google's own request headers through a sidecar, but the crate reads one vendor's header name and should keep reading one. Status codes come from Vertex before they come from the container: a 401 or 403 is Google's IAM answer and lands on [`Error::Unauthorized`](../tour.md) and [`Error::PermissionDenied`](../tour.md) with Google's message, a 429 on [`Error::RateLimited`](../tour.md) under the client's retry policy, and a 400 whose body is Google's `{"error": {"code", "message", "status"}}` rather than the wire's `{"error": {"message", "type"}}` decodes with whatever message the tolerant error reader finds; that shape is worth a probe when a real endpoint is available, as the three hosted 400 shapes were on [the hosted run](../verification/hosted-typesafe.md).

## Sending labels in a header

Google lets a caller attach a small set of key-value labels to a model call so that the call can be told apart afterwards, on Google's side, from every other call the same project made: which service made it, which workflow, which environment. The vendor page on custom metadata labels documents two transports, as relayed by the clients that use them: a `labels` object in the request body of `generateContent`, and an HTTP header, `X-Vertex-AI-Labels`, whose value is the JSON object of labels encoded in base64, for the methods whose body belongs to the model and cannot carry a Google field, `rawPredict` and `streamRawPredict` among them. Which of the two is honoured on a self-deployed endpoint's `rawPredict`, as against a partner model's, is the first thing to read off the page.

For this crate the header is the right transport whichever the endpoint honours, for a reason that has nothing to do with Google. The body of `POST /v1/systemone` is the wire's, and the response is verified against it. A label in the body would have to go through [`CallOptions::extra`](../implementation.md), which sends it as given, and the hosted API refuses an unknown top-level field with a 400 (verified on 2026-10-03), while a container passes it to the model, which may or may not ignore it; a label in a header touches neither the body nor the model. Headers also stop at the [`SystemOne`](../design.md) trait: they enter no recording and no [`request_hash`](../implementation.md), so a replay of a labelled call matches an unlabelled one, which is what a label that describes the caller rather than the question should do.

The client already carries headers at two levels, and labels fit them without a new API.

- [`ClientBuilder::default_header`](../tour.md) sets a header on every request the client makes. The labels that describe the process (the service, the environment) go here, once, when the client is built.
- [`CallOptions::header`](../tour.md) sets a header on one call, through `Client::evaluate_with`. A label that describes the call (the workflow, the step) goes here.

Precedence is replacement, not merging: a per-call header of the same name replaces the default one for that call. Since the whole label set travels in one header value, a call that adds one label must send the full set, the process's labels included. The caller assembles the map once per call and encodes it; the crate does not know the labels are a map.

The header name is not reserved. The reserved names are the ones the client sets itself (`authorization`, `content-type`, `user-agent`, `x-typesafe-retry-count`) and the ones HTTP owns (`host`, `content-length`, `transfer-encoding`, `connection`, `te`, `upgrade`); anything else is sent as given. `HeaderValue::set_sensitive` is for secrets and a label set is not one, but marking it sensitive costs nothing and keeps the value out of `reqwest`'s debug output if a label ever carries something it should not.

```rust
use std::collections::BTreeMap;

use judgment::client::{CallOptions, ClientBuilder, HeaderName, HeaderValue};

const LABELS_HEADER: HeaderName = HeaderName::from_static("x-vertex-ai-labels");

/// The label set as the vendor documents the header: base64 of a JSON object.
/// A BTreeMap keeps the encoding stable for the same labels. Encoding is the
/// caller's dependency (the `base64` crate here); the judgment crate does not
/// know this header exists.
fn labels_header(labels: &BTreeMap<&str, &str>) -> HeaderValue {
    use base64::Engine as _;
    let json = serde_json::to_vec(labels).expect("a map of strings is JSON");
    let encoded = base64::engine::general_purpose::STANDARD.encode(json);
    HeaderValue::from_str(&encoded).expect("base64 is a valid header value")
}

let process_labels = BTreeMap::from([("service", "my-service"), ("env", "prod")]);

let client = ClientBuilder::default()
    .base_url(endpoint_url)
    .api_key(access_token)
    .default_header(LABELS_HEADER, labels_header(&process_labels))
    .build()?;

// One call that adds the workflow: the full set, since a per-call header
// replaces the default one rather than merging with it.
let mut call_labels = process_labels.clone();
call_labels.insert("workflow", "triage");
let options = CallOptions::new().header(LABELS_HEADER, labels_header(&call_labels))?;
let response = client.evaluate_with(&state, &questions, options).await?;
```

Google's label rules are the ones every labelled resource on the platform follows, and the vendor page is where to confirm them for this header: at most 64 labels; keys of 1 to 63 characters and values of 0 to 63; lowercase letters, digits, underscores and dashes; a key starts with a lowercase letter. Normalising a value to those rules (lowercase, illegal characters to `_`, truncated to 63) is the caller's job, before the header is built, and a value that cannot be made legal is dropped rather than sent: a request Vertex refuses for a malformed label is a 400, which the client classifies as [`Error::InvalidRequest`](../tour.md) with Google's message and does not retry, so one bad label fails the call it is on. The crate should not learn these rules. They are one vendor's, for one header, and a client that validated them would be wrong for the next vendor's header; the label module belongs in the application, where its tests can pin the vendor's limits.

Two things the labels do not reach, by design. They do not appear on the `typesafe.evaluate` span or in any [`Observer`](../tour.md) report: no header value is a span field, because a header can be a credential, and the crate does not know which headers are not. A caller that wants its labels on its own traces puts them on its own span. And they do not appear in a recording, as said above, so a `.jud` recording of a labelled call is the same document as of an unlabelled one.

## What the crate would need to change

Ranked by how little they change, since the client's shape is a contract (`tests/contract.rs`, the README's guarantees).

1. **Nothing, for the labels.** Two header levels and a replacement rule are enough, and the encoding is a dozen lines in the caller. An example under `examples/` that builds the header could be added when a consumer asks for one; a helper in the crate would name a vendor's header, which the client otherwise does not.

2. **The evaluate call's URL, for path B.** The smallest change that makes a Vertex endpoint a base URL and nothing more is a builder option that sets the full URL of the evaluate call, with `base_url` left for servers that serve the standard paths: a `ClientBuilder::systemone_url(url)` that `Client::url("v1/systemone")` returns as is when set. `list_models` keeps joining `v1/models` onto the base URL and is a 404 on Vertex, as it is on Laya. This is a contract change (the client sends to a URL the base URL did not produce), so `tests/contract.rs`, the CHANGELOG and the README move together, and the recordings still replay because a URL enters no hash. The alternative with no Rust change is a `vertex` backend in `tools/systemone/serve.py` that maps `/v1/systemone` onto `:rawPredict` and adds the bearer from Application Default Credentials, as the `cloudflare` backend does for its token; it is the right first step, because it verifies the endpoint before the client is changed for it, and it may be the last step if no consumer embeds the crate against Vertex directly.

3. **A rotating bearer, for A and B.** A `ClientBuilder::bearer_source(Arc<dyn Fn() -> String + Send + Sync>)` read before each attempt would let a Google token source drive the header without rebuilding the client. The cost is that the key check at build time (the Python SDK's rules, a blank key refused before any call) moves to call time for that path, and that a credential enters the retry loop, which is shared with no vendor's names in it today. Rebuilding the client on rotation has neither cost and is one line in the caller. The option is worth adding when a consumer holds one client across token lifetimes and measures the rebuild as a problem, and not before.

4. **The error reader against Google's 400 body.** Google's `{"error": {"code", "message", "status", "details"}}` is not the wire's `{"error": {"message", "type"}}`. The reader is tolerant and should surface the message; whether it does, and what `Error::InvalidRequest::issues` holds, is a probe for the first live run, not a change to make in advance.

Nothing on this list changes a question, an answer or `Response::verify`. The model behind a Vertex endpoint is one of the open models the compatible-servers page describes, with that model's option caps and confidence semantics, and a threshold tuned on Jev is as meaningless against it as against the same model on Ollama until it is refit with `judgment::eval`.

## How this would be verified

The live tests in `tests/live.rs` take a server as `JUDGMENT_LIVE_BASE_URL` and a model as `JUDGMENT_LIVE_MODEL`, and every server the crate has been checked against has a page under `docs/verification/` recording the run ([How the crate is checked](../verification/method.md)).

- **Path A first.** Ollama with `clef-flash` on a Cloud Run service with a GPU, or `tev1:0.8b` on CPU for the cheapest run, serves `/v1/systemone` as it does locally, so `mise run live:ollama` against the service's URL exercises the whole client over Google's front door, IAM included: the ID token goes in `TYPESAFE_API_KEY` for the run's hour. A pass records that the crate's A path holds on Google Cloud; a departure (a header Cloud Run strips, a body size limit, a timeout at the edge) is what the page is for.

- **Path B through the shim.** The same container on a Vertex endpoint, with `tools/systemone/serve.py vertex` in front, runs the same tests through `:rawPredict`. Three probes beyond the tests: the model list (expected 404 or 400, classified as `Error::Http`), Google's 400 body through the error reader, and a 401 with an expired token.

- **The labels header, on B.** One labelled call with a legal set, read back on Google's side where the vendor page says labels appear; one with an over-long value, expecting Google's 400 and `Error::InvalidRequest` without a retry; one with the header on a path A server, expecting it to be ignored. The findings decide whether the header is honoured on a self-deployed endpoint or only on a partner model's, which is the open question that most changes what a consumer can do.

Each run becomes a page under `docs/verification/`, and the compatible-servers table gets a row per server reached.

## Open questions

- Whether `rawPredict` on a self-deployed endpoint honours `X-Vertex-AI-Labels`, or only the partner-model paths do; the vendor page is the source, this environment could not read it.
- Whether the caller's request headers reach a custom container through `rawPredict`, which would let a container speaking the wire read the label header for its own logging; Google's custom-container page says what the container is sent and is the place to check.
- Whether a System One publisher appears in the partner catalogue; the day one does, path C is path B with a publisher's path and the same two changes.
- Whether any consumer embeds the crate against Vertex directly rather than through a service of its own, which decides whether change 2 is a builder option or stays a shim.
