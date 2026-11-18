Dies ist die erste Ausgabe von Dango, einem nativen macOS-Editor für die Arbeit mit Coding-Agenten. Editor,
Terminals, ein Browser, Git, CI, ein Debugger, AI-Threads und eine Software Factory für Aufträge teilen sich ein
Fenster je Ordner. Die App ist in Rust auf AppKit geschrieben; WebKit steckt nur im Browser-Tab. Die Oberfläche gibt es
in 35 Sprachen.

## Voraussetzungen
- macOS 14 oder neuer, auf Apple Silicon oder Intel.
- Für die AI-Threads der gewählte Agent: Claude braucht den Befehl `claude`, angemeldet; Codex braucht Node.js, und
  die App installiert sein festgelegtes Paket auf Nachfrage; Antigravity und OpenCode nutzen dein eigenes `agy` oder
  `opencode` oder einen festgelegten Download; der eigene Agent braucht einen API-Schlüssel oder Ollama bzw.
  LM Studio auf diesem Mac.
- Sprachserver und Debug-Adapter installiert die App auf Nachfrage. gopls und `dlv` brauchen Go; die Server für
  TypeScript, Python, JSON, YAML, CSS, HTML, ESLint und Shell und der JavaScript-Debugger brauchen Node.js; clangd,
  sourcekit-lsp und `lldb-dap` kommen mit den Xcode Command Line Tools.
- Vorhersagen beim Tippen brauchen ein GitHub-Copilot-Abo (das kostenlose genügt) oder ein Fill-in-the-Middle-Modell
  in Ollama oder LM Studio.

## Fenster
- Jede Datei, jedes Terminal, jede Browserseite, jeder AI-Thread, Diff und jede Vorschau ist ein macOS-Tab im Fenster
  des Ordners. Das „+“ am Ende der Tableiste und ⌘T bieten eine neue Datei, ein Terminal, den Browser und jeden
  installierten Agenten an, auch in einem eigenen Git-Worktree.
- Die Seitenleiste wechselt zwischen Dateien, Suche, Agenten, Git, Debuggen, CI, Tests, Container und der Software
  Factory; ein Rechtsklick blendet aus, was du nicht brauchst. In der Symbolleiste stehen Zurück und Vorwärts durch die
  Stellen, an denen du gearbeitet hast (⌃- und ⌃⇧-), der Branch, Schnell öffnen und die Aktivitätsübersicht.
- Ein Fenster teilt sich in Bereiche nebeneinander oder übereinander, beliebig tief verschachtelt, wie in Xcode; jeder
  Bereich hält seine Dokumente hintereinander, mit einer eigenen Tableiste. „Daneben öffnen“ zeigt ein zweites Dokument
  neben dem ersten.
- Nach einem Neustart oder Absturz kommt jedes Fenster mit seinen Tabs und Bereichen zurück, Editoren mit ungesichertem
  Text mit diesem Text.
- Bilder, Audio, Video, Schriften und Office-Dokumente öffnen in einem Tab über Quick Look, PDFs über PDFKit.

## Editor
- Syntaxfarben, Zeilennummern, Änderungsmarken gegenüber dem letzten Commit, Suchen und Ersetzen, Schnell öffnen (⌘P)
  und Projektsuche (⇧⌘F). Farben und Markdown-Stile kommen auch in Dateien mit vielen tausend Zeilen mit, ohne das
  Tippen aufzuhalten.
- Automatische Einrückung, Tabs oder Leerzeichen nach `.editorconfig`, Klammerpaare, Kommentar umschalten, Faltung,
  Gehe zu Zeile, sichtbare Leerzeichen, Zeilenumbruch, eine Minimap und angeheftete Kopfzeilen der umschließenden
  Blöcke.
- Mehrere Einfügemarken: ⌥-Klick, ⌥-Ziehen für eine Spalte, ⌘D für das nächste Vorkommen.
- **Vim-Modus:** im Menü „Editor“ einzuschalten, mit Normal-, Einfüge- und Visual-Modus, Bewegungen und Operatoren,
  Registern, Marken, Makros, Suche und Ex-Befehlen; seine Meldungen erscheinen in der Sprache der Oberfläche.
- Emmet-Kürzel in der Vervollständigung von HTML, CSS und JSX, und auf Wunsch werden die Probleme von ESLint beim
  Sichern behoben.
- Dateien behalten Kodierung, Byte-Order-Mark und Zeilenenden. Ändert sich eine Datei auf der Platte, lädt der Editor
  sie neu, solange er keinen ungesicherten Text hat; sonst bietet ein Balken die Unterschiede und das Zusammenführen an.
