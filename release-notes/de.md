<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Dies ist die erste Ausgabe von Dango, einem nativen macOS-Editor für die Arbeit mit Coding-Agenten. Editor,
Terminals, ein Browser, Git, CI, ein Debugger und AI-Threads teilen sich ein Fenster je Ordner. Die App ist in Rust
auf AppKit geschrieben; WebKit steckt nur im Browser-Tab. Die Oberfläche gibt es in 35 Sprachen.

## Voraussetzungen
- macOS 14 oder neuer, auf Apple Silicon oder Intel.
- Für die AI-Threads der gewählte Agent: Claude braucht den Befehl `claude`, angemeldet; Codex braucht Node.js, und
  die App installiert sein festgelegtes Paket auf Nachfrage; Antigravity und OpenCode nutzen dein eigenes `agy` oder
  `opencode` oder einen festgelegten Download; der eigene Agent braucht einen API-Schlüssel oder Ollama bzw.
  LM Studio auf diesem Mac.
- Sprachserver und Debug-Adapter installiert die App auf Nachfrage. gopls und `dlv` brauchen Go; die Server für
  TypeScript, Python, JSON, YAML, CSS, HTML und Shell und der JavaScript-Debugger brauchen Node.js; clangd,
  sourcekit-lsp und `lldb-dap` kommen mit den Xcode Command Line Tools.
- Vorhersagen beim Tippen brauchen ein GitHub-Copilot-Abo (das kostenlose genügt) oder ein Fill-in-the-Middle-Modell
  in Ollama oder LM Studio.

## Fenster
- Jede Datei, jedes Terminal, jede Browserseite, jeder AI-Thread, Diff und jede Vorschau ist ein macOS-Tab im Fenster
  des Ordners. Das „+“ am Ende der Tableiste und ⌘T bieten eine neue Datei, ein Terminal, den Browser und jeden
  installierten Agenten an, auch in einem eigenen Git-Worktree.
- Die Seitenleiste wechselt zwischen Dateien, Suche, Agenten, Git, CI und Debuggen (⌘1–⌘6); ein Rechtsklick blendet
  aus, was du nicht brauchst. In der Symbolleiste stehen Zurück und Vorwärts durch die Stellen, an denen du gearbeitet
  hast (⌃- und ⌃⇧-), der Branch, Schnell öffnen und die Aktivitätsübersicht.
- „Daneben öffnen“ zeigt ein zweites Dokument neben dem ersten. Nach einem Neustart oder Absturz kommt jedes Fenster
  mit seinen Tabs zurück, Editoren mit ungesichertem Text mit diesem Text.
- Bilder, Audio, Video, Schriften und Office-Dokumente öffnen in einem Tab über Quick Look, PDFs über PDFKit.

## Editor
- Syntaxfarben, Zeilennummern, Änderungsmarken gegenüber dem letzten Commit, Suchen und Ersetzen, Schnell öffnen (⌘P)
  und Projektsuche (⇧⌘F).
- Automatische Einrückung, Tabs oder Leerzeichen nach `.editorconfig`, Klammerpaare, Kommentar umschalten, Faltung,
  Gehe zu Zeile, sichtbare Leerzeichen und Zeilenumbruch.
- Mehrere Einfügemarken: ⌥-Klick, ⌥-Ziehen für eine Spalte, ⌘D für das nächste Vorkommen.
- Dateien behalten Kodierung, Byte-Order-Mark und Zeilenenden. Ändert sich eine Datei auf der Platte, lädt der Editor
  sie neu, solange er keinen ungesicherten Text hat; sonst bietet ein Balken die Unterschiede und das Zusammenführen an.
- **Vorhersagen beim Tippen:** Während du tippst, erscheint ein Vorschlag in grauer Schrift; Tab übernimmt ihn. Er
  kommt von GitHub Copilot oder von einem lokalen Modell in Ollama oder LM Studio, je nach Wahl in den Einstellungen;
  ein lokales Modell lässt deinen Code auf diesem Mac.

