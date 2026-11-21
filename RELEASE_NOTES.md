## Security
- Links in an agent's answers, plans and reports, in the Markdown preview and in PDFs no longer open any app with a
  single click. Web addresses open in a browser tab, files inside the project in an editor tab; anything else, such as
  a file elsewhere, a network share or another app's link, asks first and shows where it leads.
- Sign-in and Remote Control addresses that an agent or an MCP server hands over open only when they are web
  addresses.
- Folders that hold far more than one project, such as your home folder, Desktop, Documents, Downloads or the root of
  a volume, can no longer be trusted as a whole; trust them project by project. The trust question now says that it
  covers the folders inside, too.
- CI tokens and sign-ins never go out over plain http to a server other than this Mac; a repository whose remote
  starts with `http://` is asked over https.
- Without gitleaks, edit predictions wait before code goes to a provider outside this Mac until you choose to send
  unchecked. A project's own gitleaks settings no longer switch the check off.
- The own agent can no longer be led to write outside the project through a link that appears after its write was
  approved, and files with further hard links get a file of their own instead of being written through.
- A language server's completions can no longer hang the editor, and its renames and deletions of a linked file act
  on the link, not on what it points to.
- Pictures you attach to a prompt are decoded by the sandboxed decoding service, as everywhere else.
- A program in the terminal can replace the clipboard only while its terminal is in front, and the terminal says so.
- rust-analyzer installs from a release pinned with its checksum instead of whatever was latest.

## Search and navigation
- Open Quickly says when no file matches and shows whether it searches files or the project's symbols.
- The project's symbols include TypeScript and JavaScript without a file of theirs open. Exact matches from the
  project and in the language you work in come first; symbols from libraries and installed packages are marked.
- View › Search puts the focus into the search field.

## Editor
- The editor keeps the focus when a Markdown file opens with its preview beside it, so the Format menu works at once.
  Editor commands also work while the focus is in the sidebar.
- A proposal at the end of a line or in a box below it says that Tab takes it.
- Tabs of the same name in the tab overview are told apart, for example a file and its preview in the browser.
- The View menu no longer offers macOS's own "Hide Tab Bar", which Dango's tabs do not use.

## Languages and debugging
- The JSON server now checks files, and servers that register their diagnostics after starting, such as basedpyright
  and sourcekit-lsp, show them.
- Where pip refuses to install into Python, as Homebrew's does, the debugger offers to install debugpy into a virtual
  environment of the project.
