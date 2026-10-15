---
title: Ask a question about an image
description: How to send an image with a request and ask a Noul, a Choice or a Score about it, from the shell with jud --asset and in Rust with Image, Assets and answer_with_assets; the reference that places the image in the state, what the crate checks before the upload, how the answer is recorded and replayed, and what it costs.
status: current
last_reviewed: 2026-10-15
tags: [judgment, images, assets, how-to, jud, rust]
---

# Ask a question about an image

**You will** send an image with a request and read a calibrated answer about it, from the shell and from Rust, and replay the answer with no key. **Prerequisites:** [Install](../start/install.md); [Visual judgments](../concepts/visual-judgments.md) for what an image in a request is. A key is needed for the live call only.

## From the shell

A rubric asks about an image the way it asks about anything in the state: by naming it. The state carries a *reference*, any text, where the image belongs, and the request declares which image the reference stands for.

```yaml
apiVersion: jud/v1
kind: Rubric
metadata:
  name: door
  minor: 4
spec:
  questions:
    person:
      type: noul
      instructions: Is there a person at the door in `photo`?
  policy:
    person: {threshold: 0.6}
```

`--asset REF=PATH` adds the image under the reference the state uses; the file is checked against the API's limits, base64-encoded and sent with the header the API requires, and nothing else in the request changes:

```sh
echo '{"photo": "door", "when": "7 pm"}' | jud door.jud --asset door=frames/door-1912.png
```

A reference the state does not carry is refused before any call ("referenced nowhere"), as is a file that is not a PNG or a JPEG, or one outside the limits. An image every request sends goes in the rubric instead, under `spec.assets` by a path relative to the file ([Assets](../reference/jud-format.md#assets)); a cases document declares each case's image the same way, so `jud record`, `jud eval` and `jud tune` need no flag.

## In Rust

```rust no_run
use judgment::{Assets, Client, Image, Questions, SystemOne};

async fn run() -> judgment::Result<()> {
    // The format and the size are read from the bytes and held to the API's
    // limits here, so a bad image is an error before any upload.
    let photo = Image::read("frames/door-1912.png")?;
    let mut assets = Assets::new();
    // `add` names the image by its bytes: `img-` and 32 hex digits of their
    // SHA-256. `insert(reference, image)` takes a name of your own.
    let door = assets.add(photo)?;
    let state = serde_json::json!({ "photo": door, "when": "7 pm" });

    let mut questions = Questions::new();
    let person = questions.noul("person", "Is there a person at the door in `photo`?", None)?;

    let client = Client::from_env()?;
    let response = client
        .answer_with_assets(&state, "jev-latest", &questions, &assets)
        .await?;
    let person = response.get(&person)?;
    if person.is_yes(0.6) {
        println!("someone is at the door ({:.2})", person.yes);
    }
    Ok(())
}
```

`answer_with_assets` is on the `SystemOne` trait, so a `Fake`, a `Recorder` or a `Replay` takes the same call: the fake refuses a reference the state does not carry as the API does, and the recorder keys the recording by the image's bytes as well as the state. A backend written elsewhere that has not implemented it refuses a request with images rather than dropping them. `Request::new(..).with_assets(&assets)` is the same request for `Client::evaluate_with`, when the call needs options.

## What the crate checks, and what it reports

| Check | Where | Error |
|---|---|---|
| PNG or JPEG, not animated; each side 256 to 1568 pixels; 78,400 to 2,458,624 pixels; at most 16 MiB | `Image::read`, `Image::from_bytes`, `Image::from_base64` | `Error::InvalidImage` |
| A reference 1 to 1024 bytes, all of them at most 65,536; the same reference not two different images | `Assets::add`, `Assets::insert` | `Error::InvalidAsset` |
| Every reference appears in the state or the questions; the body at most 32 MiB | `Client::evaluate_with`, before the upload | `Error::InvalidAsset`, `Error::RequestTooLarge` |

The hosted API refuses the same with a 400, and a 413 for a body over its size limits; both read as `Error::is_request_too_large` when the remedy is a smaller image. The images' count and bytes reach the `Observer` (`on_assets`) once per call, and the `typesafe.evaluate` span carries the count as `images`, so an application's own metrics can price a visual judgment.

An image costs input tokens in proportion to its pixels ([Visual judgments](../concepts/visual-judgments.md)): resize before sending when the target stays legible. `jev-latest` (Jev 1.13) answers a Noul, a Choice or a Score about an image; a Box needs `jev-preview` until the alias moves ([Locate something with a Box question](locate-with-a-box.md)).

## Next

- [Locate something with a Box question](locate-with-a-box.md): where, not only whether.
- [Record, replay and test](record-replay-and-test.md): the recording is keyed by the image.
- [Label cases](label-cases.md): a cases document names each case's image beside its state.
