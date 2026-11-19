## AI tabs
- An AI tab that has been out of view for three minutes lets its Claude process go while the session is idle. The
  conversation and what you typed stay in the tab, and the next prompt resumes the session. A tab keeps its process
  while Claude works, a prompt waits, a question is open, a subagent, background task or command still runs, or
  Remote Control is on.
- Ending an agent also ends the commands it started in a process group or session of their own, which could outlive
  it before.
- The timeline adds new entries at its end instead of indexing all earlier rows again.

## Speed
- Restored windows open faster: a file tab without unsaved changes loads its file only when it comes to the front.
- A project search whose panel is hidden pauses and continues when the panel shows.
- The Git change markers of the editor run at most one comparison at a time per file; edits during it are merged into
  one follow-up, and the file's committed text is read again only when the commit changed.
- The minimap of a file with folded code keeps one entry per unfolded stretch instead of one per line.

## Search
- The filter chevron lines up with the replace chevron above it.
