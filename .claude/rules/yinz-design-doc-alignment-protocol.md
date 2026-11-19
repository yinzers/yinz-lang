---
name: Yinz Design Doc Alignment Protocol
description: Applies when planning or doing Yinz work that builds toward a design doc.
trigger: relevance
---

A plan is a route to the design, never an override of it. Why: a plan that contradicts its design doc ships the wrong thing, however consistent the plan is with itself. Before the plan is approved:

- **Name the governing IMP-* or SCRATCH-* design doc, or confirm none exists.** If none exists, the design is written first. Why: a plan without a design anchor decides the design mid-execution.
- **Read the cited doc and confirm it specifies the mechanism, not just mentions the topic.** If it is silent, report that silence, then get the design written or the citation corrected. Why: a topical citation anchors the plan to nothing, and the gap surfaces phases later.
- **State every divergence as "doc X says A; this plan does B because <reason>", and get Chief's confirmation.** Why: an unstated divergence is a plan defect.
- **Make sure any deferral the plan relies on is already in the roadmap or scope docs.** Why: a deferral the plan invents can cut the milestone at the wrong line.
- **Confirm by reading the code any claim about how existing code the plan does not touch behaves.** Why: a wrong guess costs a correction phases later, while the check is one read.
- **Check the plan against its cited design doc, not only against itself; a contradiction is a defect that blocks the plan's approval.** Why: a plan can be internally consistent and still violate the design.

While doing the work:

- **When the code or task disagrees with the governing design doc, stop and report "doc X says A; the work does B".** The doc wins unless Chief's confirmation changes it. Why: a silent divergence ships a design nobody chose, and it is cheapest to fix before more code builds on it.
