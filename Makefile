# Entry points only; the build logic lives in xtask/src.
.DEFAULT_GOAL := help
.PHONY: help build run icon lint check check-dictation perf fuzz test test-integration release release-notarized

help: ## List the targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the app into target/release with dictation, signed with the local certificate; raises the build number
	cargo xtask bundle --release

run: ## Build the debug app into target/debug and open it
	cargo xtask run

icon: ## Compile the app icon after a change in Icon Composer (assets/AppIcon.icon); the only target that needs Xcode
	cargo xtask icon

lint: ## Formatting and clippy, also with the selftest feature; warnings fail
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy --package ide-app --all-targets --features selftest -- -D warnings
	cargo clippy --package ide-terminal --package ide-lsp --package ide-git --package ide-agent --all-targets --features ide-terminal/fuzzing,ide-lsp/fuzzing,ide-git/fuzzing,ide-agent/fuzzing -- -D warnings

check: lint ## lint, then all cargo tests; needs no keychain, no window and no network
	cargo test --workspace

check-dictation: ## Clippy and the app's tests with dictation, which builds whisper.cpp with cmake; for changes to it
	cargo clippy --package ide-app --all-targets --features dictation -- -D warnings
	cargo clippy --package ide-app --all-targets --features dictation,selftest -- -D warnings
	cargo test --package ide-app --features dictation

perf: ## Timing budgets (E25) in a release build; meaningful only while the Mac is otherwise idle
	cargo test --release -p ide-project -- --ignored --test-threads=1 tree::tests::a_folder_of_many index::tests::twenty_thousand index::tests::two_hundred_thousand
	cargo test --release -p ide-document -- --ignored --nocapture budgets
	cargo test --release -p ide-terminal --test throughput -- --ignored --test-threads=1

fuzz: ## The crates' fuzz targets with the pinned nightly, FUZZ_SECONDS each (60 by default); ARGS=<target> picks some
	cargo xtask fuzz $(ARGS)

test: check ## check, then the selftest probe and the accessibility audit, left to right and right to left
	cargo xtask selftest probe a11y a11y-rtl

test-integration: ## Real servers, git and CI with this Mac's sign-ins; ARGS=--list shows the plan, ARGS=--paid adds the agent CLIs
	cargo xtask integration $(ARGS)

release: ## Dry run of the release with dictation: universal app, disk image, update feed and Cask in target/release-dry-run; sends nothing anywhere
	cargo xtask release

release-notarized: check ## The release: Developer ID, notarized by Apple, into dist/; raises the build number
	cargo xtask release --notarize
