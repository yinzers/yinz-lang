---
name: Yinz Compile Speed Protocol
description: Applies when adding or changing a compiler analysis or pass.
trigger: relevance
---

- **State the compile-time cost of every new analysis or pass in its design.** Why: compile speed is a goal, and an unmeasured cost compounds across passes.
- **Run it as an incremental query where possible.** Why: unchanged code is then not re-analyzed.
- **Run expensive whole-program work only in release builds.** Why: debug builds are the edit loop, so they must stay fast.
