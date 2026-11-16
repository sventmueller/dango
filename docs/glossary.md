# Glossary

The terms of the interface in each of the app's languages (E15), so that every translation of
`resources/<lang>.lproj/Localizable.strings` uses one word for one thing. The keys are the German texts in the code;
`en.lproj` gives the English meaning. Each language below has its own table; a translation that departs from it
changes the table first.

## Rules for every language
- **Placeholders:** every `{}` of the key stays, as many as the key has and in the same order, since the code fills
  them one after the other. Where the language wants another order, the sentence is rephrased.
- **Symbols and names:** `⌘ ⌥ ⇧ ⌃ ↩ ⎋`, key names such as Esc, and file names, commands and flags stay as they are.
  Product and service names are never translated: Claude, Codex, Gemini, Antigravity, Anthropic, OpenAI, OpenRouter,
  Ollama, LM Studio, GitHub Copilot (Copilot), Lowspeak, Git, GitHub, GitLab, ACP, MCP, CI, OAuth, PATH, JSON,
  Finder, macOS, Mac, claude.ai, Remote Control (Anthropic's feature of that name).
- **The app's name** never appears in a translation; where a key names the app, it has a `{}` for it.
- **macOS first:** menu items, buttons and system words that macOS has (the Standard terms below) take the wording
  of macOS in that language, letter for letter, including how it writes the ellipsis of an item that opens a
  dialog and whether a space goes before it.
- **Address:** as macOS addresses the user in that language; the German keys say "du" because German macOS does.
- **Typography:** the quotation marks, apostrophes and number formats of the language as macOS uses them.
- **Length:** buttons, menu items, column titles and tab names stay about as short as the English; explanations may
  run longer.
- **Case:** the casing macOS uses in that language; title case in menus is an English habit only.
- **Escapes in `.strings`:** `\"` for a quote inside a text, `\\` for a backslash, `\n` for a line break, as many line
  breaks as the English.
- **Reuse:** a key that splat (`../splat/resources`) already translates takes splat's translation, so both apps say
  the same, unless splat's text means something else here or departs from macOS. So far that is the case for
  Zeilenumbruch (here Wrap Lines), Neu (the diff's column of new lines), Weiter (Continue), Übernehmen (Use Key),
  Hinweis (the language server's severity Hint) and Freigeben (Approve, of a specification) in every language, Im
  Finder zeigen in ar, ca, ro, sl and zh-Hant, Rückwärts suchen in sk, Vollbildmodus in vi and Zoomen in ja.

## Terms
What each term means in this app. The Standard terms are macOS's own; the others are the app's.

| German key | English | Meaning |
|---|---|---|
| **Standard** | | |
| Ablage | File | the menu |
| Bearbeiten | Edit | the menu |
| Darstellung | View | the menu |
| Fenster | Window | the menu |
| Hilfe | Help | the menu |
| Dienste | Services | the submenu of the app menu |
| Über {} | About {} | app menu |
| Einstellungen | Settings | app menu and the settings window |
| {} ausblenden / Andere ausblenden / Alle einblenden | Hide {} / Hide Others / Show All | app menu |
| {} beenden | Quit {} | app menu |
| Widerrufen / Wiederholen | Undo / Redo | Edit menu |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cut / Copy / Paste / Delete | Edit menu |
| Alles auswählen | Select All | Edit menu |
| Suchen … / Weitersuchen / Rückwärts suchen | Find… / Find Next / Find Previous | Find submenu |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimize / Zoom / Bring All to Front | Window menu |
| Vollbildmodus | Enter Full Screen | View menu |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Hide Toolbar / Toggle Sidebar | View menu |
| Sichern | Save | write a file (macOS German says "Sichern") |
| Öffnen / Schließen | Open / Close | |
| Abbrechen / OK | Cancel / OK | buttons |
| Im Finder zeigen | Show in Finder | |
| Papierkorb | Trash | |
| Schlüsselbund | Keychain | where keys and tokens are kept |
| Zwischenablage | Clipboard | |
| **Window and editor** | | |
| Tab | tab | one document, terminal, browser page or AI thread in a panel |
| AI-Tab | AI tab | the tab of an agent's thread |
| Panel | panel | a part of the window beside the editor, such as the Git panel |
| Seitenleiste | sidebar | files, agents, Git, CI on the left |
| Vorschau | preview | a tab opened by a single click, replaced by the next one; also the rendered view of a Markdown, diagram or PDF file |
| Bereich | pane | a part of a window tab beside or above another, holding documents behind one another, with its own tab bar while one pane holds more than one document (E145) |
| Aktivität | Activity | the list of running processes, sessions and tasks |
| Projekt / Ordner | project / folder | the folder a window shows |
| Vertrauen, vertrauen | trust | the user allows a folder to run what it configures |
| Platte | disk | "on disk": the file as stored, against the editor's text |
| Unterschiede | differences | a diff |
| Zusammenführen | merge | of changes, also Git's merge |
| Konflikt | conflict | |
| Treffer | match | of a search |
| Suche | search | the sidebar panel that searches the project |
| Ersetzen | replace | in the search panel and in a file |
| Sprachserver | language server | LSP |
| Hinweis (Schweregrad) | hint | the language server's lowest severity, after Fehler, Warnung and Info |
| Inlay-Hinweis | inlay hint | what the language server shows inside a line without it being in the file, such as an inferred type (E83) |
| Minimap | minimap | the column at the editor's trailing edge that shows the whole file small, with the visible part marked |
| Sticky Scroll | sticky scroll | the headers of the blocks enclosing the top line, pinned at the editor's top |
| Diagnosen am Zeilenende | diagnostics at line end | a line's most serious diagnostic shown after its end (E130) |
| Vim-Modus | Vim mode | the editor's modal editing after Vim, switched on in the Editor menu (E104–E106) |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK | Vim's mode in the status bar, in capitals and short as Vim shows it; a language whose Vim has its own message catalog takes the names from there (for Normal, which Vim leaves blank, the word the catalog uses for that mode), the others keep Vim's English names |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Pattern not found: {} … | Vim's messages in the status bar, with Vim's error number in front so that `:help E486` finds it; a language whose Vim has its own message catalog takes the wording from there, the others translate Vim's English; what names an option or a value Dango has not (`'modifiable'`, the register's name) is left out, and a message Vim does not have (no selection, unknown digraph, what the Vim mode cannot do) has no number, nor has Befehl nicht unterstützt: {}, which docs/phase-12.md words for every Ex command beyond the few the Vim mode knows |
| Vorschlag | suggestion | what edit prediction proposes to write next, in grey until Tab takes it (E92) |
| Vorhersage | Prediction | edit prediction: making suggestions while typing, and its settings tab |
| Ausschlussliste | exclusion list | patterns of file names that never go to a provider |
| Blame | blame | who changed a line last, at the line's end or in a column before the line numbers (E86); kept as git's word |
| Hinweise (Rotor) | Hints | the VoiceOver rotor that lists an editor's inlay hints and blame |
| Schnellkorrektur | quick fix | a change the language server offers for the place of the cursor, such as a missing import |
| Refactoring | refactor | a change of the code's structure the language server offers, such as extracting a function |
| Quellaktion | source action | a change of the whole file the language server offers, such as sorting its imports |
| Implementierung / Typdefinition / Deklaration | implementation / type definition / declaration | where the language server finds a trait's or interface's implementations, the type of a symbol, and its declaration apart from its definition |
| Aufrufhierarchie / Typhierarchie | call hierarchy / type hierarchy | the tree of a function's calls or of a type's relatives in the search panel |
| Aufrufer / Aufgerufene | callers / callees | the two directions of the call hierarchy |
| Obertypen / Untertypen | supertypes / subtypes | the two directions of the type hierarchy |
| indexieren | index | a language server reads the project before it can answer; "Wird indexiert …" |
| Lesezeichen / Verlauf / Adresse | bookmark / history / address | of the browser tab |
| Bytes | bytes | the unit |
| Exportieren | export | the submenu that writes Markdown as PDF, HTML or Word |
| Format | format | the menu of Markdown formatting |
| Diktat | dictation | speech to text |
| **Agents** | | |
| Agent | agent | a coding agent such as Claude, Codex or Gemini |
| Eigener Agent | own agent | the agent built into the app |
| Eigene Agenten | custom agents | the subagents a user defines for Claude, not the own agent |
| Thread (AI-Thread, CLI-Thread) | thread | one conversation with an agent as the app shows it |
| Sitzung | session | a running conversation with an agent's process |
| Subagent, Subagenten | subagent | an agent that the session's agent starts for part of its work |
| Hintergrundprozess | background process | a shell command or monitor an agent keeps running beside its turn; a kind of Aufgabe |
| Unterhaltung | conversation | the messages of a thread |
| Zug | turn | one message of the user and the agent's work on it |
| Antwort | answer | the agent's reply |
| Rückfrage | question | the agent asks the user |
| Freigabe, Freigabemodus | permission, permission mode | the agent asks before it uses a tool |
| Umgehen | bypass | the permission mode that never asks |
| Erlauben / Ablehnen / Immer erlauben … | Allow / Deny / Always Allow… | answers to a permission |
| Plan | plan | the agent's plan before it acts |
| To-dos | to-dos | the agent's to-do list |
| Aufgabe | task | work an agent runs in the background |
| Auftrag | assignment | what a subagent was asked to do; not Aufgabe |
| Werkzeug | tool | what an agent may use: shell, files, web, MCP |
| Skills | skills | Claude's skills |
| Befehl | command | a shell or slash command |
| Hook | hook | Claude's and Git's hooks |
| Sandbox | sandbox | the limits an agent runs in |
| Fremder Inhalt | external content | text from a web page, a terminal, a CI log or a file outside the project, given to the agent as data |
| Sicherungspunkt | checkpoint | the project's state taken before each prompt |
| Zurückspulen | rewind | back to a checkpoint or an earlier message |
| Verdichten | compact | shorten the conversation to fit the context |
| Kontext, Kontextfenster | context, context window | |
| Denktiefe | effort | how long the model thinks |
| Modell / Anbieter / Endpunkt | model / provider / endpoint | |
| Token, Tokens | token, tokens | |
| API-Schlüssel | API key | |
| Register | registry | the ACP registry of agents |
| Adapter | adapter | the program that connects an agent's CLI over ACP |
| Debug-Adapter | debug adapter | the program that speaks the Debug Adapter Protocol for one language, such as lldb-dap |
| Bereit | ready | an agent whose program is there and can start |
| Prüfsumme | checksum | |
| Anmeldung, anmelden | sign-in, sign in | with an agent's or a forge's service |
| Konto | account | |
| Abo | subscription | a paid plan of an agent's service, used up against its limits rather than per token |
| **Git and CI** | | |
| Repository | repository | |
| Commit, committen | commit | |
| Secrets | secrets | passwords and tokens the secrets check finds before a commit |
| vormerken | stage | |
| Zurücknehmen | unstage | the opposite of vormerken |
| nicht versioniert | untracked | |
| Abschnitt (Git) | hunk | a block of changed lines |
| Branch / Remote | branch / remote | |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase | Git operations |
| Worktree | worktree | a second working copy for a thread |
| Stash, Stashes | stash, stashes | changes parked with `git stash` and applied again later; one entry is a Stash |
| Übernehmen (Worktree) | take over | bring a worktree thread's work into the user's branch |
| Arbeitskopie | working tree | |
| Lauf, Läufe | run, runs | of CI |
| Host | host | the server of a CI service, such as a self-managed GitLab |
| Job | job | of CI |
| Log | log | |
| **Debugger** | | |
| Debuggen | Debug | the menu, the sixth panel and the act of running a program under the debugger (E94) |
| Haltepunkt, Haltepunkte | breakpoint, breakpoints | where a debug session pauses the program, set beside a line number |
| Logpunkt | logpoint | a breakpoint that writes a message to the debug console instead of pausing |
| Bedingung / Trefferzahl | condition / hit count | when a breakpoint pauses: an expression that must be true, how often it must be hit |
| Aufrufliste | call stack | the frames of a paused thread, innermost first |
| Frame | frame | one level of the call stack, a function with its variables; kept as the developers' word |
| Überwachter Ausdruck, Überwachte Ausdrücke | watch expression, watch expressions | an expression the inspector evaluates whenever the program pauses |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continue / Step Over / Step Into / Step Out | the debugger's commands, as Visual Studio, VS Code or JetBrains IDEs name them in the language where one of them is localized |
| Anhängen / Trennen | attach / detach | joining a running program, and leaving it running when the session stops |
| An Prozess anhängen | Attach to Process | the Debug menu item that joins a running program |
| Neu starten / Stopp | Restart / Stop | the debugger's commands for the session; the same words restart a language server and stop an agent's turn |
| Variablen | variables | the variables of the paused frame, listed in the inspector |
| Angehalten | paused | a program the debugger stopped, as at a breakpoint |
| Debug-Konsole | debug console | the tab with the program's output and the expressions evaluated in the paused frame |
| Startkonfiguration | launch configuration | how a program is started for debugging, as `.vscode/launch.json` describes it |
| Inspektor | Inspector | the column at the window's trailing edge with details, ⌥⌘I (macOS's word) |
| **Tests** | | |
| Tests | Tests | the panel that lists a folder's tests by package and file with their state, fifth in the sidebar (E111) |
| Test | test | one test of the project, run or debugged from the Tests panel, the gutter or the Debug menu; also the Activity's kind of a test run |
| **Containers** | | |
| Container | Containers | the panel that shows a Docker context's Compose projects, containers, images and volumes, eighth in the sidebar (⌘8), and its menu (E136) |
| Docker-Kontext | Docker context | the Docker engine a window's Containers panel talks to, as `docker context` names it; chosen per window |
| **Leitstand (phase 13)** | | |
| Leitstand | Control Room | the ninth panel (⌘9) and the tab with every work order of a folder, and the factory behind them (E175) |
| Auftrag (Leitstand) | work order | a goal with testable criteria that the factory plans, builds and checks in a worktree of its own; distinct from Auftrag, what a subagent was asked to do |
| Nachweis | evidence | what proves a work order's state: gates, the reviewer's findings, cost and provenance, its deviations first (E163) |
| Abweichung | deviation | something the evidence names before everything else, such as secrets not checked |
| Abnahme, abnehmen | acceptance, accept | G2: the user accepts the evidence, whole or hunk by hunk (C55); also the station of that name |
| Freigeben (Spezifikation) | approve | G1: the user approves the specification |
| Planer | planner | the role that writes the specification, read only (E162) |
| Builder | builder | the role that writes in the work order's worktree |
| Prüfer | reviewer | the role that judges against the criteria, read only, of another vendor than the builders |
| Späher | scout | the role that reads and sums up, cheap or local |
| Prüfstation, Gate | gate | an automatic check of the review: path scope, secrets, gate commands (E163) |
| Prüfbefehl | gate command | a command `.dango/factory.toml` names, which a gate runs in the sandbox |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Intake / Specification / Build / Review / Acceptance / Take-Over / Done | the stations of a work order, in their order (E166) |
| Risikoklasse | risk class | low, medium or high, expected by the user and measured from the diff |
| Pfadbereich | path scope | the paths a work order may change |
| Herkunft | provenance | which role, agent and model made each commit of a work order |
| Station | station | one step of a work order's way from Eingang to Erledigt |
| Kriterium, Kriterien | criterion, criteria | a testable condition of a work order's goal, one per line |
| Prüfrunde | review round | one pass of the reviewer, with the builder's rework after it |
| Befund | finding | what the reviewer reports on one criterion |
| Hersteller, Herstellerwechsel | vendor, change of vendor | the company behind an agent's models, such as Anthropic or OpenAI; the reviewer is of another vendor than the builders |
| Kontingent | quota | how much of a service's usage window is left, as the service reports it |
| Workflow-Datei | workflow file | `.dango/factory.toml`, the project's rules for its work orders, shared with the team |
| Stand | state | the commit of a work order that gates, reviewer and acceptance refer to |
| Erstdurchlaufquote, Erstdurchlauf, Quote | first-pass rate, first pass, rate | the share of work orders whose first review round passed, by role, agent, model and risk class; work orders with interventions or a change of agent do not count; routing uses it to break a tie (E180) |
| Kennzahlen | figures | the table of the Leitstand tab with first-pass rate, review rounds, cost and runtime, and the median wait for the user (E180) |
| Regelvorschlag | rule proposal | a work order in which a scout proposes a change to AGENTS.md or CLAUDE.md from recurring findings, waiting for approval and acceptance like any other |
| wiederkehrender Auftrag | recurring work order | a work order the workflow file creates by itself at an interval, only while the app runs |
| Abstand (wiederkehrender Auftrag) | interval | how often a recurring work order comes: täglich, wöchentlich, alle {} Tage |
| eingereiht | queued | a work order that waits its turn: for a free slot, for a work order whose path scope overlaps, or for the take-over into the same branch ahead of it (E174) |
| Rolle | role | planner, builder, reviewer or scout, the part an agent plays in a work order |
| Einwand | objection | the builder's answer in its first turn that the criteria do not hold; the work order waits until the user changes them, continues or aborts (E178) |
| Rangliste | ranked list | a role's ordered entries of agent, model and effort; a work order takes the first one that is ready (E183) |
| Prüfer-Regel | reviewer rule | whether the reviewer must (Pflicht) or should (bevorzugt) be of another vendor than the builders (E183) |
| Vorgabe, auf Vorgabe zurücksetzen | default, reset to default | a value from the user's settings, as against one the workflow file sets; the dialog of a folder's Leitstand names it as the value's origin (E183) |
| Mitkopieren / Einrichtung | copy along / setup | the files copied into a work order's worktree, and the command that prepares the worktree (E188) |
| Prüfrelevante Dateien | files that matter for the review | path patterns of tests and configuration whose changes raise the measured risk class to high (E183) |
| Issue | issue | an issue on GitHub or GitLab, which a work order can come from (E176) |
| Merge Request | merge request | what an accepted work order is submitted as: GitLab's merge request, GitHub's pull request; worded as the language's CI panel words it (E177) |
| **Agent configuration (E146)** | | |
| Rechte | Permissions | the overlay with an agent's permission rules; the rules are Freigaben in the sense of the Freigabe term |
| Erlauben / Fragen / Verbieten | Allow / Ask / Deny | the effect of a permission rule; Verbieten is the language's word for denying or forbidding an action, Fragen the agent asking first |
| Gedächtnis | Memory | what an agent keeps across sessions, and the overlay with its instruction files |
| Anweisungen | Instructions | files such as `CLAUDE.md` and `AGENTS.md` an agent reads as standing instructions |
| Ausgabestil | Output style | how an agent words its answers, a setting of Claude Code |
| Auswahl | Matcher | the pattern of a hook that selects the tool calls it runs for |
| Plugin, Plugins | plugin, plugins | an extension of an agent installed from a marketplace |
| Marktplatz, Marktplätze | marketplace, marketplaces | a catalog plugins are installed from |
| Frage nebenbei | Side question | `/btw`: a question the agent answers beside the conversation, which neither joins it |
| Header | Headers | the HTTP headers sent to an MCP server |
| Umgebung | Environment | the environment variables a program or MCP server starts with |
| Bericht | Report | a bug report or idea sent to Anthropic or OpenAI with `/bug` |
| Grenzen | Limits | the usage limits of a plan, as `/status` lists them |

## Languages

### Arabic (`ar`)
- Address: second person singular in the generic masculine form, as macOS does ("هل تريد…؟", "اختر", "أدخل"); menu items
  and buttons are verbal nouns ("فتح", "حفظ", "إظهار الشريط الجانبي")
- Typography: “…” quotation marks (U+201C/U+201D in logical order, as macOS's Arabic texts write them); Arabic comma
  "،", semicolon "؛" and question mark "؟"; the ellipsis of an item that opens a dialog follows the word without a space
  ("الإعدادات…", "فتح…"); Western digits; a right-to-left mark (U+200F) opens a label that would otherwise begin with a
  placeholder or Latin text, and a left-to-right mark (U+200E) keeps a leading or trailing dot of a file name or command
  on its side; Latin names and commands stay in Latin script inside the right-to-left text
- Git and CI words: Arabic for the concepts that have settled Arabic words (مستودع repository, فرع branch, المستودع
  البعيد remote, دمج merge, تعارض conflict, تجهيز stage, مقطع hunk, نسخة العمل working tree, غير متتبَّع untracked,
  تشغيل run, وظيفة job, سجل log, ضم for taking a worktree's work over); the Git operations stay as Latin loanwords as
  Arabic developers say and type them (Commit, Push, Pull, Fetch, Rebase, Revert, Cherry-Pick, Upstream, Worktree), with
  an Arabic verbal noun where a sentence needs a verb ("إنشاء Commit", "تنفيذ Push", plural "عمليات Commit"); "AI" stays
  in Latin, as the German key keeps it ("سلسلة AI")

| German key | Arabic |
|---|---|
| Ablage | ملف |
| Bearbeiten | تعديل |
| Darstellung | عرض |
| Fenster | نافذة |
| Hilfe | مساعدة |
| Dienste | خدمات |
| Über {} | حول {} |
| Einstellungen | الإعدادات |
| {} ausblenden / Andere ausblenden / Alle einblenden | إخفاء {} / إخفاء الأخرى / إظهار الكل |
| {} beenden | إنهاء {} |
| Widerrufen / Wiederholen | تراجع / إعادة |
| Ausschneiden / Kopieren / Einsetzen / Löschen | قص / نسخ / لصق / حذف |
| Alles auswählen | تحديد الكل |
| Suchen … / Weitersuchen / Rückwärts suchen | بحث… / بحث عن التالي / بحث عن السابق |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | تصغير / تكبير/تصغير / إحضار الكل إلى الأمام |
| Vollbildmodus | الدخول إلى وضع ملء الشاشة |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | إخفاء شريط الأدوات / إظهار الشريط الجانبي أو إخفاؤه |
| Sichern | حفظ |
| Öffnen / Schließen | فتح / إغلاق |
| Abbrechen / OK | إلغاء / موافق |
| Im Finder zeigen | إظهار في Finder |
| Papierkorb | سلة المهملات |
| Schlüsselbund | سلسلة المفاتيح |
| Zwischenablage | الحافظة |
| Tab | علامة تبويب |
| AI-Tab | علامة تبويب AI |
| Panel | لوحة |
| Seitenleiste | الشريط الجانبي |
| Vorschau | معاينة |
| Bereich | جزء |
| Aktivität | النشاط |
| Projekt / Ordner | مشروع / مجلد |
| Vertrauen, vertrauen | الثقة، الوثوق (يثق) |
| Platte | القرص ("على القرص") |
| Unterschiede | الفروقات |
| Zusammenführen | دمج |
| Konflikt | تعارض |
| Treffer | نتيجة، نتائج |
| Suche | بحث |
| Ersetzen | استبدال |
| Sprachserver | خادم اللغة |
| Hinweis (Schweregrad) | تلميح |
| Inlay-Hinweis | تلميح مضمّن |
| Minimap | الخريطة المصغرة |
| Sticky Scroll | التمرير المثبّت |
| Diagnosen am Zeilenende | التشخيصات في نهاية السطر |
| Vim-Modus | وضع Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: لم يُعثر على النمط: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | اقتراح |
| Vorhersage | التنبؤ |
| Ausschlussliste | قائمة الاستبعاد |
| Blame | Blame |
| Hinweise (Rotor) | التلميحات |
| Schnellkorrektur | إصلاح سريع |
| Refactoring | إعادة هيكلة |
| Quellaktion | إجراء على المصدر |
| Implementierung / Typdefinition / Deklaration | التنفيذ / تعريف النوع / الإعلان |
| Aufrufhierarchie / Typhierarchie | تسلسل الاستدعاءات / تسلسل الأنواع |
| Aufrufer / Aufgerufene | الدوال المستدعية / الدوال المستدعاة |
| Obertypen / Untertypen | الأنواع الأعلى / الأنواع الفرعية |
| indexieren | الفهرسة |
| Lesezeichen / Verlauf / Adresse | إشارة مرجعية / السجل / العنوان |
| Bytes | بايت |
| Exportieren | تصدير |
| Format | التنسيق |
| Diktat | الإملاء |
| Agent | وكيل |
| Eigener Agent | الوكيل المدمج |
| Eigene Agenten | الوكلاء المخصصون |
| Thread (AI-Thread, CLI-Thread) | سلسلة (سلسلة AI، سلسلة CLI) |
| Sitzung | جلسة |
| Subagent | وكيل فرعي |
| Hintergrundprozess | عملية في الخلفية |
| Unterhaltung | محادثة |
| Zug | دور |
| Antwort | رد |
| Rückfrage | سؤال |
| Freigabe, Freigabemodus | إذن، وضع الأذونات |
| Umgehen | تجاوز |
| Erlauben / Ablehnen / Immer erlauben … | سماح / رفض / السماح دائمًا… |
| Plan | خطة |
| To-dos | قائمة المهام |
| Aufgabe | مهمة |
| Auftrag | التكليف |
| Werkzeug | أداة |
| Skills | المهارات |
| Befehl | أمر |
| Hook | خطاف |
| Sandbox | بيئة معزولة |
| Fremder Inhalt | محتوى خارجي |
| Sicherungspunkt | نقطة استعادة |
| Zurückspulen | إرجاع |
| Verdichten | ضغط |
| Kontext, Kontextfenster | السياق، نافذة السياق |
| Denktiefe | عمق التفكير |
| Modell / Anbieter / Endpunkt | نموذج / مزوّد / نقطة نهاية |
| Token, Tokens | رمز مميز، رموز مميزة |
| API-Schlüssel | مفتاح API |
| Register | فهرس (فهرس ACP) |
| Adapter | المحوّل |
| Debug-Adapter | محوّل التصحيح |
| Bereit | جاهز |
| Prüfsumme | المجموع الاختباري |
| Anmeldung, anmelden | تسجيل الدخول |
| Konto | حساب |
| Abo | الاشتراك |
| Repository | مستودع |
| Commit, committen | Commit، إنشاء Commit |
| Secrets | بيانات سرية |
| vormerken | تجهيز |
| Zurücknehmen | إلغاء التجهيز |
| nicht versioniert | غير متتبَّع |
| Abschnitt (Git) | مقطع |
| Branch / Remote | فرع / المستودع البعيد |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Stash | Stash |
| Übernehmen (Worktree) | ضم |
| Arbeitskopie | نسخة العمل |
| Lauf, Läufe | تشغيل، عمليات التشغيل |
| Host | مضيف |
| Job | وظيفة |
| Log | سجل |
| **Debugger** | |
| Debuggen | تصحيح |
| Haltepunkt, Haltepunkte | نقطة توقف، نقاط التوقف |
| Logpunkt | نقطة تسجيل |
| Bedingung / Trefferzahl | الشرط / عدد الإصابات |
| Aufrufliste | مكدس الاستدعاءات |
| Frame | إطار |
| Überwachter Ausdruck, Überwachte Ausdrücke | تعبير مراقَب، تعبيرات مراقَبة |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | متابعة / تخطٍّ / دخول / خروج |
| Anhängen / Trennen | إرفاق / فصل |
| An Prozess anhängen | إرفاق بعملية |
| Neu starten / Stopp | إعادة التشغيل / إيقاف |
| Variablen | المتغيرات |
| Angehalten | متوقف مؤقتًا |
| Debug-Konsole | وحدة تحكم التصحيح |
| Startkonfiguration | إعداد التشغيل |
| Inspektor | المفتش |
| **Tests** | |
| Tests | الاختبارات |
| Test | اختبار |
| **Containers** | |
| Container | الحاويات |
| Docker-Kontext | سياق Docker |
| **Leitstand (phase 13)** | |
| Leitstand | غرفة التحكم |
| Auftrag (Leitstand) | أمر عمل، أوامر العمل |
| Nachweis | الإثبات |
| Abweichung | انحراف، الانحرافات |
| Abnahme, abnehmen | القبول، قبول |
| Freigeben (Spezifikation) | اعتماد |
| Planer | المخطِّط |
| Builder | المنفّذ |
| Prüfer | المراجِع |
| Späher | المستكشف |
| Prüfstation, Gate | بوابة |
| Prüfbefehl | أمر البوابة |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | الاستلام / المواصفة / البناء / المراجعة / القبول / الضم / مكتمل |
| Risikoklasse | فئة المخاطر |
| Pfadbereich | نطاق المسارات |
| Herkunft | المنشأ |
| Station | محطة |
| Kriterium, Kriterien | معيار، معايير |
| Prüfrunde | جولة مراجعة |
| Befund | ملاحظة |
| Hersteller, Herstellerwechsel | المورّد، تغيير المورّد |
| Kontingent | الحصة |
| Workflow-Datei | ملف سير العمل |
| Stand | الحالة |
| Erstdurchlaufquote, Erstdurchlauf, Quote | نسبة النجاح من المرة الأولى، المرة الأولى، النسبة |
| Kennzahlen | المؤشرات |
| Regelvorschlag | اقتراح قاعدة |
| wiederkehrender Auftrag | أمر عمل متكرر |
| Abstand (wiederkehrender Auftrag) | الفاصل الزمني |
| eingereiht | في قائمة الانتظار |
| Rolle | الدور |
| Einwand | اعتراض (اعتراض المنفّذ) |
| Rangliste | قائمة الأولوية |
| Prüfer-Regel | قاعدة المراجِع (إلزامي / مفضَّل) |
| Vorgabe, auf Vorgabe zurücksetzen | افتراضي، استعادة الافتراضي |
| Mitkopieren / Einrichtung | نسخ أيضًا / الإعداد |
| Prüfrelevante Dateien | الملفات المهمة للمراجعة |
| Issue | Issue (in Latin script) |
| Merge Request | Merge Request (in Latin script, masculine; "إرسال كـ Merge Request") |
| Rechte | الأذونات |
| Erlauben / Fragen / Verbieten | سماح / استئذان / رفض |
| Gedächtnis | الذاكرة |
| Anweisungen | التعليمات |
| Ausgabestil | نمط الإخراج |
| Auswahl | المطابِق |
| Plugin, Plugins | إضافة، الإضافات |
| Marktplatz, Marktplätze | سوق، الأسواق |
| Frage nebenbei | سؤال جانبي |
| Header | الترويسات |
| Umgebung | البيئة |
| Bericht | التقرير |
| Grenzen | الحدود |

### Catalan (`ca`)
- Address: informal "tu", as macOS Catalan does ("Vols desar els canvis?", "Torna‑ho a provar"); menu items and buttons
  in the imperative ("Desa", "Obre", "Cancel·la")
- Typography: “…” curly double quotes as macOS Catalan uses them (not « »), typographic apostrophe ’, "l·l" with the
  middle dot, clitic pronouns joined with the non-breaking hyphen as in "Mostra‑ho tot"; the ellipsis follows the word
  without a space ("Cerca…", "Configuració…"); a space before % ("50 %")
- Git and CI words: kept as loanwords: commit, push, pull, fetch, rebase, cherry-pick, hook, worktree, job, log, token,
  sandbox ("fer commit", "fer push", "Fent pull…"); translated: repositori, branca, remot, fusionar (merge), preparar
  (stage), fragment (hunk), sense seguiment (untracked), còpia de treball, execució (CI run); "AI" is "IA" ("pestanya
  d’IA"); "file" is "fitxer" as splat's "Fitxers" has it

| German key | Catalan |
|---|---|
| Ablage | Arxiu |
| Bearbeiten | Edita |
| Darstellung | Mostra |
| Fenster | Finestra |
| Hilfe | Ajuda |
| Dienste | Serveis |
| Über {} | Sobre el {} |
| Einstellungen | Configuració |
| {} ausblenden / Andere ausblenden / Alle einblenden | Oculta el {} / Oculta la resta / Mostra‑ho tot |
| {} beenden | Surt del {} |
| Widerrufen / Wiederholen | Desfés / Refés |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Retalla / Copia / Enganxa / Elimina |
| Alles auswählen | Selecciona‑ho tot |
| Suchen … / Weitersuchen / Rückwärts suchen | Cerca… / Cerca el següent / Cerca l’anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimitza / Zoom / Porta‑ho tot a primer pla |
| Vollbildmodus | Passa a pantalla completa |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Oculta la barra d’eines / Mostra o oculta la barra lateral |
| Sichern | Desa |
| Öffnen / Schließen | Obre / Tanca |
| Abbrechen / OK | Cancel·la / D’acord |
| Im Finder zeigen | Mostra al Finder |
| Papierkorb | paperera |
| Schlüsselbund | clauer |
| Zwischenablage | porta-retalls |
| Tab | pestanya |
| AI-Tab | pestanya d’IA |
| Panel | tauler |
| Seitenleiste | barra lateral |
| Vorschau | previsualització |
| Bereich | àrea |
| Aktivität | Activitat |
| Projekt / Ordner | projecte / carpeta |
| Vertrauen, vertrauen | confiança, confiar (“Confia‑hi”) |
| Platte | disc (“al disc”) |
| Unterschiede | diferències |
| Zusammenführen | fusionar (“Fusiona”) |
| Konflikt | conflicte |
| Treffer | resultat |
| Suche | Cerca |
| Ersetzen | Substitueix |
| Sprachserver | servidor de llenguatge |
| Hinweis (Schweregrad) | Suggeriment |
| Inlay-Hinweis | indicació inline |
| Minimap | Minimapa |
| Sticky Scroll | Desplaçament fix |
| Diagnosen am Zeilenende | Diagnòstics al final de la línia |
| Vim-Modus | Mode Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERIR / SUBSTITUIR / VISUAL / VISUAL LÍNIA / VISUAL BLOC (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: No s’ha trobat el patró: {} … (as this language's Vim says them) |
| Vorschlag | suggeriment |
| Vorhersage | Predicció |
| Ausschlussliste | llista d’exclusió |
| Blame | Blame |
| Hinweise (Rotor) | Indicacions |
| Schnellkorrektur | correcció ràpida |
| Refactoring | refactorització |
| Quellaktion | acció de codi font |
| Implementierung / Typdefinition / Deklaration | implementació / definició del tipus / declaració |
| Aufrufhierarchie / Typhierarchie | Jerarquia de crides / Jerarquia de tipus |
| Aufrufer / Aufgerufene | Cridadors / Cridats |
| Obertypen / Untertypen | Supertipus / Subtipus |
| indexieren | indexar |
| Lesezeichen / Verlauf / Adresse | marcador / historial / adreça |
| Bytes | bytes |
| Exportieren | Exporta |
| Format | Format |
| Diktat | dictat |
| Agent | agent |
| Eigener Agent | agent propi |
| Eigene Agenten | Agents personalitzats |
| Thread (AI-Thread, CLI-Thread) | fil (fil d’IA, fil de CLI) |
| Sitzung | sessió |
| Subagent | subagent |
| Hintergrundprozess | procés en segon pla |
| Unterhaltung | conversa |
| Zug | torn |
| Antwort | resposta |
| Rückfrage | pregunta |
| Freigabe, Freigabemodus | permís, mode de permisos |
| Umgehen | Omet els permisos |
| Erlauben / Ablehnen / Immer erlauben … | Permet / Denega / Permet sempre… |
| Plan | pla |
| To-dos | Tasques pendents |
| Aufgabe | tasca |
| Auftrag | encàrrec |
| Werkzeug | eina |
| Skills | Habilitats |
| Befehl | ordre |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Contingut extern |
| Sicherungspunkt | punt de control |
| Zurückspulen | retrocedir (“Retrocedeix”) |
| Verdichten | compactar |
| Kontext, Kontextfenster | context, finestra de context |
| Denktiefe | raonament |
| Modell / Anbieter / Endpunkt | model / proveïdor / punt de connexió |
| Token, Tokens | token, tokens |
| API-Schlüssel | clau d’API |
| Register | registre |
| Adapter | adaptador |
| Debug-Adapter | adaptador de depuració |
| Bereit | A punt |
| Prüfsumme | suma de verificació |
| Anmeldung, anmelden | inici de sessió, iniciar la sessió |
| Konto | compte |
| Abo | Subscripció |
| Repository | repositori |
| Commit, committen | commit, fer commit |
| Secrets | secrets |
| vormerken | preparar |
| Zurücknehmen | Treu de la preparació |
| nicht versioniert | sense seguiment |
| Abschnitt (Git) | fragment |
| Branch / Remote | branca / remot |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | còpia de treball |
| Lauf, Läufe | execució, execucions |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Depura |
| Haltepunkt, Haltepunkte | punt d’interrupció, punts d’interrupció |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condició / nombre d’encerts |
| Aufrufliste | pila de crides |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | expressió vigilada, expressions vigilades |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continua / Passa per sobre / Entra / Surt |
| Anhängen / Trennen | adjunta / separa |
| An Prozess anhängen | Adjunta al procés |
| Neu starten / Stopp | Reinicia / Atura |
| Variablen | variables |
| Angehalten | en pausa |
| Debug-Konsole | consola de depuració |
| Startkonfiguration | configuració d’inici |
| Inspektor | Inspector |
| **Tests** | |
| Tests | Proves |
| Test | prova |
| **Containers** | |
| Container | Contenidors |
| Docker-Kontext | context de Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Sala de control |
| Auftrag (Leitstand) | ordre de treball |
| Nachweis | evidència |
| Abweichung | desviació |
| Abnahme, abnehmen | acceptació, acceptar (“Accepta”) |
| Freigeben (Spezifikation) | aprovar (“Aprova”) |
| Planer | planificador |
| Builder | constructor |
| Prüfer | revisor |
| Späher | explorador |
| Prüfstation, Gate | control |
| Prüfbefehl | ordre de control |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Entrada / Especificació / Construcció / Revisió / Acceptació / Incorporació / Acabat |
| Risikoklasse | classe de risc |
| Pfadbereich | àmbit de rutes |
| Herkunft | procedència |
| Station | etapa |
| Kriterium, Kriterien | criteri, criteris |
| Prüfrunde | ronda de revisió |
| Befund | troballa |
| Hersteller, Herstellerwechsel | fabricant, canvi de fabricant |
| Kontingent | quota |
| Workflow-Datei | fitxer de flux de treball |
| Stand | estat |
| Erstdurchlaufquote, Erstdurchlauf, Quote | taxa de primera passada, primera passada, taxa |
| Kennzahlen | mètriques |
| Regelvorschlag | proposta de regla |
| wiederkehrender Auftrag | ordre de treball recurrent |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | a la cua |
| Rolle | rol |
| Einwand | objecció |
| Rangliste | llista de prioritat |
| Prüfer-Regel | regla del revisor (obligatori / preferit) |
| Vorgabe, auf Vorgabe zurücksetzen | per omissió, restableix el valor per omissió |
| Mitkopieren / Einrichtung | copia també / inicialització |
| Prüfrelevante Dateien | fitxers rellevants per a la revisió |
| Issue | issue (feminine: la issue) |
| Merge Request | merge request (feminine: la merge request) |
| Rechte | Permisos |
| Erlauben / Fragen / Verbieten | Permet / Pregunta / Denega |
| Gedächtnis | memòria |
| Anweisungen | instruccions |
| Ausgabestil | estil de sortida |
| Auswahl | Patró |
| Plugin, Plugins | connector, connectors |
| Marktplatz, Marktplätze | mercat, mercats |
| Frage nebenbei | pregunta al marge |
| Header | capçaleres |
| Umgebung | entorn |
| Bericht | informe |
| Grenzen | Límits |
| Sitzung starten | Engega la sessió (not Inicia la sessió, which is signing in) |

### Czech (`cs`)
- Address: formal "vy" (vykání) with plural imperatives ("Zadejte", "Vyberte"), as macOS Czech; menu items and buttons
  are infinitives ("Otevřít", "Uložit"), help tags third person ("Vloží obrázek")
- Typography: „…“ (U+201E, U+201C), ‚…‘ inside; menu items end in "…" without a space ("Hledat…", "Přejmenovat…");
  sentence case; "50 %" with a space, decimal comma
- Git and CI words: kept as Czech developers say them: commit (verb "commitnout"), push, pull, fetch, rebase,
  cherry-pick, revert, fast-forward, hook, worktree, job, log, token; translated where Czech has the established word:
  větev (branch), vzdálený repozitář (remote), repozitář, sloučit/sloučení (merge), připravit k commitu (stage), blok
  (hunk), pracovní kopie, běh (CI run)

| German key | Czech |
|---|---|
| Ablage | Soubor |
| Bearbeiten | Úpravy |
| Darstellung | Zobrazení |
| Fenster | Okno |
| Hilfe | Nápověda |
| Dienste | Služby |
| Über {} | O aplikaci {} |
| Einstellungen | Nastavení |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skrýt {} / Skrýt ostatní / Zobrazit vše |
| {} beenden | Ukončit {} |
| Widerrufen / Wiederholen | Odvolat akci / Opakovat akci |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Vyjmout / Zkopírovat / Vložit / Smazat |
| Alles auswählen | Vybrat vše |
| Suchen … / Weitersuchen / Rückwärts suchen | Hledat… / Hledat další / Hledat předchozí |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizovat / Přepnout velikost / Převést vše do popředí |
| Vollbildmodus | Spustit režim celé obrazovky |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skrýt panel nástrojů / Zobrazit/skrýt boční panel |
| Sichern | Uložit |
| Öffnen / Schließen | Otevřít / Zavřít |
| Abbrechen / OK | Zrušit / OK |
| Im Finder zeigen | Zobrazit ve Finderu |
| Papierkorb | Koš |
| Schlüsselbund | Klíčenka |
| Zwischenablage | Schránka |
| Tab | panel |
| AI-Tab | panel AI |
| Panel | podokno |
| Seitenleiste | boční panel |
| Vorschau | náhled |
| Bereich | oblast |
| Aktivität | Aktivita |
| Projekt / Ordner | projekt / složka |
| Vertrauen, vertrauen | důvěra, důvěřovat |
| Platte | disk |
| Unterschiede | rozdíly |
| Zusammenführen | sloučit, sloučení |
| Konflikt | konflikt |
| Treffer | výsledek |
| Suche | Hledání |
| Ersetzen | Nahradit |
| Sprachserver | jazykový server |
| Hinweis (Schweregrad) | Nápověda |
| Inlay-Hinweis | vložená nápověda |
| Minimap | Minimapa |
| Sticky Scroll | Pevné posouvání |
| Diagnosen am Zeilenende | Diagnostika na konci řádku |
| Vim-Modus | Režim Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Vzor nenalezen: {} … (as this language's Vim says them; E748 translated, which the old catalog lacks, and E20 given the noun it drops) |
| Vorschlag | návrh |
| Vorhersage | Předpověď |
| Ausschlussliste | seznam vyloučení |
| Blame | Blame |
| Hinweise (Rotor) | Nápovědy |
| Schnellkorrektur | rychlá oprava |
| Refactoring | refaktorování |
| Quellaktion | akce zdrojového kódu |
| Implementierung / Typdefinition / Deklaration | implementace / definice typu / deklarace |
| Aufrufhierarchie / Typhierarchie | Hierarchie volání / Hierarchie typů |
| Aufrufer / Aufgerufene | Volající / Volané |
| Obertypen / Untertypen | Nadtypy / Podtypy |
| indexieren | indexovat |
| Lesezeichen / Verlauf / Adresse | záložka / historie / adresa |
| Bytes | B |
| Exportieren | Exportovat |
| Format | Formát |
| Diktat | diktování |
| Agent | agent |
| Eigener Agent | vlastní agent |
| Eigene Agenten | Vlastní agenti |
| Thread (AI-Thread, CLI-Thread) | vlákno (vlákno AI, vlákno CLI) |
| Sitzung | relace |
| Subagent | subagent |
| Hintergrundprozess | proces na pozadí |
| Unterhaltung | konverzace |
| Zug | kolo |
| Antwort | odpověď |
| Rückfrage | dotaz |
| Freigabe, Freigabemodus | povolení, režim povolení |
| Umgehen | Obejít |
| Erlauben / Ablehnen / Immer erlauben … | Povolit / Zamítnout / Vždy povolit… |
| Plan | plán |
| To-dos | Úkoly |
| Aufgabe | úloha |
| Auftrag | zadání |
| Werkzeug | nástroj |
| Skills | Dovednosti |
| Befehl | příkaz |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Cizí obsah |
| Sicherungspunkt | kontrolní bod |
| Zurückspulen | přetočit zpět |
| Verdichten | zestručnit |
| Kontext, Kontextfenster | kontext, kontextové okno |
| Denktiefe | hloubka uvažování |
| Modell / Anbieter / Endpunkt | model / poskytovatel / koncový bod |
| Token, Tokens | token, tokeny |
| API-Schlüssel | klíč API |
| Register | registr |
| Adapter | adaptér |
| Debug-Adapter | ladicí adaptér |
| Bereit | Připraveno |
| Prüfsumme | kontrolní součet |
| Anmeldung, anmelden | přihlášení, přihlásit se |
| Konto | účet |
| Abo | Předplatné |
| Repository | repozitář |
| Commit, committen | commit, commitnout |
| Secrets | tajné údaje |
| vormerken | připravit k commitu |
| Zurücknehmen | Zrušit přípravu |
| nicht versioniert | nesledováno |
| Abschnitt (Git) | blok |
| Branch / Remote | větev / vzdálený repozitář |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | převzít |
| Arbeitskopie | pracovní kopie |
| Lauf, Läufe | běh, běhy |
| Host | hostitel |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Ladit |
| Haltepunkt, Haltepunkte | zarážka, zarážky |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | podmínka / počet zásahů |
| Aufrufliste | zásobník volání |
| Frame | rámec |
| Überwachter Ausdruck, Überwachte Ausdrücke | sledovaný výraz, sledované výrazy |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Pokračovat / Krokovat s přeskočením / Krokovat s vnořením / Krokovat s vystoupením |
| Anhängen / Trennen | připojit / odpojit |
| An Prozess anhängen | Připojit k procesu |
| Neu starten / Stopp | Restartovat / Zastavit |
| Variablen | proměnné |
| Angehalten | pozastaveno |
| Debug-Konsole | konzole ladění |
| Startkonfiguration | konfigurace spuštění |
| Inspektor | Inspektor |
| **Tests** | |
| Tests | Testy |
| Test | test |
| **Containers** | |
| Container | Kontejnery |
| Docker-Kontext | kontext Dockeru |
| **Leitstand (phase 13)** | |
| Leitstand | velín |
| Auftrag (Leitstand) | zakázka |
| Nachweis | doklad |
| Abweichung | odchylka |
| Abnahme, abnehmen | přijetí, přijmout |
| Freigeben (Spezifikation) | schválit (schválení) |
| Planer | Plánovač |
| Builder | Stavitel |
| Prüfer | Recenzent |
| Späher | Průzkumník |
| Prüfstation, Gate | brána |
| Prüfbefehl | kontrolní příkaz |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Vstup / Specifikace / Stavba / Revize / Přijetí / Převzetí / Hotovo |
| Risikoklasse | třída rizika (nízká / střední / vysoká) |
| Pfadbereich | rozsah cest |
| Herkunft | původ |
| Station | stanice |
| Kriterium, Kriterien | kritérium, kritéria |
| Prüfrunde | cyklus revize (not kolo, which is Zug) |
| Befund | zjištění |
| Hersteller, Herstellerwechsel | výrobce, změna výrobce |
| Kontingent | kvóta |
| Workflow-Datei | soubor pracovního postupu |
| Stand | stav |
| Erstdurchlaufquote, Erstdurchlauf, Quote | podíl úspěchu napoprvé, napoprvé, podíl |
| Kennzahlen | metriky |
| Regelvorschlag | návrh pravidla |
| wiederkehrender Auftrag | opakovaná zakázka |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | ve frontě |
| Rolle | role |
| Einwand | námitka |
| Rangliste | žebříček |
| Prüfer-Regel | pravidlo recenzenta (povinně / přednostně) |
| Vorgabe, auf Vorgabe zurücksetzen | výchozí, vrátit na výchozí |
| Mitkopieren / Einrichtung | kopírovat s sebou / příprava |
| Prüfrelevante Dateien | soubory důležité pro revizi |
| Issue | issue (neuter, indeclinable) |
| Merge Request | merge request (masculine: merge requestu, merge requesty) |
| Rechte | Povolení |
| Erlauben / Fragen / Verbieten | Povolit / Zeptat se / Zamítnout |
| Gedächtnis | paměť |
| Anweisungen | instrukce |
| Ausgabestil | styl výstupu |
| Auswahl | Vzor |
| Plugin, Plugins | plugin, pluginy |
| Marktplatz, Marktplätze | tržiště |
| Frage nebenbei | vedlejší otázka |
| Header | hlavičky |
| Umgebung | prostředí |
| Bericht | hlášení |
| Grenzen | Limity |

### Danish (`da`)
- Address: informal "du", as Danish macOS addresses the user
- Typography: “…” quotation marks (as splat and Danish macOS), "Indstillinger…" with the ellipsis directly after the
  word, no space; a file is an "arkiv" as in Danish macOS and splat
- Git and CI words: kept as loanwords, as Danish developers say them: repository, commit/committe, branch, remote, push,
  pull, fetch, merge ("merget"), rebase, cherry-pick, stage/unstage ("staget", "ikke staget"), worktree, hook, token,
  job, log, secret; translated: arbejdskopi (working tree), blok (hunk), kørsel (CI run), vært (host)

| German key | Danish |
|---|---|
| Ablage | Arkiv |
| Bearbeiten | Rediger |
| Darstellung | Oversigt |
| Fenster | Vindue |
| Hilfe | Hjælp |
| Dienste | Tjenester |
| Über {} | Om {} |
| Einstellungen | Indstillinger |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skjul {} / Skjul andre / Vis alle |
| {} beenden | Slut {} |
| Widerrufen / Wiederholen | Fortryd / Gentag |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klip / Kopier / Sæt ind / Slet |
| Alles auswählen | Vælg alt |
| Suchen … / Weitersuchen / Rückwärts suchen | Find… / Find næste / Find forrige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimer / Zoom / Anbring alle forrest |
| Vollbildmodus | Start fuld skærm |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skjul værktøjslinje / Vis/skjul indholdsoversigt |
| Sichern | Gem |
| Öffnen / Schließen | Åbn / Luk |
| Abbrechen / OK | Annuller / OK |
| Im Finder zeigen | Vis i Finder |
| Papierkorb | papirkurv (Flyt til papirkurven) |
| Schlüsselbund | nøglering |
| Zwischenablage | udklipsholder |
| Tab | fane |
| AI-Tab | AI-fane |
| Panel | panel |
| Seitenleiste | indholdsoversigt |
| Vorschau | eksempel (eksempelfane) |
| Bereich | rude |
| Aktivität | Aktivitet |
| Projekt / Ordner | projekt / mappe |
| Vertrauen, vertrauen | tillid, stole på (betroet mappe) |
| Platte | disk (på disken) |
| Unterschiede | forskelle |
| Zusammenführen | merge (merge, merget, merge-konflikt) |
| Konflikt | konflikt |
| Treffer | resultat |
| Suche | Søgning |
| Ersetzen | Erstat |
| Sprachserver | sprogserver |
| Hinweis (Schweregrad) | Tip |
| Inlay-Hinweis | inlay-tip |
| Minimap | Minikort |
| Sticky Scroll | Fastgjort rulning |
| Diagnosen am Zeilenende | Diagnoser ved linjeslut |
| Vim-Modus | Vim-tilstand |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INDSÆT / ERSTAT / VISUEL / VISUEL LINJE / VISUEL BLOK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Mønster ikke fundet: {} … (as this language's Vim says them) |
| Vorschlag | forslag |
| Vorhersage | Forudsigelse |
| Ausschlussliste | udelukkelsesliste |
| Blame | Blame |
| Hinweise (Rotor) | Tips |
| Schnellkorrektur | hurtig rettelse |
| Refactoring | refaktorering |
| Quellaktion | kildehandling |
| Implementierung / Typdefinition / Deklaration | implementering / typedefinition / erklæring |
| Aufrufhierarchie / Typhierarchie | Kaldhierarki / Typehierarki |
| Aufrufer / Aufgerufene | Kaldere / Kaldte |
| Obertypen / Untertypen | Supertyper / Undertyper |
| indexieren | indeksere |
| Lesezeichen / Verlauf / Adresse | bogmærke / historik / adresse |
| Bytes | byte |
| Exportieren | Eksporter |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | Egen agent (in a sentence: appens egen agent) |
| Eigene Agenten | Brugerdefinerede agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (AI-tråd, CLI-tråd) |
| Sitzung | session |
| Subagent | subagent |
| Hintergrundprozess | baggrundsproces |
| Unterhaltung | samtale |
| Zug | runde |
| Antwort | svar |
| Rückfrage | spørgsmål |
| Freigabe, Freigabemodus | tilladelse, tilladelsestilstand |
| Umgehen | Omgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillad / Afvis / Tillad altid… |
| Plan | plan |
| To-dos | Gøremål |
| Aufgabe | opgave |
| Auftrag | opgave |
| Werkzeug | værktøj |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Fremmed indhold |
| Sicherungspunkt | kontrolpunkt |
| Zurückspulen | spol tilbage (tilbagespoling) |
| Verdichten | komprimere (komprimering) |
| Kontext, Kontextfenster | kontekst, kontekstvindue |
| Denktiefe | tænkedybde |
| Modell / Anbieter / Endpunkt | model / udbyder / slutpunkt |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-nøgle |
| Register | register |
| Adapter | adapter |
| Debug-Adapter | debug-adapter |
| Bereit | Klar |
| Prüfsumme | kontrolsum |
| Anmeldung, anmelden | login, log ind |
| Konto | konto |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committe |
| Secrets | secrets |
| vormerken | stage (staget, ikke staget) |
| Zurücknehmen | Unstage |
| nicht versioniert | ikke versionsstyret |
| Abschnitt (Git) | blok |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | overfør |
| Arbeitskopie | arbejdskopi |
| Lauf, Läufe | kørsel, kørsler |
| Host | vært |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Fejlfind |
| Haltepunkt, Haltepunkte | brudpunkt, brudpunkter |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | betingelse / antal træffere |
| Aufrufliste | kaldstak |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | overvåget udtryk, overvågede udtryk |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Fortsæt / Træd over / Træd ind / Træd ud |
| Anhängen / Trennen | tilknyt / frakobl |
| An Prozess anhängen | Tilknyt proces |
| Neu starten / Stopp | Genstart / Stop |
| Variablen | variabler |
| Angehalten | standset |
| Debug-Konsole | fejlfindingskonsol |
| Startkonfiguration | startkonfiguration |
| Inspektor | Inspektør |
| **Tests** | |
| Tests | Tests |
| Test | test |
| **Containers** | |
| Container | Containere |
| Docker-Kontext | Docker-kontekst |
| **Leitstand (phase 13)** | |
| Leitstand | Kontrolrum (Kontrolrummet) |
| Auftrag (Leitstand) | arbejdsordre, arbejdsordrer |
| Nachweis | dokumentation |
| Abweichung | afvigelse, afvigelser |
| Abnahme, abnehmen | accept, acceptere (button: Accepter) |
| Freigeben (Spezifikation) | godkend (godkendelse) |
| Planer | Planlægger |
| Builder | Builder |
| Prüfer | Reviewer |
| Späher | Spejder |
| Prüfstation, Gate | gate, gates |
| Prüfbefehl | gate-kommando, gate-kommandoer |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Modtagelse / Specifikation / Build / Review / Accept / Overførsel / Færdig |
| Risikoklasse | risikoklasse |
| Pfadbereich | stiområde |
| Herkunft | oprindelse |
| Station | station |
| Kriterium, Kriterien | kriterium, kriterier |
| Prüfrunde | reviewrunde |
| Befund | fund |
| Hersteller, Herstellerwechsel | leverandør, leverandørskift |
| Kontingent | kvote |
| Workflow-Datei | workflow-arkiv |
| Stand | tilstand |
| Erstdurchlaufquote, Erstdurchlauf, Quote | førsteforsøgsrate, første forsøg, rate |
| Kennzahlen | nøgletal |
| Regelvorschlag | regelforslag |
| wiederkehrender Auftrag | tilbagevendende arbejdsordre |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | i kø |
| Rolle | rolle |
| Einwand | indsigelse |
| Rangliste | rangliste |
| Prüfer-Regel | reviewer-regel (påkrævet / foretrukket) |
| Vorgabe, auf Vorgabe zurücksetzen | standard, gendan standard |
| Mitkopieren / Einrichtung | kopiér med / opsætning |
| Prüfrelevante Dateien | review-relevante arkiver |
| Issue | issue (et issue, issuet) |
| Merge Request | merge request (en merge request) |
| Rechte | Tilladelser |
| Erlauben / Fragen / Verbieten | Tillad / Spørg / Afvis |
| Gedächtnis | hukommelse |
| Anweisungen | instruktioner |
| Ausgabestil | outputstil |
| Auswahl | Mønster |
| Plugin, Plugins | plugin, plugins |
| Marktplatz, Marktplätze | markedsplads, markedspladser |
| Frage nebenbei | sidespørgsmål |
| Header | headere |
| Umgebung | miljø |
| Bericht | rapport |
| Grenzen | Grænser |

### Greek (`el`)
- Address: formal plural (εσείς), imperatives such as "Επιλέξτε", "Εισαγάγετε", "Δοκιμάστε ξανά", as macOS Greek does;
  menu items and buttons are nouns of action ("Άνοιγμα", "Αποθήκευση ως…"), in sentence case
- Typography: «…» quotation marks (“…” inside them), ";" as the question mark, "Εύρεση…" with the ellipsis character and
  no space; "50%" without a space; units "ώ.", "λεπ.", "δευτ.", "χιλ.", "εκ."
- Git and CI words: kept as indeclinable neuter loanwords as Greek developers say them: commit, push, pull, fetch,
  rebase, cherry-pick, branch, remote, upstream, stage/unstage ("σε stage", "εκτός stage"), worktree, hook, job, token,
  sandbox, fast-forward; translated where Greek has an established word: αποθετήριο (repository), συγχώνευση (merge),
  διένεξη (conflict), τμήμα (hunk), αντίγραφο εργασίας (working tree), εκτέλεση (CI run), αρχείο καταγραφής (log); "AI"
  stays as "AI" ("καρτέλα AI", "νήμα AI")

| German key | Greek |
|---|---|
| Ablage | Αρχείο |
| Bearbeiten | Επεξεργασία |
| Darstellung | Προβολή |
| Fenster | Παράθυρο |
| Hilfe | Βοήθεια |
| Dienste | Υπηρεσίες |
| Über {} | Πληροφορίες για το {} |
| Einstellungen | Ρυθμίσεις |
| {} ausblenden / Andere ausblenden / Alle einblenden | Απόκρυψη {} / Απόκρυψη άλλων / Εμφάνιση όλων |
| {} beenden | Τερματισμός {} |
| Widerrufen / Wiederholen | Αναίρεση / Επανάληψη |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Αποκοπή / Αντιγραφή / Επικόλληση / Διαγραφή |
| Alles auswählen | Επιλογή όλων |
| Suchen … / Weitersuchen / Rückwärts suchen | Εύρεση… / Εύρεση επόμενου / Εύρεση προηγούμενου |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Ελαχιστοποίηση / Ζουμ / Μεταφορά όλων σε πρώτο πλάνο |
| Vollbildmodus | Είσοδος σε πλήρη οθόνη |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Απόκρυψη γραμμής εργαλείων / Εμφάνιση/απόκρυψη πλαϊνής στήλης |
| Sichern | Αποθήκευση |
| Öffnen / Schließen | Άνοιγμα / Κλείσιμο |
| Abbrechen / OK | Ακύρωση / OK |
| Im Finder zeigen | Εμφάνιση στο Finder |
| Papierkorb | Κάδος |
| Schlüsselbund | κλειδοθήκη |
| Zwischenablage | πρόχειρο |
| Tab | καρτέλα |
| AI-Tab | καρτέλα AI |
| Panel | πλαίσιο |
| Seitenleiste | πλαϊνή στήλη |
| Vorschau | προεπισκόπηση |
| Bereich | περιοχή |
| Aktivität | Δραστηριότητα |
| Projekt / Ordner | έργο / φάκελος |
| Vertrauen, vertrauen | εμπιστοσύνη, εμπιστεύομαι |
| Platte | δίσκος |
| Unterschiede | διαφορές |
| Zusammenführen | συγχώνευση |
| Konflikt | διένεξη |
| Treffer | αποτέλεσμα |
| Suche | Αναζήτηση |
| Ersetzen | Αντικατάσταση |
| Sprachserver | διακομιστής γλώσσας |
| Hinweis (Schweregrad) | Υπόδειξη |
| Inlay-Hinweis | ενσωματωμένη υπόδειξη |
| Minimap | Μικροχάρτης |
| Sticky Scroll | Σταθερή κύλιση |
| Diagnosen am Zeilenende | Διαγνωστικά στο τέλος γραμμής |
| Vim-Modus | Λειτουργία Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Δεν βρέθηκε το μοτίβο: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | πρόταση |
| Vorhersage | Πρόβλεψη |
| Ausschlussliste | λίστα εξαιρέσεων |
| Blame | Blame |
| Hinweise (Rotor) | Υποδείξεις |
| Schnellkorrektur | γρήγορη διόρθωση |
| Refactoring | αναδιάρθρωση |
| Quellaktion | ενέργεια πηγαίου κώδικα |
| Implementierung / Typdefinition / Deklaration | υλοποίηση / ορισμός τύπου / δήλωση |
| Aufrufhierarchie / Typhierarchie | Ιεραρχία κλήσεων / Ιεραρχία τύπων |
| Aufrufer / Aufgerufene | Καλούντες / Καλούμενες |
| Obertypen / Untertypen | Υπερτύποι / Υποτύποι |
| indexieren | ευρετηρίαση |
| Lesezeichen / Verlauf / Adresse | σελιδοδείκτης / ιστορικό / διεύθυνση |
| Bytes | byte |
| Exportieren | Εξαγωγή |
| Format | Μορφή |
| Diktat | υπαγόρευση |
| Agent | πράκτορας |
| Eigener Agent | ενσωματωμένος πράκτορας |
| Eigene Agenten | Προσαρμοσμένοι πράκτορες |
| Thread (AI-Thread, CLI-Thread) | νήμα (νήμα AI, νήμα CLI) |
| Sitzung | συνεδρία |
| Subagent | υποπράκτορας |
| Hintergrundprozess | διεργασία παρασκηνίου |
| Unterhaltung | συνομιλία |
| Zug | γύρος |
| Antwort | απάντηση |
| Rückfrage | ερώτηση |
| Freigabe, Freigabemodus | άδεια, λειτουργία αδειών |
| Umgehen | Παράκαμψη |
| Erlauben / Ablehnen / Immer erlauben … | Να επιτρέπεται / Άρνηση / Να επιτρέπεται πάντα… |
| Plan | σχέδιο |
| To-dos | Εκκρεμότητες |
| Aufgabe | εργασία |
| Auftrag | ανάθεση |
| Werkzeug | εργαλείο |
| Skills | Δεξιότητες |
| Befehl | εντολή |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Εξωτερικό περιεχόμενο |
| Sicherungspunkt | σημείο ελέγχου |
| Zurückspulen | επιστροφή |
| Verdichten | συμπύκνωση |
| Kontext, Kontextfenster | συμφραζόμενα, παράθυρο συμφραζομένων |
| Denktiefe | βάθος σκέψης |
| Modell / Anbieter / Endpunkt | μοντέλο / πάροχος / τελικό σημείο |
| Token, Tokens | token (indeclinable) |
| API-Schlüssel | κλειδί API |
| Register | μητρώο |
| Adapter | προσαρμογέας |
| Debug-Adapter | προσαρμογέας αποσφαλμάτωσης |
| Bereit | Έτοιμο |
| Prüfsumme | άθροισμα ελέγχου |
| Anmeldung, anmelden | σύνδεση, συνδέομαι |
| Konto | λογαριασμός |
| Abo | Συνδρομή |
| Repository | αποθετήριο |
| Commit, committen | commit, κάνω commit |
| Secrets | μυστικά |
| vormerken | κάνω stage (σε stage / εκτός stage) |
| Zurücknehmen | Unstage |
| nicht versioniert | μη παρακολουθούμενο |
| Abschnitt (Git) | τμήμα |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | ενσωμάτωση |
| Arbeitskopie | αντίγραφο εργασίας |
| Lauf, Läufe | εκτέλεση, εκτελέσεις |
| Host | host |
| Job | job |
| Log | αρχείο καταγραφής |
| **Debugger** | |
| Debuggen | Αποσφαλμάτωση |
| Haltepunkt, Haltepunkte | σημείο διακοπής, σημεία διακοπής |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | συνθήκη / αριθμός προσπελάσεων |
| Aufrufliste | στοίβα κλήσεων |
| Frame | πλαίσιο |
| Überwachter Ausdruck, Überwachte Ausdrücke | παρακολουθούμενη έκφραση, παρακολουθούμενες εκφράσεις |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Συνέχεια / Βήμα πάνω από / Είσοδος / Έξοδος |
| Anhängen / Trennen | σύνδεση / αποσύνδεση |
| An Prozess anhängen | Σύνδεση σε διεργασία |
| Neu starten / Stopp | Επανεκκίνηση / Διακοπή |
| Variablen | μεταβλητές |
| Angehalten | σε παύση |
| Debug-Konsole | κονσόλα αποσφαλμάτωσης |
| Startkonfiguration | διαμόρφωση εκκίνησης |
| Inspektor | Επιθεωρητής |
| **Tests** | |
| Tests | Δοκιμές |
| Test | δοκιμή |
| **Containers** | |
| Container | Container |
| Docker-Kontext | Docker context |
| **Leitstand (phase 13)** | |
| Leitstand | Κέντρο ελέγχου |
| Auftrag (Leitstand) | δελτίο εργασίας |
| Nachweis | τεκμηρίωση |
| Abweichung | απόκλιση |
| Abnahme, abnehmen | αποδοχή, αποδέχομαι |
| Freigeben (Spezifikation) | Έγκριση (εγκρίνω) |
| Planer | Σχεδιαστής |
| Builder | Υλοποιητής |
| Prüfer | Αξιολογητής |
| Späher | Ανιχνευτής |
| Prüfstation, Gate | πύλη ελέγχου |
| Prüfbefehl | εντολή ελέγχου |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Παραλαβή / Προδιαγραφή / Υλοποίηση / Αξιολόγηση / Αποδοχή / Ενσωμάτωση / Ολοκληρώθηκε |
| Risikoklasse | κατηγορία κινδύνου |
| Pfadbereich | εύρος διαδρομών |
| Herkunft | προέλευση |
| Station | σταθμός |
| Kriterium, Kriterien | κριτήριο, κριτήρια |
| Prüfrunde | κύκλος αξιολόγησης |
| Befund | εύρημα |
| Hersteller, Herstellerwechsel | κατασκευαστής, αλλαγή κατασκευαστή |
| Kontingent | ποσόστωση |
| Workflow-Datei | αρχείο workflow |
| Stand | κατάσταση |
| Erstdurchlaufquote, Erstdurchlauf, Quote | ποσοστό επιτυχίας με την πρώτη, με την πρώτη, ποσοστό |
| Kennzahlen | δείκτες |
| Regelvorschlag | πρόταση κανόνα |
| wiederkehrender Auftrag | επαναλαμβανόμενο δελτίο εργασίας |
| Abstand (wiederkehrender Auftrag) | διάστημα |
| eingereiht | σε αναμονή |
| Rolle | ρόλος |
| Einwand | ένσταση |
| Rangliste | κατάταξη |
| Prüfer-Regel | κανόνας αξιολογητή (υποχρεωτικά / κατά προτίμηση) |
| Vorgabe, auf Vorgabe zurücksetzen | προεπιλογή, επαναφορά προεπιλογής |
| Mitkopieren / Einrichtung | αντιγραφή επίσης / προετοιμασία |
| Prüfrelevante Dateien | αρχεία σημαντικά για την αξιολόγηση |
| Issue | issue (indeclinable neuter) |
| Merge Request | merge request (indeclinable neuter) |
| Rechte | Άδειες |
| Erlauben / Fragen / Verbieten | Να επιτρέπεται / Να ερωτάται / Άρνηση |
| Gedächtnis | μνήμη |
| Anweisungen | οδηγίες |
| Ausgabestil | στυλ εξόδου |
| Auswahl | Μοτίβο |
| Plugin, Plugins | πρόσθετο, πρόσθετα |
| Marktplatz, Marktplätze | αγορά, αγορές |
| Frage nebenbei | παράπλευρη ερώτηση |
| Header | κεφαλίδες |
| Umgebung | περιβάλλον |
| Bericht | αναφορά |
| Grenzen | Όρια |

### Spanish (`es`)
- Address: "tú", as macOS in Spanish (Spain) does: imperatives "Introduce", "Elige", "Abre"; possessives "tu", "tus"
- Typography: «comillas angulares» as splat uses them, “inglesas” only inside them; ellipsis "…" directly after the
  word, without a space, as in "Buscar…" and "Abrir carpeta…"; "50 %" with a space; menu items and buttons in sentence
  case, infinitive verbs ("Guardar como…", "Cerrar pestaña"); errors as "No se ha podido…"
- Git and CI words: kept as Spanish developers say them: commit ("hacer commit"), push, pull, fetch, rebase,
  cherry-pick, revert, fast-forward, upstream, hook, worktree, diff, job, log, token, endpoint, sandbox (masculine);
  translated where Spanish Git usage (Pro Git, git's own messages) has a word: rama, remoto, repositorio, fusionar,
  preparar/retirar (stage/unstage), fragmento (hunk), sin seguimiento (untracked), copia de trabajo, ejecución (CI run);
  "AI" is "IA"

| German key | Spanish |
|---|---|
| Ablage | Archivo |
| Bearbeiten | Edición |
| Darstellung | Visualización |
| Fenster | Ventana |
| Hilfe | Ayuda |
| Dienste | Servicios |
| Über {} | Acerca de {} |
| Einstellungen | Ajustes |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar {} / Ocultar otras apps / Mostrar todo |
| {} beenden | Salir de {} |
| Widerrufen / Wiederholen | Deshacer / Rehacer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Pegar / Eliminar |
| Alles auswählen | Seleccionar todo |
| Suchen … / Weitersuchen / Rückwärts suchen | Buscar… / Buscar siguiente / Buscar anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Zoom / Traer todo al frente |
| Vollbildmodus | Usar pantalla completa |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar barra de herramientas / Mostrar u ocultar barra lateral |
| Sichern | Guardar |
| Öffnen / Schließen | Abrir / Cerrar |
| Abbrechen / OK | Cancelar / Aceptar |
| Im Finder zeigen | Mostrar en el Finder |
| Papierkorb | papelera (Trasladar a la papelera) |
| Schlüsselbund | llavero |
| Zwischenablage | portapapeles |
| Tab | pestaña |
| AI-Tab | pestaña de IA |
| Panel | panel |
| Seitenleiste | barra lateral |
| Vorschau | vista previa |
| Bereich | área |
| Aktivität | Actividad |
| Projekt / Ordner | proyecto / carpeta |
| Vertrauen, vertrauen | confianza, confiar en (carpeta de confianza) |
| Platte | disco (en el disco) |
| Unterschiede | diferencias |
| Zusammenführen | fusionar |
| Konflikt | conflicto |
| Treffer | resultado |
| Suche | Búsqueda |
| Ersetzen | Reemplazar |
| Sprachserver | servidor de lenguaje |
| Hinweis (Schweregrad) | Sugerencia |
| Inlay-Hinweis | sugerencia insertada |
| Minimap | Minimapa |
| Sticky Scroll | Desplazamiento fijo |
| Diagnosen am Zeilenende | Diagnósticos al final de la línea |
| Vim-Modus | Modo Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERTAR / REEMPLAZAR / VISUAL / LÍNEA VISUAL / BLOQUE VISUAL (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Patrón no encontrado: {} … (as this language's Vim says them) |
| Vorschlag | propuesta |
| Vorhersage | Predicción |
| Ausschlussliste | lista de exclusión |
| Blame | Blame |
| Hinweise (Rotor) | Sugerencias |
| Schnellkorrektur | corrección rápida |
| Refactoring | refactorización |
| Quellaktion | acción de código fuente |
| Implementierung / Typdefinition / Deklaration | implementación / definición de tipo / declaración |
| Aufrufhierarchie / Typhierarchie | Jerarquía de llamadas / Jerarquía de tipos |
| Aufrufer / Aufgerufene | Llamadores / Llamados |
| Obertypen / Untertypen | Supertipos / Subtipos |
| indexieren | indexar |
| Lesezeichen / Verlauf / Adresse | marcador / historial / dirección |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formato |
| Diktat | dictado |
| Agent | agente |
| Eigener Agent | agente propio |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | hilo (hilo de IA, hilo de CLI) |
| Sitzung | sesión |
| Subagent | subagente |
| Hintergrundprozess | proceso en segundo plano |
| Unterhaltung | conversación |
| Zug | turno |
| Antwort | respuesta |
| Rückfrage | pregunta |
| Freigabe, Freigabemodus | permiso, modo de permisos |
| Umgehen | Omitir permisos |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Denegar / Permitir siempre… |
| Plan | plan |
| To-dos | Pendientes |
| Aufgabe | tarea |
| Auftrag | encargo |
| Werkzeug | herramienta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox (el sandbox) |
| Fremder Inhalt | Contenido externo |
| Sicherungspunkt | punto de control |
| Zurückspulen | rebobinar |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, ventana de contexto |
| Denktiefe | razonamiento (bajo, medio, alto, muy alto, máximo) |
| Modell / Anbieter / Endpunkt | modelo / proveedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | clave de API |
| Register | registro |
| Adapter | adaptador |
| Debug-Adapter | adaptador de depuración |
| Bereit | Listo |
| Prüfsumme | suma de comprobación |
| Anmeldung, anmelden | inicio de sesión, iniciar sesión |
| Konto | cuenta |
| Abo | Suscripción |
| Repository | repositorio |
| Commit, committen | commit, hacer commit |
| Secrets | secretos |
| vormerken | preparar (zurücknehmen: retirar) |
| Zurücknehmen | Retirar |
| nicht versioniert | sin seguimiento |
| Abschnitt (Git) | fragmento |
| Branch / Remote | rama / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | copia de trabajo |
| Lauf, Läufe | ejecución, ejecuciones |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Depurar |
| Haltepunkt, Haltepunkte | punto de interrupción, puntos de interrupción |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condición / número de aciertos |
| Aufrufliste | pila de llamadas |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | expresión vigilada, expresiones vigiladas |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continuar / Paso a paso por procedimientos / Paso a paso por instrucciones / Paso a paso para salir |
| Anhängen / Trennen | asociar / desasociar |
| An Prozess anhängen | Asociar a proceso |
| Neu starten / Stopp | Reiniciar / Detener |
| Variablen | variables |
| Angehalten | en pausa |
| Debug-Konsole | consola de depuración |
| Startkonfiguration | configuración de inicio |
| Inspektor | Inspector |
| **Tests** | |
| Tests | Pruebas |
| Test | prueba |
| **Containers** | |
| Container | Contenedores |
| Docker-Kontext | contexto de Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Sala de control |
| Auftrag (Leitstand) | orden de trabajo |
| Nachweis | evidencia |
| Abweichung | desviación |
| Abnahme, abnehmen | aceptación, aceptar |
| Freigeben (Spezifikation) | aprobar |
| Planer | planificador |
| Builder | constructor |
| Prüfer | revisor |
| Späher | explorador |
| Prüfstation, Gate | control |
| Prüfbefehl | comando de control |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Entrada / Especificación / Construcción / Revisión / Aceptación / Incorporación / Terminado |
| Risikoklasse | clase de riesgo |
| Pfadbereich | ámbito de rutas |
| Herkunft | procedencia |
| Station | etapa |
| Kriterium, Kriterien | criterio, criterios |
| Prüfrunde | ronda de revisión |
| Befund | hallazgo |
| Hersteller, Herstellerwechsel | fabricante, cambio de fabricante |
| Kontingent | cuota |
| Workflow-Datei | archivo de flujo de trabajo |
| Stand | estado |
| Erstdurchlaufquote, Erstdurchlauf, Quote | tasa de primera pasada, primera pasada, tasa |
| Kennzahlen | métricas |
| Regelvorschlag | propuesta de regla |
| wiederkehrender Auftrag | orden de trabajo recurrente |
| Abstand (wiederkehrender Auftrag) | intervalo |
| eingereiht | en cola |
| Rolle | rol |
| Einwand | objeción |
| Rangliste | lista de prioridad |
| Prüfer-Regel | regla del revisor (obligatorio / preferido) |
| Vorgabe, auf Vorgabe zurücksetzen | por omisión, restaurar valor por omisión |
| Mitkopieren / Einrichtung | copiar también / inicialización |
| Prüfrelevante Dateien | archivos relevantes para la revisión |
| Issue | issue (masculine: el issue) |
| Merge Request | merge request (masculine: el merge request) |
| Rechte | Permisos |
| Erlauben / Fragen / Verbieten | Permitir / Preguntar / Denegar |
| Gedächtnis | memoria |
| Anweisungen | instrucciones |
| Ausgabestil | estilo de salida |
| Auswahl | Patrón |
| Plugin, Plugins | plugin, plugins |
| Marktplatz, Marktplätze | marketplace, marketplaces |
| Frage nebenbei | pregunta aparte |
| Header | cabeceras |
| Umgebung | entorno |
| Bericht | informe |
| Grenzen | Límites |
| Sitzung starten | Arrancar la sesión (not Iniciar sesión, which is signing in) |

### Finnish (`fi`)
- Address: informal singular "sinä", as Finnish macOS uses it ("Haluatko tallentaa…?", imperatives "Valitse", "Avaa");
  placeholders stay in the nominative, with Apple's colon form ("Tietoja: {}") or a head noun ("palveluun {}", "haaraan
  {}") where Finnish would inflect them
- Typography: ”…” (the same right double quotation mark on both sides) and ’ as apostrophe; menu items that open a
  dialog end in "…" without a space ("Avaa…", "Asetukset…"); a space before % ("50 %"), "t" for hours ("{} t")
- Git and CI words: commit (verb "commitoida", button "Commitoi"), Push, Pull, Fetch, Rebase, Revert, Cherry-pick,
  merge-commit, worktree and hook stay as loanwords with Finnish endings; branch → haara, remote → etärepositorio
  (remote branch → etähaara), repository → repositorio, hunk → lohko, stage → valmistella (staged → valmisteltu, unstage
  → peru valmistelu), untracked → seuraamaton, working tree → työkopio, CI run → ajo, job → työ, log → loki, token →
  token; "AI" stays as in the German key (AI-välilehti, AI-ketju)

| German key | Finnish |
|---|---|
| Ablage | Tiedosto |
| Bearbeiten | Muokkaa |
| Darstellung | Näytä |
| Fenster | Ikkuna |
| Hilfe | Ohje |
| Dienste | Palvelut |
| Über {} | Tietoja: {} |
| Einstellungen | Asetukset |
| {} ausblenden / Andere ausblenden / Alle einblenden | Kätke {} / Kätke muut / Näytä kaikki |
| {} beenden | Lopeta {} |
| Widerrufen / Wiederholen | Peru / Tee sittenkin |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Leikkaa / Kopioi / Sijoita / Poista |
| Alles auswählen | Valitse kaikki |
| Suchen … / Weitersuchen / Rückwärts suchen | Etsi… / Etsi seuraava / Etsi edellinen |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Pienennä / Zoomaa / Tuo kaikki eteen |
| Vollbildmodus | Siirry koko näytön tilaan |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Kätke työkalupalkki / Näytä tai kätke sivupalkki |
| Sichern | Tallenna |
| Öffnen / Schließen | Avaa / Sulje |
| Abbrechen / OK | Kumoa / OK |
| Im Finder zeigen | Näytä Finderissa |
| Papierkorb | Roskakori |
| Schlüsselbund | Avainnippu |
| Zwischenablage | Leikepöytä |
| Tab | välilehti |
| AI-Tab | AI-välilehti |
| Panel | paneeli |
| Seitenleiste | sivupalkki |
| Vorschau | esikatselu |
| Bereich | ruutu |
| Aktivität | Toiminta |
| Projekt / Ordner | projekti / kansio |
| Vertrauen, vertrauen | luottamus, luottaa (button "Luota") |
| Platte | levy ("levyllä") |
| Unterschiede | erot |
| Zusammenführen | yhdistäminen, yhdistää |
| Konflikt | ristiriita |
| Treffer | tulos (as splat: "Ei tuloksia") |
| Suche | Haku |
| Ersetzen | Korvaa |
| Sprachserver | kielipalvelin |
| Hinweis (Schweregrad) | Vihje |
| Inlay-Hinweis | upotettu vihje |
| Minimap | Minikartta |
| Sticky Scroll | Kiinnitetty vieritys |
| Diagnosen am Zeilenende | Diagnostiikka rivin lopussa |
| Vim-Modus | Vim-tila |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAALI / SYÖTTÖ / KORVAUS / VALINTA / VALINTARIVI / VALINTALOHKO (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Kuviota ei löydy: {} … (as this language's Vim says them) |
| Vorschlag | ehdotus |
| Vorhersage | Ennustus |
| Ausschlussliste | poissulkemisluettelo |
| Blame | Blame |
| Hinweise (Rotor) | Vihjeet |
| Schnellkorrektur | pikakorjaus |
| Refactoring | refaktorointi |
| Quellaktion | lähdekoodin toiminto |
| Implementierung / Typdefinition / Deklaration | toteutus / tyypin määrittely / esittely |
| Aufrufhierarchie / Typhierarchie | Kutsuhierarkia / Tyyppihierarkia |
| Aufrufer / Aufgerufene | Kutsujat / Kutsuttavat |
| Obertypen / Untertypen | Ylätyypit / Alatyypit |
| indexieren | indeksoida |
| Lesezeichen / Verlauf / Adresse | kirjanmerkki / historia / osoite |
| Bytes | tavua |
| Exportieren | Vie |
| Format | Muoto |
| Diktat | sanelu |
| Agent | agentti |
| Eigener Agent | oma agentti |
| Eigene Agenten | Mukautetut agentit |
| Thread (AI-Thread, CLI-Thread) | ketju (AI-ketju, CLI-ketju) |
| Sitzung | istunto |
| Subagent | aliagentti |
| Hintergrundprozess | taustaprosessi |
| Unterhaltung | keskustelu |
| Zug | vuoro |
| Antwort | vastaus |
| Rückfrage | kysymys |
| Freigabe, Freigabemodus | lupa, lupatila |
| Umgehen | Ohita |
| Erlauben / Ablehnen / Immer erlauben … | Salli / Hylkää / Salli aina… |
| Plan | suunnitelma |
| To-dos | Tehtävälista |
| Aufgabe | tehtävä |
| Auftrag | toimeksianto |
| Werkzeug | työkalu |
| Skills | Taidot |
| Befehl | komento |
| Hook | hook |
| Sandbox | hiekkalaatikko |
| Fremder Inhalt | Ulkopuolinen sisältö |
| Sicherungspunkt | tarkistuspiste |
| Zurückspulen | kelata taaksepäin, taaksepäin kelaus |
| Verdichten | tiivistää, tiivistäminen |
| Kontext, Kontextfenster | konteksti, konteksti-ikkuna |
| Denktiefe | ajattelun syvyys |
| Modell / Anbieter / Endpunkt | malli / palveluntarjoaja / päätepiste |
| Token, Tokens | token, tokenit ("{} tokenia") |
| API-Schlüssel | API-avain |
| Register | rekisteri |
| Adapter | sovitin |
| Debug-Adapter | virheenjäljityssovitin |
| Bereit | Valmis |
| Prüfsumme | tarkistussumma |
| Anmeldung, anmelden | kirjautuminen, kirjautua sisään |
| Konto | tili |
| Abo | Tilaus |
| Repository | repositorio |
| Commit, committen | commit, commitoida |
| Secrets | salaisuudet |
| vormerken | valmistella |
| Zurücknehmen | Peru valmistelu |
| nicht versioniert | seuraamaton |
| Abschnitt (Git) | lohko |
| Branch / Remote | haara / etärepositorio |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | ota käyttöön |
| Arbeitskopie | työkopio |
| Lauf, Läufe | ajo, ajot |
| Host | palvelin |
| Job | työ |
| Log | loki |
| **Debugger** | |
| Debuggen | Virheenjäljitys |
| Haltepunkt, Haltepunkte | keskeytyskohta, keskeytyskohdat |
| Logpunkt | lokipiste |
| Bedingung / Trefferzahl | ehto / osumien määrä |
| Aufrufliste | kutsupino |
| Frame | kehys |
| Überwachter Ausdruck, Überwachte Ausdrücke | tarkkailtava lauseke, tarkkailtavat lausekkeet |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Jatka / Askella yli / Askella sisään / Askella ulos |
| Anhängen / Trennen | liitä / irrota |
| An Prozess anhängen | Liitä prosessiin |
| Neu starten / Stopp | Käynnistä uudelleen / Pysäytä |
| Variablen | muuttujat |
| Angehalten | pysäytetty |
| Debug-Konsole | virheenjäljityskonsoli |
| Startkonfiguration | käynnistysmääritys |
| Inspektor | Tarkastus |
| **Tests** | |
| Tests | Testit |
| Test | testi |
| **Containers** | |
| Container | Kontit |
| Docker-Kontext | Docker-konteksti |
| **Leitstand (phase 13)** | |
| Leitstand | valvomo |
| Auftrag (Leitstand) | työtilaus |
| Nachweis | todiste |
| Abweichung | poikkeama |
| Abnahme, abnehmen | vastaanotto, ottaa vastaan (button: Ota vastaan) |
| Freigeben (Spezifikation) | hyväksyä (button: Hyväksy) |
| Planer | suunnittelija |
| Builder | rakentaja |
| Prüfer | katselmoija |
| Späher | tiedustelija |
| Prüfstation, Gate | laatuportti |
| Prüfbefehl | tarkistuskomento |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Saapuneet / Määrittely / Rakentaminen / Katselmointi / Vastaanotto / Käyttöönotto / Valmis |
| Risikoklasse | riskiluokka |
| Pfadbereich | polkurajaus |
| Herkunft | alkuperä |
| Station | vaihe |
| Kriterium, Kriterien | kriteeri, kriteerit |
| Prüfrunde | katselmointikierros |
| Befund | havainto |
| Hersteller, Herstellerwechsel | valmistaja, valmistajan vaihto |
| Kontingent | kiintiö |
| Workflow-Datei | työnkulkutiedosto |
| Stand | tila |
| Erstdurchlaufquote, Erstdurchlauf, Quote | ensikierroksen läpäisyaste, ensikierros, läpäisyaste |
| Kennzahlen | tunnusluvut |
| Regelvorschlag | sääntöehdotus |
| wiederkehrender Auftrag | toistuva työtilaus |
| Abstand (wiederkehrender Auftrag) | aikaväli |
| eingereiht | jonossa |
| Rolle | rooli |
| Einwand | vastalause (rakentaja vastustaa kriteerejä) |
| Rangliste | paremmuusjärjestys |
| Prüfer-Regel | katselmoijan sääntö |
| Vorgabe, auf Vorgabe zurücksetzen | oletus, palauta oletus |
| Mitkopieren / Einrichtung | kopioi mukana / asennus |
| Prüfrelevante Dateien | katselmoinnin kannalta olennaiset tiedostot |
| Issue | issue (issuesta, issueta) |
| Merge Request | merge request (merge requestina; GitHub: pull request) |
| Rechte | Luvat |
| Erlauben / Fragen / Verbieten | Salli / Kysy / Hylkää |
| Gedächtnis | muisti |
| Anweisungen | ohjeet |
| Ausgabestil | tulostustyyli |
| Auswahl | Kuvio |
| Plugin, Plugins | lisäosa, lisäosat |
| Marktplatz, Marktplätze | markkinapaikka, markkinapaikat |
| Frage nebenbei | sivukysymys |
| Header | otsakkeet |
| Umgebung | ympäristö |
| Bericht | raportti |
| Grenzen | Rajat |

### French (`fr`)
- Address: "vous", with "vous" imperatives ("Saisissez…", "Ouvrez…"), as macOS does
- Typography: « guillemets » with a no-break space inside, no-break space before : ; ? ! and %, apostrophe ’; ellipsis
  attached without a space, as in macOS menus ("Réglages…", "Enregistrer sous…"); sentence case everywhere
- Git and CI words: kept as developers say them: commit (verb "commiter"), push, pull, fetch, rebase, cherry-pick,
  revert, hook, worktree, job, diff, upstream; translated where French Git usage is settled: dépôt, branche, dépôt
  distant (remote), indexer/désindexer (stage/unstage), bloc (hunk), fusionner/fusion (merge), copie de travail, non
  suivi (untracked); CI: exécution (run), journal (log), jeton (token). "AI" is "IA" (onglet IA, fil IA)

| German key | French |
|---|---|
| Ablage | Fichier |
| Bearbeiten | Édition |
| Darstellung | Présentation |
| Fenster | Fenêtre |
| Hilfe | Aide |
| Dienste | Services |
| Über {} | À propos de {} |
| Einstellungen | Réglages (menu item: Réglages…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Masquer {} / Masquer les autres / Tout afficher |
| {} beenden | Quitter {} |
| Widerrufen / Wiederholen | Annuler / Rétablir |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Couper / Copier / Coller / Supprimer |
| Alles auswählen | Tout sélectionner |
| Suchen … / Weitersuchen / Rückwärts suchen | Rechercher… / Rechercher le suivant / Rechercher le précédent |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimiser / Réduire/agrandir / Tout ramener au premier plan |
| Vollbildmodus | Activer le mode plein écran |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Masquer la barre d’outils / Afficher/masquer la barre latérale |
| Sichern | Enregistrer |
| Öffnen / Schließen | Ouvrir / Fermer |
| Abbrechen / OK | Annuler / OK |
| Im Finder zeigen | Afficher dans le Finder |
| Papierkorb | corbeille (Placer dans la corbeille) |
| Schlüsselbund | trousseau |
| Zwischenablage | presse-papiers |
| Tab | onglet |
| AI-Tab | onglet IA |
| Panel | panneau |
| Seitenleiste | barre latérale |
| Vorschau | aperçu (onglet d’aperçu) |
| Bereich | zone |
| Aktivität | Activité |
| Projekt / Ordner | projet / dossier |
| Vertrauen, vertrauen | confiance, faire confiance (dossier de confiance; not trusted: non approuvé) |
| Platte | disque (sur le disque) |
| Unterschiede | différences |
| Zusammenführen | fusionner, fusion |
| Konflikt | conflit |
| Treffer | résultat |
| Suche | Recherche |
| Ersetzen | Remplacer |
| Sprachserver | serveur de langage |
| Hinweis (Schweregrad) | Suggestion |
| Inlay-Hinweis | indication intégrée |
| Minimap | Minicarte |
| Sticky Scroll | Défilement épinglé |
| Diagnosen am Zeilenende | Diagnostics en fin de ligne |
| Vim-Modus | Mode Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERTION / REMPLACEMENT / VISUEL / VISUEL LIGNE / VISUEL BLOC (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Motif introuvable : {} … (as this language's Vim says them) |
| Vorschlag | suggestion |
| Vorhersage | Prédiction |
| Ausschlussliste | liste d’exclusion |
| Blame | Blame |
| Hinweise (Rotor) | Indications |
| Schnellkorrektur | correction rapide |
| Refactoring | refactorisation |
| Quellaktion | action sur le code source |
| Implementierung / Typdefinition / Deklaration | implémentation / définition de type / déclaration |
| Aufrufhierarchie / Typhierarchie | Hiérarchie d’appels / Hiérarchie de types |
| Aufrufer / Aufgerufene | Appelants / Appelés |
| Obertypen / Untertypen | Supertypes / Sous-types |
| indexieren | indexer |
| Lesezeichen / Verlauf / Adresse | signet / historique / adresse |
| Bytes | octets |
| Exportieren | Exporter |
| Format | Format |
| Diktat | dictée |
| Agent | agent |
| Eigener Agent | agent intégré |
| Eigene Agenten | Agents personnalisés |
| Thread (AI-Thread, CLI-Thread) | fil (fil IA, fil CLI) |
| Sitzung | session |
| Subagent | sous-agent |
| Hintergrundprozess | processus en arrière-plan |
| Unterhaltung | conversation |
| Zug | tour |
| Antwort | réponse |
| Rückfrage | question |
| Freigabe, Freigabemodus | autorisation, mode d’autorisation |
| Umgehen | Contourner |
| Erlauben / Ablehnen / Immer erlauben … | Autoriser / Refuser / Toujours autoriser… |
| Plan | plan |
| To-dos | À faire |
| Aufgabe | tâche |
| Auftrag | mission |
| Werkzeug | outil |
| Skills | Skills |
| Befehl | commande |
| Hook | hook |
| Sandbox | sandbox (la sandbox) |
| Fremder Inhalt | Contenu externe |
| Sicherungspunkt | point de contrôle |
| Zurückspulen | revenir en arrière, retour en arrière |
| Verdichten | compacter, compactage |
| Kontext, Kontextfenster | contexte, fenêtre de contexte |
| Denktiefe | niveau de réflexion |
| Modell / Anbieter / Endpunkt | modèle / fournisseur / point de terminaison |
| Token, Tokens | jeton, jetons |
| API-Schlüssel | clé d’API |
| Register | registre |
| Adapter | adaptateur |
| Debug-Adapter | adaptateur de débogage |
| Bereit | Prêt |
| Prüfsumme | somme de contrôle |
| Anmeldung, anmelden | connexion, se connecter |
| Konto | compte |
| Abo | Abonnement |
| Repository | dépôt |
| Commit, committen | commit, commiter |
| Secrets | secrets |
| vormerken | indexer (zurücknehmen: désindexer) |
| Zurücknehmen | Désindexer |
| nicht versioniert | non suivi |
| Abschnitt (Git) | bloc |
| Branch / Remote | branche / dépôt distant |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | intégrer |
| Arbeitskopie | copie de travail |
| Lauf, Läufe | exécution, exécutions |
| Host | hôte |
| Job | job |
| Log | journal |
| **Debugger** | |
| Debuggen | Déboguer |
| Haltepunkt, Haltepunkte | point d’arrêt, points d’arrêt |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condition / nombre de passages |
| Aufrufliste | pile d’appels |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | expression espionnée, expressions espionnées |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continuer / Pas à pas principal / Pas à pas détaillé / Pas à pas sortant |
| Anhängen / Trennen | attacher / détacher |
| An Prozess anhängen | Attacher au processus |
| Neu starten / Stopp | Redémarrer / Arrêter |
| Variablen | variables |
| Angehalten | en pause |
| Debug-Konsole | console de débogage |
| Startkonfiguration | configuration de lancement |
| Inspektor | Inspecteur |
| **Tests** | |
| Tests | Tests |
| Test | test |
| **Containers** | |
| Container | Conteneurs |
| Docker-Kontext | contexte Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Salle de contrôle |
| Auftrag (Leitstand) | ordre de travail |
| Nachweis | preuves |
| Abweichung | écart |
| Abnahme, abnehmen | acceptation, accepter |
| Freigeben (Spezifikation) | approuver |
| Planer | planificateur |
| Builder | builder |
| Prüfer | relecteur |
| Späher | éclaireur |
| Prüfstation, Gate | contrôle |
| Prüfbefehl | commande de contrôle |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Entrée / Spécification / Construction / Relecture / Acceptation / Intégration / Terminé |
| Risikoklasse | classe de risque |
| Pfadbereich | périmètre des chemins |
| Herkunft | provenance |
| Station | étape |
| Kriterium, Kriterien | critère, critères |
| Prüfrunde | cycle de relecture |
| Befund | constat |
| Hersteller, Herstellerwechsel | fabricant, changement de fabricant |
| Kontingent | quota |
| Workflow-Datei | fichier de workflow |
| Stand | état |
| Erstdurchlaufquote, Erstdurchlauf, Quote | taux de premier passage, premier passage, taux |
| Kennzahlen | indicateurs |
| Regelvorschlag | proposition de règle |
| wiederkehrender Auftrag | ordre de travail récurrent |
| Abstand (wiederkehrender Auftrag) | intervalle |
| eingereiht | en file d’attente |
| Rolle | rôle |
| Einwand | objection (du builder) |
| Rangliste | liste de priorité |
| Prüfer-Regel | règle du relecteur |
| Vorgabe, auf Vorgabe zurücksetzen | par défaut, rétablir la valeur par défaut |
| Mitkopieren / Einrichtung | copier aussi / préparation |
| Prüfrelevante Dateien | fichiers importants pour la relecture |
| Issue | issue (f.) |
| Merge Request | merge request (f.; GitHub: pull request) |
| Rechte | Autorisations |
| Erlauben / Fragen / Verbieten | Autoriser / Demander / Refuser |
| Gedächtnis | mémoire |
| Anweisungen | instructions |
| Ausgabestil | style de sortie |
| Auswahl | Motif |
| Plugin, Plugins | plug-in, plug-ins |
| Marktplatz, Marktplätze | place de marché, places de marché |
| Frage nebenbei | question annexe |
| Header | en-têtes |
| Umgebung | environnement |
| Bericht | rapport |
| Grenzen | Limites |

### Hebrew (`he`)
- Address: neutral, as macOS Hebrew: commands and buttons as verbal nouns ("שמירה", "סגירת הכרטיסיה"), confirmation
  questions in the infinitive ("לשמור שינויים ב-״{}״?"), instructions impersonal ("יש לבחור…", "ניתן…", "באפשרותך…"); no
  gendered imperatives or pronouns; agents and apps take masculine verbs, as macOS does for apps
- Typography: gershayim quotes ״…״, geresh in abbreviations (דק׳, שע׳); the ellipsis directly after the word, no space
  ("שמירה…"); a hyphen joins a Hebrew prefix to a Latin word or placeholder ("ב-Finder", "ה-Commit", "ב-{}"); as in
  macOS, an invisible right-to-left mark (U+200F) opens a text that starts with a Latin word or a name placeholder, and
  a left-to-right mark (U+200E) keeps a command or shortcut such as "/model" or "⌘Esc" in its own order
- Git and CI words: Git's own names stay in Latin script, as Hebrew developers write them: Commit, Branch, Remote, Push,
  Pull, Fetch, Rebase, Cherry-Pick, Revert, Upstream, Fast-Forward, Worktree, Hook, Job; general ideas are Hebrew: מאגר
  (repository), מיזוג (merge), התנגשות (conflict), קטע (hunk), סימון ל-Commit (stage), עותק העבודה (working tree), הרצה
  (CI run), יומן (log); "AI" stays "AI"

| German key | Hebrew |
|---|---|
| Ablage | קובץ |
| Bearbeiten | עריכה |
| Darstellung | תצוגה |
| Fenster | חלון |
| Hilfe | עזרה |
| Dienste | שירותים |
| Über {} | מידע על {} |
| Einstellungen | הגדרות |
| {} ausblenden / Andere ausblenden / Alle einblenden | הסתרת {} / הסתרת יישומים אחרים / הצגת הכול |
| {} beenden | סיום {} |
| Widerrufen / Wiederholen | ביטול הפעולה האחרונה / שחזור הפעולה האחרונה |
| Ausschneiden / Kopieren / Einsetzen / Löschen | גזירה / העתקה / הדבקה / מחיקה |
| Alles auswählen | בחירת הכול |
| Suchen … / Weitersuchen / Rückwärts suchen | חיפוש… / מציאת הבא / מציאת הקודם |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | מזעור / זום / הבאת כל החלונות קדימה |
| Vollbildmodus | מעבר למסך מלא |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | הסתרת סרגל הכלים / הצגה או הסתרה של סרגל הצד |
| Sichern | שמירה |
| Öffnen / Schließen | פתיחה / סגירה |
| Abbrechen / OK | ביטול / אישור |
| Im Finder zeigen | הצגה ב-Finder |
| Papierkorb | פח האשפה ("In den Papierkorb legen": העברה לפח) |
| Schlüsselbund | צרור המפתחות |
| Zwischenablage | לוח העריכה |
| Tab | כרטיסיה |
| AI-Tab | כרטיסיית AI |
| Panel | חלונית |
| Seitenleiste | סרגל צד |
| Vorschau | תצוגה מקדימה |
| Bereich | אזור |
| Aktivität | פעילות |
| Projekt / Ordner | פרויקט / תיקיה |
| Vertrauen, vertrauen | אמון, לתת אמון (תיקיה מהימנה) |
| Platte | דיסק |
| Unterschiede | הבדלים |
| Zusammenführen | מיזוג |
| Konflikt | התנגשות |
| Treffer | תוצאה |
| Suche | חיפוש |
| Ersetzen | החלפה |
| Sprachserver | שרת שפה |
| Hinweis (Schweregrad) | רמז |
| Inlay-Hinweis | רמז מוטבע |
| Minimap | מפה מוקטנת |
| Sticky Scroll | גלילה נעוצה |
| Diagnosen am Zeilenende | אבחונים בסוף השורה |
| Vim-Modus | מצב Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: התבנית לא נמצאה: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | הצעה |
| Vorhersage | חיזוי |
| Ausschlussliste | רשימת החרגה |
| Blame | Blame |
| Hinweise (Rotor) | רמזים |
| Schnellkorrektur | תיקון מהיר |
| Refactoring | ריפקטורינג |
| Quellaktion | פעולת קוד מקור |
| Implementierung / Typdefinition / Deklaration | מימוש / הגדרת טיפוס / הצהרה |
| Aufrufhierarchie / Typhierarchie | היררכיית קריאות / היררכיית טיפוסים |
| Aufrufer / Aufgerufene | קוראים / נקראים |
| Obertypen / Untertypen | טיפוסי־על / תת־טיפוסים |
| indexieren | אינדוקס |
| Lesezeichen / Verlauf / Adresse | סימניה / היסטוריה / כתובת |
| Bytes | בתים |
| Exportieren | ייצוא |
| Format | עיצוב |
| Diktat | הכתבה |
| Agent | סוכן |
| Eigener Agent | סוכן מובנה |
| Eigene Agenten | סוכנים מותאמים אישית |
| Thread (AI-Thread, CLI-Thread) | שרשור (שרשור AI, שרשור CLI) |
| Sitzung | סשן |
| Subagent | סוכן משנה |
| Hintergrundprozess | תהליך ברקע |
| Unterhaltung | שיחה |
| Zug | סבב |
| Antwort | תשובה |
| Rückfrage | שאלה |
| Freigabe, Freigabemodus | הרשאה, מצב הרשאות |
| Umgehen | עקיפה |
| Erlauben / Ablehnen / Immer erlauben … | אישור / סירוב / הרשאה קבועה… |
| Plan | תוכנית |
| To-dos | משימות לביצוע |
| Aufgabe | משימה |
| Auftrag | משימה |
| Werkzeug | כלי |
| Skills | מיומנויות |
| Befehl | פקודה |
| Hook | Hook |
| Sandbox | ארגז חול |
| Fremder Inhalt | תוכן חיצוני |
| Sicherungspunkt | נקודת שחזור |
| Zurückspulen | חזרה לאחור |
| Verdichten | דחיסה |
| Kontext, Kontextfenster | הקשר, חלון ההקשר |
| Denktiefe | עומק חשיבה |
| Modell / Anbieter / Endpunkt | מודל / ספק / נקודת קצה |
| Token, Tokens | טוקן, טוקנים |
| API-Schlüssel | מפתח API |
| Register | מרשם |
| Adapter | מתאם |
| Debug-Adapter | מתאם ניפוי באגים |
| Bereit | מוכן |
| Prüfsumme | סכום ביקורת |
| Anmeldung, anmelden | התחברות, להתחבר |
| Konto | חשבון |
| Abo | מינוי |
| Repository | מאגר |
| Commit, committen | Commit, ביצוע Commit |
| Secrets | סודות |
| vormerken | סימון ל-Commit (מסומן, ביטול הסימון) |
| Zurücknehmen | ביטול הסימון |
| nicht versioniert | לא במעקב |
| Abschnitt (Git) | קטע |
| Branch / Remote | Branch / Remote |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Stash | Stash |
| Übernehmen (Worktree) | שילוב |
| Arbeitskopie | עותק העבודה |
| Lauf, Läufe | הרצה, הרצות |
| Host | מארח |
| Job | Job |
| Log | יומן |
| **Debugger** | |
| Debuggen | ניפוי באגים |
| Haltepunkt, Haltepunkte | נקודת עצירה, נקודות עצירה |
| Logpunkt | נקודת רישום |
| Bedingung / Trefferzahl | תנאי / מספר פגיעות |
| Aufrufliste | מחסנית קריאות |
| Frame | Frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | ביטוי במעקב, ביטויים במעקב |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | המשך / דילוג מעל / כניסה / יציאה |
| Anhängen / Trennen | הצמדה / ניתוק |
| An Prozess anhängen | הצמדה לתהליך |
| Neu starten / Stopp | הפעלה מחדש / עצירה |
| Variablen | משתנים |
| Angehalten | מושהה |
| Debug-Konsole | מסוף ניפוי באגים |
| Startkonfiguration | תצורת הפעלה |
| Inspektor | חוקר |
| **Tests** | |
| Tests | בדיקות |
| Test | בדיקה |
| **Containers** | |
| Container | Containers |
| Docker-Kontext | הקשר Docker |
| **Leitstand (phase 13)** | |
| Leitstand | חדר הבקרה |
| Auftrag (Leitstand) | הזמנת עבודה, הזמנות עבודה |
| Nachweis | הוכחה |
| Abweichung | חריגה |
| Abnahme, abnehmen | קבלה, קבלה |
| Freigeben (Spezifikation) | אישור |
| Planer | מתכנן |
| Builder | בונה |
| Prüfer | סוקר |
| Späher | סייר |
| Prüfstation, Gate | שער |
| Prüfbefehl | פקודת שער |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | קליטה / מפרט / בנייה / סקירה / קבלה / שילוב / הושלם |
| Risikoklasse | דרגת סיכון |
| Pfadbereich | טווח נתיבים |
| Herkunft | מקור |
| Station | תחנה |
| Kriterium, Kriterien | קריטריון, קריטריונים |
| Prüfrunde | סיבוב סקירה |
| Befund | ממצא |
| Hersteller, Herstellerwechsel | יצרן, החלפת יצרן |
| Kontingent | מכסה |
| Workflow-Datei | קובץ תהליך העבודה |
| Stand | מצב |
| Erstdurchlaufquote, Erstdurchlauf, Quote | שיעור המעבר בסבב הראשון, סבב ראשון, שיעור |
| Kennzahlen | מדדים |
| Regelvorschlag | הצעת כלל |
| wiederkehrender Auftrag | הזמנת עבודה חוזרת |
| Abstand (wiederkehrender Auftrag) | מרווח |
| eingereiht | בתור |
| Rolle | תפקיד |
| Einwand | התנגדות (הבונה מתנגד לקריטריונים) |
| Rangliste | דירוג עדיפות |
| Prüfer-Regel | כלל הסוקר |
| Vorgabe, auf Vorgabe zurücksetzen | ברירת מחדל, חזרה לברירת המחדל |
| Mitkopieren / Einrichtung | העתקה נלווית / הגדרה |
| Prüfrelevante Dateien | קבצים רלוונטיים לסקירה |
| Issue | Issue (ה-Issue) |
| Merge Request | Merge Request (GitHub: Pull Request) |
| Rechte | הרשאות |
| Erlauben / Fragen / Verbieten | מתן גישה / בקשת אישור / סירוב |
| Gedächtnis | זיכרון |
| Anweisungen | הוראות |
| Ausgabestil | סגנון פלט |
| Auswahl | תבנית |
| Plugin, Plugins | תוסף, תוספים |
| Marktplatz, Marktplätze | שוק, שווקים |
| Frage nebenbei | שאלה צדדית |
| Header | כותרות |
| Umgebung | סביבה |
| Bericht | דיווח |
| Grenzen | מגבלות |

### Hindi (`hi`)
- Address: formal "आप", with polite "-ें/-एँ" imperatives ("खोलें", "दर्ज करें", "दिखाएँ"), as macOS does
- Typography: curly quotes “…”, sentences end with the danda "।", a space before a colon in labels ("नाम :"), as splat
  does; ellipsis attached without a space, as in macOS menus ("सेटिंग्ज़…", "ढूँढें…"); Western digits, nukta in
  loanwords as macOS writes them (फ़ाइल, ज़ूम)
- Git and CI words: written in Devanagari as Hindi-speaking developers say them: कमिट (कमिट करें), ब्रांच, रिमोट, पुश,
  पुल, फ़ेच, रीबेस, मर्ज, चेरी-पिक, रिवर्ट, हुक, वर्कट्री, अपस्ट्रीम, स्टेज/अनस्टेज करें, हंक, अनट्रैक्ड, रिपॉज़िटरी;
  CI: रन, जॉब, लॉग, टोकन; commands, flags and product names stay in Latin script. "AI" stays "AI" (AI टैब, AI थ्रेड)

| German key | Hindi |
|---|---|
| Ablage | फ़ाइल |
| Bearbeiten | संपादन |
| Darstellung | दृश्य |
| Fenster | विंडो |
| Hilfe | सहायता |
| Dienste | सेवाएँ |
| Über {} | {} का परिचय |
| Einstellungen | सेटिंग्ज़ (menu item: सेटिंग…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {} छिपाएँ / अन्य छिपाएँ / सभी दिखाएँ |
| {} beenden | {} बंद करें |
| Widerrufen / Wiederholen | पहले जैसा करें / फिर करें |
| Ausschneiden / Kopieren / Einsetzen / Löschen | कट / कॉपी / पेस्ट / डिलीट करें |
| Alles auswählen | सभी चुनें |
| Suchen … / Weitersuchen / Rückwärts suchen | ढूँढें… / अगला ढूँढें / पिछला ढूँढें |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | मिनिमाइज़ करें / ज़ूम करें / सभी आगे लाएँ |
| Vollbildmodus | फ़ुल स्‍क्रीन में प्रवेश करें |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | टूलबार छिपाएँ / साइडबार दिखाएँ या छिपाएँ |
| Sichern | सहेजें |
| Öffnen / Schließen | खोलें / बंद करें |
| Abbrechen / OK | रद्द करें / ठीक |
| Im Finder zeigen | Finder में दिखाएँ |
| Papierkorb | ट्रैश |
| Schlüsselbund | कीचेन |
| Zwischenablage | क्लिपबोर्ड |
| Tab | टैब |
| AI-Tab | AI टैब |
| Panel | पैनल |
| Seitenleiste | साइडबार |
| Vorschau | प्रीव्यू |
| Bereich | फलक |
| Aktivität | ऐक्टिविटी |
| Projekt / Ordner | प्रोजेक्ट / फ़ोल्डर |
| Vertrauen, vertrauen | भरोसा, भरोसा करना (भरोसेमंद फ़ोल्डर) |
| Platte | डिस्क (डिस्क पर) |
| Unterschiede | अंतर |
| Zusammenführen | मर्ज करना |
| Konflikt | कॉन्फ़्लिक्ट |
| Treffer | परिणाम |
| Suche | खोज |
| Ersetzen | बदलें |
| Sprachserver | लैंग्वेज सर्वर |
| Hinweis (Schweregrad) | संकेत |
| Inlay-Hinweis | इनलाइन संकेत |
| Minimap | मिनीमैप |
| Sticky Scroll | स्टिकी स्क्रॉल |
| Diagnosen am Zeilenende | पंक्ति के अंत में डायग्नॉस्टिक्स |
| Vim-Modus | Vim मोड |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: पैटर्न नहीं मिला: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | सुझाव |
| Vorhersage | पूर्वानुमान |
| Ausschlussliste | बहिष्करण सूची |
| Blame | Blame |
| Hinweise (Rotor) | संकेत |
| Schnellkorrektur | त्वरित सुधार |
| Refactoring | रीफ़ैक्टरिंग |
| Quellaktion | सोर्स ऐक्शन |
| Implementierung / Typdefinition / Deklaration | इम्प्लीमेंटेशन / टाइप परिभाषा / डिक्लेरेशन |
| Aufrufhierarchie / Typhierarchie | कॉल पदानुक्रम / टाइप पदानुक्रम |
| Aufrufer / Aufgerufene | कॉल करने वाले / कॉल किए गए |
| Obertypen / Untertypen | सुपरटाइप / सबटाइप |
| indexieren | इंडेक्स करना |
| Lesezeichen / Verlauf / Adresse | बुकमार्क / इतिहास / पता |
| Bytes | बाइट |
| Exportieren | एक्सपोर्ट करें |
| Format | फ़ॉर्मैट |
| Diktat | डिक्टेशन |
| Agent | एजेंट |
| Eigener Agent | बिल्ट-इन एजेंट |
| Eigene Agenten | कस्टम एजेंट |
| Thread (AI-Thread, CLI-Thread) | थ्रेड (AI थ्रेड, CLI थ्रेड) |
| Sitzung | सेशन |
| Subagent | सब-एजेंट |
| Hintergrundprozess | बैकग्राउंड प्रोसेस |
| Unterhaltung | बातचीत |
| Zug | टर्न |
| Antwort | जवाब |
| Rückfrage | सवाल |
| Freigabe, Freigabemodus | अनुमति, अनुमति मोड |
| Umgehen | बायपास |
| Erlauben / Ablehnen / Immer erlauben … | अनुमति दें / अस्वीकार करें / हमेशा अनुमति दें… |
| Plan | योजना |
| To-dos | टू-डू |
| Aufgabe | कार्य |
| Auftrag | कार्य |
| Werkzeug | टूल |
| Skills | स्किल |
| Befehl | कमांड |
| Hook | हुक |
| Sandbox | सैंडबॉक्स |
| Fremder Inhalt | बाहरी कॉन्टेंट |
| Sicherungspunkt | चेकपॉइंट |
| Zurückspulen | रिवाइंड करना |
| Verdichten | कॉम्पैक्ट करना |
| Kontext, Kontextfenster | कॉन्टेक्स्ट, कॉन्टेक्स्ट विंडो |
| Denktiefe | सोच का स्तर |
| Modell / Anbieter / Endpunkt | मॉडल / प्रदाता / एंडपॉइंट |
| Token, Tokens | टोकन |
| API-Schlüssel | API कुंजी |
| Register | रजिस्ट्री |
| Adapter | एडैप्टर |
| Debug-Adapter | डीबग एडैप्टर |
| Bereit | तैयार |
| Prüfsumme | चेकसम |
| Anmeldung, anmelden | साइन इन, साइन इन करें |
| Konto | खाता |
| Abo | सब्सक्रिप्शन |
| Repository | रिपॉज़िटरी |
| Commit, committen | कमिट, कमिट करें |
| Secrets | सीक्रेट |
| vormerken | स्टेज करें (zurücknehmen: अनस्टेज करें) |
| Zurücknehmen | अनस्टेज करें |
| nicht versioniert | अनट्रैक्ड |
| Abschnitt (Git) | हंक |
| Branch / Remote | ब्रांच / रिमोट |
| Push / Pull / Fetch / Rebase | पुश / पुल / फ़ेच / रीबेस |
| Worktree | वर्कट्री |
| Stash | स्टैश |
| Übernehmen (Worktree) | शामिल करना |
| Arbeitskopie | वर्किंग ट्री |
| Lauf, Läufe | रन |
| Host | होस्ट |
| Job | जॉब |
| Log | लॉग |
| **Debugger** | |
| Debuggen | डीबग करें |
| Haltepunkt, Haltepunkte | ब्रेकपॉइंट, ब्रेकपॉइंट |
| Logpunkt | लॉगपॉइंट |
| Bedingung / Trefferzahl | शर्त / हिट संख्या |
| Aufrufliste | कॉल स्टैक |
| Frame | फ़्रेम |
| Überwachter Ausdruck, Überwachte Ausdrücke | वॉच एक्सप्रेशन, वॉच एक्सप्रेशन |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | जारी रखें / स्टेप ओवर / स्टेप इन / स्टेप आउट |
| Anhängen / Trennen | अटैच करें / डिटैच करें |
| An Prozess anhängen | प्रोसेस से अटैच करें |
| Neu starten / Stopp | रीस्टार्ट करें / रोकें |
| Variablen | वैरिएबल |
| Angehalten | रुका हुआ |
| Debug-Konsole | डीबग कंसोल |
| Startkonfiguration | लॉन्च कॉन्फ़िगरेशन |
| Inspektor | इंस्पेक्टर |
| **Tests** | |
| Tests | टेस्ट |
| Test | टेस्ट |
| **Containers** | |
| Container | कंटेनर |
| Docker-Kontext | Docker कॉन्टेक्स्ट |
| **Leitstand (phase 13)** | |
| Leitstand | कंट्रोल रूम |
| Auftrag (Leitstand) | वर्क ऑर्डर |
| Nachweis | प्रमाण |
| Abweichung | विचलन |
| Abnahme, abnehmen | स्वीकृति, स्वीकार करें |
| Freigeben (Spezifikation) | मंज़ूर करें (मंज़ूरी) |
| Planer | योजनाकार |
| Builder | बिल्डर |
| Prüfer | समीक्षक |
| Späher | स्काउट |
| Prüfstation, Gate | गेट |
| Prüfbefehl | गेट कमांड |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | इनटेक / स्पेसिफ़िकेशन / बिल्ड / समीक्षा / स्वीकृति / शामिल करना / पूर्ण |
| Risikoklasse | जोखिम श्रेणी |
| Pfadbereich | पाथ दायरा |
| Herkunft | उद्गम |
| Station | स्टेशन |
| Kriterium, Kriterien | मानदंड, मानदंड |
| Prüfrunde | समीक्षा दौर |
| Befund | निष्कर्ष |
| Hersteller, Herstellerwechsel | वेंडर, वेंडर परिवर्तन |
| Kontingent | कोटा |
| Workflow-Datei | वर्कफ़्लो फ़ाइल |
| Stand | स्थिति |
| Erstdurchlaufquote, Erstdurchlauf, Quote | पहली बार में पास दर, पहली बार में पास, दर |
| Kennzahlen | मेट्रिक्स |
| Regelvorschlag | नियम प्रस्ताव |
| wiederkehrender Auftrag | आवर्ती वर्क ऑर्डर |
| Abstand (wiederkehrender Auftrag) | अंतराल |
| eingereiht | कतार में |
| Rolle | भूमिका |
| Einwand | आपत्ति (बिल्डर को मानदंडों पर आपत्ति है) |
| Rangliste | रैंकिंग सूची |
| Prüfer-Regel | समीक्षक नियम |
| Vorgabe, auf Vorgabe zurücksetzen | डिफ़ॉल्ट, डिफ़ॉल्ट पर रीसेट करें |
| Mitkopieren / Einrichtung | साथ में कॉपी करें / सेटअप |
| Prüfrelevante Dateien | समीक्षा के लिए अहम फ़ाइलें |
| Issue | Issue (Latin script, as the CI panel writes it) |
| Merge Request | Merge Request (Latin script; GitHub: Pull Request) |
| Rechte | अनुमतियाँ |
| Erlauben / Fragen / Verbieten | अनुमति दें / पूछें / अस्वीकार करें |
| Gedächtnis | मेमोरी |
| Anweisungen | निर्देश |
| Ausgabestil | आउटपुट शैली |
| Auswahl | पैटर्न |
| Plugin, Plugins | प्लगइन |
| Marktplatz, Marktplätze | मार्केटप्लेस |
| Frage nebenbei | अलग से सवाल |
| Header | हेडर |
| Umgebung | एनवायरनमेंट |
| Bericht | रिपोर्ट |
| Grenzen | सीमाएँ |

### Croatian (`hr`)
- Address: formal "vi" in sentences, lowercase as macOS writes it ("Želite li…", "Odaberite…", "vaš"); menu items and
  buttons in the singular imperative as macOS has them ("Spremi", "Otvori", "Odustani")
- Typography: “…” (U+201C/U+201D, as macOS Croatian quotes), "Postavke…" with no space before the ellipsis; counts that
  would need plural forms are written "Datoteke: {}" as splat does
- Git and CI words: commit, push, pull, fetch, rebase, cherry-pick, worktree, hook, sandbox and token stay loanwords and
  are declined (commita, commitovi); branch is "grana", remote "udaljeni repozitorij", merge "spajanje" (also as the
  button and the operation name), stage "pripremiti (za commit)", hunk "blok", a CI run "pokretanje", a job "posao", a
  log "zapisnik"

| German key | Croatian |
|---|---|
| Ablage | Datoteka |
| Bearbeiten | Uredi |
| Darstellung | Prikaz |
| Fenster | Prozor |
| Hilfe | Pomoć |
| Dienste | Usluge |
| Über {} | Više o aplikaciji {} |
| Einstellungen | Postavke |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sakrij {} / Sakrij ostalo / Prikaži sve |
| {} beenden | Zatvori {} |
| Widerrufen / Wiederholen | Poništi / Ponovi |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Izreži / Kopiraj / Zalijepi / Obriši |
| Alles auswählen | Odaberi sve |
| Suchen … / Weitersuchen / Rückwärts suchen | Pronađi… / Pronađi sljedeće / Pronađi prethodno |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Smanji / Zumiraj / Postavi sve naprijed |
| Vollbildmodus | Prikaži preko cijelog zaslona |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sakrij alatnu traku / Prikaži/sakrij rubni stupac |
| Sichern | Spremi |
| Öffnen / Schließen | Otvori / Zatvori |
| Abbrechen / OK | Odustani / U redu |
| Im Finder zeigen | Prikaži u Finderu |
| Papierkorb | Smeće |
| Schlüsselbund | privjesak ključeva |
| Zwischenablage | memorijski pretinac |
| Tab | kartica |
| AI-Tab | AI kartica |
| Panel | okno |
| Seitenleiste | rubni stupac |
| Vorschau | pregled |
| Bereich | područje |
| Aktivität | Aktivnost |
| Projekt / Ordner | projekt / mapa |
| Vertrauen, vertrauen | povjerenje, vjerovati (trusted: pouzdan) |
| Platte | disk |
| Unterschiede | razlike |
| Zusammenführen | spajanje |
| Konflikt | konflikt |
| Treffer | rezultat |
| Suche | Pretraga |
| Ersetzen | Zamijeni |
| Sprachserver | jezični poslužitelj |
| Hinweis (Schweregrad) | Savjet |
| Inlay-Hinweis | umetnuti savjet |
| Minimap | Minikarta |
| Sticky Scroll | Fiksno pomicanje |
| Diagnosen am Zeilenende | Dijagnostika na kraju retka |
| Vim-Modus | Način rada Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Uzorak nije pronađen: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | prijedlog |
| Vorhersage | Predviđanje |
| Ausschlussliste | popis izuzeća |
| Blame | Blame |
| Hinweise (Rotor) | Savjeti |
| Schnellkorrektur | brzi popravak |
| Refactoring | refaktoriranje |
| Quellaktion | radnja izvornog koda |
| Implementierung / Typdefinition / Deklaration | implementacija / definicija tipa / deklaracija |
| Aufrufhierarchie / Typhierarchie | Hijerarhija poziva / Hijerarhija tipova |
| Aufrufer / Aufgerufene | Pozivatelji / Pozvani |
| Obertypen / Untertypen | Nadtipovi / Podtipovi |
| indexieren | indeksirati |
| Lesezeichen / Verlauf / Adresse | knjižna oznaka / povijest / adresa |
| Bytes | B |
| Exportieren | Eksportiraj |
| Format | Formatiraj |
| Diktat | diktat |
| Agent | agent |
| Eigener Agent | vlastiti agent |
| Eigene Agenten | Prilagođeni agenti |
| Thread (AI-Thread, CLI-Thread) | nit (AI nit, CLI nit) |
| Sitzung | sesija |
| Subagent | podagent |
| Hintergrundprozess | pozadinski proces |
| Unterhaltung | razgovor |
| Zug | krug |
| Antwort | odgovor |
| Rückfrage | pitanje |
| Freigabe, Freigabemodus | dozvola, način dozvola |
| Umgehen | Zaobilaženje |
| Erlauben / Ablehnen / Immer erlauben … | Dozvoli / Odbij / Uvijek dozvoli… |
| Plan | plan |
| To-dos | Obaveze |
| Aufgabe | zadatak |
| Auftrag | zadatak |
| Werkzeug | alat |
| Skills | Vještine |
| Befehl | naredba |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Vanjski sadržaj |
| Sicherungspunkt | kontrolna točka |
| Zurückspulen | premotavanje, Premotaj |
| Verdichten | sažimanje, Sažmi |
| Kontext, Kontextfenster | kontekst, kontekstni prozor |
| Denktiefe | dubina razmišljanja |
| Modell / Anbieter / Endpunkt | model / pružatelj / krajnja točka |
| Token, Tokens | token, tokeni |
| API-Schlüssel | API ključ |
| Register | registar |
| Adapter | adapter |
| Debug-Adapter | adapter za otklanjanje pogrešaka |
| Bereit | Spremno |
| Prüfsumme | kontrolni zbroj |
| Anmeldung, anmelden | prijava, prijaviti se |
| Konto | račun |
| Abo | Pretplata |
| Repository | repozitorij |
| Commit, committen | commit, izraditi commit |
| Secrets | tajni podaci |
| vormerken | pripremiti (za commit) |
| Zurücknehmen | Ukloni iz pripreme |
| nicht versioniert | nepraćeno |
| Abschnitt (Git) | blok |
| Branch / Remote | grana / udaljeni repozitorij |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | preuzeti |
| Arbeitskopie | radna kopija |
| Lauf, Läufe | pokretanje, pokretanja |
| Host | host |
| Job | posao |
| Log | zapisnik |
| **Debugger** | |
| Debuggen | Otklanjanje pogrešaka |
| Haltepunkt, Haltepunkte | prekidna točka, prekidne točke |
| Logpunkt | točka zapisivanja |
| Bedingung / Trefferzahl | uvjet / broj pogodaka |
| Aufrufliste | stog poziva |
| Frame | okvir |
| Überwachter Ausdruck, Überwachte Ausdrücke | praćeni izraz, praćeni izrazi |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Nastavi / Preskoči / Uđi u funkciju / Iziđi iz funkcije |
| Anhängen / Trennen | priključi / odvoji |
| An Prozess anhängen | Priključi na proces |
| Neu starten / Stopp | Ponovno pokreni / Zaustavi |
| Variablen | varijable |
| Angehalten | zaustavljeno |
| Debug-Konsole | konzola za otklanjanje pogrešaka |
| Startkonfiguration | konfiguracija pokretanja |
| Inspektor | Inspektor |
| **Tests** | |
| Tests | Testovi |
| Test | test |
| **Containers** | |
| Container | Kontejneri |
| Docker-Kontext | Docker kontekst |
| **Leitstand (phase 13)** | |
| Leitstand | kontrolna soba |
| Auftrag (Leitstand) | radni nalog |
| Nachweis | dokazi |
| Abweichung | odstupanje |
| Abnahme, abnehmen | prihvaćanje, prihvatiti |
| Freigeben (Spezifikation) | odobriti (odobrenje) |
| Planer | Planer |
| Builder | Izvođač |
| Prüfer | Recenzent |
| Späher | Izviđač |
| Prüfstation, Gate | provjera |
| Prüfbefehl | naredba provjere |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Prijem / Specifikacija / Izrada / Pregled / Prihvaćanje / Preuzimanje / Gotovo |
| Risikoklasse | razred rizika |
| Pfadbereich | opseg putanja |
| Herkunft | podrijetlo |
| Station | faza |
| Kriterium, Kriterien | kriterij, kriteriji |
| Prüfrunde | runda pregleda |
| Befund | nalaz |
| Hersteller, Herstellerwechsel | proizvođač, promjena proizvođača |
| Kontingent | kvota |
| Workflow-Datei | datoteka tijeka rada |
| Stand | stanje |
| Erstdurchlaufquote, Erstdurchlauf, Quote | udio prolaza iz prve, iz prve, udio |
| Kennzahlen | pokazatelji |
| Regelvorschlag | prijedlog pravila |
| wiederkehrender Auftrag | ponavljajući radni nalog |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | u redu čekanja |
| Rolle | uloga |
| Einwand | prigovor (izvođač ima prigovor na kriterije) |
| Rangliste | ljestvica prioriteta |
| Prüfer-Regel | pravilo recenzenta |
| Vorgabe, auf Vorgabe zurücksetzen | standardno, vrati na standardno |
| Mitkopieren / Einrichtung | kopiraj i ovo / postavljanje |
| Prüfrelevante Dateien | datoteke važne za pregled |
| Issue | issue (issuea, issuei) |
| Merge Request | merge request (merge requestovi; GitHub: pull request) |
| Rechte | Dozvole |
| Erlauben / Fragen / Verbieten | Dozvoli / Pitaj / Odbij |
| Gedächtnis | memorija |
| Anweisungen | upute |
| Ausgabestil | stil izlaza |
| Auswahl | Uzorak |
| Plugin, Plugins | dodatak, dodaci |
| Marktplatz, Marktplätze | tržište, tržišta |
| Frage nebenbei | usputno pitanje |
| Header | zaglavlja |
| Umgebung | okruženje |
| Bericht | izvješće |
| Grenzen | Ograničenja |

### Hungarian (`hu`)
- Address: formal, as Hungarian macOS: the polite third-person (Ön) verb forms with the pronoun left out ("Biztosan
  törli?", "Csak akkor nyissa meg a linket, ha megbízik…"); menu items and buttons as action nouns ("Megnyitás",
  "Mentés", "Kilépés")
- Typography: „…” quotation marks; ellipsis "…" directly after the word, no space ("Beállítások…", "Keresés…"); sentence
  case; before a `{}` the article is written "a(z)", as macOS does ("A(z) {} névjegye")
- Git and CI words: Git's operations keep their English names (commit/commitol, push, pull, fetch, rebase, worktree,
  hook, job, token); translated where Hungarian Git tools (git-gui, Visual Studio) have a settled word: tároló
  (repository), ág (branch), távoli tároló (remote), összefésülés (merge), előkészítés (stage), blokk (hunk),
  munkakönyvtár (working tree), futás (CI run), napló (log), ütközés (conflict); "AI" stays AI, as Hungarian macOS
  writes it

| German key | Hungarian |
|---|---|
| Ablage | Fájl |
| Bearbeiten | Szerkesztés |
| Darstellung | Nézet |
| Fenster | Ablak |
| Hilfe | Súgó |
| Dienste | Szolgáltatások |
| Über {} | A(z) {} névjegye |
| Einstellungen | Beállítások (menu item: Beállítások…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | A(z) {} elrejtése / Többi elrejtése / Összes megjelenítése |
| {} beenden | Kilépés a(z) {} appból |
| Widerrufen / Wiederholen | Visszavonás / Ismétlés |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Kivágás / Másolás / Beillesztés / Törlés |
| Alles auswählen | Összes kijelölése |
| Suchen … / Weitersuchen / Rückwärts suchen | Keresés… / Következő keresése / Előző keresése |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Kis méret / Nagyítás / Összes előtérbe hozása |
| Vollbildmodus | Váltás teljes képernyőre |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Eszközsor elrejtése / Oldalsáv megjelenítése/elrejtése |
| Sichern | Mentés |
| Öffnen / Schließen | Megnyitás / Bezárás |
| Abbrechen / OK | Mégsem / OK |
| Im Finder zeigen | Megjelenítés a Finderben |
| Papierkorb | Kuka (Áthelyezés a kukába) |
| Schlüsselbund | kulcskarika |
| Zwischenablage | vágólap |
| Tab | lap |
| AI-Tab | AI-lap |
| Panel | panel |
| Seitenleiste | oldalsáv |
| Vorschau | előnézet |
| Bereich | ablaktábla |
| Aktivität | Tevékenység |
| Projekt / Ordner | projekt / mappa |
| Vertrauen, vertrauen | bizalom, megbízik (megbízható mappa) |
| Platte | lemez (a lemezen) |
| Unterschiede | különbségek |
| Zusammenführen | összefésülés |
| Konflikt | ütközés |
| Treffer | találat |
| Suche | Keresés |
| Ersetzen | Csere |
| Sprachserver | nyelvi szerver |
| Hinweis (Schweregrad) | Tipp |
| Inlay-Hinweis | beágyazott tipp |
| Minimap | Minitérkép |
| Sticky Scroll | Rögzített görgetés |
| Diagnosen am Zeilenende | Diagnosztika a sor végén |
| Vim-Modus | Vim mód |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMÁL / BESZÚR / CSERE / VIZUÁLIS / VIZUÁLIS SOR / VIZUÁLIS BLOKK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Nincs találat: {} … (as this language's Vim says them) |
| Vorschlag | javaslat |
| Vorhersage | Előrejelzés |
| Ausschlussliste | kizárási lista |
| Blame | Blame |
| Hinweise (Rotor) | Tippek |
| Schnellkorrektur | gyorsjavítás |
| Refactoring | refaktorálás |
| Quellaktion | forráskód-művelet |
| Implementierung / Typdefinition / Deklaration | implementáció / típusdefiníció / deklaráció |
| Aufrufhierarchie / Typhierarchie | Hívási hierarchia / Típushierarchia |
| Aufrufer / Aufgerufene | Hívók / Hívottak |
| Obertypen / Untertypen | Szupertípusok / Altípusok |
| indexieren | indexelés |
| Lesezeichen / Verlauf / Adresse | könyvjelző / előzmények / cím |
| Bytes | bájt |
| Exportieren | Exportálás |
| Format | Formátum |
| Diktat | diktálás |
| Agent | ügynök |
| Eigener Agent | beépített ügynök |
| Eigene Agenten | Saját ügynökök |
| Thread (AI-Thread, CLI-Thread) | szál (AI-szál, CLI-szál) |
| Sitzung | munkamenet |
| Subagent | alügynök |
| Hintergrundprozess | háttérfolyamat |
| Unterhaltung | beszélgetés |
| Zug | kör |
| Antwort | válasz |
| Rückfrage | kérdés |
| Freigabe, Freigabemodus | engedély, engedélymód |
| Umgehen | Megkerülés |
| Erlauben / Ablehnen / Immer erlauben … | Engedélyezés / Elutasítás / Engedélyezés mindig… |
| Plan | terv |
| To-dos | Teendők |
| Aufgabe | feladat |
| Auftrag | feladat |
| Werkzeug | eszköz |
| Skills | Skillek |
| Befehl | parancs |
| Hook | hook |
| Sandbox | homokozó |
| Fremder Inhalt | Külső tartalom |
| Sicherungspunkt | visszaállítási pont |
| Zurückspulen | visszatekerés |
| Verdichten | tömörítés |
| Kontext, Kontextfenster | kontextus, kontextusablak |
| Denktiefe | gondolkodási mélység |
| Modell / Anbieter / Endpunkt | modell / szolgáltató / végpont |
| Token, Tokens | token, tokenek |
| API-Schlüssel | API-kulcs |
| Register | jegyzék |
| Adapter | adapter |
| Debug-Adapter | hibakereső adapter |
| Bereit | Kész |
| Prüfsumme | ellenőrzőösszeg |
| Anmeldung, anmelden | bejelentkezés, bejelentkezik |
| Konto | fiók |
| Abo | Előfizetés |
| Repository | tároló |
| Commit, committen | commit, commitol |
| Secrets | titkos adatok |
| vormerken | előkészít (unstage: előkészítés visszavonása) |
| Zurücknehmen | Előkészítés visszavonása |
| nicht versioniert | nem követett |
| Abschnitt (Git) | blokk |
| Branch / Remote | ág / távoli tároló |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | átvétel |
| Arbeitskopie | munkakönyvtár |
| Lauf, Läufe | futás, futások |
| Host | hoszt |
| Job | job |
| Log | napló |
| **Debugger** | |
| Debuggen | Hibakeresés |
| Haltepunkt, Haltepunkte | töréspont, töréspontok |
| Logpunkt | naplópont |
| Bedingung / Trefferzahl | feltétel / találatszám |
| Aufrufliste | hívási verem |
| Frame | keret |
| Überwachter Ausdruck, Überwachte Ausdrücke | figyelt kifejezés, figyelt kifejezések |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Folytatás / Átlépés / Belépés a függvénybe / Kilépés a függvényből |
| Anhängen / Trennen | csatolás / leválasztás |
| An Prozess anhängen | Csatolás folyamathoz |
| Neu starten / Stopp | Újraindítás / Leállítás |
| Variablen | változók |
| Angehalten | szüneteltetve |
| Debug-Konsole | hibakeresési konzol |
| Startkonfiguration | indítási konfiguráció |
| Inspektor | Inspektor |
| **Tests** | |
| Tests | Tesztek |
| Test | teszt |
| **Containers** | |
| Container | Konténerek |
| Docker-Kontext | Docker-kontextus |
| **Leitstand (phase 13)** | |
| Leitstand | vezérlőterem |
| Auftrag (Leitstand) | munkarendelés |
| Nachweis | bizonyíték |
| Abweichung | eltérés |
| Abnahme, abnehmen | elfogadás, elfogad |
| Freigeben (Spezifikation) | jóváhagyás, jóváhagy |
| Planer | tervező |
| Builder | építő |
| Prüfer | ellenőr |
| Späher | felderítő |
| Prüfstation, Gate | ellenőrzőpont |
| Prüfbefehl | ellenőrző parancs |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Beérkezés / Specifikáció / Építés / Ellenőrzés / Elfogadás / Átvétel / Kész |
| Risikoklasse | kockázati osztály |
| Pfadbereich | útvonal-hatókör |
| Herkunft | eredet |
| Station | állomás |
| Kriterium, Kriterien | kritérium, kritériumok |
| Prüfrunde | ellenőrzési forduló |
| Befund | megállapítás |
| Hersteller, Herstellerwechsel | gyártó, gyártóváltás |
| Kontingent | kvóta |
| Workflow-Datei | munkafolyamatfájl |
| Stand | állapot |
| Erstdurchlaufquote, Erstdurchlauf, Quote | elsőkörös sikerarány, első kör, arány |
| Kennzahlen | mutatók |
| Regelvorschlag | szabályjavaslat |
| wiederkehrender Auftrag | ismétlődő munkarendelés |
| Abstand (wiederkehrender Auftrag) | időköz |
| eingereiht | sorban áll |
| Rolle | szerep |
| Einwand | kifogás (az építő kifogásolja a kritériumokat) |
| Rangliste | rangsor |
| Prüfer-Regel | ellenőrszabály |
| Vorgabe, auf Vorgabe zurücksetzen | alapértelmezett, visszaállítás alapértelmezettre |
| Mitkopieren / Einrichtung | átmásolandó / beállítás |
| Prüfrelevante Dateien | az ellenőrzés szempontjából fontos fájlok |
| Issue | issue (issue-k, issue-hoz) |
| Merge Request | merge request (merge requestként; GitHub: pull request) |
| Rechte | Engedélyek |
| Erlauben / Fragen / Verbieten | Engedélyezés / Kérdezés / Tiltás |
| Gedächtnis | memória |
| Anweisungen | utasítások |
| Ausgabestil | kimeneti stílus |
| Auswahl | Szűrő |
| Plugin, Plugins | plug-in, plug-inek |
| Marktplatz, Marktplätze | piactér, piacterek |
| Frage nebenbei | közbevetett kérdés |
| Header | fejlécek |
| Umgebung | környezet |
| Bericht | jelentés |
| Grenzen | Korlátok |

### Indonesian (`id`)
- Address: formal "Anda", capitalized, as Indonesian macOS ("Apakah Anda ingin…", "Buka tautan hanya jika Anda
  memercayai sumbernya"); menu items and buttons as imperative verbs ("Buka", "Simpan", "Tampilkan")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Pengaturan…", "Temukan…"); menu
  items, buttons and titles in Title Case, as Indonesian macOS ("Tampilkan Semua", "Pilih Semua"), running text in
  sentence case
- Git and CI words: kept as Indonesian developers say them: commit (melakukan commit), branch, remote, push, pull,
  fetch, rebase, stage/unstage, hunk, worktree, hook, job, log, run (CI), token, endpoint, sandbox, checksum; Indonesian
  where the word is settled: repositori, salinan kerja (working tree), gabungkan (merge), konflik; "AI" stays AI

| German key | Indonesian |
|---|---|
| Ablage | File |
| Bearbeiten | Edit |
| Darstellung | Lihat |
| Fenster | Jendela |
| Hilfe | Bantuan |
| Dienste | Layanan |
| Über {} | Mengenai {} |
| Einstellungen | Pengaturan (menu item: Pengaturan…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sembunyikan {} / Sembunyikan Lainnya / Tampilkan Semua |
| {} beenden | Tutup {} |
| Widerrufen / Wiederholen | Urung / Ulang |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Potong / Salin / Tempel / Hapus |
| Alles auswählen | Pilih Semua |
| Suchen … / Weitersuchen / Rückwärts suchen | Temukan… / Temukan Berikutnya / Temukan Sebelumnya |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minikan / Zoom / Pindahkan Semua ke Depan |
| Vollbildmodus | Masuk ke Layar Penuh |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sembunyikan Bar Alat / Ubah Bar Samping |
| Sichern | Simpan |
| Öffnen / Schließen | Buka / Tutup |
| Abbrechen / OK | Batalkan / OKE |
| Im Finder zeigen | Tampilkan di Finder |
| Papierkorb | Tong Sampah (Pindahkan ke Tong Sampah) |
| Schlüsselbund | Rantai Kunci |
| Zwischenablage | papan klip |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | panel |
| Seitenleiste | bar samping |
| Vorschau | pratinjau |
| Bereich | area |
| Aktivität | Aktivitas |
| Projekt / Ordner | proyek / folder |
| Vertrauen, vertrauen | kepercayaan, memercayai (folder tepercaya; button: Percayai) |
| Platte | disk (di disk) |
| Unterschiede | perbedaan |
| Zusammenführen | gabungkan (penggabungan) |
| Konflikt | konflik |
| Treffer | hasil |
| Suche | Pencarian |
| Ersetzen | Ganti |
| Sprachserver | server bahasa |
| Hinweis (Schweregrad) | Petunjuk |
| Inlay-Hinweis | petunjuk sisipan |
| Minimap | Minimap |
| Sticky Scroll | Sticky Scroll |
| Diagnosen am Zeilenende | Diagnostik di Akhir Baris |
| Vim-Modus | Mode Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Pola tidak ditemukan: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | saran |
| Vorhersage | Prediksi |
| Ausschlussliste | daftar pengecualian |
| Blame | Blame |
| Hinweise (Rotor) | Petunjuk |
| Schnellkorrektur | perbaikan cepat |
| Refactoring | refaktor |
| Quellaktion | tindakan sumber |
| Implementierung / Typdefinition / Deklaration | implementasi / definisi tipe / deklarasi |
| Aufrufhierarchie / Typhierarchie | Hierarki Panggilan / Hierarki Tipe |
| Aufrufer / Aufgerufene | Pemanggil / Yang Dipanggil |
| Obertypen / Untertypen | Supertipe / Subtipe |
| indexieren | mengindeks |
| Lesezeichen / Verlauf / Adresse | penanda / riwayat / alamat |
| Bytes | byte |
| Exportieren | Ekspor |
| Format | Format |
| Diktat | dikte |
| Agent | agen |
| Eigener Agent | agen bawaan |
| Eigene Agenten | Agen kustom |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sesi |
| Subagent | subagen |
| Hintergrundprozess | proses latar belakang |
| Unterhaltung | percakapan |
| Zug | giliran |
| Antwort | jawaban |
| Rückfrage | pertanyaan |
| Freigabe, Freigabemodus | izin, mode izin |
| Umgehen | Lewati |
| Erlauben / Ablehnen / Immer erlauben … | Izinkan / Tolak / Selalu Izinkan… |
| Plan | rencana |
| To-dos | Daftar Tugas |
| Aufgabe | tugas |
| Auftrag | tugas |
| Werkzeug | alat |
| Skills | Skill |
| Befehl | perintah |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Konten Eksternal |
| Sicherungspunkt | titik pemulihan |
| Zurückspulen | putar balik |
| Verdichten | padatkan (pemadatan) |
| Kontext, Kontextfenster | konteks, jendela konteks |
| Denktiefe | kedalaman berpikir |
| Modell / Anbieter / Endpunkt | model / penyedia / endpoint |
| Token, Tokens | token |
| API-Schlüssel | kunci API |
| Register | registri |
| Adapter | adaptor |
| Debug-Adapter | adaptor debug |
| Bereit | Siap |
| Prüfsumme | checksum |
| Anmeldung, anmelden | masuk (Masuk ke {}), proses masuk |
| Konto | akun |
| Abo | Langganan |
| Repository | repositori |
| Commit, committen | commit, melakukan commit |
| Secrets | rahasia |
| vormerken | stage (batalkan stage) |
| Zurücknehmen | Batalkan Stage |
| nicht versioniert | tidak dilacak |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | ambil |
| Arbeitskopie | salinan kerja |
| Lauf, Läufe | run |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Debug |
| Haltepunkt, Haltepunkte | breakpoint, breakpoint |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | kondisi / jumlah hit |
| Aufrufliste | tumpukan panggilan |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | ekspresi pantauan, ekspresi pantauan |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Lanjutkan / Langkahi / Masuki / Keluari |
| Anhängen / Trennen | kaitkan / lepaskan |
| An Prozess anhängen | Kaitkan ke Proses |
| Neu starten / Stopp | Mulai Ulang / Hentikan |
| Variablen | variabel |
| Angehalten | dijeda |
| Debug-Konsole | konsol debug |
| Startkonfiguration | konfigurasi peluncuran |
| Inspektor | Inspektur |
| **Tests** | |
| Tests | Tes |
| Test | tes |
| **Containers** | |
| Container | Container |
| Docker-Kontext | konteks Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Ruang Kendali |
| Auftrag (Leitstand) | order kerja |
| Nachweis | bukti |
| Abweichung | penyimpangan |
| Abnahme, abnehmen | penerimaan, terima |
| Freigeben (Spezifikation) | Setujui (persetujuan) |
| Planer | Perencana |
| Builder | Builder |
| Prüfer | Peninjau |
| Späher | Pengintai |
| Prüfstation, Gate | gerbang |
| Prüfbefehl | perintah gerbang |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Kotak Masuk / Spesifikasi / Pembuatan / Tinjauan / Penerimaan / Pengambilan / Selesai |
| Risikoklasse | kelas risiko |
| Pfadbereich | cakupan path |
| Herkunft | asal-usul |
| Station | tahap |
| Kriterium, Kriterien | kriteria, kriteria |
| Prüfrunde | putaran tinjauan |
| Befund | temuan |
| Hersteller, Herstellerwechsel | vendor, pergantian vendor |
| Kontingent | kuota |
| Workflow-Datei | file workflow |
| Stand | keadaan |
| Erstdurchlaufquote, Erstdurchlauf, Quote | tingkat lolos pertama, lolos pertama, tingkat |
| Kennzahlen | metrik |
| Regelvorschlag | usulan aturan |
| wiederkehrender Auftrag | order kerja berulang |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | dalam antrean |
| Rolle | peran |
| Einwand | keberatan (Keberatan Builder) |
| Rangliste | daftar peringkat |
| Prüfer-Regel | aturan peninjau (Vendor lain wajib / Vendor lain diutamakan) |
| Vorgabe, auf Vorgabe zurücksetzen | Default, Kembalikan ke Default |
| Mitkopieren / Einrichtung | ikut disalin / penyiapan |
| Prüfrelevante Dateien | File Penting untuk Tinjauan |
| Issue | issue |
| Merge Request | merge request (title and button: Merge Request) |
| Rechte | Izin |
| Erlauben / Fragen / Verbieten | Izinkan / Tanyakan / Larang |
| Gedächtnis | memori |
| Anweisungen | instruksi |
| Ausgabestil | gaya keluaran |
| Auswahl | Filter |
| Plugin, Plugins | plugin |
| Marktplatz, Marktplätze | marketplace |
| Frage nebenbei | pertanyaan sampingan (title: Pertanyaan Sampingan) |
| Header | header |
| Umgebung | lingkungan |
| Bericht | laporan |
| Grenzen | Batas |

### Italian (`it`)
- Address: "tu", as Italian macOS ("Vuoi riprovare?", "Apri il link solo se ti fidi della fonte"); menu items and
  buttons as imperatives ("Apri", "Salva", "Mostra")
- Typography: “…” quotation marks, as Italian macOS writes them (not the «…» splat uses); typographic apostrophe ’
  ("un’altra", "dall’ultimo"); ellipsis "…" directly after the word, no space ("Impostazioni…", "Trova…"); sentence case
- Git and CI words: kept as Italian developers say them: repository (il repository), commit (fare il commit), branch,
  remote, push, pull, fetch, rebase, stage (mettere in stage / togliere dallo stage), worktree, hook, job, log, token,
  endpoint, sandbox, checksum, task; Italian where the word is settled: unire (merge), conflitto, blocco (hunk), copia
  di lavoro (working tree), esecuzione (CI run); "AI" stays AI ("pannello AI")

| German key | Italian |
|---|---|
| Ablage | File |
| Bearbeiten | Modifica |
| Darstellung | Vista |
| Fenster | Finestra |
| Hilfe | Aiuto |
| Dienste | Servizi |
| Über {} | Informazioni su {} |
| Einstellungen | Impostazioni (menu item: Impostazioni…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Nascondi {} / Nascondi altre / Mostra tutte |
| {} beenden | Esci da {} |
| Widerrufen / Wiederholen | Annulla / Ripristina |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Taglia / Copia / Incolla / Elimina |
| Alles auswählen | Seleziona tutto |
| Suchen … / Weitersuchen / Rückwärts suchen | Trova… / Trova successivo / Trova precedente |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Contrai / Ridimensiona / Porta tutto in primo piano |
| Vollbildmodus | Attiva modalità a tutto schermo |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Nascondi la barra strumenti / Mostra/nascondi barra laterale |
| Sichern | Salva |
| Öffnen / Schließen | Apri / Chiudi |
| Abbrechen / OK | Annulla / OK |
| Im Finder zeigen | Mostra nel Finder |
| Papierkorb | Cestino (Sposta nel Cestino) |
| Schlüsselbund | Portachiavi |
| Zwischenablage | appunti (negli appunti) |
| Tab | pannello (as Safari names tabs) |
| AI-Tab | pannello AI |
| Panel | riquadro (pannello is the tab) |
| Seitenleiste | barra laterale |
| Vorschau | anteprima |
| Bereich | area |
| Aktivität | Attività |
| Projekt / Ordner | progetto / cartella |
| Vertrauen, vertrauen | fiducia, fidarsi di (cartella attendibile) |
| Platte | disco (su disco) |
| Unterschiede | differenze |
| Zusammenführen | unire (unione) |
| Konflikt | conflitto |
| Treffer | risultato |
| Suche | Ricerca |
| Ersetzen | Sostituisci |
| Sprachserver | server di linguaggio |
| Hinweis (Schweregrad) | Suggerimento |
| Inlay-Hinweis | suggerimento inline |
| Minimap | Minimappa |
| Sticky Scroll | Scorrimento fisso |
| Diagnosen am Zeilenende | Diagnostica a fine riga |
| Vim-Modus | Modalità Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMALE / INSERISCI / SOSTITUISCI / VISUALE / VISUALE RIGA / VISUALE BLOCCO (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Espressione non trovata: {} … (as this language's Vim says them) |
| Vorschlag | proposta |
| Vorhersage | Previsione |
| Ausschlussliste | elenco di esclusione |
| Blame | Blame |
| Hinweise (Rotor) | Suggerimenti |
| Schnellkorrektur | correzione rapida |
| Refactoring | refactoring |
| Quellaktion | azione sul codice sorgente |
| Implementierung / Typdefinition / Deklaration | implementazione / definizione di tipo / dichiarazione |
| Aufrufhierarchie / Typhierarchie | Gerarchia delle chiamate / Gerarchia dei tipi |
| Aufrufer / Aufgerufene | Chiamanti / Chiamati |
| Obertypen / Untertypen | Supertipi / Sottotipi |
| indexieren | indicizzare |
| Lesezeichen / Verlauf / Adresse | segnalibro / cronologia / indirizzo |
| Bytes | byte |
| Exportieren | Esporta |
| Format | Formato |
| Diktat | dettatura |
| Agent | agente |
| Eigener Agent | agente integrato |
| Eigene Agenten | Agenti personalizzati |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sessione |
| Subagent | subagente |
| Hintergrundprozess | processo in background |
| Unterhaltung | conversazione |
| Zug | turno |
| Antwort | risposta |
| Rückfrage | domanda |
| Freigabe, Freigabemodus | permesso, modalità permessi |
| Umgehen | Ignora |
| Erlauben / Ablehnen / Immer erlauben … | Consenti / Rifiuta / Consenti sempre… |
| Plan | piano |
| To-dos | Cose da fare |
| Aufgabe | task (attività is the Activity list) |
| Auftrag | incarico |
| Werkzeug | strumento |
| Skills | Skill |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Contenuto esterno |
| Sicherungspunkt | punto di ripristino |
| Zurückspulen | riavvolgere |
| Verdichten | compattare |
| Kontext, Kontextfenster | contesto, finestra di contesto |
| Denktiefe | profondità di ragionamento |
| Modell / Anbieter / Endpunkt | modello / provider / endpoint |
| Token, Tokens | token |
| API-Schlüssel | chiave API |
| Register | registro |
| Adapter | adattatore |
| Debug-Adapter | adattatore di debug |
| Bereit | Pronto |
| Prüfsumme | checksum |
| Anmeldung, anmelden | accesso, accedere |
| Konto | account |
| Abo | Abbonamento |
| Repository | repository |
| Commit, committen | commit, fare il commit |
| Secrets | segreti |
| vormerken | mettere in stage (togliere dallo stage) |
| Zurücknehmen | Togli dallo stage |
| nicht versioniert | non tracciato |
| Abschnitt (Git) | blocco |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | riprendi |
| Arbeitskopie | copia di lavoro |
| Lauf, Läufe | esecuzione, esecuzioni |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Debug |
| Haltepunkt, Haltepunkte | punto di interruzione, punti di interruzione |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condizione / numero di passaggi |
| Aufrufliste | stack di chiamate |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | espressione di controllo, espressioni di controllo |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continua / Esegui istruzione/routine / Esegui istruzione / Esci da istruzione/routine |
| Anhängen / Trennen | collega / scollega |
| An Prozess anhängen | Collega al processo |
| Neu starten / Stopp | Riavvia / Interrompi |
| Variablen | variabili |
| Angehalten | in pausa |
| Debug-Konsole | console di debug |
| Startkonfiguration | configurazione di avvio |
| Inspektor | Inspector |
| **Tests** | |
| Tests | Test |
| Test | test |
| **Containers** | |
| Container | Container |
| Docker-Kontext | contesto Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Sala di controllo |
| Auftrag (Leitstand) | ordine di lavoro, ordini di lavoro |
| Nachweis | evidenza |
| Abweichung | deviazione |
| Abnahme, abnehmen | accettazione, accettare |
| Freigeben (Spezifikation) | approvare (Approva) |
| Planer | Pianificatore |
| Builder | Builder |
| Prüfer | Revisore |
| Späher | Esploratore |
| Prüfstation, Gate | gate |
| Prüfbefehl | comando di verifica |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Ingresso / Specifica / Build / Revisione / Accettazione / Ripresa / Completato |
| Risikoklasse | classe di rischio |
| Pfadbereich | ambito dei percorsi |
| Herkunft | provenienza |
| Station | stazione |
| Kriterium, Kriterien | criterio, criteri |
| Prüfrunde | ciclo di revisione |
| Befund | rilievo |
| Hersteller, Herstellerwechsel | produttore, cambio di produttore |
| Kontingent | quota |
| Workflow-Datei | file del workflow |
| Stand | stato |
| Erstdurchlaufquote, Erstdurchlauf, Quote | tasso di primo passaggio, primo passaggio, tasso |
| Kennzahlen | metriche |
| Regelvorschlag | proposta di regola |
| wiederkehrender Auftrag | ordine di lavoro ricorrente |
| Abstand (wiederkehrender Auftrag) | intervallo |
| eingereiht | in coda |
| Rolle | ruolo |
| Einwand | obiezione |
| Rangliste | lista di priorità |
| Prüfer-Regel | regola del revisore (Altro produttore obbligatorio / preferito) |
| Vorgabe, auf Vorgabe zurücksetzen | predefinito, Ripristina predefinito |
| Mitkopieren / Einrichtung | copia anche / configurazione (Konfiguration: file di configurazione) |
| Prüfrelevante Dateien | file rilevanti per la revisione |
| Issue | issue (la issue) |
| Merge Request | merge request (la merge request) |
| Rechte | Permessi |
| Erlauben / Fragen / Verbieten | Consenti / Chiedi / Nega |
| Gedächtnis | memoria |
| Anweisungen | istruzioni |
| Ausgabestil | stile di output |
| Auswahl | Filtro |
| Plugin, Plugins | plug-in |
| Marktplatz, Marktplätze | marketplace |
| Frage nebenbei | domanda a margine |
| Header | intestazioni |
| Umgebung | ambiente |
| Bericht | rapporto |
| Grenzen | Limiti |

### Japanese (`ja`)
- Address: no pronoun; explanations and questions in the polite です/ます form ("保存しますか?", "…してください"); menu items and
  buttons as plain verbs or nouns ("開く", "保存", "すべてを選択")
- Typography: “…” around names and interface words, as Japanese macOS ("“設定”で…"; not the 「…」 splat uses); full-width
  、。（） but half-width ? ! and ":" as macOS writes them ("アドレス:"); no space between Japanese and Latin text ("Finderに表示",
  "APIキー"); ellipsis "…" directly after the word ("設定…", "検索…"); katakana without a final long-vowel mark where Apple
  drops it (フォルダ, サーバ, プロバイダ)
- Git and CI words: katakana as Japanese developers write them: リポジトリ, コミット, ブランチ, リモート, プッシュ, プル, フェッチ, リベース, マージ,
  ステージ/ステージ解除, ハンク, ワークツリー, フック, ジョブ, ログ, トークン; kanji where Japanese Git tools use it: 競合 (conflict), 差分 (diff), 作業ツリー
  (working tree), 実行 (CI run); "AI" stays AI

| German key | Japanese |
|---|---|
| Ablage | ファイル |
| Bearbeiten | 編集 |
| Darstellung | 表示 |
| Fenster | ウインドウ |
| Hilfe | ヘルプ |
| Dienste | サービス |
| Über {} | {}について |
| Einstellungen | 設定 (menu item: 設定…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {}を非表示 / ほかを非表示 / すべてを表示 |
| {} beenden | {}を終了 |
| Widerrufen / Wiederholen | 取り消す / やり直す |
| Ausschneiden / Kopieren / Einsetzen / Löschen | カット / コピー / ペースト / 削除 |
| Alles auswählen | すべてを選択 |
| Suchen … / Weitersuchen / Rückwärts suchen | 検索… / 次を検索 / 前を検索 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | しまう / 拡大/縮小 / すべてを手前に移動 (拡大/縮小 as macOS; splat has 縮小/拡大) |
| Vollbildmodus | フルスクリーンにする |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | ツールバーを非表示 / サイドバーを切り替え |
| Sichern | 保存 |
| Öffnen / Schließen | 開く / 閉じる |
| Abbrechen / OK | キャンセル / OK |
| Im Finder zeigen | Finderに表示 |
| Papierkorb | ゴミ箱 (ゴミ箱に入れる) |
| Schlüsselbund | キーチェーン |
| Zwischenablage | クリップボード |
| Tab | タブ |
| AI-Tab | AIタブ |
| Panel | パネル |
| Seitenleiste | サイドバー |
| Vorschau | プレビュー |
| Bereich | ペイン |
| Aktivität | アクティビティ |
| Projekt / Ordner | プロジェクト / フォルダ |
| Vertrauen, vertrauen | 信頼, 信頼する (信頼済みのフォルダ) |
| Platte | ディスク (ディスク上の) |
| Unterschiede | 差分 |
| Zusammenführen | マージ |
| Konflikt | 競合 |
| Treffer | 結果 (検索結果) |
| Suche | 検索 |
| Ersetzen | 置換 |
| Sprachserver | 言語サーバ |
| Hinweis (Schweregrad) | ヒント |
| Inlay-Hinweis | インレイヒント |
| Minimap | ミニマップ |
| Sticky Scroll | 固定スクロール |
| Diagnosen am Zeilenende | 行末に診断を表示 |
| Vim-Modus | Vimモード |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | ノーマル / 挿入 / 置換 / ビジュアル / ビジュアル 行 / ビジュアル 矩形 (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: パターンは見つかりませんでした: {} … (as this language's Vim says them) |
| Vorschlag | 提案 |
| Vorhersage | 予測 |
| Ausschlussliste | 除外リスト |
| Blame | Blame |
| Hinweise (Rotor) | ヒント |
| Schnellkorrektur | クイックフィックス |
| Refactoring | リファクタリング |
| Quellaktion | ソースアクション |
| Implementierung / Typdefinition / Deklaration | 実装 / 型定義 / 宣言 |
| Aufrufhierarchie / Typhierarchie | 呼び出し階層 / 型階層 |
| Aufrufer / Aufgerufene | 呼び出し元 / 呼び出し先 |
| Obertypen / Untertypen | スーパータイプ / サブタイプ |
| indexieren | インデックス作成 |
| Lesezeichen / Verlauf / Adresse | ブックマーク / 履歴 / アドレス |
| Bytes | バイト |
| Exportieren | 書き出す |
| Format | フォーマット |
| Diktat | 音声入力 |
| Agent | エージェント |
| Eigener Agent | 内蔵エージェント |
| Eigene Agenten | カスタムエージェント |
| Thread (AI-Thread, CLI-Thread) | スレッド (AIスレッド, CLIスレッド) |
| Sitzung | セッション |
| Subagent | サブエージェント |
| Hintergrundprozess | バックグラウンドプロセス |
| Unterhaltung | 会話 |
| Zug | ターン |
| Antwort | 応答 |
| Rückfrage | 質問 |
| Freigabe, Freigabemodus | 権限, 権限モード |
| Umgehen | バイパス |
| Erlauben / Ablehnen / Immer erlauben … | 許可 / 拒否 / 常に許可… |
| Plan | プラン |
| To-dos | To-do |
| Aufgabe | タスク |
| Auftrag | タスク |
| Werkzeug | ツール |
| Skills | スキル |
| Befehl | コマンド |
| Hook | フック |
| Sandbox | サンドボックス |
| Fremder Inhalt | 外部コンテンツ |
| Sicherungspunkt | チェックポイント |
| Zurückspulen | 巻き戻し, 巻き戻す |
| Verdichten | 圧縮, 圧縮する |
| Kontext, Kontextfenster | コンテキスト, コンテキストウインドウ |
| Denktiefe | 思考の深さ |
| Modell / Anbieter / Endpunkt | モデル / プロバイダ / エンドポイント |
| Token, Tokens | トークン |
| API-Schlüssel | APIキー |
| Register | レジストリ |
| Adapter | アダプタ |
| Debug-Adapter | デバッグアダプタ |
| Bereit | 準備完了 |
| Prüfsumme | チェックサム |
| Anmeldung, anmelden | サインイン, サインインする |
| Konto | アカウント |
| Abo | サブスクリプション |
| Repository | リポジトリ |
| Commit, committen | コミット, コミットする |
| Secrets | シークレット |
| vormerken | ステージする (ステージ解除) |
| Zurücknehmen | ステージ解除 |
| nicht versioniert | 未追跡 |
| Abschnitt (Git) | ハンク |
| Branch / Remote | ブランチ / リモート |
| Push / Pull / Fetch / Rebase | プッシュ / プル / フェッチ / リベース |
| Worktree | ワークツリー |
| Stash | スタッシュ |
| Übernehmen (Worktree) | 取り込む |
| Arbeitskopie | 作業ツリー |
| Lauf, Läufe | 実行 |
| Host | ホスト |
| Job | ジョブ |
| Log | ログ |
| **Debugger** | |
| Debuggen | デバッグ |
| Haltepunkt, Haltepunkte | ブレークポイント、ブレークポイント |
| Logpunkt | ログポイント |
| Bedingung / Trefferzahl | 条件 / ヒット回数 |
| Aufrufliste | コールスタック |
| Frame | フレーム |
| Überwachter Ausdruck, Überwachte Ausdrücke | ウォッチ式、ウォッチ式 |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | 続行 / ステップオーバー / ステップイン / ステップアウト |
| Anhängen / Trennen | アタッチ / デタッチ |
| An Prozess anhängen | プロセスにアタッチ |
| Neu starten / Stopp | 再起動 / 停止 |
| Variablen | 変数 |
| Angehalten | 一時停止中 |
| Debug-Konsole | デバッグコンソール |
| Startkonfiguration | 起動構成 |
| Inspektor | インスペクタ |
| **Tests** | |
| Tests | テスト |
| Test | テスト |
| **Containers** | |
| Container | コンテナ |
| Docker-Kontext | Dockerコンテキスト |
| **Leitstand (phase 13)** | |
| Leitstand | 管制室 |
| Auftrag (Leitstand) | 作業指示 |
| Nachweis | エビデンス |
| Abweichung | 逸脱 |
| Abnahme, abnehmen | 検収, 検収する |
| Freigeben (Spezifikation) | 承認 |
| Planer | プランナー |
| Builder | ビルダー |
| Prüfer | レビュアー |
| Späher | スカウト |
| Prüfstation, Gate | ゲート |
| Prüfbefehl | ゲートコマンド |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | 受付 / 仕様 / ビルド / レビュー / 検収 / 取り込み / 完了 |
| Risikoklasse | リスククラス |
| Pfadbereich | パス範囲 |
| Herkunft | 来歴 |
| Station | 工程 |
| Kriterium, Kriterien | 基準, 基準 |
| Prüfrunde | レビューラウンド |
| Befund | 所見 |
| Hersteller, Herstellerwechsel | ベンダー, ベンダー変更 |
| Kontingent | クォータ |
| Workflow-Datei | ワークフローファイル |
| Stand | 状態 |
| Erstdurchlaufquote, Erstdurchlauf, Quote | 初回通過率, 初回通過, 率 |
| Kennzahlen | 指標 |
| Regelvorschlag | ルール提案 |
| wiederkehrender Auftrag | 定期的な作業指示 |
| Abstand (wiederkehrender Auftrag) | 間隔 |
| eingereiht | 待機中 |
| Rolle | ロール |
| Einwand | 異議 |
| Rangliste | 優先順位リスト |
| Prüfer-Regel | レビュアーのルール (別ベンダー必須 / 別ベンダー優先) |
| Vorgabe, auf Vorgabe zurücksetzen | デフォルト, デフォルトに戻す |
| Mitkopieren / Einrichtung | 一緒にコピー / セットアップ |
| Prüfrelevante Dateien | レビューに関わるファイル |
| Issue | Issue |
| Merge Request | Merge Request |
| Rechte | 権限 |
| Erlauben / Fragen / Verbieten | 許可 / 尋ねる / 拒否 |
| Gedächtnis | メモリ |
| Anweisungen | 指示 |
| Ausgabestil | 出力スタイル |
| Auswahl | フィルタ |
| Plugin, Plugins | プラグイン |
| Marktplatz, Marktplätze | マーケットプレイス |
| Frage nebenbei | 別枠の質問 |
| Header | ヘッダ |
| Umgebung | 環境 |
| Bericht | レポート |
| Grenzen | 上限 |

### Korean (`ko`)
- Address: no pronoun; sentences in the formal 합쇼체, as Korean macOS ("저장하겠습니까?", "…하십시오", "…합니다"); menu items and
  buttons as nouns ("열기", "저장", "닫기")
- Typography: ‘…’ single quotation marks, as Korean macOS writes them (not the “…” splat uses); ellipsis "…" directly
  after the word, no space ("설정…", "찾기…"); a space between a Latin name and a following Korean noun ("AI 탭", "API 키"),
  particles attached ("Finder에서 보기", "{}에 관하여")
- Git and CI words: Hangul transliterations as Korean developers say them: 커밋, 브랜치, 푸시, 풀, 페치, 리베이스, 스테이징, 헝크, 워크트리, 훅,
  잡 (CI job), 로그, 토큰; Sino-Korean where Korean Git tools use it: 저장소 (repository), 원격 (remote), 병합 (merge), 충돌
  (conflict), 작업 트리 (working tree), 실행 (CI run); "AI" stays AI

| German key | Korean |
|---|---|
| Ablage | 파일 |
| Bearbeiten | 편집 |
| Darstellung | 보기 |
| Fenster | 윈도우 |
| Hilfe | 도움말 |
| Dienste | 서비스 |
| Über {} | {}에 관하여 |
| Einstellungen | 설정 (menu item: 설정…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {} 가리기 / 기타 가리기 / 모두 보기 |
| {} beenden | {} 종료 |
| Widerrufen / Wiederholen | 실행 취소 / 실행 복귀 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 오려두기 / 복사하기 / 붙여넣기 / 삭제 |
| Alles auswählen | 전체 선택 |
| Suchen … / Weitersuchen / Rückwärts suchen | 찾기… / 다음 찾기 / 이전 찾기 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 최소화 / 확대/축소 / 모두 앞으로 가져오기 |
| Vollbildmodus | 전체 화면 시작 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 도구 막대 가리기 / 사이드바 토글 |
| Sichern | 저장 |
| Öffnen / Schließen | 열기 / 닫기 |
| Abbrechen / OK | 취소 / 확인 |
| Im Finder zeigen | Finder에서 보기 |
| Papierkorb | 휴지통 (휴지통으로 이동) |
| Schlüsselbund | 키체인 |
| Zwischenablage | 클립보드 |
| Tab | 탭 |
| AI-Tab | AI 탭 |
| Panel | 패널 |
| Seitenleiste | 사이드바 |
| Vorschau | 미리보기 |
| Bereich | 분할 창 |
| Aktivität | 활동 |
| Projekt / Ordner | 프로젝트 / 폴더 |
| Vertrauen, vertrauen | 신뢰, 신뢰하다 (신뢰하는 폴더) |
| Platte | 디스크 (디스크에 있는) |
| Unterschiede | 차이점 |
| Zusammenführen | 병합 |
| Konflikt | 충돌 |
| Treffer | 결과 |
| Suche | 검색 |
| Ersetzen | 대치 |
| Sprachserver | 언어 서버 |
| Hinweis (Schweregrad) | 힌트 |
| Inlay-Hinweis | 인레이 힌트 |
| Minimap | 미니맵 |
| Sticky Scroll | 고정 스크롤 |
| Diagnosen am Zeilenende | 줄 끝에 진단 표시 |
| Vim-Modus | Vim 모드 |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names; Korean Vim's 끼워넣기 is not the word macOS uses for insert) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: 패턴을 찾을 수 없습니다: {} … (as this language's Vim says them; E748 translated, which the catalog leaves open) |
| Vorschlag | 제안 |
| Vorhersage | 예측 |
| Ausschlussliste | 제외 목록 |
| Blame | Blame |
| Hinweise (Rotor) | 힌트 |
| Schnellkorrektur | 빠른 수정 |
| Refactoring | 리팩토링 |
| Quellaktion | 소스 동작 |
| Implementierung / Typdefinition / Deklaration | 구현 / 유형 정의 / 선언 |
| Aufrufhierarchie / Typhierarchie | 호출 계층 / 유형 계층 |
| Aufrufer / Aufgerufene | 호출자 / 호출 대상 |
| Obertypen / Untertypen | 상위 유형 / 하위 유형 |
| indexieren | 색인 생성 |
| Lesezeichen / Verlauf / Adresse | 북마크 / 방문 기록 / 주소 |
| Bytes | 바이트 |
| Exportieren | 내보내기 |
| Format | 포맷 |
| Diktat | 받아쓰기 |
| Agent | 에이전트 |
| Eigener Agent | 내장 에이전트 |
| Eigene Agenten | 사용자 에이전트 |
| Thread (AI-Thread, CLI-Thread) | 스레드 (AI 스레드, CLI 스레드) |
| Sitzung | 세션 |
| Subagent | 하위 에이전트 |
| Hintergrundprozess | 백그라운드 프로세스 |
| Unterhaltung | 대화 |
| Zug | 턴 |
| Antwort | 응답 |
| Rückfrage | 질문 |
| Freigabe, Freigabemodus | 권한, 권한 모드 |
| Umgehen | 우회 |
| Erlauben / Ablehnen / Immer erlauben … | 허용 / 거부 / 항상 허용… |
| Plan | 계획 |
| To-dos | 할 일 |
| Aufgabe | 작업 |
| Auftrag | 작업 지시 |
| Werkzeug | 도구 |
| Skills | 스킬 |
| Befehl | 명령 |
| Hook | 훅 |
| Sandbox | 샌드박스 |
| Fremder Inhalt | 외부 콘텐츠 |
| Sicherungspunkt | 체크포인트 |
| Zurückspulen | 되감기 |
| Verdichten | 압축 |
| Kontext, Kontextfenster | 컨텍스트, 컨텍스트 윈도우 |
| Denktiefe | 사고 깊이 |
| Modell / Anbieter / Endpunkt | 모델 / 제공자 / 엔드포인트 |
| Token, Tokens | 토큰 |
| API-Schlüssel | API 키 |
| Register | 레지스트리 |
| Adapter | 어댑터 |
| Debug-Adapter | 디버그 어댑터 |
| Bereit | 준비됨 |
| Prüfsumme | 체크섬 |
| Anmeldung, anmelden | 로그인, 로그인하다 |
| Konto | 계정 |
| Abo | 구독 |
| Repository | 저장소 |
| Commit, committen | 커밋, 커밋하다 |
| Secrets | 시크릿 |
| vormerken | 스테이징하다 (스테이징 취소) |
| Zurücknehmen | 스테이징 취소 |
| nicht versioniert | 추적되지 않음 |
| Abschnitt (Git) | 헝크 |
| Branch / Remote | 브랜치 / 원격 |
| Push / Pull / Fetch / Rebase | 푸시 / 풀 / 페치 / 리베이스 |
| Worktree | 워크트리 |
| Stash | 스태시 |
| Übernehmen (Worktree) | 가져오기 |
| Arbeitskopie | 작업 트리 |
| Lauf, Läufe | 실행 |
| Host | 호스트 |
| Job | 잡 (작업 is the agent's task) |
| Log | 로그 |
| **Debugger** | |
| Debuggen | 디버그 |
| Haltepunkt, Haltepunkte | 중단점, 중단점 |
| Logpunkt | 로그포인트 |
| Bedingung / Trefferzahl | 조건 / 적중 횟수 |
| Aufrufliste | 호출 스택 |
| Frame | 프레임 |
| Überwachter Ausdruck, Überwachte Ausdrücke | 감시식, 감시식 |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | 계속 / 스텝 오버 / 스텝 인 / 스텝 아웃 |
| Anhängen / Trennen | 연결 / 연결 해제 |
| An Prozess anhängen | 프로세스에 연결 |
| Neu starten / Stopp | 재시작 / 중단 |
| Variablen | 변수 |
| Angehalten | 일시 정지됨 |
| Debug-Konsole | 디버그 콘솔 |
| Startkonfiguration | 실행 구성 |
| Inspektor | 인스펙터 |
| **Tests** | |
| Tests | 테스트 |
| Test | 테스트 |
| **Containers** | |
| Container | 컨테이너 |
| Docker-Kontext | Docker 컨텍스트 |
| **Leitstand (phase 13)** | |
| Leitstand | 관제실 |
| Auftrag (Leitstand) | 작업 주문 |
| Nachweis | 증빙 |
| Abweichung | 특이 사항 |
| Abnahme, abnehmen | 검수, 검수 완료 |
| Freigeben (Spezifikation) | 승인 |
| Planer | 플래너 |
| Builder | 빌더 |
| Prüfer | 검토자 |
| Späher | 스카우트 |
| Prüfstation, Gate | 게이트 |
| Prüfbefehl | 게이트 명령 |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | 접수 / 사양 / 빌드 / 검토 / 검수 / 가져오기 / 완료 |
| Risikoklasse | 위험 등급 |
| Pfadbereich | 경로 범위 |
| Herkunft | 출처 |
| Station | 단계 |
| Kriterium, Kriterien | 기준 |
| Prüfrunde | 검토 라운드 |
| Befund | 발견 사항 |
| Hersteller, Herstellerwechsel | 벤더, 벤더 교체 |
| Kontingent | 할당량 |
| Workflow-Datei | 워크플로 파일 |
| Stand | 상태 |
| Erstdurchlaufquote, Erstdurchlauf, Quote | 1차 통과율, 1차 통과, 비율 |
| Kennzahlen | 지표 |
| Regelvorschlag | 규칙 제안 |
| wiederkehrender Auftrag | 반복 작업 주문 |
| Abstand (wiederkehrender Auftrag) | 간격 |
| eingereiht | 대기 중 |
| Rolle | 역할 |
| Einwand | 이의 |
| Rangliste | 우선순위 목록 |
| Prüfer-Regel | 검토자 규칙 (다른 벤더 필수 / 다른 벤더 우선) |
| Vorgabe, auf Vorgabe zurücksetzen | 기본값, 기본값으로 되돌리기 |
| Mitkopieren / Einrichtung | 함께 복사 / 환경 구성 |
| Prüfrelevante Dateien | 검토에 중요한 파일 |
| Issue | Issue |
| Merge Request | Merge Request |
| Rechte | 권한 |
| Erlauben / Fragen / Verbieten | 허용 / 묻기 / 거부 |
| Gedächtnis | 메모리 |
| Anweisungen | 지침 |
| Ausgabestil | 출력 스타일 |
| Auswahl | 필터 |
| Plugin, Plugins | 플러그인 |
| Marktplatz, Marktplätze | 마켓플레이스 |
| Frage nebenbei | 별도 질문 |
| Header | 헤더 |
| Umgebung | 환경 |
| Bericht | 보고서 |
| Grenzen | 한도 |

### Malay (`ms`)
- Address: "anda" in lower case, as Malay macOS ("Adakah anda ingin…", "Buka pautan hanya jika anda mempercayai
  sumbernya"); menu items and buttons as imperative verbs ("Buka", "Simpan", "Tunjukkan")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Seting…", "Cari…"); menu items,
  buttons and titles in Title Case, as Malay macOS ("Tunjukkan Semua", "Pilih Semua"), running text in sentence case
- Git and CI words: kept as Malaysian developers say them: commit (buat commit), branch, remote, push, pull, fetch,
  rebase, stage/unstage, hunk, worktree, hook, job, log, token, endpoint, sandbox, checksum, registry; Malay where the
  word is settled: repositori, salinan kerja (working tree), gabung (merge), konflik, larian (CI run); "AI" stays AI

| German key | Malay |
|---|---|
| Ablage | Fail |
| Bearbeiten | Edit |
| Darstellung | Paparan |
| Fenster | Tetingkap |
| Hilfe | Bantuan |
| Dienste | Perkhidmatan |
| Über {} | Perihal {} |
| Einstellungen | Seting (menu item: Seting…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sembunyikan {} / Sembunyikan Lain / Tunjukkan Semua |
| {} beenden | Keluar daripada {} |
| Widerrufen / Wiederholen | Buat Asal / Buat Semula |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Potong / Salin / Tampal / Padam |
| Alles auswählen | Pilih Semua |
| Suchen … / Weitersuchen / Rückwärts suchen | Cari… / Cari Seterusnya / Cari Sebelumnya |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimumkan / Zum / Hantar Semua ke Depan Sekali |
| Vollbildmodus | Masuk Skrin Penuh |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sembunyikan Bar Alat / Togol Bar Sisi |
| Sichern | Simpan |
| Öffnen / Schließen | Buka / Tutup |
| Abbrechen / OK | Batal / OK |
| Im Finder zeigen | Tunjukkan dalam Finder |
| Papierkorb | Sampah (Pindah ke Sampah) |
| Schlüsselbund | Rantai Kunci |
| Zwischenablage | papan klip |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | panel |
| Seitenleiste | bar sisi |
| Vorschau | pratonton |
| Bereich | kawasan |
| Aktivität | Aktiviti |
| Projekt / Ordner | projek / folder |
| Vertrauen, vertrauen | percaya, mempercayai (folder yang dipercayai; button: Percaya) |
| Platte | cakera (pada cakera) |
| Unterschiede | perbezaan |
| Zusammenführen | gabung (penggabungan) |
| Konflikt | konflik |
| Treffer | hasil |
| Suche | Carian |
| Ersetzen | Ganti |
| Sprachserver | pelayan bahasa |
| Hinweis (Schweregrad) | Petunjuk |
| Inlay-Hinweis | petunjuk sebaris |
| Minimap | Minimap |
| Sticky Scroll | Sticky Scroll |
| Diagnosen am Zeilenende | Diagnostik di Hujung Baris |
| Vim-Modus | Mod Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Corak tidak ditemui: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | cadangan |
| Vorhersage | Ramalan |
| Ausschlussliste | senarai pengecualian |
| Blame | Blame |
| Hinweise (Rotor) | Petunjuk |
| Schnellkorrektur | pembetulan pantas |
| Refactoring | pemfaktoran semula |
| Quellaktion | tindakan sumber |
| Implementierung / Typdefinition / Deklaration | pelaksanaan / definisi jenis / pengisytiharan |
| Aufrufhierarchie / Typhierarchie | Hierarki Panggilan / Hierarki Jenis |
| Aufrufer / Aufgerufene | Pemanggil / Yang Dipanggil |
| Obertypen / Untertypen | Superjenis / Subjenis |
| indexieren | mengindeks |
| Lesezeichen / Verlauf / Adresse | penanda buku / sejarah / alamat |
| Bytes | bait |
| Exportieren | Eksport |
| Format | Format |
| Diktat | perencanaan (as Malay macOS names Dictation) |
| Agent | ejen |
| Eigener Agent | ejen terbina dalam |
| Eigene Agenten | Ejen tersuai |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sesi |
| Subagent | subejen |
| Hintergrundprozess | proses latar belakang |
| Unterhaltung | perbualan |
| Zug | giliran |
| Antwort | jawapan |
| Rückfrage | soalan |
| Freigabe, Freigabemodus | keizinan, mod keizinan |
| Umgehen | Pintas |
| Erlauben / Ablehnen / Immer erlauben … | Benarkan / Tolak / Sentiasa Benarkan… |
| Plan | pelan |
| To-dos | Senarai Tugasan |
| Aufgabe | tugas |
| Auftrag | tugasan |
| Werkzeug | alat |
| Skills | Skill |
| Befehl | perintah |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Kandungan Luaran |
| Sicherungspunkt | titik semak |
| Zurückspulen | putar balik |
| Verdichten | padatkan (pemadatan) |
| Kontext, Kontextfenster | konteks, tetingkap konteks |
| Denktiefe | kedalaman pemikiran |
| Modell / Anbieter / Endpunkt | model / penyedia / endpoint |
| Token, Tokens | token |
| API-Schlüssel | kunci API |
| Register | registry |
| Adapter | penyesuai |
| Debug-Adapter | penyesuai nyahpepijat |
| Bereit | Sedia |
| Prüfsumme | checksum |
| Anmeldung, anmelden | daftar masuk, daftar masuk ke |
| Konto | akaun |
| Abo | Langganan |
| Repository | repositori |
| Commit, committen | commit, buat commit |
| Secrets | rahsia |
| vormerken | stage (nyahstage) |
| Zurücknehmen | Nyahstage |
| nicht versioniert | tidak dijejak |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | ambil alih |
| Arbeitskopie | salinan kerja |
| Lauf, Läufe | larian |
| Host | hos |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Nyahpepijat |
| Haltepunkt, Haltepunkte | titik henti, titik henti |
| Logpunkt | titik log |
| Bedingung / Trefferzahl | syarat / kiraan hit |
| Aufrufliste | tindanan panggilan |
| Frame | bingkai |
| Überwachter Ausdruck, Überwachte Ausdrücke | ungkapan pantauan, ungkapan pantauan |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Teruskan / Langkah Lalu / Langkah Masuk / Langkah Keluar |
| Anhängen / Trennen | lampirkan / tanggalkan |
| An Prozess anhängen | Lampirkan pada Proses |
| Neu starten / Stopp | Mula Semula / Henti |
| Variablen | pemboleh ubah |
| Angehalten | dijeda |
| Debug-Konsole | konsol nyahpepijat |
| Startkonfiguration | konfigurasi pelancaran |
| Inspektor | Pemeriksa |
| **Tests** | |
| Tests | Ujian |
| Test | ujian |
| **Containers** | |
| Container | Container |
| Docker-Kontext | konteks Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Bilik Kawalan |
| Auftrag (Leitstand) | pesanan kerja |
| Nachweis | bukti |
| Abweichung | sisihan |
| Abnahme, abnehmen | penerimaan, terima |
| Freigeben (Spezifikation) | Luluskan (kelulusan) |
| Planer | Perancang |
| Builder | Pembina |
| Prüfer | Penyemak |
| Späher | Peninjau |
| Prüfstation, Gate | gerbang |
| Prüfbefehl | perintah gerbang |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Kemasukan / Spesifikasi / Binaan / Semakan / Penerimaan / Ambil Alih / Selesai |
| Risikoklasse | kelas risiko |
| Pfadbereich | skop laluan |
| Herkunft | asal usul |
| Station | peringkat |
| Kriterium, Kriterien | kriteria, kriteria |
| Prüfrunde | pusingan semakan |
| Befund | dapatan |
| Hersteller, Herstellerwechsel | vendor, pertukaran vendor |
| Kontingent | kuota |
| Workflow-Datei | fail aliran kerja |
| Stand | keadaan |
| Erstdurchlaufquote, Erstdurchlauf, Quote | kadar lulus pertama, lulus pertama, kadar |
| Kennzahlen | metrik |
| Regelvorschlag | cadangan peraturan |
| wiederkehrender Auftrag | pesanan kerja berulang |
| Abstand (wiederkehrender Auftrag) | selang |
| eingereiht | dalam baris gilir |
| Rolle | peranan |
| Einwand | bantahan (Bantahan Pembina) |
| Rangliste | senarai kedudukan |
| Prüfer-Regel | peraturan penyemak (Vendor lain wajib / Vendor lain diutamakan) |
| Vorgabe, auf Vorgabe zurücksetzen | Lalai, Kembalikan kepada Lalai |
| Mitkopieren / Einrichtung | turut disalin / persediaan |
| Prüfrelevante Dateien | Fail Penting untuk Semakan |
| Issue | issue |
| Merge Request | merge request (title and button: Merge Request) |
| Rechte | Keizinan |
| Erlauben / Fragen / Verbieten | Benarkan / Tanya / Larang |
| Gedächtnis | memori |
| Anweisungen | arahan |
| Ausgabestil | gaya output |
| Auswahl | Penapis |
| Plugin, Plugins | plugin |
| Marktplatz, Marktplätze | pasaran |
| Frage nebenbei | soalan sampingan (title: Soalan Sampingan) |
| Header | pengepala |
| Umgebung | persekitaran |
| Bericht | laporan |
| Grenzen | Had |

### Dutch (`nl`)
- Address: "je", as Dutch macOS ("Wil je…", "Weet je zeker dat…"); menu items and buttons as imperatives, verb first
  ("Bewaar", "Toon in Finder", "Verberg andere")
- Typography: straight single quotes '…' and the straight apostrophe ', as Dutch macOS writes them ("het document
  'Notities'", "foto's"; not the ‘…’ splat uses); ellipsis "…" directly after the word, no space ("Instellingen…",
  "Zoek…"); sentence case
- Git and CI words: kept as Dutch developers say them: repository, commit/committen, branch, remote, push, pull, fetch,
  rebase, stagen/unstagen, hunk, worktree, hook, job, log, run (CI), token, endpoint, checksum; Dutch where the word is
  settled: samenvoegen (merge), conflict, werkkopie (working tree), verschillen (diff); "AI" stays AI

| German key | Dutch |
|---|---|
| Ablage | Archief |
| Bearbeiten | Wijzig |
| Darstellung | Weergave |
| Fenster | Venster |
| Hilfe | Help |
| Dienste | Voorzieningen |
| Über {} | Over {} |
| Einstellungen | Instellingen (menu item: Instellingen…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Verberg {} / Verberg andere / Toon alle |
| {} beenden | Stop {} |
| Widerrufen / Wiederholen | Herstel / Opnieuw |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Knip / Kopieer / Plak / Verwijder |
| Alles auswählen | Selecteer alles |
| Suchen … / Weitersuchen / Rückwärts suchen | Zoek… / Zoek volgende / Zoek vorige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimaliseer / Vergroot/verklein / Alles op voorgrond |
| Vollbildmodus | Schakel schermvullende weergave in |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Verberg knoppenbalk / Toon/verberg navigatiekolom |
| Sichern | Bewaar |
| Öffnen / Schließen | Open / Sluit |
| Abbrechen / OK | Annuleer / OK |
| Im Finder zeigen | Toon in Finder |
| Papierkorb | prullenmand (Verplaats naar prullenmand) |
| Schlüsselbund | sleutelhanger |
| Zwischenablage | klembord |
| Tab | tabblad |
| AI-Tab | AI-tabblad |
| Panel | paneel |
| Seitenleiste | navigatiekolom |
| Vorschau | voorvertoning |
| Bereich | deelvenster |
| Aktivität | Activiteit |
| Projekt / Ordner | project / map |
| Vertrauen, vertrauen | vertrouwen (vertrouwde map; button: Vertrouw) |
| Platte | schijf (op schijf) |
| Unterschiede | verschillen |
| Zusammenführen | samenvoegen (button: Voeg samen) |
| Konflikt | conflict |
| Treffer | resultaat |
| Suche | Zoeken |
| Ersetzen | Vervang |
| Sprachserver | taalserver |
| Hinweis (Schweregrad) | Hint |
| Inlay-Hinweis | inlay-hint |
| Minimap | Minimap |
| Sticky Scroll | Vastgezet scrollen |
| Diagnosen am Zeilenende | Diagnoses aan regeleinde |
| Vim-Modus | Vim-modus |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAAL / INVOEGEN / VERVANGEN / VISUEEL / VISUELE REGEL / VISUEEL BLOK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: patroon niet gevonden: {} … (as this language's Vim says them; E348, E748 and E354 translated, which the catalog leaves open) |
| Vorschlag | voorstel |
| Vorhersage | Voorspelling |
| Ausschlussliste | uitsluitingslijst |
| Blame | Blame |
| Hinweise (Rotor) | Hints |
| Schnellkorrektur | snelle oplossing |
| Refactoring | refactoring |
| Quellaktion | bronactie |
| Implementierung / Typdefinition / Deklaration | implementatie / typedefinitie / declaratie |
| Aufrufhierarchie / Typhierarchie | Aanroephiërarchie / Typehiërarchie |
| Aufrufer / Aufgerufene | Aanroepers / Aangeroepen |
| Obertypen / Untertypen | Supertypen / Subtypen |
| indexieren | indexeren |
| Lesezeichen / Verlauf / Adresse | bladwijzer / geschiedenis / adres |
| Bytes | bytes |
| Exportieren | Exporteer |
| Format | Opmaak |
| Diktat | dicteerfunctie (dicteren) |
| Agent | agent |
| Eigener Agent | ingebouwde agent |
| Eigene Agenten | Eigen agents |
| Thread (AI-Thread, CLI-Thread) | thread (AI-thread, CLI-thread) |
| Sitzung | sessie |
| Subagent | subagent |
| Hintergrundprozess | achtergrondproces |
| Unterhaltung | gesprek |
| Zug | beurt |
| Antwort | antwoord |
| Rückfrage | vraag |
| Freigabe, Freigabemodus | toestemming, toestemmingsmodus |
| Umgehen | Omzeil |
| Erlauben / Ablehnen / Immer erlauben … | Sta toe / Weiger / Sta altijd toe… |
| Plan | plan |
| To-dos | To-do's |
| Aufgabe | taak |
| Auftrag | opdracht |
| Werkzeug | tool |
| Skills | Skills |
| Befehl | commando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Externe inhoud |
| Sicherungspunkt | herstelpunt |
| Zurückspulen | terugspoelen |
| Verdichten | compacteren |
| Kontext, Kontextfenster | context, contextvenster |
| Denktiefe | denkdiepte |
| Modell / Anbieter / Endpunkt | model / aanbieder / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-sleutel |
| Register | register |
| Adapter | adapter |
| Debug-Adapter | debugadapter |
| Bereit | Gereed |
| Prüfsumme | checksum |
| Anmeldung, anmelden | inloggen (button: Log in) |
| Konto | account |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committen |
| Secrets | geheimen |
| vormerken | stagen (unstagen) |
| Zurücknehmen | Unstage |
| nicht versioniert | niet gevolgd |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | overnemen |
| Arbeitskopie | werkkopie |
| Lauf, Läufe | run, runs |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Debug |
| Haltepunkt, Haltepunkte | breakpoint, breakpoints |
| Logpunkt | logpunt |
| Bedingung / Trefferzahl | voorwaarde / aantal treffers |
| Aufrufliste | aanroepstack |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | bewaakte expressie, bewaakte expressies |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Ga door / Stap over / Stap in / Stap uit |
| Anhängen / Trennen | koppel / maak los |
| An Prozess anhängen | Koppel aan proces |
| Neu starten / Stopp | Herstart / Stop |
| Variablen | variabelen |
| Angehalten | onderbroken |
| Debug-Konsole | debugconsole |
| Startkonfiguration | startconfiguratie |
| Inspektor | Inspecteur |
| **Tests** | |
| Tests | Tests |
| Test | test |
| **Containers** | |
| Container | Containers |
| Docker-Kontext | Docker-context |
| **Leitstand (phase 13)** | |
| Leitstand | Controlekamer |
| Auftrag (Leitstand) | werkorder, werkorders |
| Nachweis | bewijs |
| Abweichung | afwijking, afwijkingen |
| Abnahme, abnehmen | acceptatie, accepteren (button: Accepteer) |
| Freigeben (Spezifikation) | goedkeuren (button: Keur goed; zonder goedkeuring) |
| Planer | Planner |
| Builder | Builder |
| Prüfer | Reviewer |
| Späher | Verkenner |
| Prüfstation, Gate | gate, gates |
| Prüfbefehl | gatecommando, gatecommando's |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Intake / Specificatie / Bouw / Review / Acceptatie / Overname / Voltooid |
| Risikoklasse | risicoklasse |
| Pfadbereich | padbereik |
| Herkunft | herkomst |
| Station | station |
| Kriterium, Kriterien | criterium, criteria |
| Prüfrunde | reviewronde |
| Befund | bevinding |
| Hersteller, Herstellerwechsel | leverancier, leverancierswissel |
| Kontingent | quotum |
| Workflow-Datei | workflowbestand |
| Stand | toestand |
| Erstdurchlaufquote, Erstdurchlauf, Quote | percentage eerste doorloop, eerste doorloop, percentage |
| Kennzahlen | kengetallen |
| Regelvorschlag | regelvoorstel |
| wiederkehrender Auftrag | terugkerende werkorder |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | in wachtrij |
| Rolle | rol |
| Einwand | bezwaar |
| Rangliste | ranglijst |
| Prüfer-Regel | reviewerregel (Andere leverancier verplicht / bij voorkeur) |
| Vorgabe, auf Vorgabe zurücksetzen | standaard, Zet terug naar standaard |
| Mitkopieren / Einrichtung | meekopiëren / instellen |
| Prüfrelevante Dateien | bestanden die tellen voor de review |
| Issue | issue (de issue) |
| Merge Request | merge request (de merge request) |
| Rechte | Toestemmingen |
| Erlauben / Fragen / Verbieten | Sta toe / Vraag / Verbied |
| Gedächtnis | geheugen |
| Anweisungen | instructies |
| Ausgabestil | uitvoerstijl |
| Auswahl | Filter |
| Plugin, Plugins | plug-in, plug-ins |
| Marktplatz, Marktplätze | marketplace, marketplaces |
| Frage nebenbei | tussenvraag |
| Header | headers |
| Umgebung | omgeving |
| Bericht | rapport |
| Grenzen | Limieten |

### Norwegian Bokmål (`no`)
- Address: "du", as Norwegian macOS ("Vil du…", "Er du sikker på at…"); menu items and buttons as imperatives ("Åpne",
  "Lagre", "Vis alle")
- Typography: «…» quotation marks; ellipsis "…" directly after the word, no space ("Innstillinger…", "Finn…"); sentence
  case
- Git and CI words: kept as Norwegian developers say them: repository, commit/committe, branch, remote, push, pull,
  fetch, rebase, stage/unstage, worktree, hook, token; Norwegian where the word is settled: slå sammen (merge),
  konflikt, blokk (hunk), arbeidskopi (working tree), kjøring (CI run), jobb, logg; "AI" is "KI", as Norwegian macOS
  writes it

| German key | Norwegian Bokmål |
|---|---|
| Ablage | Fil |
| Bearbeiten | Rediger |
| Darstellung | Vis |
| Fenster | Vindu |
| Hilfe | Hjelp |
| Dienste | Tjenester |
| Über {} | Om {} |
| Einstellungen | Innstillinger (menu item: Innstillinger…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skjul {} / Skjul andre / Vis alle |
| {} beenden | Avslutt {} |
| Widerrufen / Wiederholen | Angre / Utfør likevel |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klipp ut / Kopier / Lim inn / Slett |
| Alles auswählen | Marker alt |
| Suchen … / Weitersuchen / Rückwärts suchen | Finn… / Finn neste / Finn forrige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimer / Zoom / Legg alle øverst |
| Vollbildmodus | Gå til fullskjermmodus |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skjul verktøylinje / Vis eller skjul sidepanel |
| Sichern | Lagre |
| Öffnen / Schließen | Åpne / Lukk |
| Abbrechen / OK | Avbryt / OK |
| Im Finder zeigen | Vis i Finder |
| Papierkorb | papirkurv (Flytt til papirkurv) |
| Schlüsselbund | nøkkelring |
| Zwischenablage | Utklipp (in running text: utklippstavlen) |
| Tab | fane |
| AI-Tab | KI-fane |
| Panel | panel |
| Seitenleiste | sidepanel |
| Vorschau | forhåndsvisning |
| Bereich | rute |
| Aktivität | Aktivitet |
| Projekt / Ordner | prosjekt / mappe |
| Vertrauen, vertrauen | tillit, stole på (mappe du stoler på; button: Stol på) |
| Platte | disk (på disken) |
| Unterschiede | forskjeller |
| Zusammenführen | slå sammen (sammenslåing) |
| Konflikt | konflikt |
| Treffer | treff |
| Suche | Søk |
| Ersetzen | Erstatt |
| Sprachserver | språktjener (macOS says tjener for server) |
| Hinweis (Schweregrad) | Tips |
| Inlay-Hinweis | innebygd tips |
| Minimap | Minikart |
| Sticky Scroll | Festet rulling |
| Diagnosen am Zeilenende | Diagnostikk på linjeslutt |
| Vim-Modus | Vim-modus |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / SETT INN / ERSTATT / VISUELL / VISUELL LINJE / VISUELL BLOKK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Fant ikke søketeksten: {} … (as this language's Vim says them) |
| Vorschlag | forslag |
| Vorhersage | Forutsigelse |
| Ausschlussliste | utelatelsesliste |
| Blame | Blame |
| Hinweise (Rotor) | Tips |
| Schnellkorrektur | hurtigretting |
| Refactoring | refaktorering |
| Quellaktion | kildehandling |
| Implementierung / Typdefinition / Deklaration | implementering / typedefinisjon / deklarasjon |
| Aufrufhierarchie / Typhierarchie | Kallhierarki / Typehierarki |
| Aufrufer / Aufgerufene | Kallere / Kalte |
| Obertypen / Untertypen | Supertyper / Undertyper |
| indexieren | indeksere |
| Lesezeichen / Verlauf / Adresse | bokmerke / logg (as Safari) / adresse |
| Bytes | byte |
| Exportieren | Eksporter |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | innebygd agent |
| Eigene Agenten | Egne agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (KI-tråd, CLI-tråd) |
| Sitzung | økt |
| Subagent | underagent |
| Hintergrundprozess | bakgrunnsprosess |
| Unterhaltung | samtale |
| Zug | runde |
| Antwort | svar |
| Rückfrage | spørsmål |
| Freigabe, Freigabemodus | tillatelse, tillatelsesmodus |
| Umgehen | Omgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillat / Avslå / Tillat alltid… |
| Plan | plan |
| To-dos | Gjøremål |
| Aufgabe | oppgave |
| Auftrag | oppdrag |
| Werkzeug | verktøy |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandkasse |
| Fremder Inhalt | Eksternt innhold |
| Sicherungspunkt | sjekkpunkt |
| Zurückspulen | spole tilbake |
| Verdichten | komprimere |
| Kontext, Kontextfenster | kontekst, kontekstvindu |
| Denktiefe | tenkedybde |
| Modell / Anbieter / Endpunkt | modell / leverandør / endepunkt |
| Token, Tokens | token, tokener |
| API-Schlüssel | API-nøkkel |
| Register | register |
| Adapter | adapter |
| Debug-Adapter | feilsøkingsadapter |
| Bereit | Klar |
| Prüfsumme | sjekksum |
| Anmeldung, anmelden | pålogging, logge på |
| Konto | konto |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committe |
| Secrets | hemmeligheter |
| vormerken | stage (unstage) |
| Zurücknehmen | Unstage |
| nicht versioniert | ikke sporet |
| Abschnitt (Git) | blokk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | overta |
| Arbeitskopie | arbeidskopi |
| Lauf, Läufe | kjøring, kjøringer |
| Host | vert |
| Job | jobb |
| Log | logg |
| **Debugger** | |
| Debuggen | Feilsøk |
| Haltepunkt, Haltepunkte | brytepunkt, brytepunkter |
| Logpunkt | loggpunkt |
| Bedingung / Trefferzahl | betingelse / antall treff |
| Aufrufliste | anropsstakk |
| Frame | ramme |
| Überwachter Ausdruck, Überwachte Ausdrücke | overvåket uttrykk, overvåkede uttrykk |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Fortsett / Gå over / Gå inn / Gå ut |
| Anhängen / Trennen | koble til / koble fra |
| An Prozess anhängen | Koble til prosess |
| Neu starten / Stopp | Start på nytt / Stopp |
| Variablen | variabler |
| Angehalten | pauset |
| Debug-Konsole | feilsøkingskonsoll |
| Startkonfiguration | startkonfigurasjon |
| Inspektor | Inspektør |
| **Tests** | |
| Tests | Tester |
| Test | test |
| **Containers** | |
| Container | Containere |
| Docker-Kontext | Docker-kontekst |
| **Leitstand (phase 13)** | |
| Leitstand | kontrollrom |
| Auftrag (Leitstand) | arbeidsordre |
| Nachweis | bevis |
| Abweichung | avvik |
| Abnahme, abnehmen | aksept, akseptere |
| Freigeben (Spezifikation) | godkjenne (button: Godkjenn) |
| Planer | planlegger |
| Builder | bygger |
| Prüfer | kontrollør |
| Späher | speider |
| Prüfstation, Gate | kontrollpunkt |
| Prüfbefehl | kontrollkommando |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Mottak / Spesifikasjon / Bygging / Gjennomgang / Aksept / Overtakelse / Ferdig |
| Risikoklasse | risikoklasse |
| Pfadbereich | stiområde |
| Herkunft | opprinnelse |
| Station | stasjon |
| Kriterium, Kriterien | kriterium, kriterier |
| Prüfrunde | gjennomgangsrunde |
| Befund | funn |
| Hersteller, Herstellerwechsel | produsent, produsentbytte |
| Kontingent | kvote |
| Workflow-Datei | arbeidsflytfil |
| Stand | tilstand |
| Erstdurchlaufquote, Erstdurchlauf, Quote | førstegangsandel, første runde, andel |
| Kennzahlen | nøkkeltall |
| Regelvorschlag | regelforslag |
| wiederkehrender Auftrag | gjentakende arbeidsordre |
| Abstand (wiederkehrender Auftrag) | intervall |
| eingereiht | i kø |
| Rolle | rolle |
| Einwand | innvending |
| Rangliste | rangering |
| Prüfer-Regel | kontrollørregel (Annen produsent påkrevd / foretrukket) |
| Vorgabe, auf Vorgabe zurücksetzen | standard, tilbakestill til standard |
| Mitkopieren / Einrichtung | kopier med / oppsett |
| Prüfrelevante Dateien | filer som teller for gjennomgangen |
| Issue | issue (issuen) |
| Merge Request | merge request |
| Rechte | Tillatelser |
| Erlauben / Fragen / Verbieten | Tillat / Spør / Nekt |
| Gedächtnis | minne |
| Anweisungen | instruksjoner |
| Ausgabestil | utdatastil |
| Auswahl | Filter |
| Plugin, Plugins | plugin, plugins |
| Marktplatz, Marktplätze | markedsplass, markedsplasser |
| Frage nebenbei | sidespørsmål |
| Header | headere |
| Umgebung | miljø |
| Bericht | rapport |
| Grenzen | Grenser |

### Polish (`pl`)
- Address: informal second person singular with the pronoun left out, as Polish macOS ("Czy chcesz…", "Otwieraj łącze
  tylko wtedy, gdy ufasz jego źródłu"); menu items and buttons as imperatives ("Otwórz", "Zachowaj", "Pokaż")
- Typography: „…” quotation marks; ellipsis "…" directly after the word, no space ("Ustawienia…", "Znajdź…"); sentence
  case; the About item is the bare app name with an ellipsis, as Polish macOS writes it ("{}…")
- Git and CI words: kept as Polish developers say them: commit (commitować, od ostatniego commitu), push, pull, fetch,
  rebase, worktree, hook, job, log, token; Polish where Polish Git tools (git-gui, VS Code) use it: repozytorium, gałąź
  (branch), repozytorium zdalne (remote), scalanie (merge), przygotuj/cofnij przygotowanie (stage/unstage), fragment
  (hunk), kopia robocza (working tree), uruchomienie (CI run), konflikt; "AI" stays AI

| German key | Polish |
|---|---|
| Ablage | Plik |
| Bearbeiten | Edycja |
| Darstellung | Widok |
| Fenster | Okno |
| Hilfe | Pomoc |
| Dienste | Usługi |
| Über {} | {}… |
| Einstellungen | Ustawienia (menu item: Ustawienia…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ukryj {} / Ukryj pozostałe / Pokaż wszystkie |
| {} beenden | Zakończ {} |
| Widerrufen / Wiederholen | Cofnij / Przywróć |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Wytnij / Kopiuj / Wklej / Usuń |
| Alles auswählen | Zaznacz wszystko |
| Suchen … / Weitersuchen / Rückwärts suchen | Znajdź… / Znajdź następne / Znajdź poprzednie |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizuj okno / Zoom / Umieść wszystko na wierzchu |
| Vollbildmodus | Tryb pełnoekranowy |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ukryj pasek narzędzi / Przełącz pasek boczny |
| Sichern | Zachowaj |
| Öffnen / Schließen | Otwórz / Zamknij |
| Abbrechen / OK | Anuluj / OK |
| Im Finder zeigen | Pokaż w Finderze |
| Papierkorb | Kosz (Przenieś do Kosza) |
| Schlüsselbund | pęk kluczy |
| Zwischenablage | schowek |
| Tab | karta |
| AI-Tab | karta AI |
| Panel | panel |
| Seitenleiste | pasek boczny |
| Vorschau | podgląd |
| Bereich | okienko |
| Aktivität | Aktywność |
| Projekt / Ordner | projekt / folder |
| Vertrauen, vertrauen | zaufanie, ufać (zaufany folder; button: Zaufaj) |
| Platte | dysk (na dysku) |
| Unterschiede | różnice |
| Zusammenführen | scalanie, scalić |
| Konflikt | konflikt |
| Treffer | wynik |
| Suche | Wyszukiwanie |
| Ersetzen | Zastąp |
| Sprachserver | serwer języka |
| Hinweis (Schweregrad) | Wskazówka |
| Inlay-Hinweis | wskazówka w tekście |
| Minimap | Minimapa |
| Sticky Scroll | Przyklejone przewijanie |
| Diagnosen am Zeilenende | Diagnostyka na końcu wiersza |
| Vim-Modus | Tryb Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMALNY / WPROWADZANIE / ZAMIANA / WIZUALNY / WIZUALNY LINIOWY / WIZUALNY BLOKOWY (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Nie znaleziono wzorca: {} … (as this language's Vim says them) |
| Vorschlag | propozycja |
| Vorhersage | Przewidywanie |
| Ausschlussliste | lista wykluczeń |
| Blame | Blame |
| Hinweise (Rotor) | Wskazówki |
| Schnellkorrektur | szybka poprawka |
| Refactoring | refaktoryzacja |
| Quellaktion | akcja źródłowa |
| Implementierung / Typdefinition / Deklaration | implementacja / definicja typu / deklaracja |
| Aufrufhierarchie / Typhierarchie | Hierarchia wywołań / Hierarchia typów |
| Aufrufer / Aufgerufene | Wywołujące / Wywoływane |
| Obertypen / Untertypen | Nadtypy / Podtypy |
| indexieren | indeksowanie |
| Lesezeichen / Verlauf / Adresse | zakładka / historia / adres |
| Bytes | bajty |
| Exportieren | Eksportuj |
| Format | Format |
| Diktat | dyktowanie |
| Agent | agent |
| Eigener Agent | wbudowany agent |
| Eigene Agenten | Własne agenty |
| Thread (AI-Thread, CLI-Thread) | wątek (wątek AI, wątek CLI) |
| Sitzung | sesja |
| Subagent | subagent |
| Hintergrundprozess | proces w tle |
| Unterhaltung | rozmowa |
| Zug | tura |
| Antwort | odpowiedź |
| Rückfrage | pytanie |
| Freigabe, Freigabemodus | uprawnienie, tryb uprawnień |
| Umgehen | Pomiń |
| Erlauben / Ablehnen / Immer erlauben … | Pozwól / Nie pozwalaj / Zawsze pozwalaj… |
| Plan | plan |
| To-dos | Do zrobienia |
| Aufgabe | zadanie |
| Auftrag | zlecenie |
| Werkzeug | narzędzie |
| Skills | Umiejętności |
| Befehl | polecenie |
| Hook | hook |
| Sandbox | piaskownica |
| Fremder Inhalt | Treść zewnętrzna |
| Sicherungspunkt | punkt kontrolny |
| Zurückspulen | przewiń do tyłu (przewijanie do tyłu) |
| Verdichten | kompaktowanie, kompaktuj |
| Kontext, Kontextfenster | kontekst, okno kontekstu |
| Denktiefe | głębokość myślenia |
| Modell / Anbieter / Endpunkt | model / dostawca / punkt końcowy |
| Token, Tokens | token, tokeny |
| API-Schlüssel | klucz API |
| Register | rejestr |
| Adapter | adapter |
| Debug-Adapter | adapter debugowania |
| Bereit | Gotowy |
| Prüfsumme | suma kontrolna |
| Anmeldung, anmelden | logowanie, zalogować się |
| Konto | konto |
| Abo | Subskrypcja |
| Repository | repozytorium |
| Commit, committen | commit, commitować |
| Secrets | sekrety |
| vormerken | przygotuj (cofnij przygotowanie) |
| Zurücknehmen | Cofnij przygotowanie |
| nicht versioniert | nieśledzony |
| Abschnitt (Git) | fragment |
| Branch / Remote | gałąź / repozytorium zdalne |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | przejmij |
| Arbeitskopie | kopia robocza |
| Lauf, Läufe | uruchomienie, uruchomienia |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Debugowanie |
| Haltepunkt, Haltepunkte | punkt przerwania, punkty przerwania |
| Logpunkt | punkt rejestrowania |
| Bedingung / Trefferzahl | warunek / liczba trafień |
| Aufrufliste | stos wywołań |
| Frame | ramka |
| Überwachter Ausdruck, Überwachte Ausdrücke | obserwowane wyrażenie, obserwowane wyrażenia |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Kontynuuj / Przekrocz / Wkrocz / Wyjdź |
| Anhängen / Trennen | dołącz / odłącz |
| An Prozess anhängen | Dołącz do procesu |
| Neu starten / Stopp | Uruchom ponownie / Zatrzymaj |
| Variablen | zmienne |
| Angehalten | wstrzymano |
| Debug-Konsole | konsola debugowania |
| Startkonfiguration | konfiguracja uruchamiania |
| Inspektor | Inspektor |
| **Tests** | |
| Tests | Testy |
| Test | test |
| **Containers** | |
| Container | Kontenery |
| Docker-Kontext | kontekst Dockera |
| **Leitstand (phase 13)** | |
| Leitstand | sterownia |
| Auftrag (Leitstand) | zamówienie |
| Nachweis | dowód |
| Abweichung | odstępstwo |
| Abnahme, abnehmen | akceptacja, zaakceptować (button: Akceptuj) |
| Freigeben (Spezifikation) | zatwierdzić (button: Zatwierdź; zatwierdzenie) |
| Planer | Planista |
| Builder | Wykonawca |
| Prüfer | Recenzent |
| Späher | Zwiadowca |
| Prüfstation, Gate | bramka |
| Prüfbefehl | polecenie kontrolne |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Wejście / Specyfikacja / Budowa / Recenzja / Akceptacja / Przejęcie / Gotowe |
| Risikoklasse | klasa ryzyka (niska / średnia / wysoka) |
| Pfadbereich | zakres ścieżek |
| Herkunft | pochodzenie |
| Station | stacja |
| Kriterium, Kriterien | kryterium, kryteria |
| Prüfrunde | runda recenzji |
| Befund | ustalenie |
| Hersteller, Herstellerwechsel | producent, zmiana producenta |
| Kontingent | przydział |
| Workflow-Datei | plik przepływu pracy |
| Stand | stan |
| Erstdurchlaufquote, Erstdurchlauf, Quote | odsetek pierwszego podejścia, pierwsze podejście, odsetek |
| Kennzahlen | metryki |
| Regelvorschlag | propozycja reguły |
| wiederkehrender Auftrag | cykliczne zamówienie |
| Abstand (wiederkehrender Auftrag) | interwał |
| eingereiht | w kolejce |
| Rolle | rola |
| Einwand | zastrzeżenie |
| Rangliste | ranking |
| Prüfer-Regel | reguła recenzenta (Inny producent wymagany / preferowany) |
| Vorgabe, auf Vorgabe zurücksetzen | domyślne, przywróć domyślne |
| Mitkopieren / Einrichtung | kopiuj także / przygotowanie |
| Prüfrelevante Dateien | pliki istotne dla recenzji |
| Issue | issue (nieodmienne) |
| Merge Request | merge request (merge requestu, merge requesty) |
| Rechte | Uprawnienia |
| Erlauben / Fragen / Verbieten | Pozwól / Zapytaj / Zabroń |
| Gedächtnis | pamięć |
| Anweisungen | instrukcje |
| Ausgabestil | styl odpowiedzi |
| Auswahl | Filtr |
| Plugin, Plugins | wtyczka, wtyczki |
| Marktplatz, Marktplätze | marketplace, marketplace’y |
| Frage nebenbei | pytanie na boku |
| Header | nagłówki |
| Umgebung | środowisko |
| Bericht | zgłoszenie |
| Grenzen | Limity |

### Portuguese (Brazil) (`pt_BR`)
- Address: "você" with third-person verbs and "seu/sua", as Brazilian macOS ("Deseja tentar novamente?", "Abra o link
  somente se você confiar na fonte"); menu items and buttons as infinitives ("Abrir", "Salvar", "Mostrar Tudo")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Ajustes…", "Buscar…"); menu items,
  buttons and titles in Title Case, as Brazilian macOS ("Selecionar Tudo", "Mostrar Barra Lateral"), running text in
  sentence case
- Git and CI words: kept as Brazilian developers say them: commit (fazer commit), branch (a branch), push, pull, fetch,
  rebase, worktree, hook, job, log, token, endpoint, sandbox; Portuguese where the word is settled: repositório, remoto,
  mesclar (merge, as GitHub in Portuguese), preparar/remover da preparação (stage/unstage), bloco (hunk), cópia de
  trabalho (working tree), execução (CI run), conflito; "AI" is "IA"

| German key | Portuguese (Brazil) |
|---|---|
| Ablage | Arquivo |
| Bearbeiten | Editar |
| Darstellung | Visualizar |
| Fenster | Janela |
| Hilfe | Ajuda |
| Dienste | Serviços |
| Über {} | Sobre o {} |
| Einstellungen | Ajustes (menu item: Ajustes…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar {} / Ocultar Outros / Mostrar Tudo |
| {} beenden | Encerrar {} |
| Widerrufen / Wiederholen | Desfazer / Refazer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Colar / Apagar |
| Alles auswählen | Selecionar Tudo |
| Suchen … / Weitersuchen / Rückwärts suchen | Buscar… / Buscar Seguinte / Buscar Anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Zoom / Trazer Todas para a Frente |
| Vollbildmodus | Entrar em Tela Cheia |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar Barra de Ferramentas / Mostrar/Ocultar Barra Lateral |
| Sichern | Salvar |
| Öffnen / Schließen | Abrir / Fechar |
| Abbrechen / OK | Cancelar / OK |
| Im Finder zeigen | Mostrar no Finder |
| Papierkorb | Lixo (Mover para o Lixo) |
| Schlüsselbund | Chaves (as macOS names the keychain: "nas Chaves") |
| Zwischenablage | área de transferência |
| Tab | aba |
| AI-Tab | aba de IA |
| Panel | painel |
| Seitenleiste | barra lateral |
| Vorschau | pré-visualização |
| Bereich | área |
| Aktivität | Atividade |
| Projekt / Ordner | projeto / pasta |
| Vertrauen, vertrauen | confiança, confiar (pasta confiável; button: Confiar) |
| Platte | disco (no disco) |
| Unterschiede | diferenças |
| Zusammenführen | mesclar (mesclagem) |
| Konflikt | conflito |
| Treffer | resultado |
| Suche | Busca |
| Ersetzen | Substituir |
| Sprachserver | servidor de linguagem |
| Hinweis (Schweregrad) | Dica |
| Inlay-Hinweis | dica embutida |
| Minimap | Minimapa |
| Sticky Scroll | Rolagem Fixa |
| Diagnosen am Zeilenende | Diagnósticos no Fim da Linha |
| Vim-Modus | Modo Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERÇÃO / SUBSTITUIÇÃO / VISUAL / VISUAL/LINHA / VISUAL/BLOCO (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Padrão não encontrado: {} … (as this language's Vim says them) |
| Vorschlag | sugestão |
| Vorhersage | Previsão |
| Ausschlussliste | lista de exclusão |
| Blame | Blame |
| Hinweise (Rotor) | Dicas |
| Schnellkorrektur | correção rápida |
| Refactoring | refatoração |
| Quellaktion | ação de código-fonte |
| Implementierung / Typdefinition / Deklaration | implementação / definição de tipo / declaração |
| Aufrufhierarchie / Typhierarchie | Hierarquia de Chamadas / Hierarquia de Tipos |
| Aufrufer / Aufgerufene | Chamadores / Chamados |
| Obertypen / Untertypen | Supertipos / Subtipos |
| indexieren | indexar |
| Lesezeichen / Verlauf / Adresse | favorito (as Safari) / histórico / endereço |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formatar |
| Diktat | ditado |
| Agent | agente |
| Eigener Agent | agente integrado |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | thread (thread de IA, thread de CLI) |
| Sitzung | sessão |
| Subagent | subagente |
| Hintergrundprozess | processo em segundo plano |
| Unterhaltung | conversa |
| Zug | turno |
| Antwort | resposta |
| Rückfrage | pergunta |
| Freigabe, Freigabemodus | permissão, modo de permissão |
| Umgehen | Ignorar |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Negar / Permitir Sempre… |
| Plan | plano |
| To-dos | Pendências |
| Aufgabe | tarefa |
| Auftrag | tarefa |
| Werkzeug | ferramenta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conteúdo Externo |
| Sicherungspunkt | ponto de restauração |
| Zurückspulen | retroceder |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, janela de contexto |
| Denktiefe | profundidade de raciocínio |
| Modell / Anbieter / Endpunkt | modelo / provedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | chave de API |
| Register | registro |
| Adapter | adaptador |
| Debug-Adapter | adaptador de depuração |
| Bereit | Pronto |
| Prüfsumme | soma de verificação |
| Anmeldung, anmelden | início de sessão, iniciar sessão |
| Konto | conta |
| Abo | Assinatura |
| Repository | repositório |
| Commit, committen | commit, fazer commit |
| Secrets | segredos |
| vormerken | preparar (remover da preparação) |
| Zurücknehmen | Remover da Preparação |
| nicht versioniert | não rastreado |
| Abschnitt (Git) | bloco |
| Branch / Remote | branch / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | cópia de trabalho |
| Lauf, Läufe | execução, execuções |
| Host | host |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Depurar |
| Haltepunkt, Haltepunkte | ponto de interrupção, pontos de interrupção |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condição / contagem de acertos |
| Aufrufliste | pilha de chamadas |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | expressão monitorada, expressões monitoradas |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continuar / Contornar / Intervir / Sair |
| Anhängen / Trennen | anexar / desanexar |
| An Prozess anhängen | Anexar ao Processo |
| Neu starten / Stopp | Reiniciar / Parar |
| Variablen | variáveis |
| Angehalten | pausado |
| Debug-Konsole | console de depuração |
| Startkonfiguration | configuração de inicialização |
| Inspektor | Inspetor |
| **Tests** | |
| Tests | Testes |
| Test | teste |
| **Containers** | |
| Container | Contêineres |
| Docker-Kontext | contexto do Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Sala de Controle |
| Auftrag (Leitstand) | ordem de serviço, ordens de serviço |
| Nachweis | evidência |
| Abweichung | desvio |
| Abnahme, abnehmen | aceitação, aceitar |
| Freigeben (Spezifikation) | aprovar |
| Planer | Planejador |
| Builder | Construtor |
| Prüfer | Revisor |
| Späher | Explorador |
| Prüfstation, Gate | gate |
| Prüfbefehl | comando de verificação |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Entrada / Especificação / Construção / Revisão / Aceitação / Incorporação / Concluído |
| Risikoklasse | classe de risco |
| Pfadbereich | escopo de caminhos |
| Herkunft | procedência |
| Station | estação |
| Kriterium, Kriterien | critério, critérios |
| Prüfrunde | rodada de revisão |
| Befund | constatação |
| Hersteller, Herstellerwechsel | fabricante, troca de fabricante |
| Kontingent | cota |
| Workflow-Datei | arquivo de workflow |
| Stand | estado |
| Erstdurchlaufquote, Erstdurchlauf, Quote | taxa de primeira passagem, primeira passagem, taxa |
| Kennzahlen | métricas |
| Regelvorschlag | proposta de regra |
| wiederkehrender Auftrag | ordem de serviço recorrente |
| Abstand (wiederkehrender Auftrag) | intervalo |
| eingereiht | na fila |
| Rolle | função |
| Einwand | objeção |
| Rangliste | lista de prioridade |
| Prüfer-Regel | regra do revisor (Outro fabricante obrigatório / preferido) |
| Vorgabe, auf Vorgabe zurücksetzen | padrão, restaurar padrão |
| Mitkopieren / Einrichtung | copiar junto / configuração |
| Prüfrelevante Dateien | arquivos relevantes para a revisão (arquivos de teste, arquivos de configuração) |
| Issue | issue (a issue) |
| Merge Request | merge request (o merge request; Merge Request in titles) |
| Rechte | Permissões |
| Erlauben / Fragen / Verbieten | Permitir / Perguntar / Proibir |
| Gedächtnis | memória |
| Anweisungen | instruções |
| Ausgabestil | estilo de saída |
| Auswahl | Filtro |
| Plugin, Plugins | plug-in, plug-ins |
| Marktplatz, Marktplätze | marketplace, marketplaces |
| Frage nebenbei | pergunta à parte (title: Pergunta à Parte) |
| Header | cabeçalhos |
| Umgebung | ambiente |
| Bericht | relatório |
| Grenzen | Limites |
| Sitzung starten | Iniciar a Sessão (with the article, apart from Iniciar Sessão, which is Anmelden) |

### Portuguese (Portugal) (`pt_PT`)
- Address: formal third person with the pronoun left out, as European Portuguese macOS ("Pretende ignorar as
  alterações?", "Abra a hiperligação apenas se confiar na origem", "os seus"); menu items and buttons as infinitives
  ("Abrir", "Guardar", "Mostrar tudo")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Definições…", "Procurar…"); sentence
  case; European spelling and words as macOS uses them (ficheiro, ecrã, aplicação, hiperligação, registo)
- Git and CI words: kept as Portuguese developers say them: commit (fazer commit), branch, push, pull, fetch, rebase,
  worktree, hook, job, log, token, endpoint, sandbox, checksum; Portuguese where the word is settled: repositório,
  remoto, fundir (merge), preparar/anular preparação (stage/unstage), bloco (hunk), cópia de trabalho (working tree),
  execução (CI run), conflito; "AI" is "IA"

| German key | Portuguese (Portugal) |
|---|---|
| Ablage | Ficheiro |
| Bearbeiten | Edição |
| Darstellung | Visualização |
| Fenster | Janela |
| Hilfe | Ajuda |
| Dienste | Serviços |
| Über {} | Acerca do {} |
| Einstellungen | Definições (menu item: Definições…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar o {} / Ocultar outras aplicações / Mostrar tudo |
| {} beenden | Sair do {} |
| Widerrufen / Wiederholen | Desfazer / Refazer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Colar / Apagar |
| Alles auswählen | Selecionar tudo |
| Suchen … / Weitersuchen / Rückwärts suchen | Procurar… / Procurar seguinte / Procurar anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Aumentar/reduzir / Passar tudo para a frente |
| Vollbildmodus | Ecrã completo |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar barra de ferramentas / Ocultar/mostrar barra lateral |
| Sichern | Guardar |
| Öffnen / Schließen | Abrir / Fechar |
| Abbrechen / OK | Cancelar / OK |
| Im Finder zeigen | Mostrar no Finder |
| Papierkorb | Lixo (Mover para o Lixo) |
| Schlüsselbund | Porta-chaves |
| Zwischenablage | clipboard (as European Portuguese macOS) |
| Tab | separador |
| AI-Tab | separador de IA |
| Panel | painel |
| Seitenleiste | barra lateral |
| Vorschau | pré-visualização |
| Bereich | área |
| Aktivität | Atividade |
| Projekt / Ordner | projeto / pasta |
| Vertrauen, vertrauen | confiança, confiar (pasta de confiança; button: Confiar) |
| Platte | disco (no disco) |
| Unterschiede | diferenças |
| Zusammenführen | fundir (fusão) |
| Konflikt | conflito |
| Treffer | resultado |
| Suche | Pesquisa |
| Ersetzen | Substituir |
| Sprachserver | servidor de linguagem |
| Hinweis (Schweregrad) | Sugestão |
| Inlay-Hinweis | sugestão incorporada |
| Minimap | Minimapa |
| Sticky Scroll | Deslocamento fixo |
| Diagnosen am Zeilenende | Diagnósticos no fim da linha |
| Vim-Modus | Modo Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Padrão não encontrado: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | proposta |
| Vorhersage | Previsão |
| Ausschlussliste | lista de exclusão |
| Blame | Blame |
| Hinweise (Rotor) | Sugestões |
| Schnellkorrektur | correção rápida |
| Refactoring | refatorização |
| Quellaktion | ação de código-fonte |
| Implementierung / Typdefinition / Deklaration | implementação / definição de tipo / declaração |
| Aufrufhierarchie / Typhierarchie | Hierarquia de chamadas / Hierarquia de tipos |
| Aufrufer / Aufgerufene | Chamadores / Chamados |
| Obertypen / Untertypen | Supertipos / Subtipos |
| indexieren | indexar |
| Lesezeichen / Verlauf / Adresse | marcador / histórico / endereço |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formatação |
| Diktat | ditado |
| Agent | agente |
| Eigener Agent | agente integrado |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | thread (thread de IA, thread de CLI) |
| Sitzung | sessão |
| Subagent | subagente |
| Hintergrundprozess | processo em segundo plano |
| Unterhaltung | conversa |
| Zug | turno |
| Antwort | resposta |
| Rückfrage | pergunta |
| Freigabe, Freigabemodus | permissão, modo de permissões |
| Umgehen | Ignorar |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Recusar / Permitir sempre… |
| Plan | plano |
| To-dos | Tarefas pendentes |
| Aufgabe | tarefa |
| Auftrag | tarefa |
| Werkzeug | ferramenta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conteúdo externo |
| Sicherungspunkt | ponto de restauro |
| Zurückspulen | retroceder |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, janela de contexto |
| Denktiefe | profundidade de raciocínio |
| Modell / Anbieter / Endpunkt | modelo / fornecedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | chave da API |
| Register | registo |
| Adapter | adaptador |
| Debug-Adapter | adaptador de depuração |
| Bereit | Pronto |
| Prüfsumme | checksum |
| Anmeldung, anmelden | início de sessão, iniciar sessão |
| Konto | conta |
| Abo | Subscrição |
| Repository | repositório |
| Commit, committen | commit, fazer commit |
| Secrets | segredos |
| vormerken | preparar (anular preparação) |
| Zurücknehmen | Anular preparação |
| nicht versioniert | não seguido |
| Abschnitt (Git) | bloco |
| Branch / Remote | branch / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | cópia de trabalho |
| Lauf, Läufe | execução, execuções |
| Host | anfitrião |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Depurar |
| Haltepunkt, Haltepunkte | ponto de interrupção, pontos de interrupção |
| Logpunkt | logpoint |
| Bedingung / Trefferzahl | condição / contagem de acertos |
| Aufrufliste | pilha de chamadas |
| Frame | frame |
| Überwachter Ausdruck, Überwachte Ausdrücke | expressão observada, expressões observadas |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continuar / Passar por cima / Entrar / Sair |
| Anhängen / Trennen | anexar / desanexar |
| An Prozess anhängen | Anexar ao processo |
| Neu starten / Stopp | Reiniciar / Parar |
| Variablen | variáveis |
| Angehalten | em pausa |
| Debug-Konsole | consola de depuração |
| Startkonfiguration | configuração de arranque |
| Inspektor | Inspetor |
| **Tests** | |
| Tests | Testes |
| Test | teste |
| **Containers** | |
| Container | Contentores |
| Docker-Kontext | contexto do Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Sala de controlo |
| Auftrag (Leitstand) | ordem de trabalho, ordens de trabalho |
| Nachweis | evidência |
| Abweichung | desvio |
| Abnahme, abnehmen | aceitação, aceitar |
| Freigeben (Spezifikation) | aprovar |
| Planer | Planeador |
| Builder | Construtor |
| Prüfer | Revisor |
| Späher | Explorador |
| Prüfstation, Gate | gate |
| Prüfbefehl | comando de verificação |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Entrada / Especificação / Construção / Revisão / Aceitação / Incorporação / Concluído |
| Risikoklasse | classe de risco |
| Pfadbereich | âmbito de caminhos |
| Herkunft | proveniência |
| Station | estação |
| Kriterium, Kriterien | critério, critérios |
| Prüfrunde | ronda de revisão |
| Befund | constatação |
| Hersteller, Herstellerwechsel | fabricante, mudança de fabricante |
| Kontingent | quota |
| Workflow-Datei | ficheiro de workflow |
| Stand | estado |
| Erstdurchlaufquote, Erstdurchlauf, Quote | taxa de primeira passagem, primeira passagem, taxa |
| Kennzahlen | métricas |
| Regelvorschlag | proposta de regra |
| wiederkehrender Auftrag | ordem de trabalho recorrente |
| Abstand (wiederkehrender Auftrag) | intervalo |
| eingereiht | em fila |
| Rolle | função |
| Einwand | objeção |
| Rangliste | lista de prioridade |
| Prüfer-Regel | regra do revisor (Outro fabricante obrigatório / preferido) |
| Vorgabe, auf Vorgabe zurücksetzen | predefinição, repor predefinição |
| Mitkopieren / Einrichtung | copiar também / configuração |
| Prüfrelevante Dateien | ficheiros relevantes para a revisão (ficheiros de teste, ficheiros de configuração) |
| Issue | issue (a issue) |
| Merge Request | merge request (o merge request) |
| Rechte | Permissões |
| Erlauben / Fragen / Verbieten | Permitir / Perguntar / Proibir |
| Gedächtnis | memória |
| Anweisungen | instruções |
| Ausgabestil | estilo de saída |
| Auswahl | Filtro |
| Plugin, Plugins | plug-in, plug-ins |
| Marktplatz, Marktplätze | marketplace, marketplaces |
| Frage nebenbei | pergunta à parte |
| Header | cabeçalhos |
| Umgebung | ambiente |
| Bericht | relatório |
| Grenzen | Limites |
| Sitzung starten | Iniciar a sessão (with the article, apart from Iniciar sessão, which is Anmelden) |

### Romanian (`ro`)
- Address: formal "dumneavoastră" (written "dvs."), as macOS Romanian; menu items, buttons and instructions in the
  second person plural imperative ("Salvați", "Anulați", "Introduceți…"), never "tu"
- Typography: „…” (U+201E, U+201D); ș and ț with comma below (U+0219, U+021B), never with cedilla; menu items end in "…"
  without a space ("Configurări…", "Salvați ca…"); sentence case
- Git and CI words: kept as Romanian developers say them, with Romanian endings: commit (commitul, a face commit),
  remote, push, pull, fetch, rebase, worktree, hook, sandbox, token (tokenuri), job (joburi), endpoint; Romanian where
  the word is settled: depozit (repository), ramură (branch), comasare (merge, as macOS says "Comasați"), a pregăti
  pentru commit (stage), fragment (hunk), copie de lucru (working tree), rulare (CI run), jurnal (log); "AI" is "IA", as
  macOS writes it

| German key | Romanian |
|---|---|
| Ablage | Fișier |
| Bearbeiten | Editare |
| Darstellung | Vizualizare |
| Fenster | Fereastră |
| Hilfe | Ajutor |
| Dienste | Servicii |
| Über {} | Despre {} |
| Einstellungen | Configurări (menu item: Configurări…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ascundeți {} / Ascundeți restul / Afișați tot |
| {} beenden | Închideți aplicația {} |
| Widerrufen / Wiederholen | Infirmați / Refaceți |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Tăiați / Copiați / Lipiți / Ștergeți |
| Alles auswählen | Selectați tot |
| Suchen … / Weitersuchen / Rückwärts suchen | Găsiți… / Găsiți următorul / Găsiți anteriorul |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizați / Zoom / Aduceți tot în față |
| Vollbildmodus | Afișați pe tot ecranul |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ascundeți bara de instrumente / Afișați/ascundeți bara laterală |
| Sichern | Salvați |
| Öffnen / Schließen | Deschideți / Închideți |
| Abbrechen / OK | Anulați / OK |
| Im Finder zeigen | Afișați în Finder |
| Papierkorb | Coș |
| Schlüsselbund | portchei |
| Zwischenablage | clipboard |
| Tab | filă |
| AI-Tab | filă IA |
| Panel | panou |
| Seitenleiste | bară laterală |
| Vorschau | previzualizare |
| Bereich | zonă (as "Bereich" of a fold, which German names alike) |
| Aktivität | Activitate |
| Projekt / Ordner | proiect / dosar |
| Vertrauen, vertrauen | încredere, a avea încredere în |
| Platte | disc (pe disc) |
| Unterschiede | diferențe |
| Zusammenführen | comasare, a comasa |
| Konflikt | conflict |
| Treffer | rezultat |
| Suche | Căutare |
| Ersetzen | Înlocuiți |
| Sprachserver | server de limbaj |
| Hinweis (Schweregrad) | Sugestie |
| Inlay-Hinweis | sugestie inline |
| Minimap | Minihartă |
| Sticky Scroll | Derulare fixată |
| Diagnosen am Zeilenende | Diagnostice la sfârșitul rândului |
| Vim-Modus | Mod Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Modelul nu a fost găsit: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | propunere |
| Vorhersage | Predicție |
| Ausschlussliste | listă de excludere |
| Blame | Blame |
| Hinweise (Rotor) | Sugestii |
| Schnellkorrektur | remediere rapidă |
| Refactoring | refactorizare |
| Quellaktion | acțiune pentru sursă |
| Implementierung / Typdefinition / Deklaration | implementare / definiția tipului / declarație |
| Aufrufhierarchie / Typhierarchie | Ierarhia apelurilor / Ierarhia tipurilor |
| Aufrufer / Aufgerufene | Apelanți / Apelate |
| Obertypen / Untertypen | Supertipuri / Subtipuri |
| indexieren | indexare |
| Lesezeichen / Verlauf / Adresse | marcaj / istoric / adresă |
| Bytes | octeți |
| Exportieren | Exportați |
| Format | Format |
| Diktat | dictare |
| Agent | agent |
| Eigener Agent | agent propriu |
| Eigene Agenten | Agenți personalizați |
| Thread (AI-Thread, CLI-Thread) | conversație (conversație IA, conversație CLI) |
| Sitzung | sesiune |
| Subagent | subagent |
| Hintergrundprozess | proces în fundal |
| Unterhaltung | discuție |
| Zug | rundă |
| Antwort | răspuns |
| Rückfrage | întrebare |
| Freigabe, Freigabemodus | permisiune, mod de permisiuni |
| Umgehen | Ocolire |
| Erlauben / Ablehnen / Immer erlauben … | Permiteți / Refuzați / Permiteți întotdeauna… |
| Plan | plan |
| To-dos | De făcut |
| Aufgabe | sarcină |
| Auftrag | însărcinare |
| Werkzeug | instrument |
| Skills | Skilluri |
| Befehl | comandă |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conținut extern |
| Sicherungspunkt | punct de control |
| Zurückspulen | derulare înapoi, a derula înapoi |
| Verdichten | compactare, a compacta |
| Kontext, Kontextfenster | context, fereastră de context |
| Denktiefe | profunzimea gândirii |
| Modell / Anbieter / Endpunkt | model / furnizor / endpoint |
| Token, Tokens | token, tokenuri |
| API-Schlüssel | cheie API |
| Register | registru |
| Adapter | adaptor |
| Debug-Adapter | adaptor de depanare |
| Bereit | Gata |
| Prüfsumme | sumă de control |
| Anmeldung, anmelden | autentificare, a se autentifica |
| Konto | cont |
| Abo | Abonament |
| Repository | depozit |
| Commit, committen | commit, a face commit |
| Secrets | secrete |
| vormerken | a pregăti pentru commit |
| Zurücknehmen | Anulați pregătirea |
| nicht versioniert | neversionat |
| Abschnitt (Git) | fragment |
| Branch / Remote | ramură / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | preluați |
| Arbeitskopie | copie de lucru |
| Lauf, Läufe | rulare, rulări |
| Host | gazdă |
| Job | job, joburi |
| Log | jurnal |
| **Debugger** | |
| Debuggen | Depanare |
| Haltepunkt, Haltepunkte | punct de întrerupere, puncte de întrerupere |
| Logpunkt | punct de jurnal |
| Bedingung / Trefferzahl | condiție / număr de atingeri |
| Aufrufliste | stivă de apeluri |
| Frame | cadru |
| Überwachter Ausdruck, Überwachte Ausdrücke | expresie urmărită, expresii urmărite |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Continuați / Treceți peste / Intrați / Ieșiți |
| Anhängen / Trennen | atașare / detașare |
| An Prozess anhängen | Atașați la proces |
| Neu starten / Stopp | Reporniți / Opriți |
| Variablen | variabile |
| Angehalten | întrerupt |
| Debug-Konsole | consolă de depanare |
| Startkonfiguration | configurație de pornire |
| Inspektor | Inspector |
| **Tests** | |
| Tests | Teste |
| Test | test |
| **Containers** | |
| Container | Containere |
| Docker-Kontext | context Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Camera de control |
| Auftrag (Leitstand) | ordin de lucru |
| Nachweis | dovadă |
| Abweichung | abatere |
| Abnahme, abnehmen | acceptare, acceptați |
| Freigeben (Spezifikation) | Aprobați (aprobare) |
| Planer | Planificator |
| Builder | Constructor |
| Prüfer | Evaluator |
| Späher | Cercetaș |
| Prüfstation, Gate | control |
| Prüfbefehl | comandă de control |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Primire / Specificație / Construire / Evaluare / Acceptare / Preluare / Finalizat |
| Risikoklasse | clasă de risc |
| Pfadbereich | domeniu de căi |
| Herkunft | proveniență |
| Station | stație |
| Kriterium, Kriterien | criteriu, criterii |
| Prüfrunde | ciclu de evaluare |
| Befund | constatare |
| Hersteller, Herstellerwechsel | producător, schimbare de producător |
| Kontingent | cotă |
| Workflow-Datei | fișier workflow |
| Stand | stare |
| Erstdurchlaufquote, Erstdurchlauf, Quote | rata primei treceri, prima trecere, rată |
| Kennzahlen | indicatori |
| Regelvorschlag | propunere de regulă |
| wiederkehrender Auftrag | ordin de lucru recurent |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | în coadă |
| Rolle | rol |
| Einwand | obiecție |
| Rangliste | clasament |
| Prüfer-Regel | regula evaluatorului (Alt producător obligatoriu / preferat) |
| Vorgabe, auf Vorgabe zurücksetzen | implicit, reveniți la valoarea implicită |
| Mitkopieren / Einrichtung | copiate în plus / configurare |
| Prüfrelevante Dateien | fișiere relevante pentru evaluare (fișiere de test, fișiere de configurație) |
| Issue | issue (issue-ul, issue-uri) |
| Merge Request | merge request (merge requestul, merge requesturi) |
| Rechte | Permisiuni |
| Erlauben / Fragen / Verbieten | Permiteți / Întrebați / Refuzați (Verbieten as Ablehnen) |
| Gedächtnis | memorie |
| Anweisungen | instrucțiuni |
| Ausgabestil | stil de ieșire |
| Auswahl | Filtru |
| Plugin, Plugins | plugin, pluginuri |
| Marktplatz, Marktplätze | piață, piețe |
| Frage nebenbei | întrebare separată |
| Header | antete |
| Umgebung | mediu |
| Bericht | raport |
| Grenzen | Limite |

### Russian (`ru`)
- Address: formal "Вы", capitalized as macOS Russian writes it ("Вы", "Ваш"); menu items and buttons in the infinitive
  ("Сохранить", "Скрыть боковое меню"); instructions in the "Вы" imperative ("Выберите…", "Откройте…")
- Typography: «…» («„…“» inside); "е" instead of "ё", as macOS writes it ("учетная запись", "удаленный"); menu items end
  in "…" without a space ("Настройки…", "Сохранить как…"); dash with spaces " — "; the OK button is "ОК" in Cyrillic
  letters; sentence case
- Git and CI words: Git's operation names stay in Latin as Russian developers write them: push, pull, fetch, rebase,
  worktree; Russian otherwise: репозиторий, коммит (закоммитить), ветка (branch), удаленный репозиторий (remote),
  индексировать (stage), фрагмент (hunk), слияние (merge), рабочая копия, хук, песочница, токен, запуск (CI run),
  задание (job), журнал (log); "AI" is "ИИ", as macOS writes it

| German key | Russian |
|---|---|
| Ablage | Файл |
| Bearbeiten | Правка |
| Darstellung | Вид |
| Fenster | Окно |
| Hilfe | Справка |
| Dienste | Службы |
| Über {} | О приложении {} |
| Einstellungen | Настройки (menu item: Настройки…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Скрыть {} / Скрыть остальные / Показать все |
| {} beenden | Завершить {} |
| Widerrufen / Wiederholen | Отменить / Повторить |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Вырезать / Скопировать / Вставить / Удалить |
| Alles auswählen | Выбрать все |
| Suchen … / Weitersuchen / Rückwärts suchen | Найти… / Найти далее / Найти ранее |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Убрать в Dock / Изменить масштаб / Все окна — на передний план |
| Vollbildmodus | Перейти в полноэкранный режим |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Скрыть панель инструментов / Показать/скрыть боковое меню |
| Sichern | Сохранить |
| Öffnen / Schließen | Открыть / Закрыть |
| Abbrechen / OK | Отменить / ОК |
| Im Finder zeigen | Показать в Finder |
| Papierkorb | Корзина |
| Schlüsselbund | связка ключей |
| Zwischenablage | буфер обмена |
| Tab | вкладка |
| AI-Tab | вкладка ИИ |
| Panel | панель |
| Seitenleiste | боковое меню |
| Vorschau | просмотр |
| Bereich | область |
| Aktivität | Активность |
| Projekt / Ordner | проект / папка |
| Vertrauen, vertrauen | доверие, доверять |
| Platte | диск (на диске) |
| Unterschiede | различия |
| Zusammenführen | слияние, слить |
| Konflikt | конфликт |
| Treffer | результат |
| Suche | Поиск |
| Ersetzen | Заменить |
| Sprachserver | языковой сервер |
| Hinweis (Schweregrad) | Подсказка |
| Inlay-Hinweis | встроенная подсказка |
| Minimap | Миникарта |
| Sticky Scroll | Закрепленная прокрутка |
| Diagnosen am Zeilenende | Диагностика в конце строки |
| Vim-Modus | Режим Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names; Russian Vim's names run to three words) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Не найдены совпадения с образцом {} … (as this language's Vim says them) |
| Vorschlag | предложение |
| Vorhersage | Предсказание |
| Ausschlussliste | список исключений |
| Blame | Blame |
| Hinweise (Rotor) | Подсказки |
| Schnellkorrektur | быстрое исправление |
| Refactoring | рефакторинг |
| Quellaktion | действие с исходным кодом |
| Implementierung / Typdefinition / Deklaration | реализация / определение типа / объявление |
| Aufrufhierarchie / Typhierarchie | Иерархия вызовов / Иерархия типов |
| Aufrufer / Aufgerufene | Вызывающие / Вызываемые |
| Obertypen / Untertypen | Супертипы / Подтипы |
| indexieren | индексация |
| Lesezeichen / Verlauf / Adresse | закладка / история / адрес |
| Bytes | байт |
| Exportieren | Экспортировать |
| Format | Формат |
| Diktat | диктовка |
| Agent | агент |
| Eigener Agent | собственный агент |
| Eigene Agenten | Пользовательские агенты |
| Thread (AI-Thread, CLI-Thread) | чат (ИИ-чат, CLI-чат) |
| Sitzung | сеанс |
| Subagent | субагент |
| Hintergrundprozess | фоновый процесс |
| Unterhaltung | диалог |
| Zug | запрос |
| Antwort | ответ |
| Rückfrage | вопрос |
| Freigabe, Freigabemodus | разрешение, режим разрешений |
| Umgehen | Обход разрешений |
| Erlauben / Ablehnen / Immer erlauben … | Разрешить / Запретить / Разрешать всегда… |
| Plan | план |
| To-dos | Список дел |
| Aufgabe | задача |
| Auftrag | поручение |
| Werkzeug | инструмент |
| Skills | Навыки |
| Befehl | команда |
| Hook | хук |
| Sandbox | песочница |
| Fremder Inhalt | Внешнее содержимое |
| Sicherungspunkt | контрольная точка |
| Zurückspulen | откат, откатить |
| Verdichten | сжатие, сжать |
| Kontext, Kontextfenster | контекст, контекстное окно |
| Denktiefe | глубина рассуждений |
| Modell / Anbieter / Endpunkt | модель / провайдер / конечная точка |
| Token, Tokens | токен, токены |
| API-Schlüssel | ключ API |
| Register | реестр |
| Adapter | адаптер |
| Debug-Adapter | адаптер отладки |
| Bereit | Готов |
| Prüfsumme | контрольная сумма |
| Anmeldung, anmelden | вход, войти |
| Konto | учетная запись |
| Abo | Подписка |
| Repository | репозиторий |
| Commit, committen | коммит, закоммитить |
| Secrets | секреты |
| vormerken | индексировать |
| Zurücknehmen | Убрать из индекса |
| nicht versioniert | не отслеживается |
| Abschnitt (Git) | фрагмент |
| Branch / Remote | ветка / удаленный репозиторий |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | перенести |
| Arbeitskopie | рабочая копия |
| Lauf, Läufe | запуск, запуски |
| Host | хост |
| Job | задание |
| Log | журнал |
| **Debugger** | |
| Debuggen | Отладка |
| Haltepunkt, Haltepunkte | точка останова, точки останова |
| Logpunkt | точка журнала |
| Bedingung / Trefferzahl | условие / число срабатываний |
| Aufrufliste | стек вызовов |
| Frame | фрейм |
| Überwachter Ausdruck, Überwachte Ausdrücke | отслеживаемое выражение, отслеживаемые выражения |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Продолжить / Шаг с обходом / Шаг с заходом / Шаг с выходом |
| Anhängen / Trennen | присоединиться / отсоединиться |
| An Prozess anhängen | Присоединиться к процессу |
| Neu starten / Stopp | Перезапустить / Остановить |
| Variablen | переменные |
| Angehalten | приостановлено |
| Debug-Konsole | консоль отладки |
| Startkonfiguration | конфигурация запуска |
| Inspektor | Инспектор |
| **Tests** | |
| Tests | Тесты |
| Test | тест |
| **Containers** | |
| Container | Контейнеры |
| Docker-Kontext | контекст Docker |
| **Leitstand (phase 13)** | |
| Leitstand | диспетчерская |
| Auftrag (Leitstand) | заказ |
| Nachweis | доказательства |
| Abweichung | отклонение |
| Abnahme, abnehmen | приемка, принять |
| Freigeben (Spezifikation) | утвердить (утверждение) |
| Planer | Планировщик |
| Builder | Исполнитель |
| Prüfer | Рецензент |
| Späher | Разведчик |
| Prüfstation, Gate | шлюз |
| Prüfbefehl | проверочная команда |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Входящие / Спецификация / Реализация / Проверка / Приемка / Перенос / Готово |
| Risikoklasse | класс риска |
| Pfadbereich | область путей |
| Herkunft | происхождение |
| Station | этап |
| Kriterium, Kriterien | критерий, критерии |
| Prüfrunde | раунд проверки |
| Befund | замечание |
| Hersteller, Herstellerwechsel | производитель, смена производителя |
| Kontingent | квота |
| Workflow-Datei | файл рабочего процесса |
| Stand | состояние |
| Erstdurchlaufquote, Erstdurchlauf, Quote | доля успеха с первого раза, с первого раза, доля |
| Kennzahlen | показатели |
| Regelvorschlag | предложение правила |
| wiederkehrender Auftrag | повторяющийся заказ |
| Abstand (wiederkehrender Auftrag) | интервал |
| eingereiht | в очереди |
| Rolle | роль |
| Einwand | возражение (Возражение исполнителя) |
| Rangliste | список приоритетов |
| Prüfer-Regel | правило рецензента (обязателен / предпочтителен) |
| Vorgabe, auf Vorgabe zurücksetzen | по умолчанию, вернуть значение по умолчанию |
| Mitkopieren / Einrichtung | копировать также / подготовка |
| Prüfrelevante Dateien | файлы, важные для проверок |
| Issue | issue (masculine, in Latin letters) |
| Merge Request | merge request |
| Rechte | Разрешения |
| Erlauben / Fragen / Verbieten | Разрешить / Спрашивать / Запретить (Verbieten as Ablehnen) |
| Gedächtnis | память |
| Anweisungen | инструкции |
| Ausgabestil | стиль вывода |
| Auswahl | Фильтр |
| Plugin, Plugins | плагин, плагины |
| Marktplatz, Marktplätze | маркетплейс, маркетплейсы |
| Frage nebenbei | попутный вопрос |
| Header | заголовки |
| Umgebung | окружение |
| Bericht | отчет |
| Grenzen | Лимиты |
| Verbindung | Подключение (a network connection; splat's Связь means a diagram connector) |

### Slovak (`sk`)
- Address: formal "vy" (vykanie), lowercase; menu items and buttons in the infinitive, as macOS Slovak has them
  ("Uložiť", "Zobraziť postranný panel"); instructions in the plural imperative ("Vyberte…", "Otvorte…"); help tags in
  the third person ("Zobrazí alebo skryje…")
- Typography: „…“ (U+201E, U+201C); menu items end in "…" without a space ("Nastavenia…", "Uložiť ako…"); the OK button
  is "Potvrdiť", as macOS Slovak writes it; sentence case
- Git and CI words: kept as Slovak developers say them and declined: commit (commitnúť, commitu), push, pull, fetch,
  rebase, worktree, hook, sandbox, token (tokeny), job (joby), log; Slovak where the word is settled: repozitár, vetva
  (branch), vzdialený repozitár (remote), zlúčiť/zlúčenie (merge), pripraviť na commit (stage), úsek (hunk), pracovná
  kópia, spustenie (CI run); "AI" stays "AI"

| German key | Slovak |
|---|---|
| Ablage | Súbor |
| Bearbeiten | Upraviť |
| Darstellung | Zobraziť |
| Fenster | Okno |
| Hilfe | Pomocník |
| Dienste | Služby |
| Über {} | O aplikácii {} |
| Einstellungen | Nastavenia (menu item: Nastavenia…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skryť {} / Skryť ostatné / Zobraziť všetky |
| {} beenden | Ukončiť {} |
| Widerrufen / Wiederholen | Odvolať / Obnoviť |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Vystrihnúť / Kopírovať / Vložiť / Vymazať |
| Alles auswählen | Označiť všetko |
| Suchen … / Weitersuchen / Rückwärts suchen | Nájsť… / Nájsť ďalšie / Nájsť predošlé |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizovať / Zmeniť veľkosť / Presunúť všetky do popredia |
| Vollbildmodus | Zobraziť na celej obrazovke |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skryť panel s nástrojmi / Zobraziť/skryť postranný panel |
| Sichern | Uložiť |
| Öffnen / Schließen | Otvoriť / Zatvoriť |
| Abbrechen / OK | Zrušiť / Potvrdiť |
| Im Finder zeigen | Zobraziť vo Finderi |
| Papierkorb | Kôš |
| Schlüsselbund | kľúčenka |
| Zwischenablage | schránka |
| Tab | tab |
| AI-Tab | AI tab |
| Panel | panel |
| Seitenleiste | postranný panel |
| Vorschau | náhľad |
| Bereich | oblasť |
| Aktivität | Aktivita |
| Projekt / Ordner | projekt / priečinok |
| Vertrauen, vertrauen | dôvera, dôverovať |
| Platte | disk (na disku) |
| Unterschiede | rozdiely |
| Zusammenführen | zlúčenie, zlúčiť |
| Konflikt | konflikt |
| Treffer | výsledok |
| Suche | Vyhľadávanie |
| Ersetzen | Nahradiť |
| Sprachserver | jazykový server |
| Hinweis (Schweregrad) | Nápoveda |
| Inlay-Hinweis | vložená nápoveda |
| Minimap | Minimapa |
| Sticky Scroll | Pevné posúvanie |
| Diagnosen am Zeilenende | Diagnostika na konci riadka |
| Vim-Modus | Režim Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMÁLNY / VLOŽIŤ / NAHRADIŤ / VIZUÁLNY / VIZUÁLNY RIADOK / VIZUÁLNY BLOK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Vzor nenájdený: {} … (as this language's Vim says them) |
| Vorschlag | návrh |
| Vorhersage | Predpoveď |
| Ausschlussliste | zoznam vylúčení |
| Blame | Blame |
| Hinweise (Rotor) | Nápovedy |
| Schnellkorrektur | rýchla oprava |
| Refactoring | refaktorovanie |
| Quellaktion | akcia zdrojového kódu |
| Implementierung / Typdefinition / Deklaration | implementácia / definícia typu / deklarácia |
| Aufrufhierarchie / Typhierarchie | Hierarchia volaní / Hierarchia typov |
| Aufrufer / Aufgerufene | Volajúce / Volané |
| Obertypen / Untertypen | Nadtypy / Podtypy |
| indexieren | indexovanie |
| Lesezeichen / Verlauf / Adresse | záložka / história / adresa |
| Bytes | B |
| Exportieren | Exportovať |
| Format | Formát |
| Diktat | diktovanie |
| Agent | agent |
| Eigener Agent | vlastný agent |
| Eigene Agenten | Vlastní agenti |
| Thread (AI-Thread, CLI-Thread) | vlákno (AI vlákno, CLI vlákno) |
| Sitzung | relácia |
| Subagent | subagent |
| Hintergrundprozess | proces na pozadí |
| Unterhaltung | konverzácia |
| Zug | kolo |
| Antwort | odpoveď |
| Rückfrage | otázka |
| Freigabe, Freigabemodus | povolenie, režim povolení |
| Umgehen | Obísť |
| Erlauben / Ablehnen / Immer erlauben … | Povoliť / Zamietnuť / Vždy povoliť… |
| Plan | plán |
| To-dos | Zoznam úloh |
| Aufgabe | úloha |
| Auftrag | zadanie |
| Werkzeug | nástroj |
| Skills | Skills |
| Befehl | príkaz |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Externý obsah |
| Sicherungspunkt | kontrolný bod |
| Zurückspulen | pretočiť späť |
| Verdichten | zhustiť, zhustenie |
| Kontext, Kontextfenster | kontext, kontextové okno |
| Denktiefe | hĺbka uvažovania |
| Modell / Anbieter / Endpunkt | model / poskytovateľ / koncový bod |
| Token, Tokens | token, tokeny |
| API-Schlüssel | kľúč API |
| Register | register |
| Adapter | adaptér |
| Debug-Adapter | ladiaci adaptér |
| Bereit | Pripravené |
| Prüfsumme | kontrolný súčet |
| Anmeldung, anmelden | prihlásenie, prihlásiť sa |
| Konto | účet |
| Abo | Predplatné |
| Repository | repozitár |
| Commit, committen | commit, commitnúť |
| Secrets | tajné údaje |
| vormerken | pripraviť na commit |
| Zurücknehmen | Odobrať z prípravy |
| nicht versioniert | nesledované |
| Abschnitt (Git) | úsek |
| Branch / Remote | vetva / vzdialený repozitár |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | prevziať |
| Arbeitskopie | pracovná kópia |
| Lauf, Läufe | spustenie, spustenia |
| Host | hostiteľ |
| Job | job |
| Log | log |
| **Debugger** | |
| Debuggen | Ladenie |
| Haltepunkt, Haltepunkte | bod prerušenia, body prerušenia |
| Logpunkt | bod záznamu |
| Bedingung / Trefferzahl | podmienka / počet zásahov |
| Aufrufliste | zásobník volaní |
| Frame | rámec (rámec zásobníka) |
| Überwachter Ausdruck, Überwachte Ausdrücke | sledovaný výraz, sledované výrazy |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Pokračovať / Prejsť cez / Vstúpiť / Vystúpiť |
| Anhängen / Trennen | pripojiť / odpojiť |
| An Prozess anhängen | Pripojiť k procesu |
| Neu starten / Stopp | Reštartovať / Zastaviť |
| Variablen | premenné |
| Angehalten | pozastavené |
| Debug-Konsole | ladiaca konzola |
| Startkonfiguration | konfigurácia spustenia |
| Inspektor | Inšpektor |
| **Tests** | |
| Tests | Testy |
| Test | test |
| **Containers** | |
| Container | Kontajnery |
| Docker-Kontext | kontext Dockera |
| **Leitstand (phase 13)** | |
| Leitstand | velín |
| Auftrag (Leitstand) | zákazka |
| Nachweis | doklad |
| Abweichung | odchýlka |
| Abnahme, abnehmen | prijatie, prijať |
| Freigeben (Spezifikation) | schváliť (schválenie) |
| Planer | Plánovač |
| Builder | Staviteľ |
| Prüfer | Recenzent |
| Späher | Prieskumník |
| Prüfstation, Gate | brána |
| Prüfbefehl | kontrolný príkaz |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Vstup / Špecifikácia / Stavba / Revízia / Prijatie / Prevzatie / Hotovo |
| Risikoklasse | trieda rizika (nízka / stredná / vysoká) |
| Pfadbereich | rozsah ciest |
| Herkunft | pôvod |
| Station | stanica |
| Kriterium, Kriterien | kritérium, kritériá |
| Prüfrunde | cyklus revízie (not kolo, which is Zug) |
| Befund | zistenie |
| Hersteller, Herstellerwechsel | výrobca, zmena výrobcu |
| Kontingent | kvóta |
| Workflow-Datei | súbor pracovného postupu |
| Stand | stav |
| Erstdurchlaufquote, Erstdurchlauf, Quote | podiel úspechu na prvý raz, na prvý raz, podiel |
| Kennzahlen | metriky |
| Regelvorschlag | návrh pravidla |
| wiederkehrender Auftrag | opakovaná zákazka |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | vo fronte |
| Rolle | rola |
| Einwand | námietka |
| Rangliste | rebríček |
| Prüfer-Regel | pravidlo recenzenta (povinný / uprednostnený) |
| Vorgabe, auf Vorgabe zurücksetzen | predvolené, vrátiť na predvolené |
| Mitkopieren / Einrichtung | kopírovať so sebou / príprava |
| Prüfrelevante Dateien | súbory dôležité pre kontroly |
| Issue | issue (neuter, plural issues) |
| Merge Request | merge request (plural merge requesty) |
| Rechte | Povolenia |
| Erlauben / Fragen / Verbieten | Povoliť / Spýtať sa / Zamietnuť (Verbieten as Ablehnen) |
| Gedächtnis | pamäť |
| Anweisungen | pokyny |
| Ausgabestil | štýl výstupu |
| Auswahl | Filter |
| Plugin, Plugins | plugin, pluginy |
| Marktplatz, Marktplätze | trhovisko, trhoviská |
| Frage nebenbei | otázka bokom |
| Header | hlavičky |
| Umgebung | prostredie |
| Bericht | hlásenie |
| Grenzen | Limity |
| Verbindung | Pripojenie (a network connection; splat's Spojenie means a diagram connector) |

### Slovenian (`sl`)
- Address: formal "vi" (vikanje), lowercase; menu items and buttons in the singular imperative, as macOS Slovenian has
  them ("Shrani", "Prikaži stransko vrstico"); instructions in the plural ("Izberite…", "Odprite…"); questions as
  "Želite …?"
- Typography: »…« ; menu items put a no-break space (U+00A0) before "…", as macOS writes it ("Nastavitve …", "Shrani
  kot …"; splat has a plain space there); the OK button is "V redu"; sentence case
- Git and CI words: kept as Slovenian developers say them and declined: commit (commita, narediti commit), push, pull,
  fetch, rebase, worktree, hook, job; Slovenian where the word is settled: repozitorij, veja (branch), oddaljeni
  repozitorij (remote), združiti/združitev (merge), pripraviti za commit (stage), odsek (hunk), delovna kopija, zagon
  (CI run), dnevnik (log), peskovnik (sandbox), žeton (token); "AI" stays "AI" (macOS spells out "umetna inteligenca";
  the short form "UI" would read as user interface)

| German key | Slovenian |
|---|---|
| Ablage | Datoteka |
| Bearbeiten | Uredi |
| Darstellung | Prikaz |
| Fenster | Okno |
| Hilfe | Pomoč |
| Dienste | Storitve |
| Über {} | O aplikaciji {} |
| Einstellungen | Nastavitve (menu item: Nastavitve …) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skrij aplikacijo {} / Skrij druge / Prikaži vse |
| {} beenden | Zapri aplikacijo {} |
| Widerrufen / Wiederholen | Razveljavi / Uveljavi |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Izreži / Kopiraj / Prilepi / Izbriši |
| Alles auswählen | Izberi vse |
| Suchen … / Weitersuchen / Rückwärts suchen | Poišči … / Poišči naslednje / Poišči prejšnje |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimiraj / Povečaj / Postavi vse v ospredje |
| Vollbildmodus | Preklopi v celozaslonski način |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skrij orodno vrstico / Prikaži/skrij stransko vrstico |
| Sichern | Shrani |
| Öffnen / Schließen | Odpri / Zapri |
| Abbrechen / OK | Prekliči / V redu |
| Im Finder zeigen | Prikaži v aplikaciji Finder |
| Papierkorb | Koš |
| Schlüsselbund | veriga ključev |
| Zwischenablage | odložišče |
| Tab | zavihek |
| AI-Tab | zavihek AI |
| Panel | podokno |
| Seitenleiste | stranska vrstica |
| Vorschau | predogled |
| Bereich | območje |
| Aktivität | Aktivnost |
| Projekt / Ordner | projekt / mapa |
| Vertrauen, vertrauen | zaupanje, zaupati |
| Platte | disk (na disku) |
| Unterschiede | razlike |
| Zusammenführen | združitev, združiti |
| Konflikt | konflikt |
| Treffer | rezultat |
| Suche | Iskanje |
| Ersetzen | Zamenjaj |
| Sprachserver | jezikovni strežnik |
| Hinweis (Schweregrad) | Namig |
| Inlay-Hinweis | vdelan namig |
| Minimap | Mini zemljevid |
| Sticky Scroll | Pripeto drsenje |
| Diagnosen am Zeilenende | Diagnostika na koncu vrstice |
| Vim-Modus | Način Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Vzorca ni mogoče najti: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | predlog |
| Vorhersage | Napovedovanje |
| Ausschlussliste | seznam izključitev |
| Blame | Blame |
| Hinweise (Rotor) | Namigi |
| Schnellkorrektur | hitri popravek |
| Refactoring | refaktoriranje |
| Quellaktion | dejanje izvorne kode |
| Implementierung / Typdefinition / Deklaration | implementacija / definicija tipa / deklaracija |
| Aufrufhierarchie / Typhierarchie | Hierarhija klicev / Hierarhija tipov |
| Aufrufer / Aufgerufene | Klicatelji / Klicani |
| Obertypen / Untertypen | Nadtipi / Podtipi |
| indexieren | indeksiranje |
| Lesezeichen / Verlauf / Adresse | zaznamek / zgodovina / naslov |
| Bytes | bajti |
| Exportieren | Izvozi |
| Format | Oblikovanje |
| Diktat | narekovanje |
| Agent | agent |
| Eigener Agent | lastni agent |
| Eigene Agenten | Agenti po meri |
| Thread (AI-Thread, CLI-Thread) | nit (nit AI, nit CLI) |
| Sitzung | seja |
| Subagent | podagent |
| Hintergrundprozess | proces v ozadju |
| Unterhaltung | pogovor |
| Zug | krog |
| Antwort | odgovor |
| Rückfrage | vprašanje |
| Freigabe, Freigabemodus | dovoljenje, način dovoljenj |
| Umgehen | Obidi |
| Erlauben / Ablehnen / Immer erlauben … | Dovoli / Zavrni / Vedno dovoli … |
| Plan | načrt |
| To-dos | Opravki |
| Aufgabe | opravilo |
| Auftrag | naročilo |
| Werkzeug | orodje |
| Skills | Skills |
| Befehl | ukaz |
| Hook | hook |
| Sandbox | peskovnik |
| Fremder Inhalt | Zunanja vsebina |
| Sicherungspunkt | kontrolna točka |
| Zurückspulen | previti nazaj, previjanje nazaj |
| Verdichten | zgostiti, zgoščevanje |
| Kontext, Kontextfenster | kontekst, kontekstno okno |
| Denktiefe | globina razmisleka |
| Modell / Anbieter / Endpunkt | model / ponudnik / končna točka |
| Token, Tokens | žeton, žetoni |
| API-Schlüssel | ključ API |
| Register | register |
| Adapter | adapter |
| Debug-Adapter | adapter za razhroščevanje |
| Bereit | Pripravljeno |
| Prüfsumme | kontrolna vsota |
| Anmeldung, anmelden | vpis, vpisati se |
| Konto | račun |
| Abo | Naročnina |
| Repository | repozitorij |
| Commit, committen | commit, narediti commit |
| Secrets | skrivnosti |
| vormerken | pripraviti za commit |
| Zurücknehmen | Prekliči pripravo |
| nicht versioniert | nesledeno |
| Abschnitt (Git) | odsek |
| Branch / Remote | veja / oddaljeni repozitorij |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | prevzemi |
| Arbeitskopie | delovna kopija |
| Lauf, Läufe | zagon, zagoni |
| Host | gostitelj |
| Job | job |
| Log | dnevnik |
| **Debugger** | |
| Debuggen | Odpravljanje napak |
| Haltepunkt, Haltepunkte | prekinitvena točka, prekinitvene točke |
| Logpunkt | točka dnevnika |
| Bedingung / Trefferzahl | pogoj / število zadetkov |
| Aufrufliste | sklad klicev |
| Frame | okvir (okvir sklada) |
| Überwachter Ausdruck, Überwachte Ausdrücke | opazovani izraz, opazovani izrazi |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Nadaljuj / Preskoči / Vstopi / Izstopi |
| Anhängen / Trennen | pripni / odklopi |
| An Prozess anhängen | Pripni na proces |
| Neu starten / Stopp | Znova zaženi / Ustavi |
| Variablen | spremenljivke |
| Angehalten | ustavljeno |
| Debug-Konsole | konzola za odpravljanje napak |
| Startkonfiguration | konfiguracija zagona |
| Inspektor | Inšpektor |
| **Tests** | |
| Tests | Testi |
| Test | test |
| **Containers** | |
| Container | Vsebniki |
| Docker-Kontext | kontekst Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Nadzorna soba |
| Auftrag (Leitstand) | delovni nalog |
| Nachweis | dokazilo |
| Abweichung | odstopanje |
| Abnahme, abnehmen | sprejem, sprejeti |
| Freigeben (Spezifikation) | Odobri (odobritev) |
| Planer | Načrtovalec |
| Builder | Graditelj |
| Prüfer | Pregledovalec |
| Späher | Izvidnik |
| Prüfstation, Gate | kontrola |
| Prüfbefehl | kontrolni ukaz |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Vhod / Specifikacija / Gradnja / Pregled / Sprejem / Prevzem / Končano |
| Risikoklasse | razred tveganja |
| Pfadbereich | obseg poti |
| Herkunft | izvor |
| Station | postaja |
| Kriterium, Kriterien | merilo, merila |
| Prüfrunde | runda pregleda |
| Befund | ugotovitev |
| Hersteller, Herstellerwechsel | proizvajalec, menjava proizvajalca |
| Kontingent | kvota |
| Workflow-Datei | datoteka poteka dela |
| Stand | stanje |
| Erstdurchlaufquote, Erstdurchlauf, Quote | delež uspeha v prvem poskusu, prvi poskus, delež |
| Kennzahlen | kazalniki |
| Regelvorschlag | predlog pravila |
| wiederkehrender Auftrag | ponavljajoči se delovni nalog |
| Abstand (wiederkehrender Auftrag) | interval |
| eingereiht | v čakalni vrsti |
| Rolle | vloga |
| Einwand | ugovor |
| Rangliste | razvrstitev |
| Prüfer-Regel | pravilo pregledovalca (obvezen / zaželen) |
| Vorgabe, auf Vorgabe zurücksetzen | privzeto, ponastavi na privzeto |
| Mitkopieren / Einrichtung | kopiraj zraven / priprava |
| Prüfrelevante Dateien | datoteke, pomembne za preverjanje |
| Issue | issue (masculine, plural issues) |
| Merge Request | merge request (plural merge requesti) |
| Rechte | Dovoljenja |
| Erlauben / Fragen / Verbieten | Dovoli / Vprašaj / Zavrni (Verbieten as Ablehnen) |
| Gedächtnis | spomin |
| Anweisungen | navodila |
| Ausgabestil | slog izhoda |
| Auswahl | Filter |
| Plugin, Plugins | vtičnik, vtičniki |
| Marktplatz, Marktplätze | tržnica, tržnice |
| Frage nebenbei | vprašanje ob strani |
| Header | glave |
| Umgebung | okolje |
| Bericht | poročilo |
| Grenzen | Omejitve |

### Swedish (`sv`)
- Address: "du", as macOS Swedish ("Vill du …?", "Välj …"); menu items and buttons in the imperative ("Spara", "Göm
  verktygsfältet"); "Göm", not "Dölj", for hide, as macOS says it
- Typography: ”…” (U+201D on both sides) and ’; menu items end in "…" without a space ("Inställningar…", "Spara som…");
  sentence case
- Git and CI words: kept as Swedish developers say them, with Swedish endings: commit (committa, commiten), stagea, repo
  (repot), remote (remoten), push, pull, fetch, rebase, worktree, hook, token; Swedish where the word is settled: gren
  (branch), slå ihop/sammanslagning (merge, as macOS says "Slå ihop"), ändringsblock (hunk), arbetskopia (working tree),
  körning (CI run), jobb (job), logg (log), sandlåda (sandbox); "AI" stays "AI"

| German key | Swedish |
|---|---|
| Ablage | Arkiv |
| Bearbeiten | Redigera |
| Darstellung | Innehåll |
| Fenster | Fönster |
| Hilfe | Hjälp |
| Dienste | Tjänster |
| Über {} | Om {} |
| Einstellungen | Inställningar (menu item: Inställningar…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Göm {} / Göm övriga / Visa alla |
| {} beenden | Avsluta {} |
| Widerrufen / Wiederholen | Ångra / Gör om |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klipp ut / Kopiera / Klistra in / Radera |
| Alles auswählen | Markera allt |
| Suchen … / Weitersuchen / Rückwärts suchen | Sök… / Sök nästa / Sök föregående |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimera / Zooma / Lägg alla överst |
| Vollbildmodus | Helskärmsläge |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Göm verktygsfältet / Visa/göm sidofältet |
| Sichern | Spara |
| Öffnen / Schließen | Öppna / Stäng |
| Abbrechen / OK | Avbryt / OK |
| Im Finder zeigen | Visa i Finder |
| Papierkorb | papperskorg (papperskorgen) |
| Schlüsselbund | nyckelring |
| Zwischenablage | urklipp |
| Tab | flik |
| AI-Tab | AI-flik |
| Panel | panel |
| Seitenleiste | sidofält |
| Vorschau | förhandsvisning |
| Bereich | ruta |
| Aktivität | Aktivitet |
| Projekt / Ordner | projekt / mapp |
| Vertrauen, vertrauen | förtroende, lita på |
| Platte | disk (på disken) |
| Unterschiede | skillnader |
| Zusammenführen | slå ihop, sammanslagning |
| Konflikt | konflikt |
| Treffer | träff |
| Suche | Sök |
| Ersetzen | Ersätt |
| Sprachserver | språkserver |
| Hinweis (Schweregrad) | Tips |
| Inlay-Hinweis | infällt tips |
| Minimap | Minikarta |
| Sticky Scroll | Fäst rullning |
| Diagnosen am Zeilenende | Diagnostik vid radslut |
| Vim-Modus | Vim-läge |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INFOGA / ERSÄTT / VISUELL / VISUELL RAD / VISUELLT BLOCK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Mönster hittades inte: {} … (as this language's Vim says them) |
| Vorschlag | förslag |
| Vorhersage | Förutsägelse |
| Ausschlussliste | undantagslista |
| Blame | Blame |
| Hinweise (Rotor) | Tips |
| Schnellkorrektur | snabbkorrigering |
| Refactoring | refaktorering |
| Quellaktion | källåtgärd |
| Implementierung / Typdefinition / Deklaration | implementering / typdefinition / deklaration |
| Aufrufhierarchie / Typhierarchie | Anropshierarki / Typhierarki |
| Aufrufer / Aufgerufene | Anropare / Anropade |
| Obertypen / Untertypen | Supertyper / Undertyper |
| indexieren | indexera |
| Lesezeichen / Verlauf / Adresse | bokmärke / historik / adress |
| Bytes | byte |
| Exportieren | Exportera |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | egen agent |
| Eigene Agenten | Anpassade agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (AI-tråd, CLI-tråd) |
| Sitzung | session |
| Subagent | underagent |
| Hintergrundprozess | bakgrundsprocess |
| Unterhaltung | konversation |
| Zug | omgång |
| Antwort | svar |
| Rückfrage | fråga |
| Freigabe, Freigabemodus | behörighet, behörighetsläge |
| Umgehen | Kringgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillåt / Neka / Tillåt alltid… |
| Plan | plan |
| To-dos | Att göra |
| Aufgabe | uppgift |
| Auftrag | uppdrag |
| Werkzeug | verktyg |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandlåda |
| Fremder Inhalt | Externt innehåll |
| Sicherungspunkt | kontrollpunkt |
| Zurückspulen | spola tillbaka |
| Verdichten | komprimera, komprimering |
| Kontext, Kontextfenster | kontext, kontextfönster |
| Denktiefe | tankedjup |
| Modell / Anbieter / Endpunkt | modell / leverantör / slutpunkt |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-nyckel |
| Register | register |
| Adapter | adapter |
| Debug-Adapter | felsökningsadapter |
| Bereit | Redo |
| Prüfsumme | kontrollsumma |
| Anmeldung, anmelden | inloggning, logga in |
| Konto | konto |
| Abo | Abonnemang |
| Repository | repo |
| Commit, committen | commit, committa |
| Secrets | hemligheter |
| vormerken | stagea |
| Zurücknehmen | Unstagea |
| nicht versioniert | ospårad |
| Abschnitt (Git) | ändringsblock |
| Branch / Remote | gren / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | ta över |
| Arbeitskopie | arbetskopia |
| Lauf, Läufe | körning, körningar |
| Host | värd |
| Job | jobb |
| Log | logg |
| **Debugger** | |
| Debuggen | Felsökning |
| Haltepunkt, Haltepunkte | brytpunkt, brytpunkter |
| Logpunkt | loggpunkt |
| Bedingung / Trefferzahl | villkor / antal träffar |
| Aufrufliste | anropsstack |
| Frame | ram (stackram) |
| Überwachter Ausdruck, Überwachte Ausdrücke | bevakat uttryck, bevakade uttryck |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Fortsätt / Kliv över / Kliv in / Kliv ut |
| Anhängen / Trennen | anslut / koppla från |
| An Prozess anhängen | Anslut till process |
| Neu starten / Stopp | Starta om / Stoppa |
| Variablen | variabler |
| Angehalten | pausad |
| Debug-Konsole | felsökningskonsol |
| Startkonfiguration | startkonfiguration |
| Inspektor | Inspektör |
| **Tests** | |
| Tests | Tester |
| Test | test |
| **Containers** | |
| Container | Containrar |
| Docker-Kontext | Docker-kontext |
| **Leitstand (phase 13)** | |
| Leitstand | Kontrollrum (Kontrollrummet) |
| Auftrag (Leitstand) | arbetsorder, arbetsordrar |
| Nachweis | bevis |
| Abweichung | avvikelse, avvikelser |
| Abnahme, abnehmen | acceptans, acceptera |
| Freigeben (Spezifikation) | godkänna (button: Godkänn; godkännande) |
| Planer | Planerare |
| Builder | Byggare |
| Prüfer | Granskare |
| Späher | Spanare |
| Prüfstation, Gate | grind, grindar |
| Prüfbefehl | grindkommando, grindkommandon |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Intag / Specifikation / Bygge / Granskning / Acceptans / Övertagande / Klar |
| Risikoklasse | riskklass |
| Pfadbereich | sökvägsomfång |
| Herkunft | ursprung |
| Station | station |
| Kriterium, Kriterien | kriterium, kriterier |
| Prüfrunde | granskningsrunda |
| Befund | fynd |
| Hersteller, Herstellerwechsel | tillverkare, tillverkarbyte |
| Kontingent | kvot |
| Workflow-Datei | workflowfil |
| Stand | tillstånd |
| Erstdurchlaufquote, Erstdurchlauf, Quote | förstagångsandel, första omgången, andel |
| Kennzahlen | nyckeltal |
| Regelvorschlag | regelförslag |
| wiederkehrender Auftrag | återkommande arbetsorder |
| Abstand (wiederkehrender Auftrag) | intervall |
| eingereiht | i kö |
| Rolle | roll |
| Einwand | invändning |
| Rangliste | rangordning |
| Prüfer-Regel | granskarregel (krävs / föredras) |
| Vorgabe, auf Vorgabe zurücksetzen | förval, återställ till förval |
| Mitkopieren / Einrichtung | kopiera med / konfigurering |
| Prüfrelevante Dateien | filer som påverkar kontrollerna |
| Issue | issue (issuen, issues) |
| Merge Request | merge request |
| Rechte | Behörigheter |
| Erlauben / Fragen / Verbieten | Tillåt / Fråga / Neka (Verbieten as Ablehnen) |
| Gedächtnis | minne |
| Anweisungen | instruktioner |
| Ausgabestil | utdatastil |
| Auswahl | Filter |
| Plugin, Plugins | insticksprogram (macOS's word) |
| Marktplatz, Marktplätze | marknadsplats, marknadsplatser |
| Frage nebenbei | sidofråga |
| Header | huvuden |
| Umgebung | miljö |
| Bericht | rapport |
| Grenzen | Gränser |
| Verbindung | Anslutning (a network connection; splat's Koppling means a diagram connector) |

### Thai (`th`)
- Address: polite neutral "คุณ", as macOS Thai; menu items and buttons are bare verbs ("บันทึก", "ซ่อนแถบเครื่องมือ");
  no politeness particles (ครับ/ค่ะ)
- Typography: “…”; menu items end in "…" without a space ("การตั้งค่า…", "บันทึกเป็น…"); no full stop at the end of a
  sentence, a space separates sentences; a space between Thai and Latin words or a placeholder ("แสดงใน Finder", "ออกจาก
  {}")
- Git and CI words: Git's words stay in Latin script and capitalized like names, as Thai developers write them and as
  splat does ("Commit"): Repository, Commit, Stage, Branch, Remote, Push, Pull, Fetch, Rebase, Worktree, Hook, Job; Thai
  otherwise: ผสาน (merge), ความขัดแย้ง (conflict), ส่วนที่เปลี่ยนแปลง (hunk), สำเนาที่ใช้งาน (working tree), การรัน (CI
  run), ล็อก (log); "AI" stays "AI"

| German key | Thai |
|---|---|
| Ablage | ไฟล์ |
| Bearbeiten | แก้ไข |
| Darstellung | มุมมอง |
| Fenster | หน้าต่าง |
| Hilfe | วิธีใช้ |
| Dienste | บริการ |
| Über {} | เกี่ยวกับ {} |
| Einstellungen | การตั้งค่า (menu item: การตั้งค่า…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | ซ่อน {} / ซ่อนอื่นๆ / แสดงทั้งหมด |
| {} beenden | ออกจาก {} |
| Widerrufen / Wiederholen | เลิกทำ / ทำกลับมาใหม่ |
| Ausschneiden / Kopieren / Einsetzen / Löschen | ตัด / คัดลอก / วาง / ลบ |
| Alles auswählen | เลือกทั้งหมด |
| Suchen … / Weitersuchen / Rückwärts suchen | ค้นหา… / ค้นหาถัดไป / ค้นหาก่อนหน้า |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | ย่อ / ซูม / นำทั้งหมดมาด้านหน้า |
| Vollbildmodus | เข้าสู่โหมดเต็มหน้าจอ |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | ซ่อนแถบเครื่องมือ / แสดง/ซ่อนแถบด้านข้าง |
| Sichern | บันทึก |
| Öffnen / Schließen | เปิด / ปิด |
| Abbrechen / OK | ยกเลิก / ตกลง |
| Im Finder zeigen | แสดงใน Finder |
| Papierkorb | ถังขยะ |
| Schlüsselbund | พวงกุญแจ |
| Zwischenablage | คลิปบอร์ด |
| Tab | แถบ |
| AI-Tab | แถบ AI |
| Panel | แผง |
| Seitenleiste | แถบด้านข้าง |
| Vorschau | แสดงตัวอย่าง |
| Bereich | บานหน้าต่าง |
| Aktivität | กิจกรรม |
| Projekt / Ordner | โปรเจ็กต์ / โฟลเดอร์ |
| Vertrauen, vertrauen | ความเชื่อถือ, เชื่อถือ |
| Platte | ดิสก์ (บนดิสก์) |
| Unterschiede | ความแตกต่าง |
| Zusammenführen | ผสาน, การผสาน |
| Konflikt | ความขัดแย้ง |
| Treffer | ผลการค้นหา |
| Suche | ค้นหา |
| Ersetzen | แทนที่ |
| Sprachserver | เซิร์ฟเวอร์ภาษา |
| Hinweis (Schweregrad) | คำแนะนำ |
| Inlay-Hinweis | คำแนะนำแบบอินไลน์ |
| Minimap | มินิแมป |
| Sticky Scroll | การเลื่อนแบบตรึง |
| Diagnosen am Zeilenende | การวินิจฉัยที่ท้ายบรรทัด |
| Vim-Modus | โหมด Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / INSERT / REPLACE / VISUAL / VISUAL LINE / VISUAL BLOCK (Vim's English names) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: ไม่พบรูปแบบ: {} … (translated from Vim's English, Vim's numbers kept) |
| Vorschlag | ข้อเสนอ |
| Vorhersage | การคาดการณ์ |
| Ausschlussliste | รายการยกเว้น |
| Blame | Blame |
| Hinweise (Rotor) | คำแนะนำ |
| Schnellkorrektur | แก้ไขด่วน |
| Refactoring | ปรับโครงสร้างโค้ด |
| Quellaktion | การทำงานกับซอร์ส |
| Implementierung / Typdefinition / Deklaration | การนำไปใช้ / คำจำกัดความของชนิด / การประกาศ |
| Aufrufhierarchie / Typhierarchie | ลำดับชั้นการเรียก / ลำดับชั้นของชนิด |
| Aufrufer / Aufgerufene | ผู้เรียก / ผู้ถูกเรียก |
| Obertypen / Untertypen | ชนิดแม่ / ชนิดย่อย |
| indexieren | ทำดัชนี |
| Lesezeichen / Verlauf / Adresse | ที่คั่นหน้า / ประวัติ / ที่อยู่ |
| Bytes | ไบต์ |
| Exportieren | ส่งออก |
| Format | รูปแบบ |
| Diktat | การป้อนตามคำบอก |
| Agent | เอเจนต์ |
| Eigener Agent | เอเจนต์ในตัว |
| Eigene Agenten | เอเจนต์ที่กำหนดเอง |
| Thread (AI-Thread, CLI-Thread) | เธรด (เธรด AI, เธรด CLI) |
| Sitzung | เซสชัน |
| Subagent | เอเจนต์ย่อย |
| Hintergrundprozess | กระบวนการเบื้องหลัง |
| Unterhaltung | บทสนทนา |
| Zug | รอบ |
| Antwort | คำตอบ |
| Rückfrage | คำถาม |
| Freigabe, Freigabemodus | สิทธิ์, โหมดสิทธิ์ |
| Umgehen | ข้ามสิทธิ์ |
| Erlauben / Ablehnen / Immer erlauben … | อนุญาต / ปฏิเสธ / อนุญาตเสมอ… |
| Plan | แผน |
| To-dos | สิ่งที่ต้องทำ |
| Aufgabe | งาน |
| Auftrag | งานที่มอบหมาย |
| Werkzeug | เครื่องมือ |
| Skills | Skills |
| Befehl | คำสั่ง |
| Hook | Hook |
| Sandbox | แซนด์บ็อกซ์ |
| Fremder Inhalt | เนื้อหาจากภายนอก |
| Sicherungspunkt | จุดตรวจสอบ |
| Zurückspulen | กรอกลับ |
| Verdichten | บีบอัด |
| Kontext, Kontextfenster | บริบท, หน้าต่างบริบท |
| Denktiefe | ระดับการคิด |
| Modell / Anbieter / Endpunkt | โมเดล / ผู้ให้บริการ / เอนด์พอยต์ |
| Token, Tokens | โทเค็น |
| API-Schlüssel | คีย์ API |
| Register | รีจิสทรี |
| Adapter | อะแดปเตอร์ |
| Debug-Adapter | อะแดปเตอร์ดีบัก |
| Bereit | พร้อม |
| Prüfsumme | เช็คซัม |
| Anmeldung, anmelden | การลงชื่อเข้า, ลงชื่อเข้า |
| Konto | บัญชี |
| Abo | การสมัครใช้บริการ |
| Repository | Repository |
| Commit, committen | Commit, ทำ Commit |
| Secrets | ข้อมูลลับ |
| vormerken | Stage |
| Zurücknehmen | Unstage |
| nicht versioniert | ไม่ได้ติดตาม |
| Abschnitt (Git) | ส่วนที่เปลี่ยนแปลง |
| Branch / Remote | Branch / Remote |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Stash | Stash |
| Übernehmen (Worktree) | นำมาใช้ |
| Arbeitskopie | สำเนาที่ใช้งาน |
| Lauf, Läufe | การรัน |
| Host | โฮสต์ |
| Job | Job |
| Log | ล็อก |
| **Debugger** | |
| Debuggen | ดีบัก |
| Haltepunkt, Haltepunkte | เบรกพอยต์ |
| Logpunkt | ล็อกพอยต์ |
| Bedingung / Trefferzahl | เงื่อนไข / จำนวนครั้งที่เข้าถึง |
| Aufrufliste | สแตกการเรียก |
| Frame | เฟรม (เฟรมสแตก) |
| Überwachter Ausdruck, Überwachte Ausdrücke | นิพจน์ที่เฝ้าดู |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | ดำเนินการต่อ / ก้าวข้าม / ก้าวเข้า / ก้าวออก |
| Anhängen / Trennen | แนบ / ตัดการเชื่อมต่อ |
| An Prozess anhängen | แนบกับกระบวนการ |
| Neu starten / Stopp | เริ่มใหม่ / หยุด |
| Variablen | ตัวแปร |
| Angehalten | หยุดชั่วคราว |
| Debug-Konsole | คอนโซลดีบัก |
| Startkonfiguration | การกำหนดค่าการเริ่มต้น |
| Inspektor | ตัวตรวจสอบ |
| **Tests** | |
| Tests | การทดสอบ |
| Test | การทดสอบ |
| **Containers** | |
| Container | คอนเทนเนอร์ |
| Docker-Kontext | บริบท Docker |
| **Leitstand (phase 13)** | |
| Leitstand | ศูนย์ควบคุม |
| Auftrag (Leitstand) | ใบสั่งงาน |
| Nachweis | หลักฐาน |
| Abweichung | ข้อเบี่ยงเบน |
| Abnahme, abnehmen | การตรวจรับ, ตรวจรับ |
| Freigeben (Spezifikation) | อนุมัติ |
| Planer | ผู้วางแผน |
| Builder | ผู้สร้าง |
| Prüfer | ผู้ตรวจทาน |
| Späher | ผู้สำรวจ |
| Prüfstation, Gate | ด่านตรวจ |
| Prüfbefehl | คำสั่งด่านตรวจ |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | การรับเข้า / ข้อกำหนด / การสร้าง / การตรวจทาน / การตรวจรับ / การนำมาใช้ / เสร็จสิ้น |
| Risikoklasse | ระดับความเสี่ยง |
| Pfadbereich | ขอบเขตพาธ |
| Herkunft | ที่มา |
| Station | สถานี |
| Kriterium, Kriterien | เกณฑ์ |
| Prüfrunde | รอบการตรวจทาน |
| Befund | ข้อค้นพบ |
| Hersteller, Herstellerwechsel | ผู้ผลิต, การเปลี่ยนผู้ผลิต |
| Kontingent | โควตา |
| Workflow-Datei | ไฟล์เวิร์กโฟลว์ |
| Stand | สถานะ |
| Erstdurchlaufquote, Erstdurchlauf, Quote | อัตราผ่านในรอบแรก, ผ่านในรอบแรก, อัตรา |
| Kennzahlen | ตัวชี้วัด |
| Regelvorschlag | ข้อเสนอกฎ |
| wiederkehrender Auftrag | ใบสั่งงานที่เกิดซ้ำ |
| Abstand (wiederkehrender Auftrag) | ช่วงเวลา |
| eingereiht | อยู่ในคิว |
| Rolle | บทบาท |
| Einwand | ข้อคัดค้าน, คัดค้าน |
| Rangliste | การจัดอันดับ |
| Prüfer-Regel | กฎผู้ตรวจทาน (ต้องเป็น / ควรเป็นผู้ผลิตรายอื่น) |
| Vorgabe, auf Vorgabe zurücksetzen | ค่าเริ่มต้น, คืนค่าเป็นค่าเริ่มต้น |
| Mitkopieren / Einrichtung | คัดลอกไปด้วย / การตั้งค่า |
| Prüfrelevante Dateien | ไฟล์ที่เกี่ยวข้องกับการตรวจสอบ |
| Issue | Issue |
| Merge Request | Merge Request |
| Rechte | สิทธิ์ |
| Erlauben / Fragen / Verbieten | อนุญาต / ถาม / ปฏิเสธ (Verbieten as Ablehnen) |
| Gedächtnis | ความจำ |
| Anweisungen | คำชี้แจง |
| Ausgabestil | รูปแบบเอาต์พุต |
| Auswahl | ตัวกรอง |
| Plugin, Plugins | ปลั๊กอิน |
| Marktplatz, Marktplätze | มาร์เก็ตเพลส |
| Frage nebenbei | คำถามแทรก |
| Header | เฮดเดอร์ |
| Umgebung | สภาพแวดล้อม |
| Bericht | รายงาน |
| Grenzen | ขีดจำกัด |
| Verbindung | การเชื่อมต่อ (a network connection; splat's เส้นเชื่อม means a diagram connector) |

### Turkish (`tr`)
- Address: formal "siz", as macOS Turkish ("… istediğinizden emin misiniz?", "Seçin"); menu items and buttons in the
  bare imperative ("Kaydet", "Kenar Çubuğunu Göster"); yes/no questions in the passive ("… kaydedilsin mi?")
- Typography: “…”; a suffix after a name or Latin word is joined with ’ ("Finder’da", "commit’ler"); where a suffix
  would have to follow a placeholder, rephrase with a colon as macOS does ("Gizle: {}", "Çık: {}"); menu items end in
  "…" without a space ("Ayarlar…", "Farklı Kaydet…"); menu items and buttons in title case, as macOS Turkish writes them
  ("Tümünü Seç", "Araç Çubuğunu Gizle"), sentences in sentence case
- Git and CI words: kept as Turkish developers say them: commit (commit etmek), push, pull, fetch, rebase, worktree,
  hook, token; Turkish where the word is settled: depo (repository), dal (branch), uzak depo (remote),
  birleştir/birleştirme (merge), çakışma (conflict), hazırla (stage), parça (hunk), çalışma kopyası, çalıştırma (CI
  run), iş (job), günlük (log), korumalı alan (sandbox); "AI" stays "AI" (macOS writes "yapay zekâ" in running text)

| German key | Turkish |
|---|---|
| Ablage | Dosya |
| Bearbeiten | Düzen |
| Darstellung | Görüntü |
| Fenster | Pencere |
| Hilfe | Yardım |
| Dienste | Servisler |
| Über {} | {} Hakkında |
| Einstellungen | Ayarlar (menu item: Ayarlar…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Gizle: {} / Diğerlerini Gizle / Tümünü Göster |
| {} beenden | Çık: {} |
| Widerrufen / Wiederholen | Geri Al / Yinele |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Kes / Kopyala / Yapıştır / Sil |
| Alles auswählen | Tümünü Seç |
| Suchen … / Weitersuchen / Rückwärts suchen | Bul… / Sonrakini Bul / Öncekini Bul |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Simge Durumuna Küçült / Büyüt/Küçült / Tümünü Öne Getir |
| Vollbildmodus | Tam Ekrana Geç |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Araç Çubuğunu Gizle / Kenar Çubuğunu Göster/Gizle |
| Sichern | Kaydet |
| Öffnen / Schließen | Aç / Kapat |
| Abbrechen / OK | Vazgeç / Tamam |
| Im Finder zeigen | Finder’da Göster |
| Papierkorb | Çöp Sepeti |
| Schlüsselbund | Anahtar Zinciri |
| Zwischenablage | Pano |
| Tab | sekme |
| AI-Tab | AI sekmesi |
| Panel | panel |
| Seitenleiste | kenar çubuğu |
| Vorschau | önizleme |
| Bereich | bölme |
| Aktivität | Etkinlik |
| Projekt / Ordner | proje / klasör |
| Vertrauen, vertrauen | güven, güvenmek |
| Platte | disk (diskte) |
| Unterschiede | farklar |
| Zusammenführen | birleştirme, birleştir |
| Konflikt | çakışma |
| Treffer | sonuç |
| Suche | Arama |
| Ersetzen | Değiştir |
| Sprachserver | dil sunucusu |
| Hinweis (Schweregrad) | İpucu |
| Inlay-Hinweis | satır içi ipucu |
| Minimap | Mini Harita |
| Sticky Scroll | Yapışkan Kaydırma |
| Diagnosen am Zeilenende | Satır Sonunda Tanılamalar |
| Vim-Modus | Vim Modu |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / EKLE / DEĞİŞTİR / GÖRSEL / GÖRSEL SATIR / GÖRSEL BLOK (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Dizgi bulunamadı: {} … (as this language's Vim says them) |
| Vorschlag | öneri |
| Vorhersage | Tahmin |
| Ausschlussliste | hariç tutma listesi |
| Blame | Blame |
| Hinweise (Rotor) | İpuçları |
| Schnellkorrektur | hızlı düzeltme |
| Refactoring | yeniden düzenleme |
| Quellaktion | kaynak işlemi |
| Implementierung / Typdefinition / Deklaration | uygulama / tür tanımı / bildirim |
| Aufrufhierarchie / Typhierarchie | Çağrı Hiyerarşisi / Tür Hiyerarşisi |
| Aufrufer / Aufgerufene | Çağıranlar / Çağrılanlar |
| Obertypen / Untertypen | Üst Türler / Alt Türler |
| indexieren | dizinleme |
| Lesezeichen / Verlauf / Adresse | yer işareti / geçmiş / adres |
| Bytes | bayt |
| Exportieren | Dışa Aktar |
| Format | Biçim |
| Diktat | dikte |
| Agent | ajan |
| Eigener Agent | yerleşik ajan |
| Eigene Agenten | Özel ajanlar |
| Thread (AI-Thread, CLI-Thread) | sohbet (AI sohbeti, CLI sohbeti) |
| Sitzung | oturum |
| Subagent | alt ajan |
| Hintergrundprozess | arka plan işlemi |
| Unterhaltung | konuşma |
| Zug | tur |
| Antwort | yanıt |
| Rückfrage | soru |
| Freigabe, Freigabemodus | izin, izin modu |
| Umgehen | Atla |
| Erlauben / Ablehnen / Immer erlauben … | İzin Ver / Reddet / Her Zaman İzin Ver… |
| Plan | plan |
| To-dos | Yapılacaklar |
| Aufgabe | görev |
| Auftrag | görev |
| Werkzeug | araç |
| Skills | Beceriler |
| Befehl | komut |
| Hook | hook |
| Sandbox | korumalı alan |
| Fremder Inhalt | Dış İçerik |
| Sicherungspunkt | kontrol noktası |
| Zurückspulen | geri sarma, geri sar |
| Verdichten | sıkıştırma, sıkıştır |
| Kontext, Kontextfenster | bağlam, bağlam penceresi |
| Denktiefe | düşünme derinliği |
| Modell / Anbieter / Endpunkt | model / sağlayıcı / uç nokta |
| Token, Tokens | token, token’lar |
| API-Schlüssel | API anahtarı |
| Register | kayıt defteri |
| Adapter | bağdaştırıcı |
| Debug-Adapter | hata ayıklama bağdaştırıcısı |
| Bereit | Hazır |
| Prüfsumme | sağlama toplamı |
| Anmeldung, anmelden | giriş, giriş yap |
| Konto | hesap |
| Abo | Abonelik |
| Repository | depo |
| Commit, committen | commit, commit et |
| Secrets | gizli bilgiler |
| vormerken | hazırla |
| Zurücknehmen | Hazırlıktan Çıkar |
| nicht versioniert | izlenmiyor |
| Abschnitt (Git) | parça |
| Branch / Remote | dal / uzak depo |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | aktar |
| Arbeitskopie | çalışma kopyası |
| Lauf, Läufe | çalıştırma, çalıştırmalar |
| Host | sunucu |
| Job | iş |
| Log | günlük |
| **Debugger** | |
| Debuggen | Hata Ayıkla |
| Haltepunkt, Haltepunkte | kesme noktası, kesme noktaları |
| Logpunkt | günlük noktası |
| Bedingung / Trefferzahl | koşul / isabet sayısı |
| Aufrufliste | çağrı yığını |
| Frame | çerçeve (yığın çerçevesi) |
| Überwachter Ausdruck, Überwachte Ausdrücke | izlenen ifade, izlenen ifadeler |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Devam Et / Üzerinden Adımla / İçine Adımla / Dışına Adımla |
| Anhängen / Trennen | bağlan / bağlantıyı kes |
| An Prozess anhängen | İşleme Bağlan |
| Neu starten / Stopp | Yeniden Başlat / Durdur |
| Variablen | değişkenler |
| Angehalten | duraklatıldı |
| Debug-Konsole | hata ayıklama konsolu |
| Startkonfiguration | başlatma yapılandırması |
| Inspektor | Denetçi |
| **Tests** | |
| Tests | Testler |
| Test | test |
| **Containers** | |
| Container | Container’lar |
| Docker-Kontext | Docker bağlamı |
| **Leitstand (phase 13)** | |
| Leitstand | Kontrol Odası |
| Auftrag (Leitstand) | iş emri, iş emirleri |
| Nachweis | kanıt |
| Abweichung | sapma |
| Abnahme, abnehmen | kabul, kabul et |
| Freigeben (Spezifikation) | onay, onayla |
| Planer | Planlayıcı |
| Builder | Builder |
| Prüfer | İnceleyici |
| Späher | Keşifçi |
| Prüfstation, Gate | geçit |
| Prüfbefehl | geçit komutu |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Alım / Belirtim / Yapım / İnceleme / Kabul / Aktarım / Tamamlandı |
| Risikoklasse | risk sınıfı |
| Pfadbereich | yol kapsamı |
| Herkunft | köken |
| Station | istasyon |
| Kriterium, Kriterien | ölçüt, ölçütler |
| Prüfrunde | inceleme döngüsü |
| Befund | bulgu |
| Hersteller, Herstellerwechsel | üretici, üretici değişikliği |
| Kontingent | kota |
| Workflow-Datei | iş akışı dosyası |
| Stand | durum |
| Erstdurchlaufquote, Erstdurchlauf, Quote | ilk geçiş oranı, ilk geçiş, oran |
| Kennzahlen | metrikler |
| Regelvorschlag | kural önerisi |
| wiederkehrender Auftrag | yinelenen iş emri |
| Abstand (wiederkehrender Auftrag) | aralık |
| eingereiht | sırada |
| Rolle | rol |
| Einwand | itiraz |
| Rangliste | öncelik sırası |
| Prüfer-Regel | inceleyici kuralı (zorunlu / tercih edilir) |
| Vorgabe, auf Vorgabe zurücksetzen | Saptanmış, Saptanmışa Döndür |
| Mitkopieren / Einrichtung | birlikte kopyala / kurulum |
| Prüfrelevante Dateien | inceleme için önemli dosyalar |
| Issue | issue (issue’dan, issue’ya) |
| Merge Request | merge request (as the CI panel; Merge Request in titles) |
| Rechte | İzinler |
| Erlauben / Fragen / Verbieten | İzin Ver / Sor / Reddet (Verbieten as Ablehnen) |
| Gedächtnis | bellek |
| Anweisungen | talimatlar |
| Ausgabestil | çıktı stili (title: Çıktı Stili) |
| Auswahl | Filtre |
| Plugin, Plugins | eklenti, eklentiler |
| Marktplatz, Marktplätze | pazar yeri, pazar yerleri |
| Frage nebenbei | yan soru (title: Yan Soru) |
| Header | üstbilgiler |
| Umgebung | ortam |
| Bericht | rapor |
| Grenzen | Sınırlar |

### Ukrainian (`uk`)
- Address: formal "ви", lowercase as macOS Ukrainian writes it ("ви", "ваш"); menu items and buttons in the infinitive
  ("Зберегти", "Сховати бічну панель"); instructions in the "ви" imperative ("Виберіть…", "Відкрийте…")
- Typography: «…»; apostrophe "ʼ" (U+02BC), as macOS writes it ("обʼєднати", "компʼютер", "вʼязка"); menu items end in
  "…" without a space ("Параметри…", "Зберегти як…"); the OK button is "OK" in Latin letters; sentence case
- Git and CI words: Git's operation names stay in Latin as Ukrainian developers write them: push, pull, fetch, rebase,
  worktree; Ukrainian otherwise: репозиторій, коміт (закомітити), гілка (branch), віддалений репозиторій (remote),
  індексувати (stage), фрагмент (hunk), злиття (merge), робоча копія, хук, пісочниця, токен, запуск (CI run), завдання
  (job), журнал (log); "AI" is "ШІ", as macOS writes it

| German key | Ukrainian |
|---|---|
| Ablage | Файл |
| Bearbeiten | Редагування |
| Darstellung | Перегляд |
| Fenster | Вікно |
| Hilfe | Довідка |
| Dienste | Служби |
| Über {} | Про {} |
| Einstellungen | Параметри (menu item: Параметри…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Сховати {} / Сховати решту / Показати всі |
| {} beenden | Завершити {} |
| Widerrufen / Wiederholen | Відмінити / Повторити |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Вирізати / Скопіювати / Вставити / Видалити |
| Alles auswählen | Вибрати всі |
| Suchen … / Weitersuchen / Rückwärts suchen | Пошук… / Знайти наступне / Знайти попереднє |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Згорнути / Оптимізувати / Усі наперед |
| Vollbildmodus | Увійти до повноекранного режиму |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Сховати панель інструментів / Показати/сховати бічну панель |
| Sichern | Зберегти |
| Öffnen / Schließen | Відкрити / Закрити |
| Abbrechen / OK | Скасувати / OK |
| Im Finder zeigen | Показати у Finder |
| Papierkorb | Смітник |
| Schlüsselbund | вʼязка |
| Zwischenablage | буфер обміну |
| Tab | вкладка |
| AI-Tab | вкладка ШІ |
| Panel | панель |
| Seitenleiste | бічна панель |
| Vorschau | попередній перегляд |
| Bereich | область |
| Aktivität | Активність |
| Projekt / Ordner | проєкт / папка |
| Vertrauen, vertrauen | довіра, довіряти |
| Platte | диск (на диску) |
| Unterschiede | відмінності |
| Zusammenführen | злиття, злити |
| Konflikt | конфлікт |
| Treffer | результат |
| Suche | Пошук |
| Ersetzen | Замінити |
| Sprachserver | мовний сервер |
| Hinweis (Schweregrad) | Підказка |
| Inlay-Hinweis | вбудована підказка |
| Minimap | Мінікарта |
| Sticky Scroll | Закріплене прокручування |
| Diagnosen am Zeilenende | Діагностика в кінці рядка |
| Vim-Modus | Режим Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | НОРМАЛЬНИЙ / ВСТАВИТИ / ЗАМІНА / ВИБІР / ВИБІР РЯДКІВ / ВИБІР БЛОКУ (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Зразок не знайдено: {} … (as this language's Vim says them) |
| Vorschlag | пропозиція |
| Vorhersage | Передбачення |
| Ausschlussliste | список винятків |
| Blame | Blame |
| Hinweise (Rotor) | Підказки |
| Schnellkorrektur | швидке виправлення |
| Refactoring | рефакторинг |
| Quellaktion | дія з вихідним кодом |
| Implementierung / Typdefinition / Deklaration | реалізація / визначення типу / оголошення |
| Aufrufhierarchie / Typhierarchie | Ієрархія викликів / Ієрархія типів |
| Aufrufer / Aufgerufene | Викликають / Викликані |
| Obertypen / Untertypen | Супертипи / Підтипи |
| indexieren | індексування |
| Lesezeichen / Verlauf / Adresse | закладка / історія / адреса |
| Bytes | Б |
| Exportieren | Експортувати |
| Format | Формат |
| Diktat | диктування |
| Agent | агент |
| Eigener Agent | власний агент |
| Eigene Agenten | Користувацькі агенти |
| Thread (AI-Thread, CLI-Thread) | чат (ШІ-чат, CLI-чат) |
| Sitzung | сеанс |
| Subagent | субагент |
| Hintergrundprozess | фоновий процес |
| Unterhaltung | діалог |
| Zug | хід |
| Antwort | відповідь |
| Rückfrage | запитання |
| Freigabe, Freigabemodus | дозвіл, режим дозволів |
| Umgehen | Обхід |
| Erlauben / Ablehnen / Immer erlauben … | Дозволити / Заборонити / Завжди дозволяти… |
| Plan | план |
| To-dos | Список справ |
| Aufgabe | задача |
| Auftrag | доручення |
| Werkzeug | інструмент |
| Skills | Навички |
| Befehl | команда |
| Hook | хук |
| Sandbox | пісочниця |
| Fremder Inhalt | Зовнішній вміст |
| Sicherungspunkt | контрольна точка |
| Zurückspulen | відкат, відкотити |
| Verdichten | стиснення, стиснути |
| Kontext, Kontextfenster | контекст, контекстне вікно |
| Denktiefe | глибина міркувань |
| Modell / Anbieter / Endpunkt | модель / провайдер / кінцева точка |
| Token, Tokens | токен, токени |
| API-Schlüssel | ключ API |
| Register | реєстр |
| Adapter | адаптер |
| Debug-Adapter | адаптер налагодження |
| Bereit | Готово |
| Prüfsumme | контрольна сума |
| Anmeldung, anmelden | вхід, увійти |
| Konto | обліковий запис |
| Abo | Підписка |
| Repository | репозиторій |
| Commit, committen | коміт, закомітити |
| Secrets | секрети |
| vormerken | індексувати |
| Zurücknehmen | Вилучити з індексу |
| nicht versioniert | не відстежується |
| Abschnitt (Git) | фрагмент |
| Branch / Remote | гілка / віддалений репозиторій |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | перенести |
| Arbeitskopie | робоча копія |
| Lauf, Läufe | запуск, запуски |
| Host | хост |
| Job | завдання |
| Log | журнал |
| **Debugger** | |
| Debuggen | Налагодження |
| Haltepunkt, Haltepunkte | точка зупину, точки зупину |
| Logpunkt | точка журналювання |
| Bedingung / Trefferzahl | умова / кількість влучань |
| Aufrufliste | стек викликів |
| Frame | кадр |
| Überwachter Ausdruck, Überwachte Ausdrücke | контрольний вираз, контрольні вирази |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Продовжити / Крок з обходом / Крок із заходом / Крок із виходом |
| Anhängen / Trennen | приєднатися / відʼєднатися |
| An Prozess anhängen | Приєднатися до процесу |
| Neu starten / Stopp | Перезапустити / Зупинити |
| Variablen | змінні |
| Angehalten | призупинено |
| Debug-Konsole | консоль налагодження |
| Startkonfiguration | конфігурація запуску |
| Inspektor | Інспектор |
| **Tests** | |
| Tests | Тести |
| Test | тест |
| **Containers** | |
| Container | Контейнери |
| Docker-Kontext | контекст Docker |
| **Leitstand (phase 13)** | |
| Leitstand | диспетчерська |
| Auftrag (Leitstand) | замовлення |
| Nachweis | докази |
| Abweichung | відхилення |
| Abnahme, abnehmen | приймання, прийняти |
| Freigeben (Spezifikation) | затвердити (затвердження) |
| Planer | Планувальник |
| Builder | Виконавець |
| Prüfer | Рецензент |
| Späher | Розвідник |
| Prüfstation, Gate | шлюз |
| Prüfbefehl | команда перевірки |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Вхідні / Специфікація / Реалізація / Перевірка / Приймання / Перенесення / Готово |
| Risikoklasse | клас ризику |
| Pfadbereich | область шляхів |
| Herkunft | походження |
| Station | етап |
| Kriterium, Kriterien | критерій, критерії |
| Prüfrunde | раунд перевірки |
| Befund | зауваження |
| Hersteller, Herstellerwechsel | виробник, зміна виробника |
| Kontingent | квота |
| Workflow-Datei | файл робочого процесу |
| Stand | стан |
| Erstdurchlaufquote, Erstdurchlauf, Quote | частка успіху з першого разу, з першого разу, частка |
| Kennzahlen | показники |
| Regelvorschlag | пропозиція правила |
| wiederkehrender Auftrag | повторюване замовлення |
| Abstand (wiederkehrender Auftrag) | інтервал |
| eingereiht | у черзі |
| Rolle | роль |
| Einwand | заперечення |
| Rangliste | список пріоритетів |
| Prüfer-Regel | правило рецензента (обовʼязково / бажано) |
| Vorgabe, auf Vorgabe zurücksetzen | типове, повернути типове значення |
| Mitkopieren / Einrichtung | копіювати також / підготовка |
| Prüfrelevante Dateien | файли, важливі для перевірки |
| Issue | issue |
| Merge Request | merge request (as the CI panel) |
| Rechte | Дозволи |
| Erlauben / Fragen / Verbieten | Дозволити / Запитувати / Заборонити (Verbieten as Ablehnen) |
| Gedächtnis | памʼять |
| Anweisungen | інструкції |
| Ausgabestil | стиль виводу |
| Auswahl | Фільтр |
| Plugin, Plugins | плагін, плагіни |
| Marktplatz, Marktplätze | маркетплейс, маркетплейси |
| Frage nebenbei | побіжне запитання |
| Header | заголовки |
| Umgebung | середовище |
| Bericht | звіт |
| Grenzen | Ліміти |
| Verbindung | Підключення (a network connection; splat's Зʼєднання means a diagram connector) |

### Vietnamese (`vi`)
- Address: "bạn", as macOS Vietnamese ("Bạn có chắc chắn muốn …?"); menu items and buttons are bare verbs ("Lưu", "Ẩn
  thanh công cụ"), instructions without a pronoun where possible ("Chọn …")
- Typography: “…”; menu items end in "…" without a space ("Cài đặt…", "Lưu thành…"); sentence case, but the capitals
  macOS uses inside standard items stay as they are ("Chuyển sang Toàn Màn hình", "Ẩn Thanh bên")
- Git and CI words: kept as Vietnamese developers say them: commit, stage, remote, push, pull, fetch, rebase, worktree,
  hook, sandbox, token, job; Vietnamese where the word is settled: kho lưu trữ (repository), nhánh (branch), hợp nhất
  (merge), xung đột (conflict), khối thay đổi (hunk), bản sao làm việc (working tree), lần chạy (CI run), nhật ký (log);
  "AI" stays "AI"

| German key | Vietnamese |
|---|---|
| Ablage | Tệp |
| Bearbeiten | Sửa |
| Darstellung | Xem |
| Fenster | Cửa sổ |
| Hilfe | Trợ giúp |
| Dienste | Dịch vụ |
| Über {} | Giới thiệu về {} |
| Einstellungen | Cài đặt (menu item: Cài đặt…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ẩn {} / Ẩn các mục khác / Hiển thị tất cả |
| {} beenden | Thoát {} |
| Widerrufen / Wiederholen | Hoàn tác / Làm lại |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cắt / Sao chép / Dán / Xóa |
| Alles auswählen | Chọn Tất cả |
| Suchen … / Weitersuchen / Rückwärts suchen | Tìm… / Tìm mục tiếp theo / Tìm mục trước |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Thu nhỏ / Thu phóng / Đưa tất cả lên Phía trước |
| Vollbildmodus | Chuyển sang Toàn Màn hình |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ẩn thanh công cụ / Hiển thị/Ẩn Thanh bên |
| Sichern | Lưu |
| Öffnen / Schließen | Mở / Đóng |
| Abbrechen / OK | Hủy / OK |
| Im Finder zeigen | Hiển thị trong Finder |
| Papierkorb | Thùng rác |
| Schlüsselbund | Chuỗi khóa |
| Zwischenablage | bảng nhớ tạm |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | khung |
| Seitenleiste | thanh bên |
| Vorschau | xem trước |
| Bereich | ngăn |
| Aktivität | Hoạt động |
| Projekt / Ordner | dự án / thư mục |
| Vertrauen, vertrauen | sự tin cậy, tin cậy |
| Platte | ổ đĩa (trên ổ đĩa) |
| Unterschiede | khác biệt |
| Zusammenführen | hợp nhất |
| Konflikt | xung đột |
| Treffer | kết quả |
| Suche | Tìm kiếm |
| Ersetzen | Thay thế |
| Sprachserver | máy chủ ngôn ngữ |
| Hinweis (Schweregrad) | Gợi ý |
| Inlay-Hinweis | gợi ý nội tuyến |
| Minimap | Bản đồ thu nhỏ |
| Sticky Scroll | Cuộn cố định |
| Diagnosen am Zeilenende | Chẩn đoán ở cuối dòng |
| Vim-Modus | Chế độ Vim |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | NORMAL / CHÈN / THAY THẾ / VISUAL / DÒNG VISUAL / KHỐI VISUAL (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: Không tìm thấy mẫu (pattern): {} … (as this language's Vim says them; E748 translated, which the catalog leaves open) |
| Vorschlag | đề xuất |
| Vorhersage | Dự đoán |
| Ausschlussliste | danh sách loại trừ |
| Blame | Blame |
| Hinweise (Rotor) | Gợi ý |
| Schnellkorrektur | sửa nhanh |
| Refactoring | tái cấu trúc |
| Quellaktion | tác vụ mã nguồn |
| Implementierung / Typdefinition / Deklaration | triển khai / định nghĩa kiểu / khai báo |
| Aufrufhierarchie / Typhierarchie | Phân cấp lệnh gọi / Phân cấp kiểu |
| Aufrufer / Aufgerufene | Nơi gọi / Được gọi |
| Obertypen / Untertypen | Kiểu cha / Kiểu con |
| indexieren | lập chỉ mục |
| Lesezeichen / Verlauf / Adresse | dấu trang / lịch sử / địa chỉ |
| Bytes | byte |
| Exportieren | Xuất |
| Format | Định dạng |
| Diktat | đọc chính tả |
| Agent | tác nhân |
| Eigener Agent | tác nhân tích hợp |
| Eigene Agenten | Tác nhân tùy chỉnh |
| Thread (AI-Thread, CLI-Thread) | luồng (luồng AI, luồng CLI) |
| Sitzung | phiên |
| Subagent | tác nhân phụ |
| Hintergrundprozess | tiến trình nền |
| Unterhaltung | cuộc hội thoại |
| Zug | lượt |
| Antwort | câu trả lời |
| Rückfrage | câu hỏi |
| Freigabe, Freigabemodus | quyền, chế độ quyền |
| Umgehen | Bỏ qua quyền |
| Erlauben / Ablehnen / Immer erlauben … | Cho phép / Từ chối / Luôn cho phép… |
| Plan | kế hoạch |
| To-dos | Việc cần làm |
| Aufgabe | tác vụ |
| Auftrag | nhiệm vụ |
| Werkzeug | công cụ |
| Skills | Skill |
| Befehl | lệnh |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Nội dung bên ngoài |
| Sicherungspunkt | điểm kiểm tra |
| Zurückspulen | tua lại |
| Verdichten | thu gọn |
| Kontext, Kontextfenster | ngữ cảnh, cửa sổ ngữ cảnh |
| Denktiefe | mức độ suy nghĩ |
| Modell / Anbieter / Endpunkt | mô hình / nhà cung cấp / điểm cuối |
| Token, Tokens | token |
| API-Schlüssel | khóa API |
| Register | sổ đăng ký |
| Adapter | bộ chuyển đổi |
| Debug-Adapter | bộ chuyển đổi gỡ lỗi |
| Bereit | Sẵn sàng |
| Prüfsumme | tổng kiểm tra |
| Anmeldung, anmelden | đăng nhập |
| Konto | tài khoản |
| Abo | Gói đăng ký |
| Repository | kho lưu trữ |
| Commit, committen | commit |
| Secrets | thông tin bí mật |
| vormerken | stage |
| Zurücknehmen | Bỏ stage |
| nicht versioniert | chưa theo dõi |
| Abschnitt (Git) | khối thay đổi |
| Branch / Remote | nhánh / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Stash | stash |
| Übernehmen (Worktree) | tiếp nhận |
| Arbeitskopie | bản sao làm việc |
| Lauf, Läufe | lần chạy |
| Host | máy chủ |
| Job | job |
| Log | nhật ký |
| **Debugger** | |
| Debuggen | Gỡ lỗi |
| Haltepunkt, Haltepunkte | điểm ngắt |
| Logpunkt | điểm ghi nhật ký |
| Bedingung / Trefferzahl | điều kiện / số lần chạm |
| Aufrufliste | ngăn xếp lệnh gọi |
| Frame | khung |
| Überwachter Ausdruck, Überwachte Ausdrücke | biểu thức theo dõi |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | Tiếp tục / Bước qua / Bước vào / Bước ra |
| Anhängen / Trennen | đính kèm / ngắt kết nối |
| An Prozess anhängen | Đính kèm vào tiến trình |
| Neu starten / Stopp | Khởi động lại / Dừng |
| Variablen | biến |
| Angehalten | đã tạm dừng |
| Debug-Konsole | bảng điều khiển gỡ lỗi |
| Startkonfiguration | cấu hình khởi chạy |
| Inspektor | Bộ kiểm tra |
| **Tests** | |
| Tests | Kiểm thử |
| Test | kiểm thử |
| **Containers** | |
| Container | Container |
| Docker-Kontext | ngữ cảnh Docker |
| **Leitstand (phase 13)** | |
| Leitstand | Phòng điều khiển |
| Auftrag (Leitstand) | phiếu công việc |
| Nachweis | bằng chứng |
| Abweichung | sai lệch |
| Abnahme, abnehmen | nghiệm thu, nghiệm thu |
| Freigeben (Spezifikation) | phê duyệt |
| Planer | người lập kế hoạch |
| Builder | người xây dựng |
| Prüfer | người xem xét |
| Späher | người trinh sát |
| Prüfstation, Gate | cổng kiểm tra |
| Prüfbefehl | lệnh kiểm tra |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | Hộp đến / Đặc tả / Xây dựng / Xem xét / Nghiệm thu / Tiếp nhận / Hoàn tất |
| Risikoklasse | mức rủi ro |
| Pfadbereich | phạm vi đường dẫn |
| Herkunft | nguồn gốc |
| Station | trạm |
| Kriterium, Kriterien | tiêu chí |
| Prüfrunde | vòng xem xét |
| Befund | phát hiện |
| Hersteller, Herstellerwechsel | hãng, đổi hãng |
| Kontingent | hạn mức |
| Workflow-Datei | tệp quy trình |
| Stand | trạng thái |
| Erstdurchlaufquote, Erstdurchlauf, Quote | tỷ lệ đạt ngay lần đầu, đạt lần đầu, tỷ lệ |
| Kennzahlen | chỉ số |
| Regelvorschlag | đề xuất quy tắc |
| wiederkehrender Auftrag | phiếu công việc định kỳ |
| Abstand (wiederkehrender Auftrag) | khoảng thời gian |
| eingereiht | đang chờ |
| Rolle | vai trò |
| Einwand | phản đối |
| Rangliste | bảng xếp hạng |
| Prüfer-Regel | quy tắc người xem xét (bắt buộc / ưu tiên) |
| Vorgabe, auf Vorgabe zurücksetzen | mặc định, đặt lại về mặc định |
| Mitkopieren / Einrichtung | sao chép kèm / thiết lập |
| Prüfrelevante Dateien | tệp quan trọng cho việc xem xét |
| Issue | issue |
| Merge Request | merge request (as the CI panel) |
| Rechte | Quyền |
| Erlauben / Fragen / Verbieten | Cho phép / Hỏi / Từ chối (Verbieten as Ablehnen) |
| Gedächtnis | bộ nhớ |
| Anweisungen | hướng dẫn |
| Ausgabestil | kiểu đầu ra |
| Auswahl | Bộ lọc |
| Plugin, Plugins | plugin |
| Marktplatz, Marktplätze | marketplace |
| Frage nebenbei | câu hỏi bên lề |
| Header | header |
| Umgebung | môi trường |
| Bericht | báo cáo |
| Grenzen | Giới hạn |

### Chinese, Simplified (`zh-Hans`)
- Address: "你", as macOS Simplified Chinese (never "您"); menu items and buttons are bare verbs ("保存", "隐藏工具栏")
- Typography: “…” and full-width punctuation (，。：？！（）); menu items end in "…" without a space ("设置…", "保存为…"); a space
  between Chinese and Latin text or a placeholder, as splat writes it ("关于 {}", "AI 标签页"), although macOS's own strings
  leave it out
- Git and CI words: Chinese, as Chinese developers and Git's Chinese documentation use them: 仓库, 提交, 暂存, 分支, 远程仓库, 推送 /
  拉取 / 抓取 / 变基, 合并, 冲突, 变更块 (hunk), 工作树 (worktree), 工作区 (working tree), 钩子 (hook), 运行 (CI run), 作业 (job), 日志 (log);
  "Token" and "AI" stay Latin

| German key | Chinese (Simplified) |
|---|---|
| Ablage | 文件 |
| Bearbeiten | 编辑 |
| Darstellung | 显示 |
| Fenster | 窗口 |
| Hilfe | 帮助 |
| Dienste | 服务 |
| Über {} | 关于 {} |
| Einstellungen | 设置 (menu item: 设置…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | 隐藏 {} / 隐藏其他 / 全部显示 |
| {} beenden | 退出 {} |
| Widerrufen / Wiederholen | 撤销 / 重做 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 剪切 / 拷贝 / 粘贴 / 删除 |
| Alles auswählen | 全选 |
| Suchen … / Weitersuchen / Rückwärts suchen | 查找… / 查找下一个 / 查找上一个 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 最小化 / 缩放 / 前置全部窗口 |
| Vollbildmodus | 进入全屏幕 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 隐藏工具栏 / 显示/隐藏边栏 |
| Sichern | 保存 |
| Öffnen / Schließen | 打开 / 关闭 |
| Abbrechen / OK | 取消 / 好 |
| Im Finder zeigen | 在访达中显示 |
| Papierkorb | 废纸篓 |
| Schlüsselbund | 钥匙串 |
| Zwischenablage | 剪贴板 |
| Tab | 标签页 |
| AI-Tab | AI 标签页 |
| Panel | 面板 |
| Seitenleiste | 边栏 |
| Vorschau | 预览 |
| Bereich | 窗格 |
| Aktivität | 活动 |
| Projekt / Ordner | 项目 / 文件夹 |
| Vertrauen, vertrauen | 信任 |
| Platte | 磁盘 (磁盘上) |
| Unterschiede | 差异 |
| Zusammenführen | 合并 |
| Konflikt | 冲突 |
| Treffer | 结果 |
| Suche | 搜索 |
| Ersetzen | 替换 |
| Sprachserver | 语言服务器 |
| Hinweis (Schweregrad) | 提示 |
| Inlay-Hinweis | 内嵌提示 |
| Minimap | 缩略图 |
| Sticky Scroll | 粘性滚动 |
| Diagnosen am Zeilenende | 在行尾显示诊断 |
| Vim-Modus | Vim 模式 |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | 正常 / 插入 / 替换 / 可视 / 可视 行 / 可视 块 (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: 找不到模式：{} … (as this language's Vim says them) |
| Vorschlag | 建议 |
| Vorhersage | 预测 |
| Ausschlussliste | 排除列表 |
| Blame | Blame |
| Hinweise (Rotor) | 提示 |
| Schnellkorrektur | 快速修复 |
| Refactoring | 重构 |
| Quellaktion | 源代码操作 |
| Implementierung / Typdefinition / Deklaration | 实现 / 类型定义 / 声明 |
| Aufrufhierarchie / Typhierarchie | 调用层级 / 类型层级 |
| Aufrufer / Aufgerufene | 调用方 / 被调用方 |
| Obertypen / Untertypen | 超类型 / 子类型 |
| indexieren | 索引 |
| Lesezeichen / Verlauf / Adresse | 书签 / 历史记录 / 地址 |
| Bytes | 字节 |
| Exportieren | 导出 |
| Format | 格式 |
| Diktat | 听写 |
| Agent | 智能体 |
| Eigener Agent | 内置智能体 |
| Eigene Agenten | 自定义智能体 |
| Thread (AI-Thread, CLI-Thread) | 对话 (AI 对话, CLI 对话) |
| Sitzung | 会话 |
| Subagent | 子智能体 |
| Hintergrundprozess | 后台进程 |
| Unterhaltung | 对话内容 |
| Zug | 轮 |
| Antwort | 回答 |
| Rückfrage | 提问 |
| Freigabe, Freigabemodus | 权限, 权限模式 |
| Umgehen | 绕过 |
| Erlauben / Ablehnen / Immer erlauben … | 允许 / 拒绝 / 始终允许… |
| Plan | 计划 |
| To-dos | 待办事项 |
| Aufgabe | 任务 |
| Auftrag | 任务 |
| Werkzeug | 工具 |
| Skills | 技能 |
| Befehl | 命令 |
| Hook | 钩子 |
| Sandbox | 沙盒 |
| Fremder Inhalt | 外部内容 |
| Sicherungspunkt | 检查点 |
| Zurückspulen | 回退 |
| Verdichten | 压缩 |
| Kontext, Kontextfenster | 上下文, 上下文窗口 |
| Denktiefe | 思考深度 |
| Modell / Anbieter / Endpunkt | 模型 / 提供商 / 端点 |
| Token, Tokens | Token |
| API-Schlüssel | API 密钥 |
| Register | 注册表 |
| Adapter | 适配器 |
| Debug-Adapter | 调试适配器 |
| Bereit | 就绪 |
| Prüfsumme | 校验和 |
| Anmeldung, anmelden | 登录 |
| Konto | 账户 |
| Abo | 订阅 |
| Repository | 仓库 |
| Commit, committen | 提交 |
| Secrets | 机密信息 |
| vormerken | 暂存 |
| Zurücknehmen | 取消暂存 |
| nicht versioniert | 未跟踪 |
| Abschnitt (Git) | 变更块 |
| Branch / Remote | 分支 / 远程仓库 |
| Push / Pull / Fetch / Rebase | 推送 / 拉取 / 抓取 / 变基 |
| Worktree | 工作树 |
| Stash | 储藏 |
| Übernehmen (Worktree) | 并入 |
| Arbeitskopie | 工作区 |
| Lauf, Läufe | 运行 |
| Host | 主机 |
| Job | 作业 |
| Log | 日志 |
| **Debugger** | |
| Debuggen | 调试 |
| Haltepunkt, Haltepunkte | 断点 |
| Logpunkt | 日志点 |
| Bedingung / Trefferzahl | 条件 / 命中次数 |
| Aufrufliste | 调用堆栈 |
| Frame | 栈帧 |
| Überwachter Ausdruck, Überwachte Ausdrücke | 监视表达式 |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | 继续 / 单步跳过 / 单步进入 / 单步跳出 |
| Anhängen / Trennen | 附加 / 分离 |
| An Prozess anhängen | 附加到进程 |
| Neu starten / Stopp | 重新启动 / 停止 |
| Variablen | 变量 |
| Angehalten | 已暂停 |
| Debug-Konsole | 调试控制台 |
| Startkonfiguration | 启动配置 |
| Inspektor | 检查器 |
| **Tests** | |
| Tests | 测试 |
| Test | 测试 |
| **Containers** | |
| Container | 容器 |
| Docker-Kontext | Docker 上下文 |
| **Leitstand (phase 13)** | |
| Leitstand | 控制室 |
| Auftrag (Leitstand) | 工单 |
| Nachweis | 证据 |
| Abweichung | 偏差 |
| Abnahme, abnehmen | 验收, 验收 |
| Freigeben (Spezifikation) | 批准 |
| Planer | 规划者 |
| Builder | 构建者 |
| Prüfer | 审查者 |
| Späher | 侦察者 |
| Prüfstation, Gate | 门禁 |
| Prüfbefehl | 门禁命令 |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | 接收 / 规格 / 构建 / 审查 / 验收 / 并入 / 完成 |
| Risikoklasse | 风险等级 |
| Pfadbereich | 路径范围 |
| Herkunft | 来源 |
| Station | 阶段 |
| Kriterium, Kriterien | 标准 |
| Prüfrunde | 审查轮次 |
| Befund | 发现 |
| Hersteller, Herstellerwechsel | 厂商, 更换厂商 |
| Kontingent | 配额 |
| Workflow-Datei | 工作流文件 |
| Stand | 状态 |
| Erstdurchlaufquote, Erstdurchlauf, Quote | 一次通过率, 一次通过, 通过率 |
| Kennzahlen | 指标 |
| Regelvorschlag | 规则提议 |
| wiederkehrender Auftrag | 定期工单 |
| Abstand (wiederkehrender Auftrag) | 间隔 |
| eingereiht | 排队中 |
| Rolle | 角色 |
| Einwand | 异议 |
| Rangliste | 优先级列表 |
| Prüfer-Regel | 审查者规则（必须 / 优先） |
| Vorgabe, auf Vorgabe zurücksetzen | 默认, 还原为默认 |
| Mitkopieren / Einrichtung | 一并拷贝 / 准备 |
| Prüfrelevante Dateien | 对审查重要的文件 |
| Issue | Issue |
| Merge Request | Merge Request (as the CI panel; submitting is 发起, since 提交 is commit) |
| Rechte | 权限 |
| Erlauben / Fragen / Verbieten | 允许 / 询问 / 拒绝 (Verbieten as Ablehnen) |
| Gedächtnis | 记忆 |
| Anweisungen | 指令 |
| Ausgabestil | 输出样式 |
| Auswahl | 匹配器 |
| Plugin, Plugins | 插件 |
| Marktplatz, Marktplätze | 市场 |
| Frage nebenbei | 顺便提问 |
| Header | 标头 |
| Umgebung | 环境 |
| Bericht | 报告 |
| Grenzen | 限制 |
| Verbindung | 连接 (a network connection; splat's 连线 means a diagram connector) |

### Chinese, Traditional (`zh-Hant`)
- Address: "你", as macOS Traditional Chinese (never "您"); Taiwan usage and macOS's Taiwan words (檔案, 視窗, 設定, 打開); menu
  items and buttons are bare verbs ("儲存", "隱藏工具列")
- Typography: 「…」 (『…』 inside) and full-width punctuation (，。：？！（）); menu items end in "⋯" (U+22EF) without a space, as
  macOS writes it ("設定⋯", "儲存為⋯"); a space between Chinese and Latin text or a placeholder, as splat writes it ("關於 {}",
  "AI 標籤頁"), although macOS's own strings leave it out
- Git and CI words: Chinese, as Taiwanese developers and the Traditional Chinese Git tools use them: 儲存庫, 提交, 暫存, 分支,
  遠端, 推送 / 提取 / 擷取 / 重訂基底, 合併, 衝突, 變更區塊 (hunk), 工作樹 (worktree), 工作目錄 (working tree), 掛鉤 (hook), 執行 (CI run), 作業 (job),
  記錄 (log); "Token" and "AI" stay Latin

| German key | Chinese (Traditional) |
|---|---|
| Ablage | 檔案 |
| Bearbeiten | 編輯 |
| Darstellung | 顯示方式 |
| Fenster | 視窗 |
| Hilfe | 輔助說明 |
| Dienste | 服務 |
| Über {} | 關於 {} |
| Einstellungen | 設定 (menu item: 設定⋯) |
| {} ausblenden / Andere ausblenden / Alle einblenden | 隱藏 {} / 隱藏其他 / 顯示全部 |
| {} beenden | 結束 {} |
| Widerrufen / Wiederholen | 還原 / 重做 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 剪下 / 拷貝 / 貼上 / 刪除 |
| Alles auswählen | 全選 |
| Suchen … / Weitersuchen / Rückwärts suchen | 尋找⋯ / 尋找下一個 / 尋找上一個 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 縮到最小 / 縮放 / 將此程式所有視窗移至最前 |
| Vollbildmodus | 進入全螢幕 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 隱藏工具列 / 顯示／隱藏側邊欄 |
| Sichern | 儲存 |
| Öffnen / Schließen | 打開 / 關閉 |
| Abbrechen / OK | 取消 / 好 |
| Im Finder zeigen | 顯示於 Finder |
| Papierkorb | 垃圾桶 |
| Schlüsselbund | 鑰匙圈 |
| Zwischenablage | 剪貼板 |
| Tab | 標籤頁 |
| AI-Tab | AI 標籤頁 |
| Panel | 面板 |
| Seitenleiste | 側邊欄 |
| Vorschau | 預覽 |
| Bereich | 窗格 |
| Aktivität | 活動 |
| Projekt / Ordner | 專案 / 檔案夾 |
| Vertrauen, vertrauen | 信任 |
| Platte | 磁碟 (磁碟上) |
| Unterschiede | 差異 |
| Zusammenführen | 合併 |
| Konflikt | 衝突 |
| Treffer | 結果 |
| Suche | 搜尋 |
| Ersetzen | 取代 |
| Sprachserver | 語言伺服器 |
| Hinweis (Schweregrad) | 提示 |
| Inlay-Hinweis | 內嵌提示 |
| Minimap | 縮圖 |
| Sticky Scroll | 固定捲動 |
| Diagnosen am Zeilenende | 在行尾顯示診斷 |
| Vim-Modus | Vim 模式 |
| NORMAL / EINFÜGEN / ERSETZEN / VISUELL / VISUELL ZEILE / VISUELL BLOCK | 一般 / 插入 / 取代 / 選取 / 選取 行 / 選取 區塊 (as this language's Vim shows them) |
| E486: Muster nicht gefunden: {} … (Vim's messages) | E486: 找不到 {} … (as this language's Vim says them; E748 translated, which the catalog leaves open) |
| Vorschlag | 建議 |
| Vorhersage | 預測 |
| Ausschlussliste | 排除列表 |
| Blame | Blame |
| Hinweise (Rotor) | 提示 |
| Schnellkorrektur | 快速修正 |
| Refactoring | 重構 |
| Quellaktion | 原始碼動作 |
| Implementierung / Typdefinition / Deklaration | 實作 / 類型定義 / 宣告 |
| Aufrufhierarchie / Typhierarchie | 呼叫階層 / 類型階層 |
| Aufrufer / Aufgerufene | 呼叫者 / 被呼叫者 |
| Obertypen / Untertypen | 超類型 / 子類型 |
| indexieren | 建立索引 |
| Lesezeichen / Verlauf / Adresse | 書籤 / 瀏覽記錄 / 網址 |
| Bytes | 位元組 |
| Exportieren | 輸出 |
| Format | 格式 |
| Diktat | 聽寫 |
| Agent | 代理程式 |
| Eigener Agent | 內建代理程式 |
| Eigene Agenten | 自訂代理程式 |
| Thread (AI-Thread, CLI-Thread) | 對話 (AI 對話, CLI 對話) |
| Sitzung | 工作階段 |
| Subagent | 子代理程式 |
| Hintergrundprozess | 背景程序 |
| Unterhaltung | 對話內容 |
| Zug | 輪 |
| Antwort | 回答 |
| Rückfrage | 提問 |
| Freigabe, Freigabemodus | 權限, 權限模式 |
| Umgehen | 略過權限 |
| Erlauben / Ablehnen / Immer erlauben … | 允許 / 拒絕 / 永遠允許⋯ |
| Plan | 計畫 |
| To-dos | 待辦事項 |
| Aufgabe | 任務 |
| Auftrag | 指派內容 |
| Werkzeug | 工具 |
| Skills | 技能 |
| Befehl | 指令 |
| Hook | 掛鉤 |
| Sandbox | 沙盒 |
| Fremder Inhalt | 外部內容 |
| Sicherungspunkt | 檢查點 |
| Zurückspulen | 回溯 |
| Verdichten | 壓縮 |
| Kontext, Kontextfenster | 上下文, 上下文視窗 |
| Denktiefe | 思考深度 |
| Modell / Anbieter / Endpunkt | 模型 / 供應商 / 端點 |
| Token, Tokens | Token |
| API-Schlüssel | API 金鑰 |
| Register | 登錄庫 |
| Adapter | 轉接器 |
| Debug-Adapter | 除錯轉接器 |
| Bereit | 就緒 |
| Prüfsumme | 核對碼 |
| Anmeldung, anmelden | 登入 |
| Konto | 帳號 |
| Abo | 訂閱 |
| Repository | 儲存庫 |
| Commit, committen | 提交 |
| Secrets | 機密資訊 |
| vormerken | 暫存 |
| Zurücknehmen | 取消暫存 |
| nicht versioniert | 未追蹤 |
| Abschnitt (Git) | 變更區塊 |
| Branch / Remote | 分支 / 遠端 |
| Push / Pull / Fetch / Rebase | 推送 / 提取 / 擷取 / 重訂基底 |
| Worktree | 工作樹 |
| Stash | 儲藏 |
| Übernehmen (Worktree) | 整合 |
| Arbeitskopie | 工作目錄 |
| Lauf, Läufe | 執行 |
| Host | 主機 |
| Job | 作業 |
| Log | 記錄 |
| **Debugger** | |
| Debuggen | 除錯 |
| Haltepunkt, Haltepunkte | 中斷點 |
| Logpunkt | 記錄點 |
| Bedingung / Trefferzahl | 條件 / 命中次數 |
| Aufrufliste | 呼叫堆疊 |
| Frame | 堆疊框 |
| Überwachter Ausdruck, Überwachte Ausdrücke | 監看運算式 |
| Fortsetzen / Überspringen / Hineinspringen / Heraus | 繼續 / 步過 / 步入 / 步出 |
| Anhängen / Trennen | 附加 / 分離 |
| An Prozess anhängen | 附加到行程 |
| Neu starten / Stopp | 重新啟動 / 停止 |
| Variablen | 變數 |
| Angehalten | 已暫停 |
| Debug-Konsole | 除錯主控台 |
| Startkonfiguration | 啟動設定 |
| Inspektor | 檢閱器 |
| **Tests** | |
| Tests | 測試 |
| Test | 測試 |
| **Containers** | |
| Container | 容器 |
| Docker-Kontext | Docker 上下文 |
| **Leitstand (phase 13)** | |
| Leitstand | 控制室 |
| Auftrag (Leitstand) | 工單 |
| Nachweis | 證據 |
| Abweichung | 偏差 |
| Abnahme, abnehmen | 驗收, 驗收 |
| Freigeben (Spezifikation) | 核准 |
| Planer | 規劃者 |
| Builder | 建構者 |
| Prüfer | 審查者 |
| Späher | 偵察者 |
| Prüfstation, Gate | 關卡 |
| Prüfbefehl | 關卡指令 |
| Eingang / Spezifikation / Bau / Prüfung / Abnahme / Übernahme / Erledigt | 接收 / 規格 / 建構 / 審查 / 驗收 / 整合 / 完成 |
| Risikoklasse | 風險等級 |
| Pfadbereich | 路徑範圍 |
| Herkunft | 來源 |
| Station | 階段 |
| Kriterium, Kriterien | 標準 |
| Prüfrunde | 審查輪次 |
| Befund | 發現 |
| Hersteller, Herstellerwechsel | 廠商, 更換廠商 |
| Kontingent | 配額 |
| Workflow-Datei | 工作流程檔案 |
| Stand | 狀態 |
| Erstdurchlaufquote, Erstdurchlauf, Quote | 一次通過率, 一次通過, 通過率 |
| Kennzahlen | 指標 |
| Regelvorschlag | 規則提議 |
| wiederkehrender Auftrag | 定期工單 |
| Abstand (wiederkehrender Auftrag) | 間隔 |
| eingereiht | 排隊中 |
| Rolle | 角色 |
| Einwand | 異議 |
| Rangliste | 優先順序清單 |
| Prüfer-Regel | 審查者規則（必須 / 優先） |
| Vorgabe, auf Vorgabe zurücksetzen | 預設, 回復為預設 |
| Mitkopieren / Einrichtung | 一併拷貝 / 準備 |
| Prüfrelevante Dateien | 對審查重要的檔案 |
| Issue | Issue (commenting on it: 留言) |
| Merge Request | Merge Request (as the CI panel; submitting is 發起, since 提交 is commit) |
| Rechte | 權限 |
| Erlauben / Fragen / Verbieten | 允許 / 詢問 / 拒絕 (Verbieten as Ablehnen) |
| Gedächtnis | 記憶 |
| Anweisungen | 指示 |
| Ausgabestil | 輸出樣式 |
| Auswahl | 比對器 |
| Plugin, Plugins | 外掛模組 |
| Marktplatz, Marktplätze | 市集 |
| Frage nebenbei | 順便提問 |
| Header | 標頭 |
| Umgebung | 環境 |
| Bericht | 報告 |
| Grenzen | 限制 |
