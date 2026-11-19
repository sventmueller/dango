## Speed and memory
- Everything an agent starts, in any language, such as builds, tests, shells and MCP servers, runs in macOS's
  background state: the lowest priority, with disk and network access throttled. The agents themselves, with Claude's
  subagents, run at medium priority. Agents at full load no longer slow Dango down; this applies to the agents of the
  Software Factory as well.
- Restored windows start only what shows: an AI tab behind others starts its CLI when it comes to the front or gets a
  prompt, and browser, site preview and terminal tabs load their page or start their shell when they first show.
- A long project search no longer draws all its results again while it runs. The Software Factory, CI logs and the
  Tests panel do less while out of view; tests and debug configurations are read when their panel first shows, and
  hidden CI and Containers panels read nothing until they show.
- Long sessions need less memory: a finished diff keeps the lines its card shows, an answered permission request lets
  go of its input, the terminal's glyph cache no longer grows without bound, and the Files and Git panels let go of
  rows that are gone.
- The program itself is about 40 % smaller.

## Windows and menus
- Closing a pane moves its tabs into the pane before it, else the one after it, instead of closing them. Only the pane
  of a preview and the last pane close with their tabs.
- The View menu lists the panels in the order of the sidebar's symbols.
- "Manage Agents …" left the + menu; it stays under "+ New Session" in the Agents panel.

## Fixes
- "Include and Exclude Files" in the search panel no longer shows a stray letter beside its arrow.
- Closing the comment popover of a specification no longer crashes.
