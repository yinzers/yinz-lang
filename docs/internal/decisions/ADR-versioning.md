---
name: "ADR-versioning"
description: "Locks Yinz's versioning policy: pre-release decisions are deleted (not superseded) and post-release breaking changes require a major version bump with no backwards-compatibility shims."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "decision"
---

# Versioning — Design Decisions

---

## Pre-Release: Delete Replaced Decisions

Pre-release, replaced decisions are deleted from design files. No "superseded" sections.

**Why**: Superseded sections accumulate and create confusion. If a decision was wrong, keeping it around serves no one. The git history preserves the reasoning if anyone ever needs it.

---

## Post-Release: Major Bumps for Breaking Changes, No Backwards Compatibility

When breaking changes ship, bump the major version. No backwards compatibility shims. No deprecation cycles. No "acceptable for now" compatibility layers.

**Why**: Backwards compatibility shims accumulate technical debt, create two-tier API surfaces, and force the language to carry dead weight forever. Every JS developer knows what version numbers mean. If the version number changed, callers know to update. Clean codebase beats padded API.

**Post-release, use "deprecated" markers** when removing something in the next major version — give users one major version of warning before removal. But the deprecated thing is not kept around indefinitely.
