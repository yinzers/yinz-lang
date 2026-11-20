---
name: "IMP-versioning"
description: "Yinz's versioning policy: pre-release replaced designs are deleted (not superseded), and post-release breaking changes need a major version bump with no backwards-compatibility shims."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-19"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Versioning

Design document for how Yinz versions itself and treats breaking changes. Not user-facing.

---

## Pre-release: delete replaced designs

Pre-release, a replaced design is deleted from its doc. Docs hold no "superseded" sections; git history keeps the old reasoning.

---

## Post-release: major bumps for breaking changes

- A breaking change ships as a major version bump.
- There are no backwards-compatibility shims, no deprecation cycles and no "acceptable for now" compatibility layers.
- A version number tells callers whether to update; a clean codebase beats a padded API.
- When a major version will remove something, the current version marks it `deprecated` for exactly one major version of warning. The deprecated thing is not kept beyond that.

Before v1.0, breaking changes (including compiled ABI and the `--json` schema) are allowed between releases; see the stdlib corollary in [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) (never ship a "v2 alongside v1": fix v1 in place pre-1.0).
