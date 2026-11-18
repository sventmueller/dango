<p><img src="../../readme-icon.png" alt="Dango-Symbol" width="147" height="147"></p>

# Dango

**Der native macOS-Editor für die Arbeit mit Coding-Agenten.**

Dango bringt den Code, die Agenten, die ihn ändern, und alles drumherum in ein Fenster je Ordner: einen Editor mit Sprachservern und Vorhersagen beim Tippen, Terminals, einen Browser, Git, CI, einen Debugger und AI-Threads mit Claude, Codex, Antigravity, OpenCode oder einem Agenten, der in die App eingebaut ist. Jeder Agent arbeitet in derselben Zeitleiste, fragt, bevor er Dateien ändert oder Befehle ausführt, und hinterlässt vor jedem Prompt einen Sicherungspunkt, sodass du Nachricht für Nachricht prüfen und zurückspulen kannst, was er getan hat. Nichts, was ein Ordner einrichtet, läuft, bevor du ihm vertraust, und fremder Text erreicht einen Agenten als nicht vertrauenswürdig gekennzeichnet. Diktiere in den Editor und ins Nachrichtenfeld, auf Wunsch mit einer Spracherkennung, die auf deinem Mac bleibt. Dango ist in Rust auf echten AppKit-Steuerelementen geschrieben, ohne Electron, spricht 35 Sprachen und hat keine Telemetrie.

