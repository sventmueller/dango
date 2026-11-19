## AI tabs
- "Show Output" of a background command opens a tab that follows the output while the command runs, instead of a
  copy of it at that moment. The tab opens at the end of the output and stays there as new lines come, until you
  scroll up; scrolling back to the end follows again. "Show Output" on a tab that is open brings it to the front at
  the end.
  - A command that writes its output to a file itself, such as `make > build.log 2>&1`, shows that file as it grows.
  - Once the command ends, the tab shows its whole output; "Open in Editor" opens the file behind it.
  - A tab reads the output only while it is in front.
