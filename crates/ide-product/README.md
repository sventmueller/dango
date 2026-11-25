# ide-product

The product's display name, its version and the identifiers its stored data hangs on, kept in one place (E18). It has no code that runs: the other crates, the app and the build tool read its constants at compile time.

```mermaid
flowchart LR
    product[["ide-product constants"]]
    product -->|"NAME, BUNDLE_ID, URL_SCHEME, CASK, UPDATE_PUBLIC_KEY"| xtask["xtask: bundle, cask, release, update feed"]
    product -->|"NAME, BUNDLE_ID, COMMAND"| app["the app: menus, window titles, command line tool"]
    product -->|"BUNDLE_ID, PROJECT_DIR, PREVIOUS_PROJECT_DIRS"| core["ide-core: data folders, project settings"]
    product -->|"NAME as user agent and client name"| net["ide-acp, ide-agent, ide-ci, ide-lsp"]
    product -->|"VERSION as client version"| clients["ide-acp, ide-codex, ide-predict"]
    product -->|"OAuth client ids"| ci["ide-ci: auth::product_client"]
    product -->|"NAME, BUNDLE_ID as checkpoint author"| git["ide-git"]
    product -->|"NAME as TERM_PROGRAM"| terminal["ide-terminal"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).