[![Dango für macOS herunterladen](https://img.shields.io/badge/Download-Dango_für_macOS-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Dango.dmg herunterladen](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: signiert und von Apple notarisiert, für macOS 14 oder neuer auf Apple Silicon und Intel. Der Link ist ab der ersten Veröffentlichung verfügbar.

**Einen Fehler gefunden?** [Eröffne ein Issue](https://github.com/sventmueller/dango/issues/new) und schreib dazu, mit welchem Agenten und in welcher Art von Projekt er auftrat.

[![Rust, Edition 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 oder neuer](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon und Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit über objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 Sprachen der Oberfläche](https://img.shields.io/badge/languages-35-4C9A2A)
![Keine Telemetrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signiert mit Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lizenz: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lizenz)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
**Deutsch** ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Inhalt

- [Funktionen im Überblick](#funktionen-im-überblick)
- [Voraussetzungen](#voraussetzungen)
- [Installation](#installation)
- [Erste Schritte](#erste-schritte)
- [AI-Agenten](#ai-agenten)
  - [Die Zeitleiste](#die-zeitleiste)
  - [Freigaben und Modi](#freigaben-und-modi)
  - [Sicherungspunkte und Worktrees](#sicherungspunkte-und-worktrees)
- [Editor](#editor)
- [Sprachserver](#sprachserver)
- [Tests](#tests)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Container](#container)
- [Markdown](#markdown)
- [Diktat](#diktat)
- [Vertrauen](#vertrauen)
- [Sandbox](#sandbox)
- [Darstellung und Barrierefreiheit](#darstellung-und-barrierefreiheit)
- [Sprachen](#sprachen)
- [Tastaturkurzbefehle](#tastaturkurzbefehle)
- [Datenschutz und Sicherheit](#datenschutz-und-sicherheit)
- [Wo die Daten liegen](#wo-die-daten-liegen)
- [Bekannte Grenzen](#bekannte-grenzen)
- [Aus dem Quellcode bauen](#aus-dem-quellcode-bauen)
- [Lizenz](#lizenz)

## Funktionen im Überblick

- **Jeder Agent in einer Zeitleiste.** Claude über seinen eigenen Befehl `claude`, Codex, Antigravity und OpenCode über das Agent Client Protocol, weitere aus dem ACP-Register und ein in die App eingebauter Agent für Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama und LM Studio. Antworten, Werkzeugaufrufe, Freigabe-Anfragen, Pläne, To-dos, Subagenten und Hintergrundaufgaben sehen gleich aus, welcher Agent auch läuft.
- **Prüfen und zurückspulen.** Vor jedem Prompt legt Dango einen Sicherungspunkt an. Was ein Agent geändert hat, lässt sich je Nachricht prüfen und zurückspulen, und ein Thread kann in einem eigenen Git-Worktree arbeiten und seine Arbeit per Merge oder Cherry-Pick übergeben.
- **Ein Fenster je Ordner.** Dateien, Terminals, Browserseiten, AI-Threads, Diffs und Vorschauen sind macOS-Tabs dieses Fensters. Die Seitenleiste enthält Dateien, Suche, Agenten, Git, Tests, Debuggen, CI und Container.
- **Ein echter Code-Editor.** Syntaxfarben, mehrere Einfügemarken, Faltung, `.editorconfig`, Sprachserver für 17 Sprachen mit Vervollständigung, Gehe zu Definition, Referenzen, Umbenennen, Schnellkorrekturen und Hierarchien, dazu Vorhersagen beim Tippen von GitHub Copilot oder einem lokalen Modell.
- **Debugger, Git und CI eingebaut.** Haltepunkte und Schrittbetrieb für Rust, C, C++, Swift, Go, JavaScript, TypeScript und Python; einzelne Abschnitte vormerken, Branches, Stashes, Konflikte und ein Commit-Graph; Läufe, Jobs und Logs aus GitHub Actions und GitLab CI.
- **Vorsichtig mit deinen Ordnern.** Ein Ordner, dem du nicht vertraust, startet nichts: keine Git-Prozesse, Sprachserver, Hooks oder Agenten. Zerstörerische Git-Befehle werden nie dauerhaft erlaubt, und Commits werden auf Secrets geprüft.
- **Diktat.** Sprich in den Editor und ins Nachrichtenfeld, online über OpenAI oder Groq oder lokal mit Whisper und Parakeet.
- **Durch und durch nativ.** AppKit-Fenster, -Tabs, -Menüs und -Symbolleisten, TextKit 2 für den Editor, Metal für das Terminal, WebKit nur im Browser-Tab. Dunkelmodus, Erhöhter Kontrast und VoiceOver funktionieren wie in jeder Mac-App.
- **In deiner Sprache.** Die Oberfläche gibt es in 35 Sprachen: Arabisch, Katalanisch, Chinesisch (vereinfacht), Chinesisch (traditionell), Kroatisch, Tschechisch, Dänisch, Niederländisch, Englisch, Finnisch, Französisch, Deutsch, Griechisch, Hebräisch, Hindi, Ungarisch, Indonesisch, Italienisch, Japanisch, Koreanisch, Malaiisch, Norwegisch, Polnisch, Portugiesisch (Brasilien), Portugiesisch (Portugal), Rumänisch, Russisch, Slowakisch, Slowenisch, Spanisch, Schwedisch, Thai, Türkisch, Ukrainisch und Vietnamesisch.

## Voraussetzungen

- macOS 14 Sonoma oder neuer, auf Apple Silicon oder Intel. Dango wird auf macOS 27 entwickelt und getestet.
- Für die AI-Threads der Agent, den du nutzen willst; siehe [AI-Agenten](#ai-agenten).
- Für Sprachserver und Debug-Adapter, die Dango auf Nachfrage installiert: Go für gopls und `dlv`; Node.js für die Server für TypeScript, Python, JSON, YAML, CSS, HTML und Shell und den JavaScript-Debugger; die Xcode Command Line Tools für clangd, sourcekit-lsp und `lldb-dap`. rust-analyzer wird aus seinen GitHub-Releases geladen.
- Für Vorhersagen beim Tippen ein GitHub-Copilot-Abo (das kostenlose genügt) oder ein Fill-in-the-Middle-Modell in Ollama oder LM Studio.

## Installation

**Vom Disk-Image.** Jede Ausgabe ist ein Disk-Image, `Dango-<version>.dmg`, mit einer universellen App für Apple Silicon und Intel, mit einer Developer ID signiert und von Apple notarisiert. Öffne es und ziehe Dango auf den Ordner „Programme“. Ein Homebrew-Cask installiert dasselbe Image. Veröffentlicht ist noch keine Ausgabe.

**Updates.** Dango vom Disk-Image hält sich mit [Sparkle](https://sparkle-project.org) selbst aktuell. Beim zweiten Start fragt es, ob es selbst nach Updates suchen darf; bis du zustimmst, stellt es keine Anfrage. **Dango › Nach Updates suchen …** sucht sofort. Updates sind mit dem eigenen Schlüssel der App signiert, den Sparkle prüft, bevor es etwas installiert. Ein selbst gebautes Dango hat keine Update-Funktion.

**Aus dem Quellcode.** Siehe [Aus dem Quellcode bauen](#aus-dem-quellcode-bauen) und bewege dann `target/release/Dango.app` nach `/Applications`.

**Der Terminal-Befehl.** **Dango › Befehl „dango“ installieren …** schreibt ein kleines Skript nach `~/.local/bin/dango`, ohne Administratorrechte; `~/.local/bin` muss in deinem `PATH` stehen. Danach öffnet `dango .` den aktuellen Ordner und `dango file.rs` eine Datei. Dango ersetzt dort nie eine Datei, die es nicht selbst geschrieben hat.

## Erste Schritte

1. **Öffne einen Ordner** mit ⌘O oder `dango <folder>`. Beim ersten Mal fragt Dango, ob du ihm vertraust (siehe [Vertrauen](#vertrauen)).
2. **Die Seitenleiste** wechselt zwischen Dateien (⌘1), Suche (⌘2), Agenten (⌘3), Git (⌘4), Tests (⌘5), Debuggen (⌘6), CI (⌘7) und Container (⌘8); ein Rechtsklick blendet aus, was du nicht brauchst, und ⌃⌘S klappt die Seitenleiste weg.
3. **Jedes Dokument ist ein Tab** im Fenster des Ordners, mit den eigenen Tabs von macOS. Das „+“ am Ende der Tableiste und ⌘T bieten eine neue Datei, ein Terminal, den Browser und jeden installierten Agenten an, auch in einem eigenen Git-Worktree; hältst du das „+“ gedrückt, erscheint dasselbe als Menü.
4. **Schnell öffnen** mit ⌘P findet Dateien und mit `#` die Symbole des Projekts; ⇧⌘F sucht und ersetzt im Projekt.
5. **Sprich mit einem Agenten** in einem AI-Tab. ⌘Esc bringt den Tastaturfokus in sein Nachrichtenfeld und zurück.
6. **Einstellungen** mit ⌘, enthalten die Tabs Allgemein, CI, Agenten, Eigener Agent, Sprachserver, Diktat und Vorhersage.

In der Symbolleiste stehen Zurück und Vorwärts durch die Stellen, an denen du gearbeitet hast (⌃- und ⌃⇧-), der Branch, Schnell öffnen und die Aktivitätsübersicht, die alles zeigt, was läuft. **Daneben öffnen** im Kontextmenü einer Datei zeigt ein zweites Dokument neben dem ersten. Nach einem Neustart oder Absturz kommt jedes Fenster mit seinen Tabs zurück, Editoren mit ungesichertem Text mit diesem Text. Bilder, Audio, Video, Schriften und Office-Dokumente öffnen in einem Tab über Quick Look, PDFs über PDFKit.

## AI-Agenten

| Agent | Wie er läuft | Was er braucht |
|---|---|---|
| Claude | der Befehl `claude`, direkt über sein stream-json-Protokoll angesprochen | `claude` installiert und angemeldet; Dango sucht im `PATH` der Login-Shell, in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` und `/usr/local/bin`. Die Anmeldung geschieht nur in `claude` selbst |
| Codex | `codex-acp` über das Agent Client Protocol (ACP) | Node.js und npm; Dango installiert das festgelegte Paket auf Nachfrage |
| Antigravity | `agy acp` | dein eigenes `agy` im `PATH` oder der festgelegte Download, installiert auf Nachfrage |
| OpenCode | `opencode acp` | dein eigenes `opencode` im `PATH` oder der festgelegte Download, installiert auf Nachfrage |
| Weitere ACP-Agenten | aus dem ACP-Register, das nur auf Nachfrage geladen wird | was der Eintrag im Register nennt; Einträge ohne Prüfsumme installieren erst, nachdem du bestätigt hast |
| Eigener Agent | in die App eingebaut, mit Werkzeugen für Dateien, Suche, Befehle, Diagnosen und Webabrufe | ein API-Schlüssel für Anthropic, OpenAI, Google Gemini oder OpenRouter, im Schlüsselbund aufbewahrt; Ollama oder LM Studio auf diesem Mac ohne Schlüssel; jeder Server mit der API von OpenAI oder Anthropic |

**Einstellungen › Agenten** zeigt, welche Agenten dieser Mac hat, installiert die anderen und ihre Updates und öffnet das ACP-Register. Agenten, die Dango installiert, sind auf erprobte Versionen festgelegt und gegen ihre veröffentlichten Prüfsummen geprüft (npm integrity oder SHA-256); Dango führt keine unversionierten `npx`-Pakete aus.

### Die Zeitleiste

Jeder Agent arbeitet in einem AI-Tab mit derselben Zeitleiste: Antworten, Werkzeugaufrufe mit ihrer Ausgabe, Freigabe-Anfragen, Rückfragen, Pläne zum Annehmen und Kommentieren (⌥⌘K), To-dos, Subagenten und Hintergrundaufgaben. Zwei Knöpfe im Kopf des Tabs listen die laufenden Subagenten und Hintergrundprozesse. Erwähne eine Datei, `@terminal` oder `@browser`, um dem Agenten ihren Inhalt mitzugeben. Ein grüner Punkt am Tab zeigt eine arbeitende Sitzung, ein roter eine, die auf dich wartet, und eine Mitteilung meldet, wenn eine Sitzung fertig ist oder wartet, während du woanders bist. Bei Claude zeigt die Statuszeile den Kontext und die Nutzung.

### Freigaben und Modi

Neue Threads beginnen im Modus „Manuell“: Der Agent fragt, bevor er Dateien ändert oder Befehle ausführt. Für zerstörerische Git-Befehle (`push --force`, `reset --hard`, `clean -fd`) bietet Dango kein „Immer erlauben“ an. Der eigene Agent hält seine „Immer erlauben“-Regeln je Projekt, listet sie unter **Einstellungen › Eigener Agent** und fragt bei zerstörerischen Git-Befehlen jedes Mal, was auch gespeichert ist; Claude und die ACP-Agenten halten solche Regeln in ihrer eigenen Konfiguration. Mit „Alles erlauben“ und ohne Sandbox zeigt der Thread eine dauerhafte Warnung.

### Sicherungspunkte und Worktrees

Vor jedem Prompt legt Dango einen Sicherungspunkt an, als Commit unter `refs/ide-checkpoints/` im Repository des Projekts oder in einem eigenen Ordner für ein Projekt ohne Git; ein normaler Push oder Fetch nimmt sie nie mit. Was ein Agent geändert hat, lässt sich je Nachricht prüfen und zurückspulen. Ein Thread kann in einem eigenen Git-Worktree arbeiten: Das Git-Panel zeigt, was er geändert hat, und übernimmt die Arbeit per Merge oder Cherry-Pick.

## Editor

- **Syntaxfarben, Zeilennummern und Änderungsmarken** gegenüber dem letzten Commit, Suchen und Ersetzen, Schnell öffnen (⌘P) und Projektsuche (⇧⌘F).
- **Die Einrückung** folgt der Datei und `.editorconfig`: Tabs oder Leerzeichen, automatische Einrückung, Klammerpaare.
- **Mehrere Einfügemarken:** ⌥-Klick, ⌥-Ziehen für eine Spalte, ⌃⇧↑ und ⌃⇧↓, ⌘D für das nächste Vorkommen.
- **Faltung** (⌥⌘← und ⌥⌘→, mit ⇧ für alles), Kommentar umschalten (⌘/), Gehe zu Zeile (⌃G), sichtbare Leerzeichen und Zeilenumbruch (⌥Z).
- **Dateien behalten ihre Kodierung**, Byte-Order-Mark und Zeilenenden. Ändert sich eine Datei auf der Platte, lädt der Editor sie neu, solange er keinen ungesicherten Text hat; sonst bietet ein Balken die Unterschiede und das Zusammenführen an.
- **Vorhersagen beim Tippen.** Während du tippst, erscheint ein Vorschlag in grauer Schrift, und Tab übernimmt ihn. Er kommt von GitHub Copilot oder von einem lokalen Modell in Ollama oder LM Studio, je nach Wahl unter **Einstellungen › Vorhersage**; ein lokales Modell lässt deinen Code auf diesem Mac.
- **Minimap und Sticky Scroll** (Menü Editor): ein schmales Bild der ganzen Datei neben dem Text, und die Kopfzeilen der Blöcke, in denen die oberste Zeile steht, über ihr festgehalten.
- **Diagnosen am Zeilenende** (Menü Darstellung): die schwerste Meldung jeder Zeile nach einer Sekunde Ruhe, mit einem Symbol für ihren Schweregrad.
- **Live-Vorschau statischer Seiten:** Editor › Im Browser-Tab öffnen liefert die Seiten eines vertrauten Ordners aus Dango selbst aus und lädt den Tab neu, wenn eine Datei gesichert wird.
- **Prettier** formatiert JavaScript, TypeScript, CSS, HTML, Markdown und mehr, wo das Projekt es einrichtet, mit dem eigenen `node_modules/.bin/prettier` des Projekts; die Einstellung `formatter` wählt den Formatierer je Sprache.
- **Vim-Modus** (Editor › Vim-Modus): Normal-, Einfüge-, Visual- und Ersetzen-Modus, Zähler, Operatoren mit Bewegungen und Textobjekten, `.`, Register, Marken und Makros. `u` und ⌃R nutzen das Widerrufen des Editors, und ein Einfügedurchgang ist ein Schritt. `"+` und `"*` sind die Zwischenablage; die Einstellung `vimClipboard` macht auch das unbenannte Register zur Zwischenablage. `/`, `?`, `n`, `N`, `*`, `:s` und `:%s` suchen mit den regulären Ausdrücken von Rust, nicht mit Vims Mustern: kein `\v`, und `\b` statt `\<` und `\>`. Ex-Befehle sind `:w`, `:q`, `:wq`, `:x`, `:noh` und `:<line>`. Im Normal-Modus nimmt Vim ⌃G dem Gehe zu Zeile weg.
- **Abgleich der Einstellungen** über einen Ordner in iCloud Drive (**Einstellungen › Allgemein**): Deine Einstellungen und Tastaturkurzbefehle folgen dir auf deine anderen Macs; Pfade, lokale Adressen und Einwilligungen bleiben auf jedem für sich.

## Sprachserver

Go, Rust, TypeScript und JavaScript, Python, C, C++ und Objective-C, Swift, Shell-Skripte, JSON, YAML, CSS, HTML, TOML, Markdown und Lua, dazu eigene Server unter **Einstellungen › Sprachserver**. Dango installiert einen Server auf Nachfrage in seinen eigenen Werkzeugordner und nutzt das `tsc` eines TypeScript-Projekts, wenn es als Sprachserver dienen kann.

Was sie bringen: Diagnosen (⌘' und ⇧⌘' springen zwischen ihnen), Hover (⌃⌘I), Vervollständigung (⌥Esc), Signaturhilfe, Gehe zu Definition (⌃⌘J), Typdefinition und Implementierung, Referenzen (⌃⇧⌘F), Aufruf- und Typhierarchie (⌃⇧⌘H), Umbenennen (⌃⌘E), Formatieren (⌥⇧F), semantische Farben, Faltung und die Gliederung (⇧⌘O); Typen und Parameternamen in der Zeile; Schnellkorrekturen und Refactorings hinter einer Glühbirne (⌥⌘A); die Symbole des Projekts mit ⌥⌘O.

Neben dem eigenen Server einer Datei prüft ESLint das JavaScript und TypeScript von Projekten, die es nutzen, mit seinen Schnellkorrekturen, und die Kürzel von Emmet erscheinen in der Vervollständigung für HTML, CSS und JSX. Dango installiert beide auf Nachfrage; unter **Einstellungen › Allgemein** behebt es die Probleme von ESLint beim Sichern, wenn du willst.

## Tests

Das Panel „Tests“ listet die Tests eines Ordners nach Paket und Datei mit ihrem Zustand: `cargo test` oder cargo-nextest, Go, pytest, Vitest und Jest. Ein Symbol an jedem Test in der Spalte der Zeilennummern führt ihn aus, ⌥-Klick debuggt ihn, und ein fehlgeschlagenes Assert markiert seine Zeile. Das Menü „Debuggen“ führt den Test an der Einfügemarke aus (⌃⌥⌘U), die letzten Tests noch einmal (⌃⌥⌘G) oder alle, und debuggt den Test an der Einfügemarke. Tests laufen nur in vertrauten Ordnern, weil sie den Code des Projekts ausführen; `cargo test` listet seine Tests erst, wenn du darum bittest, denn das Auflisten baut jedes Testprogramm.

## Debugger

Haltepunkte, auch mit Bedingungen und als Logpunkte, Schrittbetrieb, Threads mit ihrer Aufrufliste, Variablen und überwachte Ausdrücke im Inspektor (⌥⌘I), Werte neben dem Code, solange das Programm angehalten ist, und eine Debug-Konsole, die im angehaltenen Frame auswertet.

| Sprache | Debug-Adapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` aus den Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | der JavaScript-Debugger, auf Node.js |
| Python | debugpy |

Die `.vscode/launch.json` eines Projekts wird gelesen. Das Menü „Debuggen“ nutzt die Kürzel von Xcode: ⌘\\ setzt einen Haltepunkt, ⌘Y schaltet alle ein oder aus, ⌃⌘Y fährt fort, F6, F7 und F8 springen über, hinein und heraus, ⌘. stoppt.

## Terminal

Terminals in Tabs (⌃\`) mit der Umgebung der Login-Shell, gezeichnet mit Metal. ⌘K leert den Bildschirm. Ein eingefügtes oder hineingezogenes Bild geht an einen Coding-Agenten, der im Terminal läuft, sonst als Pfad einer gesicherten Kopie. ⌘-Klick öffnet Links und Dateinamen. Einen Entwicklungsserver, den das Terminal startet, kannst du im Browser-Tab öffnen.

## Browser

Ein Browser-Tab mit Adressfeld (⌘L), Zurück und Vor (⌘[ und ⌘]), Neu laden (⌘R), Suche, Downloads mit Rückfrage und dem Web-Inspector. `@browser` gibt die Seite einem Agenten mit, gekennzeichnet als fremder Text.

## Git

Das Git-Panel merkt ganze Dateien oder einzelne Abschnitte vor und verwirft sie, committet, committet und pusht oder synchronisiert, wechselt und erstellt Branches, hält Stashes, löst Merge-Konflikte und zeichnet einen Graph der letzten Commits. Blame zeigt, wer die Zeile am Cursor zuletzt geändert hat, oder eine Spalte für die ganze Datei. Ein Knopf schreibt die Commit-Nachricht per AI; der Diff geht erst hinaus, wenn gitleaks darin kein Secret gefunden hat, und jeder Commit wird ebenso auf Secrets geprüft.

## CI

GitHub Actions und GitLab CI, auch GitHub Enterprise Server und selbst betriebenes GitLab:

- Läufe, Jobs und Logs des aktuellen Branches;
- fehlgeschlagene Jobs, einen einzelnen Job oder, bei GitHub, einen ganzen Lauf neu starten; manuelle Jobs starten oder wartende Deployments freigeben; eine neue Pipeline oder einen von Hand startbaren Workflow starten; abbrechen;
- die offenen Pull Requests bzw. Merge Requests und die Issues, die im Browser-Tab öffnen;
- das Log eines fehlgeschlagenen Jobs, einem Agenten übergeben, gekennzeichnet als fremder Text.

Das Token kommt aus dem Schlüsselbund, von `gh` oder `glab`, aus der Umgebung der Login-Shell oder ist eines, das du unter **Einstellungen › CI** eingibst.

## Container

Das Panel „Container“ zeigt die Compose-Projekte von Docker mit ihren Diensten, einzelne Container, Images und Volumes und folgt den Ereignissen von Docker, während sie geschehen. Eine Compose-Datei des Ordners erscheint als Projekt, schon bevor einer ihrer Container existiert. Container starten, stoppen und starten neu; ihre Logs und eine Shell öffnen sich in Terminal-Tabs, und ein veröffentlichter Port öffnet sich im Browser-Tab. Das Löschen eines Containers, eines Images oder eines Volumes und das Aufräumen ungenutzter Daten fragen vorher nach. Die Compose-Befehle up, down und pull laufen nur in vertrauten Ordnern, weil Compose baut und ausführt, was das Projekt beschreibt. Das Menü des Panels wählt den Docker-Kontext für das Fenster, ohne den deiner Shells zu ändern; dieselben Befehle stehen im Menü „Container“. Das Panel braucht die Befehlszeile `docker`, Docker 24.0.6 oder neuer.

## Markdown

- **Vorschau.** Eine Markdown-Datei öffnet mit ihrer Vorschau daneben; **Darstellung › Vorschau daneben ein-/ausblenden** (⇧⌘V) schließt und öffnet sie, und **Einstellungen › Allgemein** schaltet die automatische Vorschau ab. Die Vorschau zeichnet Formeln, Diagramme in Mermaid, Graphviz und PlantUML und die Bilder des Ordners; Bilder aus dem Netz laden nur in einem vertrauten Ordner oder wenn du sie in der Vorschau anforderst.
- **Menü „Format“:** fett (⌘B), kursiv (⌘I), durchgestrichen (⇧⌘X), Code (⌥⇧⌘C), Link (⇧⌘K), Überschriften 1–3 (⌥⌘1–⌥⌘3), Aufzählungen und nummerierte Listen, Aufgaben, Zitate und Codeblöcke. Jeder Befehl schaltet um, auf der Auswahl oder den Zeilen, und ist ein Schritt zum Widerrufen.
- **Einsetzen.** Text aus Word oder einer Webseite kommt als Markdown an, seine Bilder als Dateien neben dem Dokument; ⌥⇧⌘V setzt reinen Text ein.
- **Export.** **Ablage › Exportieren** schreibt PDF, eine einzelne HTML-Datei mit den Bildern darin oder ein Word-Dokument. Bilder aus dem Netz gehen nur aus einem vertrauten Ordner mit.
- **In Splat öffnen.** Ist [Splat](https://github.com/sventmueller/splat) installiert, öffnen **Ablage › In Splat öffnen** und die Kontextmenüs der Dateiliste und der Tabs das Dokument dort, nachdem es gesichert wurde.

## Diktat

Diktiere in das Nachrichtenfeld eines AI-Tabs oder in den Editor: mit dem Mikrofon-Knopf dort, **Bearbeiten › Diktat starten** (⌃⌥D) oder der fn-Taste (gedrückt halten zum Aufnehmen, zweimal tippen, um weiter aufzunehmen). Die Spracherkennung ist die von [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak selbst muss nicht installiert sein.

- **Einstellungen › Diktat** legt Modus und Modell fest (online mit OpenAI oder Groq, lokal mit Whisper oder Parakeet), dazu die Sprache, die Nachbearbeitung, das Mikrofon und dein eigenes Vokabular.
- **Lokale Modelle** werden von Hugging Face geladen, wenn du eines wählst, mit Fortschritt und der Möglichkeit abzubrechen. Sie landen in den Ordnern, die auch Lowspeak nutzt, sodass jede App nutzt, was die andere geladen hat. Ein lokales Modell lässt die Aufnahme auf diesem Mac.
- **Die Online-Transkription** braucht einen Schlüssel für OpenAI oder Groq, den Dango in einem eigenen Eintrag im Schlüsselbund ablegt.
- **Von Lowspeak.** Ist Lowspeak anders als vorgegeben eingestellt, bietet der Tab „Diktat“ einmal an, seine Einstellungen zu übernehmen, und auf Wunsch seine Schlüssel; macOS fragt, bevor Dango einen davon liest (wähle „Erlauben“, nicht „Immer erlauben“).
- **Berechtigungen.** macOS fragt beim ersten Diktat nach dem Mikrofon. Die fn-Taste wirkt, solange Dango die aktive App ist und Lowspeak nicht läuft; sie braucht die Berechtigung für Bedienungshilfen, nach der Dango erst nach dem ersten Diktat fragt, nie beim Start.

## Vertrauen

Ein Ordner kann Programme starten lassen, sobald darin etwas gestartet wird: Git-Hooks und Repository-Einstellungen wie `core.fsmonitor`, Sprachserver aus `node_modules`, Agenten-Hooks, `.mcp.json`. Wenn du einen Ordner zum ersten Mal öffnest, fragt Dango, ob du ihm vertraust.

- **Vertraut:** Git, Sprachserver, Debugger, Agenten und CI laufen für diesen Ordner und alles darunter.
- **Eingeschränkt:** Du kannst lesen und bearbeiten; die Änderungsmarken im Editor kommen aus dem direkten Lesen von `.git`. Nichts, was der Ordner einrichtet, wird gestartet. Die Panels für Git, CI und AI sagen das und bieten an, dem Ordner zu vertrauen.

Text, der nicht von dir kommt, kann Anweisungen tragen, die einen Agenten lenken sollen (Prompt-Injection). Die Ausgabe eines Terminals (`@terminal`), eine Browserseite (`@browser`), ein CI-Log, das einem Agenten übergeben wird, eine Datei außerhalb des Projekts und längerer Text, der aus einer Webseite eingesetzt wird, gehen deshalb gekennzeichnet mit: Der Chip sagt „Fremder Inhalt“, und der Agent bekommt den Text mit dem Hinweis, dass es nicht vertrauenswürdige Daten sind, deren Anweisungen er nicht befolgen darf. Die Webseiten des eigenen Agenten kommen mit demselben Hinweis. Eingesetzter Text gilt als Text aus einer Webseite, wenn der Browser das in der Zwischenablage vermerkt, wie Safari, Chrome und Firefox es tun; Text von anderswo und eine kurze Wendung aus wenigen Wörtern bleiben dein eigener.

## Sandbox

Dango selbst läuft ohne App Sandbox, weil jedes Programm, das es startet, sie erben würde: Terminals, Git, Sprachserver und Agenten könnten nicht arbeiten. Stattdessen hat jeder AI-Thread einen Sandbox-Schalter, der anfangs aus ist. Was er begrenzt, hängt vom Agenten ab, und der Tooltip des Schalters sagt es:

| Agent | Mit eingeschalteter Sandbox |
|---|---|
| Claude | Shell-Befehle laufen in Claudes eigener Sandbox; seine Werkzeuge für Dateien und Web nicht |
| Codex | Befehle und Änderungen schreiben nur im Projekt, ohne Netzwerk |
| Eigener Agent | Dateien, Webabrufe und Befehle bleiben im Projekt und in den erlaubten Ordnern, ohne Netzwerk (`sandbox-exec`) |
| Antigravity, OpenCode | keine Sandbox, die Dango einschalten kann |

## Darstellung und Barrierefreiheit

- **Der Dunkelmodus** folgt dem System; **Erhöhter Kontrast** und **Bewegung reduzieren** werden beachtet.
- **VoiceOver** benennt die Seitenleiste, die Tabs, die Karten der AI-Zeitleiste, die Zeilen in Git und CI, den Debugger und den Text des Terminals. Haltepunkte werden nie allein durch Farbe unterschieden.
- **Tastatur.** Jeder Befehl hat ein Kürzel, und alle lassen sich in `keybindings.json` ändern; ⌥⌘\` springt zum nächsten Panel.
- **Rechts nach links.** Auf Arabisch und Hebräisch wird das Fenster gespiegelt, während Code-Ansichten von links nach rechts laufen.

Was Dango über AppKit hinaus beiträgt, Bereich für Bereich, steht in [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Sprachen

Die Oberfläche ist in 35 Sprachen übersetzt: Arabisch, Katalanisch, Chinesisch (vereinfacht), Chinesisch (traditionell), Kroatisch, Tschechisch, Dänisch, Niederländisch, Englisch, Finnisch, Französisch, Deutsch, Griechisch, Hebräisch, Hindi, Ungarisch, Indonesisch, Italienisch, Japanisch, Koreanisch, Malaiisch, Norwegisch, Polnisch, Portugiesisch (Brasilien), Portugiesisch (Portugal), Rumänisch, Russisch, Slowakisch, Slowenisch, Spanisch, Schwedisch, Thai, Türkisch, Ukrainisch und Vietnamesisch.

Dango folgt der Sprachreihenfolge unter **Systemeinstellungen › Allgemein › Sprache & Region**; dort kannst du unter **Apps** auch eine Sprache nur für Dango wählen. Gibt es keine Übersetzung für deine Sprache, nimmt es Englisch. Die Begriffe folgen einem Glossar, [docs/glossary.md](../../../docs/glossary.md), und den Übersetzungen von Apple, sodass Dango sich liest wie der Rest deines Mac.

## Tastaturkurzbefehle

| Aktion | Kurzbefehl |
|---|---|
| Ordner öffnen, neues Fenster, Fenster schließen | ⌘O, ⇧⌘N, ⇧⌘W |
| Neue Datei, sichern, sichern unter | ⌘N, ⌘S, ⇧⌘S |
| Neuer Tab, Tab schließen, nächster und vorheriger Tab | ⌘T, ⌘W, ⇧⌘] und ⇧⌘[ |
| Schnell öffnen, im Projekt suchen | ⌘P, ⇧⌘F |
| Dateien, Suche, Agenten, Git, Tests, Debuggen, CI, Container | ⌘1 bis ⌘8 |
| Seitenleiste ein- oder ausblenden, nächstes Panel | ⌃⌘S, ⌥⌘\` |
| Zurück und vorwärts durch bearbeitete Stellen | ⌃- und ⌃⇧- |
| Kommentar umschalten, gehe zu Zeile | ⌘/, ⌃G |
| Nächstes Vorkommen, Einfügemarke darüber und darunter | ⌘D, ⌃⇧↑ und ⌃⇧↓ |
| Falten, aufklappen, alles falten, alles aufklappen | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Zeilenumbruch, Inspektor | ⌥Z, ⌥⌘I |
| Vervollständigung, Hover, gehe zu Definition | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referenzen, Aufrufhierarchie, umbenennen | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symbole in der Datei, im Projekt | ⇧⌘O, ⌥⌘O |
| Schnellkorrektur, formatieren | ⌥⌘A, ⌥⇧F |
| Nächstes und vorheriges Problem | ⌘' und ⇧⌘' |
| Markdown-Vorschau, als reinen Text einsetzen | ⇧⌘V, ⌥⇧⌘V |
| Fett, kursiv, durchgestrichen, Code, Link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Überschrift 1 bis 3 | ⌥⌘1 bis ⌥⌘3 |
| Neues Terminal, Terminal leeren | ⌃\`, ⌘K |
| Nachrichtenfeld des Agenten fokussieren, Plan kommentieren | ⌘Esc, ⌥⌘K |
| Adressfeld, neu laden, zurück, vor (Browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Diktat starten oder stoppen | ⌃⌥D |
| Haltepunkt, alle Haltepunkte ein oder aus | ⌘\\, ⌘Y |
| Test an der Einfügemarke ausführen, letzten Testlauf wiederholen | ⌃⌥⌘U, ⌃⌥⌘G |
| Fortfahren, darüber, hinein, heraus, stoppen | ⌃⌘Y, F6, F7, F8, ⌘. |
| Einstellungen | ⌘, |

`keybindings.json` im Datenordner der App (siehe [Wo die Daten liegen](#wo-die-daten-liegen)) ändert jedes davon.

## Datenschutz und Sicherheit

- **Keine Telemetrie,** keine Analyse, keine Absturzberichte, die irgendwohin gehen. Fehler gehen ins Systemprotokoll (Konsole, unter dem Namen der App).
- **Netzwerk nur für das, was du startest:** die Agenten, die du nutzt, jeder mit seinem eigenen Dienst; das ACP-Register und die Downloads von Agenten, Sprachservern, Debug-Adaptern und Diktatmodellen, wenn du darum bittest; die CI-Hosts, bei denen du dich anmeldest; Vorhersagen von Copilot, wenn du sie gewählt hast; die Anbieter des eigenen Agenten; die Online-Transkription, wenn du sie gewählt hast; die Seiten, die du im Browser-Tab öffnest; und die Suche nach Updates, sobald du sie erlaubt hast.
- **Code verlässt den Mac** nur dort, wo du einen Dienst dafür gewählt hast: einen Agenten, die Vorhersagen von Copilot, eine Commit-Nachricht per AI. Der Diff für eine Commit-Nachricht geht erst hinaus, wenn gitleaks darin kein Secret gefunden hat.
- **Schlüssel und Tokens** bleiben im macOS-Schlüsselbund, nie in einer Einstellungsdatei. Schlüssel anderer Apps, etwa die von Lowspeak, werden nur gelesen, wenn du zustimmst, und nur einmal.
- **Festgelegte Downloads.** Agenten, Sprachserver und Debug-Adapter, die Dango installiert, sind auf erprobte Versionen festgelegt und gegen ihre Prüfsummen geprüft.
- **Codesignatur** mit der Hardened Runtime; Updates installieren nur mit der eigenen Signatur der App.

## Wo die Daten liegen

`<id>` ist die Bundle-ID, `local.agentide` für die Ausgabe (`local.agentide.debug` für Debug-Builds). Sie enthält den Anzeigenamen nicht, sodass eine Umbenennung alle Daten behält.

| Ort | Was |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (die Liste des AI-Panels), `agent-threads/` (Unterhaltungen des eigenen Agenten), `layouts/` (Tabs je Projekt), `recovery/` (ungesicherte Editoren nach einem Absturz), `checkpoints/` (für Ordner ohne Git), `worktrees/`, `agents/` und `tools/` (installierte Agenten, Sprachserver und Debug-Adapter), `task-output/` |
| `~/Library/Caches/<id>/` | eingesetzte Bilder, Skripte der Shell-Integration, das ACP-Register; kann gelöscht werden |
| `~/Library/Logs/<id>/dictation/` | die Protokolldateien des Diktats |
| `~/Library/Application Support/Lowspeak` und `FluidAudio` | lokale Diktatmodelle, mit Lowspeak geteilt |
| `~/Library/WebKit/<id>/` | Cookies und Website-Daten des Browser-Tabs |
| `<project>/.dango/settings.json` | die eigenen Einstellungen des Projekts |
| das Git-Repository des Projekts | Sicherungspunkte, als Commits unter `refs/ide-checkpoints/`; ein normaler Push oder Fetch nimmt sie nie mit |
| Schlüsselbund, Dienst `<id>` | API-Schlüssel des eigenen Agenten und des Diktats, CI-Tokens |
| `~/.local/bin/dango` | der Terminal-Befehl, wenn du ihn installiert hast |

Claude hält seine eigenen Sitzungen unter `~/.claude/projects`; Dango liest sie, um eine Sitzung fortzusetzen, ändert sie aber nicht.

## Bekannte Grenzen

- **Keine App Sandbox** für die App selbst, weil Terminals, Git, Sprachserver und Agenten sie erben würden; was ein Ordner einrichtet, läuft erst, wenn du dem Ordner vertraust. Antigravity und OpenCode haben keine Sandbox, die Dango einschalten kann.
- **Nutzungszahlen und Kontext-Aufschlüsselung von Claude** beruhen auf Teilen seines Protokolls, die als experimentell gekennzeichnet sind, und fehlen, wenn diese fehlen.
- **Die Unterhaltung zurückspulen** geht nur bei Claude; bei den anderen Agenten wird nur der Code zurückgespult.
- **Die Anmeldung bei CI über den Browser** ist aus; nutze `gh`, `glab`, ein Token oder die Umgebung.
- **Die Gemini CLI wird nicht angeboten:** Google hat ihre Anmeldung mit einem privaten Konto beendet und verweist auf Antigravity. Frühere Gemini-Threads bleiben lesbar.

## Aus dem Quellcode bauen

Du brauchst:

- macOS 14 oder neuer mit den Xcode Command Line Tools (`xcode-select --install`); Xcode selbst ist nicht nötig.
- Rust, wie `rust-toolchain.toml` es nennt. Das `rust` von Homebrew baut für den Prozessor dieses Mac; der universelle Build einer Ausgabe braucht rustup mit den Zielen `aarch64-apple-darwin` und `x86_64-apple-darwin`.
- Checkouts von [splat](https://github.com/sventmueller/splat) und [lowspeak](https://github.com/sventmueller/lowspeak) neben diesem (`../splat`, `../lowspeak`), beide auf ihrem Branch `main`; der Workspace nimmt Crates aus ihnen per Pfad.
- cmake für jeden Build mit Diktat (`make run`, `make build`, `make release`), das whisper.cpp baut. `make check` baut ohne.

| Befehl | Was er tut |
|---|---|
| `make run` | baut die Debug-App (`target/debug/Dango Debug.app`) und öffnet sie |
| `make build` | baut die Release-App (`target/release/Dango.app`), signiert mit einem lokalen Zertifikat, das der erste Build im Anmelde-Schlüsselbund anlegt; lässt `VERSION` unverändert |
| `make lint` | Formatierung und clippy; Warnungen lassen ihn scheitern |
| `make check` | `lint`, dann alle Tests; braucht keinen Schlüsselbund, kein Fenster und kein Netzwerk |
| `make test` | `check`, dann den Selbsttest `probe` und die Prüfung der Barrierefreiheit, von links nach rechts und von rechts nach links, in der echten App |
| `make perf` | die Zeitbudgets in einem Release-Build; aussagekräftig nur auf einem sonst untätigen Mac |
| `make release` | ein Probelauf der Ausgabe nach `target/release-dry-run/`, signiert mit dem lokalen Zertifikat; sendet nichts irgendwohin |
| `make release-notarized` | `check`, dann die aktuelle Version nach `dist/`: Signatur mit Developer ID, Notarisierung, der Update-Feed mit dem eigenen Schlüssel der App signiert; lässt `VERSION` unverändert |
| `make deploy` | notarisiert die nächste Version auf sauberem, grün versiegeltem `main`, committet `VERSION`, lädt die Ausgabedateien hoch und pusht Code, CI-Siegel und Versions-Tag; GitHub veröffentlicht die Ausgabe automatisch |

Debug und Release sind getrennte Apps mit eigenen Einstellungen und Daten. Bauen, Testen, die Selbsttests, die Ausgabe und die Regeln, denen der Code folgt, beschreibt [docs/development.md](../../../docs/development.md), die Entscheidungen dahinter [docs/plan.md](../../../docs/plan.md). Die Versionshinweise sind [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), übersetzt neben jeder README in `assets/readme/<language>/`.

## Lizenz

Dango ist unter einer der folgenden Lizenzen lizenziert

- Apache-Lizenz, Version 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-Lizenz ([LICENSE-MIT](../../../LICENSE-MIT))

nach deiner Wahl.

Sofern du nicht ausdrücklich etwas anderes erklärst, wird jeder Beitrag, den du absichtlich zur Aufnahme in Dango einreichst, wie in der Apache-2.0-Lizenz definiert, unter den obigen Lizenzen doppelt lizenziert, ohne zusätzliche Bedingungen.
