## Terminal
- The terminal is now Tapa's, Dango's sibling terminal app, with everything it adds while Dango keeps what its terminal did before: image paste to agents, dev servers in the browser tab, ⌘-click into the editor.
- **View › Show/Hide Input Field** (⌘I while a terminal has the focus) turns the command line into a text field with the folder and the Git branch above it. Commands are coloured by their parts, and one your shell cannot run gets a dashed red underline.
- Suggestions show in grey as you type, in the field and without it: from your history, from specs for more than 700 command-line tools and from the project, such as git branches, `package.json` scripts and make, just and cargo targets. → takes one; Tab completes and, with several candidates, opens a menu that typing filters. They never use the network.
- Command blocks: ⌘↑ and ⌘↓ jump between prompts, **Edit** selects, copies and saves a command's output, and bookmarks mark lines. With **Mark commands as blocks** on, a bar shows each block, failed commands are marked and buttons over a block show its status and duration.
- **Send Block to Agent** (Edit menu and the terminal's context menu) opens an AI tab with the command line and output of the current block attached as External Content.
- **Find** (⌘F while a terminal has the focus) searches the whole history with regular expressions, case and whole words; the scroll bar marks the matches.
- A program's notification (OSC 9 and OSC 777) and its bell put the red dot on the terminal's tab while you do not see it, and the notification goes to macOS while another app is in front. Progress a program reports (OSC 9;4) shows as a ring on its tab and a bar on the Dock icon.
- A double click selects a whole URL, path or hash, ⌥-drag selects a rectangle, **Copy with Formatting** keeps colours and styles, and **Select with the Keyboard** moves a selection with Vim's keys.
- Multi-line pastes that would run at once show first, and **Dango › Secure Keyboard Entry** keeps other apps from reading what you type into a terminal.
- Links of kinds other than web addresses, `mailto:` and files no longer open from the terminal.

## Settings
- **Settings › Terminal** switches the input field for new terminals, its Vim keys, the suggestions, the colours of commands, the underline of unknown ones, command blocks, copying the selection at once, showing multi-line pastes first and ⌥ as the Meta key, which moved here from General.
