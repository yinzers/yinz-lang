---
name: Yinz Prove the Mechanism Fired Protocol
description: "Applies when writing or changing a test that claims to prove a resolver, gated path, diagnostic text or concurrency level."
paths:
  - crates/**/tests/**
---

A test must go red when the mechanism it claims is bypassed. Why: a test that passes when the thing it checks is gone protects nothing.

- **When a resolver's answer differs from the fallback, vary the answer and check the output follows, or count the calls.** Why: output that matches the fallback proves nothing about the resolver.
- **For a gated path, show evidence that it fired:** the call in the IR, or a fire counter above 0. Never accept output equal to the fallback's. Why: the fallback produces the same output when the gate silently stays shut.
- **A diagnostic-text check asserts presence and filters on stable wording** (an error code or keyword), never a count or "is empty". Why: a count or an emptiness check passes when the diagnostic never fires or its wording changes.
- **Back a claimed concurrency level with a spawn-count or max-in-flight assertion.** Why: a sequential run gives the same result as a parallel one.
