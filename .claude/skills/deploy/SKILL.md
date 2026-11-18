---
name: deploy
description: Release Dango from this Mac. Prepare release notes, obtain a clean green CI seal with /ci, then make deploy notarizes the next version, commits VERSION, uploads a draft and atomically pushes main, its seal and its tag. Watch CI and the automatic GitHub release. Use when the owner says "/deploy", "push it", "ship it".
---

# /deploy: notarize, push, publish

Invoking `/deploy` authorizes committing all intended project changes, signing, notarizing, uploading and pushing the
release. Never force-push. Do not deploy when the owner only asks to prepare or change this workflow.

Only `make deploy` changes VERSION. It builds the next patch version in memory, notarizes the universal app and disk
image, then commits that exact number. Normal builds, self-tests and `make release-notarized` keep VERSION unchanged.
Do not bump VERSION yourself or run `make release-notarized` before `make deploy`.

## Phase 0: finish the release content

1. `git branch --show-current` must be `main`; otherwise report and stop.
2. Read the status, recent commits and previous release notes. Write `RELEASE_NOTES.md` in English and
   `assets/readme/<language>/RELEASE_NOTES.md` for every other language in `resources/`. Describe actual changes;
   update all translations, remove draft markers and leave no empty or copied previous-release notes. Finish all
   source and documentation edits before `/ci`.
3. Check prerequisites before the long CI run: a valid Developer ID Application certificate, the keychain profile
   `notary` (`xcrun notarytool history --keychain-profile notary`, discard the history output), Dango's own Sparkle
   key under account `local.agentide` matching UPDATE_PUBLIC_KEY in `crates/ide-product/src/lib.rs`, and `gh` with
   contents write access and permission to push workflow changes. Report missing credentials without printing private
   material. Never reuse another app's key or substitute dry-run assets for a notarized release.
4. If `dist/deployment.json` exists, inspect the interrupted deploy and its draft before changing any source or notes.
   `make deploy` resumes that version and verifies every checksum. Do not delete the manifest or bump VERSION just to
   retry an upload or push.

## Phase 1: seal and commit

Run `git status --porcelain` and `make seal-explain`. A clean tree and `seal here: green` may proceed. Otherwise use
`.claude/skills/ci/SKILL.md`: it runs the complete `make ci`, fixes failures, verifies the green seal and commits the
sealed tree. Proceed only with a verified green seal and a clean working tree; if either is missing, stop and report.
Check the status and seal again. The deploy's VERSION-only commit inherits the seal because VERSION is excluded.

## Phase 2: deploy

Run `make deploy` in the background with a log under `target/` and observe completion. It:

1. Requires clean main with a local green seal and no red seal here or on origin; checks GitHub access and refuses
   if main has diverged from origin.
2. Builds the next version, signs with Developer ID, notarizes and staples both app and image, verifies the image and
   signs the update feed. A failed build or notarization changes no version and pushes nothing.
3. Commits only VERSION as `Release Dango <version>` and creates its local `v<version>` tag.
4. Uploads the versioned image, identical `Dango.dmg` alias, notes, appcast, Cask and `deployment.json` with commit,
   source fingerprint and SHA-256 checksums to a draft release. A failed upload leaves the draft unpublished and
   pushes nothing; retry `make deploy` with the same source and artifacts to resume.
5. Atomically pushes main, its green seal and its version tag. A rejected push leaves the draft unpublished; fix the
   cause without force-pushing, then retry. Never replace a published release or its assets.

## Phase 3: observe both GitHub workflows

Find runs for the exact pushed SHA with `gh run list --commit <sha> --json databaseId,workflowName,status,url`.
Wait for both `ci.yml` and `release.yml` to appear and use `gh run watch <id> --exit-status` in the background. A push
alone is not proof of publication.

- CI: `seal: success`, `check: skipped` is expected. Investigate a missing or red seal.
- Release: `.github/workflows/release.yml` runs when VERSION changes or on a manual retry. It verifies the VERSION-only
  commit, tag, green seal, manifest, checksums and update feed before publishing the draft as latest. It never builds
  or notarizes on a runner.
- On a workflow failure, inspect `gh run view <id> --log-failed`. Retry a network/runner failure with
  `gh run rerun <id>`; never bump VERSION just to retry publication. For a code problem fix it, prepare new notes,
  then restart at Phase 1. Never loosen release checks.

Finally verify `gh release view v<version> --json url,isDraft,tagName,assets`: the release must be public with all
assets. Check `https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg`.

## Report

Report shipped version and commit, notarization result, green seal, both workflow URLs and results, release and download
links. On failure distinguish built, uploaded, pushed and published; a private draft is not a shipped release.
