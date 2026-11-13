---
name: deploy
description: Push Dango's main to GitHub with its green CI seal, so the GitHub workflow skips the tests it already passed. Checks the seal first and runs /ci when it is missing, then `make deploy`, then watches the GitHub run and reports. Use when the owner says "/deploy", "push it", "ship it".
---

# /deploy: push main with its seal

`make deploy` (`cargo xtask deploy`) pushes `main` and `refs/ci-seals/green/<fp>` to origin in one atomic push. The
workflow `.github/workflows/ci.yml` then asks origin for the seal of `HEAD`'s content:

| Seal on origin | The workflow |
|---|---|
| green | the `seal` job passes in a minute or two and `check` is skipped |
| none | `check` runs `make check` on a macOS runner (no window self-tests: no GUI session there) |
| red | the `seal` job fails |

The owner invoking `/deploy` is the go-ahead to commit and push this content. Never force-push, never push anything
but what `make deploy` pushes.

## Phase 0: the seal

```bash
git branch --show-current     # must be main; otherwise report and stop
git status --porcelain
make seal-explain
```

- Clean and `seal here: green` → Phase 1.
- Working tree dirty, or `seal here: none` or `red` → invoke the **`ci` skill** (Skill tool, `skill: "ci"`). It drives
  `make ci` to green, verifies the seal and commits the sealed tree. Return only when it reports both: green seal
  verified, working tree clean. If it stops without that, the deploy stops too: report what is red, push nothing.
  Then run the three commands above again as proof.

## Phase 1: push

```bash
make deploy
```

It refuses on another branch, with uncommitted changes, without a green local seal, with a red seal here or on origin.
A refusal sends you back to Phase 0, not around it.

## Phase 2: watch the GitHub run

Run this as a **background** Bash command and wait for its notification:

```bash
SHA=$(git rev-parse HEAD)
for i in $(seq 1 30); do
  ID=$(gh run list --workflow ci.yml --commit "$SHA" --limit 1 --json databaseId --jq '.[0].databaseId')
  [ -n "$ID" ] && break; sleep 10
done
if [ -z "$ID" ]; then echo "RESULT=NO_RUN"; exit 1; fi
gh run watch "$ID" --exit-status --interval 30 > /dev/null; echo "RESULT=$? run=$ID"
gh run view "$ID" --json url,conclusion,jobs --jq '.url, .conclusion, (.jobs[] | "\(.name): \(.conclusion)")'
```

- `RESULT=0` → report: run URL, `seal: success`, `check: skipped` (expected with a green seal; `check: success` means
  origin had no seal, which `make deploy` should have prevented: say so).
- `RESULT=NO_RUN` → the workflow did not start for this commit; report `gh run list --workflow ci.yml --limit 3`.
- Anything else → `gh run view <id> --log-failed`, then classify:
  - **code** (a test or lint that fails on the runner): fix it, then back to Phase 0. The fix changes the content,
    so `/ci` seals and commits it before the next push. At most three such rounds, then report.
  - **runner or network** (toolchain download, checkout of `splat`/`lowspeak`, GitHub outage): report and offer
    `gh run rerun <id>`; do not change code for it.

## Report

Pushed commits (`git log --oneline origin/main -5`), the seal ref, the run URL with each job's result, and any fix
rounds.
