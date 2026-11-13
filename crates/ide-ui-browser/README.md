# ide-ui-browser

The browser tab (E17): a `WKWebView` below a navigation row, which also previews a project's dev server (I7). Every
tab shares one persistent website data store; pages get no way into the app, open local files only inside the
window's folder and other apps' schemes never. Pop-ups open as tabs, downloads ask where to save and are marked as
coming from the web, HTTP logins and certificates accepted for a development server live in memory until the app
quits. The crate knows a window only by its id: the app adds, selects and closes the tabs and holds the browser's
`State` through `Host` (E99).

```mermaid
flowchart LR
    app["app: link, menu, restored layout"] -->|"open_tab(mtm, window, url)"| resolve["address::resolve"]
    resolve -->|"tab with that key open"| revisit["select_tab, revisit: reload a local server"]
    resolve -->|"new"| view["BrowserView"]
    view -->|"Host::add_tab"| app
    view --> web["WKWebView"]
    web -->|"decide_action"| policy{"web page, file in the folder?"}
    policy -->|"no"| failure["native failure page"]
    policy -->|"⌘-click, target=_blank"| view
    policy -->|"download"| downloads["Downloads: save panel, quarantine"]
    web -->|"server trust"| trust["trust: refused, accepted by the user"]
    web -->|"alert, confirm, prompt, HTTP login"| dialogs["dialogs: sheets naming the site"]
    view -->|"key button"| autofill["autofill: login into the main frame"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).
