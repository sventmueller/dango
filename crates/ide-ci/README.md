# ide-ci

CI for the app (E12): GitHub Actions and GitLab CI of a repository's remote, on github.com, gitlab.com, GitHub
Enterprise Server and self-managed GitLab. It works out which service a remote belongs to, finds a token strictly per
host, and reads, re-runs, starts and cancels runs and jobs, reads their logs and lists open merge requests and issues,
and reads an issue's text and labels, never its comments, for the Leitstand's intake (E176).
Every call blocks until the network answers, so the app runs it on a worker thread, and only for trusted folders
(E22). The module `panel` is what the CI panel decides without a window (E98): its phase, rows and when it reads
again.

```mermaid
flowchart TB
    panel["the app's CI panel, main thread"] -->|"Lane ide-ci: 2 threads"| lane["worker thread"]
    panel -->|"thread ide-ci-login"| login["oauth: device flow or PKCE with a loopback listener"]
    lane -->|"candidates(remote URL, HostRule)"| forge["Forge: service, API address, project"]
    lane -->|"locate for a self-managed GitLab"| forge
    lane -->|"Auth::find, Auth::fresh"| auth["Auth"]
    auth -->|"first: SecretStore of ide-secrets"| keychain[("login keychain")]
    auth -->|"then: Commands, Supervised"| cli(["gh auth token, glab auth status --show-token"])
    auth -->|"last: the login shell's environment"| env["GH_TOKEN, GITHUB_TOKEN, GITLAB_TOKEN, ..."]
    login -->|"Auth::save_grant"| keychain
    lane -->|"Client::new(Forge, token, Transport)"| client["Client: runs, jobs, log, rerun, cancel, changes, issues, issue, labelled_issues"]
    client -->|"github or gitlab requests, paged"| transport{{"Transport"}}
    transport -->|"Network: HTTPS through ide-net"| api(["GitHub or GitLab API"])
    transport -->|"Recorded, in tests"| recordings[("recorded answers")]
    lane -->|"log::lines, log::excerpt, the result via main_soon"| panel
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.
