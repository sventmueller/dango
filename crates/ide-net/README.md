# ide-net

The one way onto the network. Every client is HTTPS checked against the system's certificate store and names the
app in its user agent; installers and registers fetch through the `Net` seam, which tests stand in for; downloads
hash what arrives while they report progress and listen for `Cancel`. The installers of language servers
(`ide-lsp`) and debug adapters (`ide-dap`) share their stages, progress and errors from here; the agent installer
(`ide-acp`) shares the download.

```mermaid
flowchart LR
    ci["ide-ci: Network"] -->|"client(Client)"| agent["ureq agent: platform verifier, user agent, time limits"]
    provider["ide-agent: Http"] -->|"client(Client)"| agent
    http["Http: HTTPS only"] -->|"client(Client)"| agent
    installers["ide-lsp, ide-dap, ide-acp installers and register"] -->|"Net::get, Net::get_text"| http
    tests["tests"] -.->|"their own Net"| installers
    installers -->|"download(net, url, path, cancel, limit, progress)"| download["read, hash, write"]
    download -->|"Cancel::is_cancelled between reads"| cancelled["DownloadError::Cancelled"]
    download -->|"bytes so far, size"| progress["progress callback"]
    download -->|"SHA-256 hex"| installers
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.
