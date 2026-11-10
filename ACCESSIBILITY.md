# Accessibility

Dango is built on AppKit with system controls wherever possible, so VoiceOver, Full Keyboard Access, Increase
Contrast and the system colours work as in other Mac apps. This file says what the app adds on top, area by area, and
what does not work yet. It describes the state of the code in development (version 0.1.0); the formal acceptance with
VoiceOver is part of the release phase.

How it is checked: the self-test scenario `a11y` walks the windows with the accessibility API (names, roles, values,
the Tab order) in German, `a11y-rtl` does the same in Arabic and checks that code views stay left to right in the
right-to-left interface; the other scenarios draw the windows in light,
dark and both high-contrast appearances and check the VoiceOver summaries of the AI tab's cards. VoiceOver itself is
checked by hand.

## VoiceOver

| Area | What VoiceOver gets |
|---|---|
| Window | The sidebar's symbols (Files, AI, Git, CI, Settings) have names, and a status on a symbol is part of its name, such as "Git, 4 changes" or a failed CI run. Documents are macOS's own window tabs, which VoiceOver reads with their title; the icon before it is decoration, and a preview tab says so in its help. Two documents side by side each have a header with their name and a named close button. The short notices at the bottom of the window are announced. Decorative images (file icons, the app icon on the welcome page) are hidden. |
| Files | A standard outline view. |
| Editor | A standard text view named "Editor: <file>", with lines, selection and text attributes as usual. Drawn-only layers (whitespace marks, overlays) are hidden from VoiceOver. The change marks name the committed version they compare against. When a file changes on disk while it is open, the bar that offers to reload or merge it is announced. |
| Compare and diff views | Each line says whether it was added or removed; the table is named after the file. |
| Terminal | The terminal grid is a text area: VoiceOver reads its lines, the selection and the line of the cursor, and is told when the screen changes while the terminal has the focus (at most twice a second). Its name is the terminal's title. |
| Project search and Open Quickly | The search options say what they switch (case, whole word, regular expression), not "Aa", "ab", ".*". The chevron before the field says it shows or hides replacing; a result's replace button, which appears under the pointer, is also in the result's context menu. The Open Quickly window has a title and its result list a name. |
| AI tab | The timeline is a table ("Session Timeline"). Each card is one group that VoiceOver reads as a summary, for example "Tool Bash: Run the tests, done", "Permission for Write", "Plan for Approval" or what became of a plan; the buttons inside a card are reached by interacting with the group. The message field is named after the agent ("Message to Claude"); the status line (context, cost, state) is a group with its value; the `@` and `/` suggestions are a named list. A permission's editable input is named as such. An attachment whose text does not come from the user reads "External Content", with the explanation of prompt injection as its help. |
| AI panel and Activity | The session list and the list of running sessions and tasks are named. In the Activity overview, each button names its action together with the line it belongs to. |
| Git panel | File rows say the file and its state. The actions that otherwise appear on hover (show diff, stage, unstage, discard) are VoiceOver actions of the row. The branch menu, the commit message and the state of a running push or pull are named. |
| CI panel | Each run is spoken with its state; a job's log tab is named after the job. |
| Browser | Back, forward, reload, the address field and the loading indicator are named; downloads are a named group. Web pages are WebKit's own accessibility. |
| Dictation | The microphone buttons say whether they start or stop and read the state (listening, transcribing) as their value; the status lines show the same in words. |
| Settings | Tables are named after their first column; controls such as the font size stepper and the search engine menu are named by the label beside them. In the Agents tab, each button names its agent as well as its action. |

## Keyboard
Every command has a shortcut in one table, which `keybindings.json` in the app's data folder can override (see
[README.md](README.md#where-data-lives)). Tab moves through the window's lists and fields as they are shown, including
the file tree, the search results, the CI list and the agents' outlines. Buttons and links are reached with Tab only
when Keyboard navigation is on (System Settings › Keyboard), as in every AppKit app; the title bar and toolbar with
⌃F5. Where Tab is text, in the editor and the AI tab's message field, ⌃Tab moves on, as in every macOS text view;
in the message field ⌃⇧Tab goes back.

| Moving around | Shortcut |
|---|---|
| Files, Search, AI, Git, CI in the sidebar, with the focus moving into the panel; again to fold the sidebar | ⌘1, ⌘2, ⌘3, ⌘4, ⌘5 |
| Fold or show the sidebar | ⌃⌘S |
| Next, previous tab | ⇧⌘], ⇧⌘[ |
| The other document, when two share a tab | ⌥⌘` |
| New tab (the page that offers what a tab can show), close tab, close all tabs | ⌘T, ⌘W, ⇧⌘W |
| Into the AI tab's message field and back | ⌘Esc |
| Comment on the plan in front, or on the passage selected in it | ⌥⌘K |
| New terminal | ⌃` |
| Browser address field | ⌘L |
| Quick open, search the project, go to symbol | ⌘P, ⇧⌘F, ⇧⌘O |
| Next, previous problem | ⌘', ⇧⌘' |
| Start or stop dictation in the focused field (release builds) | ⌃⌥D |

In the AI tab's timeline, Space or Return opens and closes the selected card's details, and Esc stops the agent's
turn (background work goes on). With a plan's row selected, Tab goes into the plan's text and ⇧Tab back to the
timeline; a plan opened as a document takes the focus. In either, ⇧ with the arrow keys selects a passage and ⌥⌘K
comments on it. In the Git panel's list, Space stages or unstages the selected files, on a section's row all of its
files; in the commit message, Tab moves on and ⌥Tab types a tab. In the terminal, ⌃C and Esc go to the program in
it; ⌘K clears the terminal. The activity overview keeps the focus on its buttons while its times run on, and on the
same row when rows come or go, else on the nearest row left.

## Increase Contrast
With Increase Contrast on, macOS gives the app its high-contrast appearances, and the app follows them:
- System colours, which most of the interface uses, adapt by themselves.
- The editor's syntax colours and the AI tab's accent colour have their own high-contrast variants, in light and dark.
- Diffs (Git panel and compare views) use stronger backgrounds for added and removed lines.

## Reduce Motion
The interface has few animations: the AI timeline grows without animating, and progress indicators and popovers are
the system's. With Reduce Motion on, the terminal's visual bell sounds the alert sound instead of flashing, and the
microphone button does not pulse while it hears sound.

## Right-to-left interfaces
The layout follows the interface language's direction. Code and machine text stay left to right in every
interface: the editor with its line numbers and scroller, the terminal, compare and diff views, project search
results, CI logs, the address field, and code, tool input and output in the AI tab. In the AI tab's answers, each
paragraph takes the direction of its own text. The `a11y-rtl` self-test checks this in Arabic.

## Known gaps
- **Checked by hand only:** the self-test cannot switch on Keyboard navigation, so for buttons it checks that they are
  in the Tab order, not that Tab lands on them. Contrast and Reduce Motion are not covered by the `a11y` scenario.
