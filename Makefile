# Entry points only; the build logic lives in xtask/src.
.DEFAULT_GOAL := help
.PHONY: help build run icon lint check check-dictation perf fuzz coverage test test-integration test-claude-latest release release-notarized

help: ## List the targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the app into target/release with dictation, signed with the local certificate; raises the build number
	cargo xtask bundle --release

run: ## Build the debug app into target/debug and open it
	cargo xtask run

icon: ## Compile the app icon after a change in Icon Composer (assets/AppIcon.icon); the only target that needs Xcode
	cargo xtask icon

lint: ## Formatting, clippy, also with the selftest feature, the allowed thread-locals and the files' shape (E98); warnings fail
	cargo fmt --all --check
	cargo xtask thread-locals
	cargo xtask structure
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy --package ide-app --package ide-ui-browser --package ide-ui-export --package ide-ui-terminal --all-targets --features ide-app/selftest,ide-ui-browser/selftest,ide-ui-export/selftest,ide-ui-terminal/selftest -- -D warnings
	cargo clippy --package ide-terminal --package ide-lsp --package ide-git --package ide-agent --package ide-predict --package ide-dap --all-targets --features ide-terminal/fuzzing,ide-lsp/fuzzing,ide-git/fuzzing,ide-agent/fuzzing,ide-predict/fuzzing,ide-dap/fuzzing -- -D warnings

check: lint ## lint, then all cargo tests; needs no keychain, no window and no network
	cargo test --workspace

check-dictation: ## Clippy and the tests of the app and ide-ui-dictation with dictation, which builds whisper.cpp with cmake; for changes to it
	cargo clippy --package ide-app --package ide-ui-dictation --all-targets --features ide-app/dictation,ide-ui-dictation/engine -- -D warnings
	cargo clippy --package ide-app --package ide-ui-dictation --all-targets --features ide-app/dictation,ide-app/selftest,ide-ui-dictation/engine,ide-ui-dictation/selftest -- -D warnings
	cargo test --package ide-app --package ide-ui-dictation --features ide-app/dictation,ide-ui-dictation/engine

perf: ## Timing budgets (E25) in a release build; meaningful only while the Mac is otherwise idle
	cargo test --release -p ide-project -- --ignored --test-threads=1 tree::tests::a_folder_of_many index::tests::twenty_thousand index::tests::two_hundred_thousand
	cargo test --release -p ide-document -- --ignored --nocapture budgets
	cargo test --release -p ide-terminal --test throughput -- --ignored --test-threads=1

fuzz: ## The crates' fuzz targets with the pinned nightly, FUZZ_SECONDS each (60 by default); ARGS=<target> picks some
	cargo xtask fuzz $(ARGS)

coverage: ## Line coverage of the tests and the offline selftest scenarios into target/coverage/; ARGS=--check fails below the goals
	cargo xtask coverage $(ARGS)

test: check ## check, then the selftest probe and the accessibility audit, left to right and right to left
	cargo xtask selftest probe a11y a11y-rtl

test-integration: ## Real servers, git and CI with this Mac's sign-ins; ARGS=--list shows the plan, ARGS=--paid adds the agent CLIs
	cargo xtask integration $(ARGS)

test-claude-latest: ## The newest Claude CLI against the contract the app relies on, without a sign-in (R7); ARGS=--signed-in adds a short paid turn
	cargo xtask claude-latest $(ARGS)

release: ## Dry run of the release with dictation: universal app, disk image, update feed and Cask in target/release-dry-run; sends nothing anywhere
	cargo xtask release

release-notarized: check ## The release: Developer ID, notarized by Apple, into dist/; raises the build number
	cargo xtask release --notarize