- **Vorhersagen beim Tippen:** Während du tippst, erscheint ein Vorschlag in grauer Schrift; Tab übernimmt ihn. Er
  kommt von GitHub Copilot oder von einem lokalen Modell in Ollama oder LM Studio, je nach Wahl in den Einstellungen;
  ein lokales Modell lässt deinen Code auf diesem Mac.

## Sprachserver
Go, Rust, TypeScript und JavaScript, Python, C, C++ und Objective-C, Swift, Shell-Skripte, JSON, YAML, CSS, HTML,
TOML, Markdown und Lua, ESLint neben dem TypeScript-Server, dazu eigene Server aus den Einstellungen. Diagnosen, auch
am Zeilenende, Hover, Vervollständigung, Signaturhilfe, Gehe zu Definition, Typdefinition und Implementierung,
Referenzen, Aufruf- und Typhierarchie, Umbenennen, Formatieren, semantische Farben, Faltung und Gliederung; Typen und
Parameternamen in der Zeile; Schnellkorrekturen und Refactorings hinter einer Glühbirne (⌥⌘A); die Symbole des
Projekts mit ⌥⌘O oder `#` in Schnell öffnen. Kann das `tsc` eines TypeScript-Projekts als Sprachserver dienen, wird es
genutzt.

## Debugger
Haltepunkte, auch mit Bedingungen, Schrittbetrieb, Threads mit ihrer Aufrufliste, Variablen und überwachte Ausdrücke
im Inspektor (⌥⌘I), Werte neben dem Code, solange das Programm angehalten ist, und eine Debug-Konsole, die im
angehaltenen Frame auswertet. Rust, C, C++ und Swift über `lldb-dap`, Go über `dlv`, JavaScript und TypeScript über
den JavaScript-Debugger, Python über debugpy. Die `.vscode/launch.json` eines Projekts wird gelesen. Das Menü
„Debuggen“ nutzt die Kürzel von Xcode.

## Tests und Container
- Das Panel „Tests“ listet die Tests von Rust-, Go-, Python-, Jest-, Vitest- und Bun-Projekten schon vor dem ersten
  Lauf, führt einen, eine Datei oder alle aus, zeigt jedes Ergebnis live und markiert es in der Randspalte des Editors.
- Das Panel „Container“ listet die Container, Images, Volumes und Compose-Projekte von Docker; es startet, stoppt und
  entfernt sie, folgt ihren Logs und öffnet eine Shell in einem Terminal-Tab.

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
meldet, wenn eine Sitzung fertig ist oder wartet, während du woanders bist. Zeitleiste, Nachrichtenfeld und
Statuszeile haben in den Einstellungen eine eigene Schriftgröße.
- **Claude** über den unveränderten Befehl `claude`, mit Sitzungen, Subagenten, Hintergrundaufgaben, Kontext, Nutzung
  und Remote Control vom Handy.
- **Codex** über seinen App-Server und **OpenCode** über seinen Server, jeweils im eigenen Protokoll, mit ihren
  Threads, Zurückspulen im Agenten selbst und Remote Control für Codex.
- **ACP-Agenten:** Antigravity, dazu weitere aus dem ACP-Register auf Nachfrage; installierte Agenten sind auf
  erprobte Versionen festgelegt und gegen ihre Prüfsummen geprüft.
- **Eigener Agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio und Server mit der API von OpenAI
  oder Anthropic, mit Werkzeugen für Dateien, Suche, Befehle, Diagnosen und Webabrufe; Schlüssel bleiben im
  Schlüsselbund.
- Die eigenen Befehle eines Agenten öffnen sich als Overlays des Tabs, für jeden Agenten gleich: Status, Export,
  MCP-Server, Rechte, Hooks, Gedächtnis, Konfiguration, Skills, Plugins, Modell, Konto und Feedback.
- Ein Sandbox-Schalter je Thread begrenzt, was die Befehle eines Agenten dürfen, soweit der Agent das unterstützt.
- Fremder Text, etwa Terminalausgabe, eine Webseite oder ein CI-Log, geht als nicht vertrauenswürdig gekennzeichnet an
  einen Agenten.

## Software Factory
Aufträge statt einzelner Prompts: ein Ziel mit Abnahmekriterien, von Hand geschrieben oder aus einem Issue übernommen.
Jeder Auftrag wird in einem eigenen Git-Worktree geplant, gebaut und geprüft, von Agenten verschiedener Hersteller in
festen Rollen: Ein Planer schreibt die Spezifikation, ein Builder erledigt die Arbeit, und ein Prüfer eines anderen
Herstellers prüft jedes Kriterium. Die Prüfbefehle des Projekts, etwa Build und Tests, laufen in einer Sandbox, und
der Builder behebt, was sie finden.
- Du entscheidest an zwei Stellen: Du gibst die Spezifikation frei, und du nimmst das Ergebnis mit seinem Nachweis ab,
  ganz oder Abschnitt für Abschnitt. Ohne deine Abnahme wird nichts übernommen.