## Sprachserver
Go, Rust, TypeScript und JavaScript, Python, C, C++ und Objective-C, Swift, Shell-Skripte, JSON, YAML, CSS, HTML,
TOML, Markdown und Lua, dazu eigene Server aus den Einstellungen. Diagnosen, Hover, Vervollständigung,
Signaturhilfe, Gehe zu Definition, Typdefinition und Implementierung, Referenzen, Aufruf- und Typhierarchie,
Umbenennen, Formatieren, semantische Farben, Faltung und Gliederung; Typen und Parameternamen in der Zeile;
Schnellkorrekturen und Refactorings hinter einer Glühbirne (⌥⌘A); die Symbole des Projekts mit ⌥⌘O oder `#` in
Schnell öffnen. Kann das `tsc` eines TypeScript-Projekts als Sprachserver dienen, wird es genutzt.

## Debugger
Haltepunkte, auch mit Bedingungen, Schrittbetrieb, Threads mit ihrer Aufrufliste, Variablen und überwachte Ausdrücke
im Inspektor (⌥⌘I), Werte neben dem Code, solange das Programm angehalten ist, und eine Debug-Konsole, die im
angehaltenen Frame auswertet. Rust, C, C++ und Swift über `lldb-dap`, Go über `dlv`, JavaScript und TypeScript über
den JavaScript-Debugger, Python über debugpy. Die `.vscode/launch.json` eines Projekts wird gelesen. Das Menü
„Debuggen“ nutzt die Kürzel von Xcode.

## Terminal
Terminals in Tabs mit der Umgebung der Login-Shell, gezeichnet mit Metal. Ein eingefügtes oder hineingezogenes Bild
geht an einen Coding-Agenten, der im Terminal läuft, sonst als Pfad einer gesicherten Kopie. ⌘-Klick öffnet Links und
Dateinamen. Einen Entwicklungsserver, den das Terminal startet, kann man im Browser-Tab öffnen.

## Markdown
Eine Markdown-Datei öffnet mit ihrer Vorschau daneben (⇧⌘V), mit Formeln, Diagrammen in Mermaid, Graphviz und
PlantUML und den Bildern des Ordners. Ein Menü „Format“ für fett, kursiv, Links, Überschriften, Listen, Zitate und
Codeblöcke; Text aus Word oder einer Webseite kommt als Markdown an. **Ablage › Exportieren** schreibt PDF, eine
einzelne HTML-Datei oder ein Word-Dokument. Ist Splat installiert, öffnet **Ablage › In Splat öffnen** das Dokument
dort.

## Browser
Ein Browser-Tab mit Adressfeld, Zurück und Vor, Suche, Downloads mit Rückfrage und dem Web-Inspector.

## Git
Ein Git-Panel mit Vormerken und Verwerfen ganzer Dateien oder einzelner Abschnitte, Commits, Committen und Pushen
oder Synchronisieren, Branches, Stashes, Merge-Konflikten und einem Graph der letzten Commits. Blame zeigt, wer die
Zeile am Cursor zuletzt geändert hat, oder eine Spalte für die ganze Datei. Ein Knopf schreibt die Commit-Nachricht
per AI; der Diff geht erst hinaus, wenn gitleaks darin kein Secret gefunden hat, und jeder Commit wird ebenso auf
Secrets geprüft. Ein AI-Thread kann in einem eigenen Git-Worktree arbeiten; das Panel zeigt, was er geändert hat, und
übernimmt die Arbeit per Merge oder Cherry-Pick.

## CI
GitHub Actions und GitLab CI, auch GitHub Enterprise Server und selbst betriebenes GitLab: Läufe, Jobs und Logs des
aktuellen Branches; fehlgeschlagene Jobs, einen einzelnen Job oder (bei GitHub) einen ganzen Lauf neu starten,
manuelle Jobs starten oder wartende Deployments freigeben, eine neue Pipeline oder einen von Hand startbaren Workflow
starten und abbrechen. Die offenen Merge Requests bzw. Pull Requests und Issues stehen im selben Panel und öffnen im
Browser-Tab. Das Log eines fehlgeschlagenen Jobs lässt sich einem Agenten übergeben, gekennzeichnet als fremder Text.
Das Token kommt aus dem Schlüsselbund, von `gh` oder `glab`, aus der Umgebung der Login-Shell oder wird eingegeben.

