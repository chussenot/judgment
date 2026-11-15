---
title: The official TypeSafe SDKs' releases, read for wire changes
description: A reading, on 2026-11-13, of every release of TypeSafe's Python SDK (0.5.7 to 0.7.5) and JavaScript SDK (0.5.7 to 0.7.0), of the model changelog, the request rules and the OpenAPI document, for what each changed on the wire (a request or response shape, a header, a default, a limit) and what this crate does about it; Python 0.7.0, which a third-party quickstart pins, changed nothing on the wire, and three things the SDKs or the request rules do are not mirrored here.
status: current
last_reviewed: 2026-11-13
tags: [research, judgment, typesafe, sdk, wire, compatibility]
---

# The official TypeSafe SDKs' releases, read for wire changes

A third-party quickstart, the model card of [GS1-4B](https://huggingface.co/bigai/GS1-4B), installs `typesafe-sdk==0.7.0`, and nobody here had read that release's changelog. This page reads every release of both official SDKs, and the pages of TypeSafe's documentation that say what the API accepts, for one question: did a request or response shape, a header, a default or a limit change, and does the crate know?

**In short:** Python 0.7.0 changed nothing on the wire. It swapped the SDK's own serialisation library and added an argument that types the response for the caller. Across all the releases, every wire change is one the crate already has (images, Box, the images header, zstd, a null token count) or took a decision on (compression is off by default here, [decision 0029](../decisions/0029-request-compression-is-a-feature-and-a-switch.md)). The OpenAPI document TypeSafe serves is the one vendored here. Three things are not mirrored, and are listed [at the end](#what-the-crate-does-not-mirror): how long an idle connection is kept, what a number that is not finite becomes, and the nesting rule.

## Sources

All read on 2026-11-13.

- The Python SDK's changelog on [TypeSafe's documentation](https://docs.typesafe.ai/sdk/python/changelog.md), which matches the [GitHub releases](https://github.com/typesafe-ai/typesafe-sdk-python/releases) entry for entry; the dates are the upload times on [PyPI](https://pypi.org/project/typesafe-sdk/#history).
- The JavaScript SDK's changelog on [TypeSafe's documentation](https://docs.typesafe.ai/sdk/javascript/changelog.md), which matches its [GitHub releases](https://github.com/typesafe-ai/typesafe-sdk-js/releases); the dates are npm's for `@typesafe-ai/sdk`.
- [The model changelog](https://docs.typesafe.ai/changelog.md), [Models](https://docs.typesafe.ai/models.md), [Request rules and errors](https://docs.typesafe.ai/api/errors.md), and the SDK pages for [Python](https://docs.typesafe.ai/sdk/python.md) and [JavaScript](https://docs.typesafe.ai/sdk/javascript.md).
- [The OpenAPI document](https://api.typesafe.ai/openapi.json), compared with `tests/fixtures/typesafe-openapi.json`: identical, both at version 0.2.0.

What could not be read: **there is no deprecations page.** `https://docs.typesafe.ai/deprecations` and its `.md` form answer 404, and the documentation's index, `llms.txt`, names none. No page read gives any model, field, header or SDK version a retirement date. Neither SDK repository has a `CHANGELOG.md` at its root (404), so an unreleased section, if one exists, was not seen. A changelog says what its authors chose to list: "add more request validation checks" appears twice below and names no check.

## The Python SDK, `typesafe-sdk`

| Version | Uploaded (UTC) | What the changelog lists | On the wire | In this crate |
|---|---|---|---|---|
| 0.5.7 | 2026-09-11 | The initial public release. Its GitHub heading says 2026-09-12 and the documentation's 2026-09-14. | The baseline. | |
| 0.6.0 | 2026-09-15 | **Breaking:** `Score.criteria` is an ordered sequence, no longer a dictionary keyed by integers. Abstract input types, HTTP details in error messages, invalid `RetryPolicy` values handled, picklable exceptions and responses. | None known: it is the type of the SDK's argument, and the document's `criteria` for a Score is an array. | A Score's levels are a list, sent as an array. |
| **0.7.0** | 2026-09-18 | **Breaking:** the serialisation library changed from `msgspec` to `pydantic`. `system_one` takes a `response_model` argument, a pydantic model for the caller's own typing. A `str` subclass is serialised as a string, not as a list of characters. | **None.** All three are inside the SDK. | Nothing to mirror. |
| 0.7.1 | 2026-09-21 | The API key is validated early and kept out of logged exceptions. Examples for AI gateways. | None. | The release whose client behaviour the crate mirrors: the key's rules and the retry policy ([Internals](../internals.md)). |
| 0.7.2 | 2026-09-26 | An `http2` extra. | None: a transport option. | |
| 0.7.3 | 2026-10-09 | **A default:** pooled connections are kept alive for 30 seconds. "Add more request validation checks", not listed. A crash fix for concurrent TLS connections. | No shape. A client default. | Not mirrored, [below](#what-the-crate-does-not-mirror). |
| 0.7.4 | 2026-10-09 | Non-finite numbers are serialised as strings instead of invalid JSON tokens. | What a `NaN` or an infinity in the state becomes in the body. | Different, [below](#what-the-crate-does-not-mirror). |
| 0.7.5 | 2026-10-14 | **A default:** request bodies are compressed with zstd, "non-official APIs may not support this immediately". `Image` assets, `Box` questions and answers, `attach_assets`. | `Content-Encoding: zstd` on a body of 1 KiB or more; the `assets` map and the images header; the `box` question and answer. | The `zstd` feature, off by default by decision 0029, since 0.17.0; images and Box since 0.14.0. |

0.7.5 is the newest release. A `0.0.1a0` of 2026-09-09 is on PyPI with no changelog entry.

## The JavaScript SDK, `@typesafe-ai/sdk`

| Version | Published (UTC) | What the changelog lists | On the wire | In this crate |
|---|---|---|---|---|
| 0.5.7 | 2026-09-12 | The initial public release; its GitHub title says 2026-09-11. | The baseline. | |
| 0.6.0 | 2026-09-15 | **Breaking:** `Score.criteria` as an ordered sequence. | None known, as for Python 0.6.0. | A list, sent as an array. |
| 0.7.0 | 2026-10-14 | **Breaking:** Node.js 22 or newer; request payload types are no longer exported. Images, experimental, behind `TypeSafe-Danger-Zone-Images: true`. `box` questions, with `jev-preview`. **A default:** bodies of 1 KiB or more are compressed with zstd on Node.js 22.15 or newer, `compression: false` turns it off. Null token counts in `usage` are allowed. "Add more request validation checks", not listed. An invalid request rejects the promise instead of throwing. | The images header, `box`, `Content-Encoding: zstd`, and a `usage` count that may be `null`. | Images and Box since 0.14.0; the `zstd` feature; an absent or `null` count is read as zero. |

0.7.0 is the newest release.

## The API's own pages

- **Models.** `jev-latest` and `jev-preview` both point to `jev-1.14.0`, released on 2026-10-14; `jev-1.13.0` is listed beside it with no retirement date. The model changelog has that one entry.
- **Limits in the document.** 1 to 255 options for a Choice and 1 to 10 levels for a Score; no limit on the questions of a request beyond its tokens (64k for a request, 32k for the state with one question, 262k for both under the long-context header). The crate's builder asks for at least 2 options and 2 levels, which is its own rule ([The crate](../../reference/crate.md#limits)).
- **Request rules.** A body of at most 32 MiB; UTF-8 JSON with no byte order mark; at most 64 nested levels of objects and arrays in each of `state`, `instructions` and `criteria`; finite numbers only. A body over the size is a 413, the others a 400.
- **Statuses.** 400, 401, 413, 415, 422, 429 and 529, each of which the crate reads into the error group that says what fixes it.

## What the crate does not mirror

None of these is a change to a shape the crate sends or reads. Each is a place where the SDKs or the request rules do something the crate does not, stated so that it is a decision and not an accident.

- **Idle connections.** The Python SDK keeps a pooled connection for 30 seconds since 0.7.3, and its default client opens at most 100. The crate sets no pool option, so `reqwest`'s own defaults apply (90 seconds idle and no cap on idle connections per host, in `reqwest` 0.13.5). Should a server close an idle connection sooner than that, the next request on it would fail as a transport failure, which the crate retries by default.
- **A number that is not finite.** The request rules refuse `NaN` and infinities with a 400, and the Python SDK sends them as strings since 0.7.4. `serde_json` writes a non-finite float as `null`, so a state that holds one goes out as valid JSON with `null` where the number was: no error on either side, and the model is asked about a state that lost a value (observed on 2026-11-13 by serialising a request).
- **Nesting.** The crate does not count the nesting of a state, an instruction or a criterion before a call. A request past 64 levels is the server's to refuse, a 400 the crate reads as `Error::InvalidRequest`.
