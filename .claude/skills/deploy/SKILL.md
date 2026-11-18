---
name: deploy
description: Release Dango from this Mac. Prepare release notes and commit the release content without asking for a CI seal, then make deploy notarizes the next version, commits VERSION, uploads a draft, atomically pushes main and its tag, with a green seal when one exists, and publishes the release itself; no GitHub runner takes part. Use when the owner says "/deploy", "push it", "ship it".
---

# /deploy: notarize, push, publish from this Mac

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
   source and documentation edits before the commit of Phase 1.
3. Check prerequisites before committing: a valid Developer ID Application certificate, the keychain profile
   `notary` (`xcrun notarytool history --keychain-profile notary`, discard the history output), Dango's own Sparkle
   key under account `local.agentide` matching UPDATE_PUBLIC_KEY in `crates/ide-product/src/lib.rs`, and `gh` with
   contents write access and permission to push workflow changes. Report missing credentials without printing private
   material. Never reuse another app's key or substitute dry-run assets for a notarized release.
4. If `dist/deployment.json` exists, inspect the interrupted deploy and its draft before changing any source or notes.
   `make deploy` resumes that version and verifies every checksum. Do not delete the manifest or bump VERSION just to
   retry an upload or push.

## Phase 1: commit

A deploy does not ask for a CI seal: run neither `/ci` nor `make seal-explain`, and do not stop because a seal is
missing or red. Run `git status --porcelain`; commit all intended project changes with a message that describes them
and leave nothing uncommitted, since `make deploy` refuses a dirty tree. A green seal that `/ci` minted earlier for
exactly this content is pushed along as the record of what was tested; no GitHub workflow tests the push.

## Phase 2: deploy

Run `make deploy` in the background with a log under `target/` and observe completion. It:

1. Requires clean main, no seal; checks GitHub access and refuses if main has diverged from origin.
2. Builds the next version, signs with Developer ID, notarizes and staples both app and image, verifies the image and
   signs the update feed. A failed build or notarization changes no version and pushes nothing.
3. Commits only VERSION as `Release Dango <version>` and creates its local `v<version>` tag.
4. Uploads the versioned image, identical `Dango.dmg` alias, notes, appcast, Cask and `deployment.json` with commit,
   source fingerprint and SHA-256 checksums to a draft release. A failed upload leaves the draft unpublished and
   pushes nothing; retry `make deploy` with the same source and artifacts to resume.
5. Atomically pushes main and its version tag, plus the green seal of its content if there is one locally. A rejected
   push leaves the draft unpublished; fix the cause without force-pushing, then retry. Never replace a published
   release or its assets.
6. Publishes the draft as latest once origin's main is the release commit and GitHub's SHA-256 of every draft file
   matches the manifest, then removes `dist/deployment.json`. A failed publication leaves the draft and the manifest;
   rerun `make deploy`, which resumes the same version, re-uploads and publishes. Never publish the draft by hand
   and never loosen these checks.

## Phase 3: verify the release

Verify `gh release view v<version> --json url,isDraft,tagName,assets`: the release must be public with all assets.
Check `https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg`.

## Report

Report shipped version and commit, notarization result, whether a green seal went along, release and download links.
On failure distinguish built, uploaded, pushed and published; a private draft is not a shipped release.