## AI
Jeder Agent arbeitet in einem AI-Tab mit derselben Zeitleiste: Antworten, Werkzeugaufrufe mit ihrer Ausgabe,
Freigabe-Anfragen, Rückfragen, Pläne zum Annehmen und Kommentieren, To-dos, Subagenten und Hintergrundaufgaben; zwei
Knöpfe im Kopf des Tabs listen die laufenden Subagenten und Hintergrundprozesse. Vor jedem Prompt legt die App einen
Sicherungspunkt an; Änderungen lassen sich je Nachricht prüfen und zurückspulen. Neue Threads beginnen im Modus
„Manuell“, zerstörerische Git-Befehle werden nie dauerhaft erlaubt. Die Aktivitätsübersicht zeigt alles, was läuft.
Ein grüner Punkt am Tab zeigt eine arbeitende Sitzung, ein roter eine, die auf dich wartet, und eine Mitteilung
meldet, wenn eine Sitzung fertig ist oder wartet, während du woanders bist.
- **Claude** über den unveränderten Befehl `claude`, mit Sitzungen, Subagenten, Hintergrundaufgaben, Kontext und
  Nutzung.
- **ACP-Agenten:** Codex, Antigravity und OpenCode, dazu weitere aus dem ACP-Register auf Nachfrage; installierte
  Agenten sind auf erprobte Versionen festgelegt und gegen ihre Prüfsummen geprüft.
- **Eigener Agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio und Server mit der API von OpenAI
  oder Anthropic, mit Werkzeugen für Dateien, Suche, Befehle, Diagnosen und Webabrufe; Schlüssel bleiben im
  Schlüsselbund.
- Ein Sandbox-Schalter je Thread begrenzt, was die Befehle eines Agenten dürfen, soweit der Agent das unterstützt.
- Fremder Text, etwa Terminalausgabe, eine Webseite oder ein CI-Log, geht als nicht vertrauenswürdig gekennzeichnet an
  einen Agenten.

## Diktat
Diktat in das Nachrichtenfeld der AI-Tabs und in den Editor über Lowspeaks Spracherkennung: der Mikrofon-Knopf, ⌃⌥D
oder die fn-Taste, solange Dango die aktive App ist. Es nutzt Lowspeaks Einstellungen und Modelle; die
Online-Transkription braucht einen Schlüssel für OpenAI oder Groq.

## Updates
Die App aktualisiert sich über Sparkle. Beim zweiten Start fragt sie, ob sie selbst nach Updates suchen darf; bis du
zustimmst, stellt sie keine Anfrage. **Dango › Nach Updates suchen …** sucht sofort.

## Barrierefreiheit
VoiceOver-Namen für Seitenleiste, Tabs, die Karten der AI-Zeitleiste, Git- und CI-Zeilen, den Debugger und den Text
des Terminals; volle Bedienung per Tastatur mit Kürzeln, die sich in `keybindings.json` ändern lassen; Erhöhter
Kontrast und Bewegung reduzieren; Code-Ansichten laufen auch in einer Oberfläche mit Rechts-nach-links-Schrift von
links nach rechts. Einzelheiten stehen in ACCESSIBILITY.md.

## Datenschutz
Keine Telemetrie, keine Analyse, keine Absturzberichte. Die App verbindet sich nur für das, was du startest: die
Agenten, die du nutzt, die Downloads, um die du bittest, die CI-Hosts, bei denen du dich anmeldest, Vorhersagen von
Copilot, wenn du sie gewählt hast, die Online-Transkription, wenn du sie gewählt hast, die Seiten, die du öffnest,
und die Suche nach Updates, sobald du sie erlaubt hast. Schlüssel und Tokens bleiben im macOS-Schlüsselbund.

## Bekannte Grenzen
- Die Anmeldung bei CI über den Browser ist aus; dafür `gh`, `glab`, ein Token oder die Umgebung nutzen.
- Die App läuft ohne App Sandbox, weil Terminals, Git, Sprachserver und Agenten sie erben würden; was ein Ordner
  einrichtet, läuft erst, wenn du dem Ordner vertraust. Antigravity und OpenCode haben keine Sandbox, die die App
  einschalten kann.
- Nutzungszahlen und Kontext-Aufschlüsselung von Claude beruhen auf Teilen seines Protokolls, die als experimentell
  gekennzeichnet sind, und fehlen, wenn diese fehlen. Die Unterhaltung zurückspulen geht nur bei Claude; bei den
  anderen Agenten wird nur der Code zurückgespult.
- Die Gemini CLI wird nicht angeboten: Google hat ihre Anmeldung mit einem privaten Konto beendet und verweist auf
  Antigravity. Frühere Gemini-Threads bleiben lesbar.