- Panel und Tab der Software Factory zeigen jeden Auftrag mit seiner Station, seinen Kosten und dem, worauf er wartet;
  der Tab eines Auftrags zeigt seine Zeitleiste, seinen Nachweis und den nächsten Schritt und öffnet seine laufende
  Sitzung als AI-Tab.
- Budgets je Auftrag und je Tag, Grenzen für Züge, eine Pause beim Beenden, im Ruhezustand und bei schwachem Akku und
  ein Protokoll, das vor jeder Wirkung geschrieben wird, sodass Aufträge nach einem Neustart weiterlaufen.
- Issues mit einem gewählten Label können als Aufträge hereinkommen; ein abgenommener Auftrag kann als Merge Request
  mit einem Kommentar an seinem Issue hinausgehen, auf deinen Klick. Aufträge mit getrennten Pfadbereichen laufen
  nebeneinander und werden nacheinander übernommen.
- Kennzahlen je Agent und Hersteller, Routing nach der Erstdurchlaufquote, Regelvorschläge für `AGENTS.md` und
  `CLAUDE.md` aus wiederkehrenden Befunden und wiederkehrende Aufträge aus der Workflow-Datei des Projekts.
- Die Workflow-Datei `.dango/factory.toml` eines Ordners hält seine Prüfstationen, Risikoklassen und Pfadbereiche;
  geänderte Prüfbefehle und alles, was die Grenzen lockert, warten auf deine Bestätigung.

## Diktat
Diktat in das Nachrichtenfeld der AI-Tabs und in den Editor über Lowspeaks Spracherkennung: der Mikrofon-Knopf, ⌃⌥D
oder die fn-Taste, solange Dango die aktive App ist. Es nutzt Lowspeaks Einstellungen und Modelle; die
Online-Transkription braucht einen Schlüssel für OpenAI oder Groq.

## Updates
Die App aktualisiert sich über Sparkle. Beim zweiten Start fragt sie, ob sie selbst nach Updates suchen darf; bis du
zustimmst, stellt sie keine Anfrage. **Dango › Nach Updates suchen …** sucht sofort.

## Barrierefreiheit
VoiceOver-Namen für Seitenleiste, Tabs, die Karten der AI-Zeitleiste, Git- und CI-Zeilen, die Software Factory, den
Debugger und den Text des Terminals; volle Bedienung per Tastatur mit Kürzeln, die sich in `keybindings.json` ändern
lassen; Erhöhter Kontrast und Bewegung reduzieren; Code-Ansichten laufen auch in einer Oberfläche mit
Rechts-nach-links-Schrift von links nach rechts. Einzelheiten stehen in ACCESSIBILITY.md.

## Datenschutz
Keine Telemetrie, keine Analyse, keine Absturzberichte. Die App verbindet sich nur für das, was du startest: die
Agenten, die du nutzt, die Downloads, um die du bittest, die CI-Hosts, bei denen du dich anmeldest, Vorhersagen von
Copilot, wenn du sie gewählt hast, die Online-Transkription, wenn du sie gewählt hast, die Seiten, die du öffnest,
und die Suche nach Updates, sobald du sie erlaubt hast. Schlüssel und Tokens bleiben im macOS-Schlüsselbund, und die
Agenten der Software Factory sehen sie nie.

## Bekannte Grenzen
- Die Anmeldung bei CI über den Browser ist aus; dafür `gh`, `glab`, ein Token oder die Umgebung nutzen.
- Die App läuft ohne App Sandbox, weil Terminals, Git, Sprachserver und Agenten sie erben würden; was ein Ordner
  einrichtet, läuft erst, wenn du dem Ordner vertraust. Antigravity und OpenCode haben keine Sandbox, die die App
  einschalten kann.
- In der Software Factory kann ein Codex-Builder oder -Prüfer außerhalb seines Worktrees lesen, weil das Protokoll von
  Codex keine Lesegrenzen kennt; was er schreibt, bleibt darin.
- Nutzungszahlen und Kontext-Aufschlüsselung von Claude beruhen auf Teilen seines Protokolls, die als experimentell
  gekennzeichnet sind, und fehlen, wenn diese fehlen. Die Unterhaltung zurückspulen geht bei Claude, Codex und
  OpenCode; bei den anderen Agenten wird nur der Code zurückgespult.
- Die Gemini CLI wird nicht angeboten: Google hat ihre Anmeldung mit einem privaten Konto beendet und verweist auf
  Antigravity. Frühere Gemini-Threads bleiben lesbar.
