## Memory
- Long agent sessions need much less memory:
  - Opening a past session reads its transcript one line at a time; a session with a 100 MB transcript now peaks at
    about 20 MB instead of 65 MB.
  - A finished tool call keeps only what its card shows: the first 400 lines of its output, and no longer the raw
    input once the card shows its fields. "N more lines" still counts every line.
  - An AI tab that stays out of view for 30 seconds lets go of its cards, and when it shows again it builds those in
    view, at the place it was.
  - After a tab closes or a past session opens, Dango hands the memory it freed back to macOS.
- Lists no longer grow in memory as they refresh. On macOS 27 every refresh of a list with rows that open left a
  disclosure triangle behind for good, and every refresh left a little of each row behind:
  - The CI, Containers, Software Factory, Tests, Debug, Git and Search lists and the call hierarchy draw their own
    chevrons.
  - Lists out of view no longer refresh; they catch up when they show.
  - The Sessions, CI, Software Factory and Containers lists keep the rows that did not change, and Git refreshes only
    the rows of files whose state changed.
- The panels of a closed window are let go with it.

## AI tabs
- "Show Output" of a background command that writes its output to a file itself, such as `make > build.log 2>&1`,
  shows that file when the CLI has none of the output.
