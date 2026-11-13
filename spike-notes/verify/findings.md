# S3 independent verification (scratch, never merged)
Toolchain: spike-notes/setup.sh with WT= fixed + curl retry (downloads were truncated). Spike build @29cd6f2.
Baseline: `main` e730851 built from `git archive` in .spike-env/main.
Harness: spike-notes/verify/asan-local.sh (asan.sh with worktree paths). Probes: spike-notes/probes + spike-notes/verify/g*.ynz.
See the closing report for the claim-by-claim result table.
