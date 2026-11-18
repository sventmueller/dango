---
name: ci
description: Drive Dango's whole pipeline to a green CI seal and commit the sealed tree. Runs `make ci` (cargo fmt, make check, coverage with every test and offline window self-test, make perf), fixes every failure itself, verifies the seal ref exists, then commits everything without invalidating it. Use when the owner says "/ci", "make it green", "seal it", "Versiegelung".
---

# /ci: a green seal on a committed tree

The deliverable is both of:

1. a **green seal** `refs/ci-seals/green/<fp>` in the local repository for the content, and
2. a **clean working tree**: everything committed, with `HEAD`'s fingerprint equal to the sealed one.

A green run whose content then changes, or a commit that leaves part of the content out, has bought nothing: GitHub's
CI runs `make check` again. This skill never pushes; that is `/deploy`'s job (`make deploy`).

## How the seal works

- `make ci` is `cargo xtask ci` (`xtask/src/ci.rs`). It refuses while the screen is locked, runs `cargo fmt --all`,
  takes the **fingerprint**, then runs fail-fast: `make check` → `cargo xtask coverage --check --strict` (cargo test
  plus every offline self-test, instrumented; fails on any failed test or scenario and below 90 % total / 50 % per
  file) → `make perf`, each under `caffeinate -di` so the display does not sleep and lock the screen. It prints each
  stage's duration; a stage that fails while the screen is locked seals nothing.
- The fingerprint (`xtask/src/seal.rs`) is the git tree of the **working tree** (temporary index: `read-tree HEAD` +
  `add --all`) without `VERSION`. Untracked, non-ignored files are part of it; ignored files are not. `VERSION` is left
  out because only `make deploy` raises it after notarization, so the release commit keeps the seal.
- Only a complete run of content that did not change during the run mints: green → `refs/ci-seals/green/<fp>`, a failed
  stage → `refs/ci-seals/red/<fp>`. **Red beats green** for the same fingerprint, so a red run poisons that content
  until the code changes (or Phase 3b).
- The ref points at a parentless commit of the sealed tree. It is local; `make deploy` pushes it with main, and
  `.github/workflows/ci.yml` then skips its tests for that content.

## Phase 0: before the run

1. Finish every edit first, docs included. Anything written after the fingerprint is taken is unsealed content.
2. `git status --porcelain`: everything listed is sealed and will be committed. Remove or ignore scratch files now.
3. Preconditions, all environmental, so check rather than burn a red seal on them:
   - the screen is unlocked (`ioreg -n Root -d1 -a | grep -A1 CGSSessionScreenIsLocked` shows no `<true/>`; `make ci`
     refuses otherwise),
   - no other coverage run or self-test is running (`pgrep -fl "xtask (coverage|selftest)"` is empty),
   - the Mac is otherwise idle (`make perf` measures wall-clock budgets).
   If one does not hold, tell the owner and wait; do not start the run.

## Phase 1: run

`make ci` takes about 25 minutes (coverage 17 to 20 of them), longer than a foreground tool call may last. Run it in
the background with its output in a log, and wait for the completion notification instead of polling:

```bash
make ci > target/ci.log 2>&1; echo "exit $?" >> target/ci.log
```

Then read the tail of `target/ci.log` (`ci: … passed in …`, `ci: took …`, `ci: sealed green: …` or the error).

## Phase 2: fix every failure

Fix every failure yourself, including ones that look older than this session or foreign; never skip a stage and do not
ask whether to fix. Read the failing output, fix the code (not the test, unless the test is wrong; never loosen a
budget or a coverage goal), and re-run only the failing stage until it is green:

| Stage | Re-run alone |
|---|---|
| format/lint/tests | `make check`, or `cargo test -p <crate>` while iterating |
| coverage | `cargo xtask selftest <scenario>` for a failed scenario; `cargo xtask coverage --check --strict` for the goals |
| perf | `make perf` on an idle Mac |

Then run the full `make ci` again; only that run seals. Up to three fix attempts per stage; after that stop and report
what is red and what was tried.

## Phase 3: verify the seal

The log line is not proof; the refs are:

```bash
FP=$(cargo xtask seal fingerprint)
git show-ref --verify "refs/ci-seals/green/$FP"                          # must print the ref
git show-ref --verify --quiet "refs/ci-seals/red/$FP" && echo "RED $FP"  # must print nothing
```

- No green ref: the run did not seal, or the tree changed after it. `make seal-explain` lists the paths that separate
  it from the nearest green seal; undo the drift or run `make ci` again. Do not commit.
- A red ref as well: Phase 3b.

### Phase 3b: red and green on the same fingerprint

Delete the red ref only when the red run demonstrably failed for a reason outside the tree (a locked screen, a busy Mac
under `make perf`, another self-test running) and a complete run of the identical content has since gone green:
`git update-ref -d refs/ci-seals/red/<fp>`. If the red verdict was about the code, fix it; that changes the fingerprint.

## Phase 4: commit without breaking the seal

Commit exactly the content that was fingerprinted, all of it, changing nothing on the way (no formatting, no doc
line, no `.gitignore` edit):

```bash
git add -A
git commit -m "<message>"
git status --porcelain                                    # must be empty
test "$(cargo xtask seal fingerprint --head)" = "$FP"    # HEAD's content is the sealed content
```

`git add -A` stages what the fingerprint's `add --all` saw, so `HEAD`'s tree minus `VERSION` equals the sealed tree.
A changed `VERSION` may ride along; it is not part of the fingerprint. Message: English, imperative, in the style of
`git log --oneline -10`, one commit for the whole tree, **no attribution lines**. If the fingerprints differ, the
commit changed or missed content: `make seal-explain` names the paths; restore them and `git commit --amend`, or run
`make ci` again.

## Phase 5: report

First line: the deliverable, or which half is missing and why. Then the stages with their durations from `ci: took`,
the fixes made (file and change), the seal ref, the commit (`sha` and subject), and "not pushed; `/deploy` pushes it".
