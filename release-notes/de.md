<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Dango 0.1.0 ist die erste Ausgabe: ein nativer macOS-Editor für die Arbeit mit Coding-Agenten. Editor, Terminals,
ein Browser, Git, CI und AI-Threads teilen sich ein Fenster; die App ist in Rust auf AppKit geschrieben, WebKit steckt
nur im Browser-Tab.

## Voraussetzungen
- macOS 14 oder neuer, auf Apple Silicon oder Intel.
- Die AI-Threads brauchen den gewählten Agenten: Claude braucht den Befehl `claude`, angemeldet; Codex und Gemini CLI
  brauchen Node.js (Gemini CLI ab Version 20), und die App installiert ihre festgelegten Pakete auf Nachfrage;
  Antigravity und OpenCode nutzen dein eigenes `agy` oder `opencode` oder einen festgelegten Download; der eigene Agent
  braucht einen API-Schlüssel oder Ollama bzw. LM Studio auf diesem Mac.
- Sprachfunktionen für Go brauchen Go, für TypeScript Node.js; gopls, rust-analyzer und den TypeScript-Server
  installiert die App auf Nachfrage.

## Editor
- Syntaxfarben, Zeilennummern, Änderungsmarken gegenüber dem letzten Commit, Suchen und Ersetzen, Schnell öffnen (⌘P)
  und Projektsuche (⇧⌘F).
- Automatische Einrückung, Tabs oder Leerzeichen nach `.editorconfig`, Klammerpaare, Kommentar umschalten, Faltung,
  Gehe zu Zeile, sichtbare Leerzeichen und Zeilenumbruch.
- Mehrere Einfügemarken: ⌥-Klick, ⌥-Ziehen für eine Spalte, ⌘D für das nächste Vorkommen.
- Dateien behalten Kodierung, Byte-Order-Mark und Zeilenenden. Ändert sich eine Datei auf der Platte, lädt der Editor
  sie neu, solange er keinen ungesicherten Text hat; sonst bietet ein Balken die Unterschiede und das Zusammenführen an.

## Language Server
Go, Rust und TypeScript: Diagnosen, Hover, Vervollständigung, Signaturhilfe, Gehe zu Definition, Referenzen,
Umbenennen, Formatieren, semantische Farben, Faltung und Gliederung. Kann das `tsc` eines TypeScript-Projekts als
Language Server dienen, wird es genutzt.

## Terminal
Terminals in Tabs und Panels, mit der Umgebung der Login-Shell. Ein eingefügtes oder hineingezogenes Bild geht an einen
Coding-Agenten, der im Terminal läuft, sonst als Pfad einer gesicherten Kopie. ⌘-Klick öffnet Links und Dateinamen.
Einen Entwicklungsserver, den das Terminal startet, kann man im Browser-Tab öffnen.

## Browser
Ein Browser-Tab mit Adressfeld, Zurück und Vor, Suche, Downloads mit Rückfrage und dem Web-Inspector.

## Git
Ein Git-Panel mit Vormerken und Verwerfen ganzer Dateien oder einzelner Abschnitte, Commits, Branches, Push, Pull und
Fetch, Merge-Konflikten und einer Prüfung auf Secrets vor dem Commit. Ein AI-Thread kann in einem eigenen
Git-Worktree arbeiten; das Panel zeigt, was er geändert hat, und übernimmt die Arbeit per Merge oder Cherry-Pick.

## CI
GitHub Actions und GitLab CI, auch GitHub Enterprise Server und selbst betriebenes GitLab: Läufe, Jobs und Logs des
aktuellen Branches; fehlgeschlagene Jobs, einen einzelnen Job oder (bei GitHub) einen ganzen Lauf neu starten,
manuelle Jobs starten oder wartende Deployments freigeben, eine neue Pipeline oder einen von Hand startbaren Workflow
starten und abbrechen. Die offenen Merge Requests bzw. Pull Requests und Issues stehen im selben Panel und öffnen im
Browser. Das Log eines fehlgeschlagenen Jobs lässt sich einem Agenten übergeben, gekennzeichnet als fremder Text. Das Token kommt aus dem Schlüsselbund, von `gh` oder `glab`, aus der
Umgebung der Login-Shell oder wird eingegeben.

