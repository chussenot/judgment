---
title: Performance
description: What a call costs in this crate outside the network, measured with divan on one machine: holding a response against 1, 16 and 64 questions, the canonical fingerprint of 1 KiB and 64 KiB of state, finding a recording for a request, and a Fake answering; the machine and toolchain the numbers came from, how to run the benchmarks, and what they do and do not say.
status: current
last_reviewed: 2026-10-17
tags: [judgment, performance, benchmarks, reference]
---

# Performance

A call to a model takes hundreds of milliseconds to seconds. What this crate adds around it is measured here, so that a change to the verification or the fingerprint that makes it a hundred times slower is seen, and so that nobody spends effort on a cost that is not there.

## Results

Run with `cargo bench --bench verify` (`mise run bench`), release profile, median of 100 samples:

| Benchmark | What it does | Median |
|---|---|---|
| `verify/1` | `Response::verify` of one question | 9 ns |
| `verify/16` | the same over 16 questions (Noul, Choice and Score in turn) | 1.1 µs |
| `verify/64` | the same over 64 questions | 5.2 µs |
| `fingerprint/1024` | the canonical fingerprint (RFC 8785, then SHA-256) of 1 KiB of state | 5.2 µs |
| `fingerprint/65536` | the same for 64 KiB | 381 µs |
| `replay_lookup` | `Replay` finding and verifying the recording for a 16-question request, fingerprint included | 33 µs |
| `fake_round_trip` | a `Fake` answering 16 questions, verified | 6.8 µs |

Verification is linear in the questions, about 80 ns each. The fingerprint is linear in the state, about 6 ns a byte, which is the cost to key a recording or compare a rubric; a replay lookup is dominated by it. None of these is near a network round trip: the whole fixed cost of a replayed call with a 64 KiB state is under half a millisecond.

## Where they were measured

| | |
|---|---|
| Date | 2026-10-17 |
| CPU | Intel Xeon @ 2.10 GHz, 4 vCPUs (a cloud VM) |
| Memory | 15 GB |
| OS | Linux 6.18 |
| Toolchain | rustc 1.94.1, `bench` profile (optimised) |
| Harness | divan 0.1, default sample count |

A shared virtual machine is noisy: the fastest sample is the best estimate of the code and the slowest says how busy the host was (the 64-question verify ranged from 5.2 µs to over 100 µs in one run). Read the medians as an order of magnitude, and compare two runs on the same machine, not across machines. Replace this table when the numbers are re-taken on a quiet one.

## Running them

```sh
mise run bench          # the numbers
mise run bench:build    # that they compile, which is what CI does
```

CI builds the benchmarks (`cargo bench --no-run`) and never runs them: a shared runner's timings would be noise, and a gate on noise teaches people to ignore it. A change that touches `src/answer.rs` or `src/eval/` and may move a number is worth a run before and after on one machine, pasted into the pull request.

## What they do not say

They measure the code in this crate, with the model's answer already in memory. They say nothing about the client's retries, connection setup or a server's latency, and the `Fake` and `Replay` they use do no I/O beyond reading the recordings once at start.