## AI
Jeder Agent arbeitet in einem AI-Tab mit derselben Zeitleiste: Antworten, Werkzeugaufrufe mit ihrer Ausgabe,
Freigabe-Anfragen, Rückfragen, Pläne zum Annehmen und Kommentieren, To-dos, Subagenten und Hintergrundaufgaben. Vor
jedem Prompt legt die App einen Sicherungspunkt an; Änderungen lassen sich je Nachricht prüfen und zurückspulen. Neue
Threads beginnen im Modus „Manuell“, zerstörerische Git-Befehle werden nie dauerhaft erlaubt. Eine Übersicht in der
Symbolleiste zeigt alles, was läuft, und eine Mitteilung samt Zahl am Dock-Symbol meldet, wenn eine Sitzung fertig ist
oder wartet, während du woanders bist.
- **Claude** über den unveränderten Befehl `claude`, mit Sitzungen, Subagenten, Hintergrundaufgaben, Kontext und
  Nutzung.
- **ACP-Agenten:** Codex, Gemini CLI, Antigravity und OpenCode, dazu weitere aus dem ACP-Register auf Nachfrage;
  installierte Agenten sind auf erprobte Versionen festgelegt und gegen ihre Prüfsummen geprüft.
- **Eigener Agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio und Server mit der API von OpenAI
  oder Anthropic, mit Werkzeugen für Dateien, Suche, Befehle, Diagnosen und Webabrufe; Schlüssel bleiben im
  Schlüsselbund.
- Ein Sandbox-Schalter je Thread begrenzt, was die Befehle eines Agenten dürfen, soweit der Agent das unterstützt.

## Panels und Wiederherstellung
Tabs lassen sich in Panels rechts oder unten öffnen und zwischen ihnen ziehen; das Layout bleibt je Projekt erhalten.
Nach einem Absturz kommen die Fenster mit ihren Tabs zurück, Editoren mit ungesichertem Text mit diesem Text.

## Diktat
Diktat in das Nachrichtenfeld der AI-Tabs und in den Editor über Lowspeaks Spracherkennung: der Mikrofon-Knopf, ⌃⌥D
oder die fn-Taste, solange Dango die aktive App ist. Es nutzt Lowspeaks Einstellungen und Modelle; die
Online-Transkription braucht einen Schlüssel für OpenAI oder Groq.

## Barrierefreiheit
VoiceOver-Namen für Seitenleiste, Tabs, die Karten der AI-Zeitleiste, Git- und CI-Zeilen und den Text des Terminals;
volle Bedienung per Tastatur mit Kürzeln, die sich in `keybindings.json` ändern lassen; Erhöhter Kontrast und Bewegung
reduzieren; Code-Ansichten laufen auch in einer Oberfläche mit Rechts-nach-links-Schrift von links nach rechts.
Einzelheiten stehen in ACCESSIBILITY.md.

## Datenschutz
Keine Telemetrie, keine Analyse, keine Absturzberichte. Die App verbindet sich nur für das, was du startest: die
Agenten, die du nutzt, die Downloads, um die du bittest, die CI-Hosts, bei denen du dich anmeldest, die
Online-Transkription, wenn du sie gewählt hast, und die Seiten, die du öffnest. Schlüssel und Tokens bleiben im
macOS-Schlüsselbund.

## Bekannte Grenzen
- Die Oberfläche gibt es auf Deutsch und Englisch.
- Noch keine Markdown-Vorschau; Formeln und Diagramme in AI-Antworten erscheinen als Code.
- Die Anmeldung bei CI über den Browser ist aus; dafür `gh`, `glab`, ein Token oder die Umgebung nutzen.
- Die App läuft ohne App Sandbox, weil Terminals, Git, Language Server und Agenten sie erben würden; was ein Ordner
  einrichtet, läuft erst, wenn du dem Ordner vertraust. Antigravity und OpenCode haben keine Sandbox, die die App
  einschalten kann.
- Nutzungszahlen und Kontext-Aufschlüsselung von Claude beruhen auf Teilen seines Protokolls, die als experimentell
  gekennzeichnet sind, und fehlen, wenn diese fehlen. Die Unterhaltung zurückspulen geht nur bei Claude; bei den
  anderen Agenten wird nur der Code zurückgespult.
- Lücken bei der Barrierefreiheit: In einer Oberfläche von rechts nach links bleibt das „+“ der Tableiste am rechten
  Rand; die Aktivitätsübersicht verliert jede Sekunde den Tastaturfokus; die Leertaste wirkt nicht auf einer
  Abschnittszeile der Git-Liste; einen Plan kommentieren geht nur mit der Maus.
