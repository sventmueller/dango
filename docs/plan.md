# Dango — Plan

## Context
Spezialisierter Editor für Vibecoder. Vorgaben: 100 % Rust auf nativem AppKit (wie splat/lowspeak), AppKit statt
WebKit wo immer möglich, linke Seitenleiste im VS-Code-Stil (Symbolspalte: Dateien, AI, Git, CI), rechts Tabs (Editor,
Terminal, Vorschau, AI, Browser; neuer Tab per „+“ mit Auswahl der Art). AI-Coder optisch und in der Detailtiefe wie
das Claude-Code-Plugin (Screenshots 1 und 2) inklusive Sicht auf laufende Agenten und Hintergrundprozesse, technisch wie
Zed mit mehreren Agenten und Modellen. HIG-konform, 35 Sprachen wie splat, Terminal nimmt Bilder aus der Zwischenablage
für Claude/Codex/Gemini/Antigravity an, LSP für Go/TypeScript/Rust, splat und lowspeak als Abhängigkeiten, Produktname
später änderbar. `/Users/Sven/Repos/vibeedit` ist leer. Größenordnung: deutlich über splat (dort ~34 000 Zeilen App +
~72 000 Zeilen Crates) — ein Projekt über viele Sitzungen, Phase für Phase lauffähig. Reviews von Codex und Fable sind
eingearbeitet (markiert mit *Review*).

## Entscheidungen

### Grundlagen
- **E1 AppKit durchgehend** *(bestätigt, mit Diagrammen/Formeln)*. Editor, AI-Antworten und Markdown-Vorschau nativ in
  TextKit. WebKit an zwei Stellen: sichtbar im Browser-Tab (E17), unsichtbar als Renderer, der Mermaid/Graphviz/
  PlantUML/LaTeX zu Bildern macht (splats `src/editor/js.rs` + `resources/js`, ~14 MB).
- **E2 Version 1 ohne App Sandbox** *(bestätigt)*: Developer ID, Hardened Runtime, notarisiertes DMG, Sparkle,
  zusätzlich Homebrew Cask. Kindprozesse erben einen Sandbox; Terminal, Agenten, git und LSP wären damit unbrauchbar.
- **E3 Seitenleiste VS-Code-Stil** *(deine Wahl)*: senkrechte Symbolspalte (`NSButton`s, SF Symbols, Badges), Panel im
  Sidebar-Split-Item (Systemmaterial, ab macOS 26 Liquid Glass). Sonst HIG: Unified Toolbar, Systemfarben, Dark Mode,
  Erhöhter Kontrast, Rechts-nach-links, VoiceOver, volle Tastaturbedienung, Standardmenüs, Einstellungen (⌘,).
  *Review:* Code-Ansichten (Editor, Terminal, Diff, Zeilennummern) bleiben auch bei arabischer/hebräischer Oberfläche
  links-nach-rechts.
- **E14 Abhängigkeiten als Pfad** *(bestätigt)* `../splat`, `../lowspeak`, dazu `[patch.crates-io] comrak` aus
  `../splat/vendor` und lowspeaks Profile (`opt-level 3` für Abhängigkeiten). Eingriffe in beide Repos auf eigenem
  Branch, abgesichert durch deren Testsuites. *Review:* lowspeak hängt hinter dem Cargo-Feature `dictation`, damit nicht
  jeder Build whisper.cpp/cmake/CoreML zahlt.
- **E15 i18n über `splat_i18n`** *(bestätigt: Übersetzung gesammelt in Phase 10)*. Deutsche Texte als Schlüssel,
  `resources/<35>.lproj/Localizable.strings`. *Review:* Die splat-Crates bringen ~114 eigene Schlüssel mit (nicht 47).
  Vollständigkeitstest existiert ab Phase 0, Pflicht ab Release. lowspeak-Fehler über lowspeaks eigene 35-Sprachen-Texte
  (`lowspeak::i18n::texts`) in der Oberflächensprache, keine Doppelpflege. Mehrzahlformen über getrennte Schlüssel je
  Zahlklasse, weil `tf` nur der Reihe nach `{}` ersetzt.
- **E16 macOS 14+, Universal Binary** *(bestätigt)*, Toolchain 1.99.1, Edition 2024, Lints von splat, keine Telemetrie.
- **E18 Produktname austauschbar** *(gewünscht)*. Anzeigename, DMG-/Cask-Name und URL-Schema aus `product.toml`;
  `build.rs` reicht sie als Konstanten in den Code, Texte setzen den Namen per Platzhalter ein, Crates tragen den
  Präfix `ide-`. *Review:* Bundle-ID, Schlüsselbund-Dienst, Defaults-Domäne und Datenordner bekommen von Anfang an eine
  neutrale, dauerhafte Kennung — dann kostet eine Umbenennung auch nach dem Release keine Datenübernahme.
  Name: **Dango** (vorher Arbeitstitel Vibeedit; alte Projektordner `.vibeedit` werden weiter gelesen).
- **E20 Mac App Store: Lite-Ausgabe nach Version 1** *(entschieden)*. *Review:* Lite heißt realistisch Editor, Vorschau,
  Browser, CI und eigener Agent ohne Shell; git, LSP und Terminal laufen im Sandbox nicht sinnvoll. Die genaue
  Fähigkeitsmatrix entsteht in der Store-Phase. Vorbereitung in V1 nur dort, wo sie nichts kostet: alle Prozessstarts
  über den Supervisor (E23), alle Ordnerzugriffe über einen Baustein (wie splats `grants.rs`).

### Editor, Dateien, Sicherheit
- **E7 Syntaxfarben über splat + LSP** *(bestätigt)*. `splat_markdown::code::tokens` (20 Sprachen + Aliase, TOML kommt
  dazu) mit splats Palette, darüber LSP Semantic Tokens für Go/TS/Rust. *Review:* `tokens` lext den ganzen Text ohne
  fortsetzbaren Zustand. Deshalb eigener Zustand je Zeilenanfang (offene Block-Kommentare/Strings), neu gelext wird nur
  ab der geänderten Zeile bis der Zustand wieder übereinstimmt, sichtbarer Bereich zuerst. Klammerpaare lexikalisch
  (Strings/Kommentare ausgenommen), Faltung über Einrückung, für Go/TS/Rust über LSP `foldingRange`/`documentSymbol`.
  Im Editor ist jeder TextKit-1-Zugriff verboten (`layoutManager` schaltet die Ansicht dauerhaft auf TextKit 1).
- **E24 Dokumentmodell** *(Review, neu)*. Ein Puffer je Datei, geteilt von allen Tabs/Panels: Revisionen,
  Edit-Transaktionen, Undo-Gruppen (eine Agenten-Änderung = ein Undo-Schritt), Positionsabbildung UTF-8/UTF-16/LSP.
  Datenintegrität über `splat-file-state` (Kodierung, BOM, CRLF/LF erhalten, ungültige Bytes, Binär-Erkennung),
  atomares Sichern, Symlinks, Groß-/Kleinschreibung. Externe Änderung (z. B. durch einen Agenten): sauberer Puffer lädt
  still neu, geänderter Puffer zeigt einen Konfliktbalken mit Diff und Zusammenführen. Verpasste Watcher-Ereignisse →
  Neuabgleich. Wiederherstellung nach Absturz auch für unbenannte Dokumente, Panel-Layout und laufende Threads.
- **E22 Projektvertrauen** *(Review, neu)*. Bevor in einem Ordner etwas ausgeführt wird, das der Ordner selbst steuert
  (git mit Repo-Konfiguration wie `core.fsmonitor`/Hooks, Language Server aus `node_modules`, Agenten-Hooks,
  `.mcp.json`, Projektregeln), fragt Dango einmal pro Ordner wie VS Code/Zed. Nicht vertraut: Lesen, Editieren,
  Git-Marken über `splat-git` ohne Prozessstart; keine Agenten, kein LSP aus dem Projekt. Hinweis bei fremdem Text als
  Agenten-Kontext (CI-Logs, Webseiten): möglicher Prompt-Injection-Inhalt.
- **E23 Prozess-Supervisor** *(Review, neu)*. Jeder Kindprozess (Terminals, LSP, Agenten, Dev-Server, git, npx-Kinder)
  läuft über einen zentralen Supervisor: eigene Prozessgruppe, Ausgabegrenzen, Zeitlimits, Herkunft (selbst gestartet /
  verbunden / nur Verlauf). Er ist die Quelle der Aktivitäts-Übersicht. Beenden der App: Hinweis bei laufenden Agenten,
  dann stdin schließen → Frist → SIGTERM an die Gruppe → SIGKILL (alacritty_terminal schickt beim Schließen nur dem
  direkten Kind SIGHUP und wartet blockierend). „Zug stoppen“ und „Hintergrundarbeit stoppen“ sind getrennte Befehle.
- **E25 Leistungsbudgets** *(Review, neu)*, gemessen wie splat und je Phase geprüft: Tastendruck < 16 ms bei 10 000
  Zeilen, 10-MB-Datei öffnen < 1 s, sehr lange Einzelzeilen ohne Hänger, Dateibaum mit 200 000 Einträgen (ohne
  ignorierte) < 1 s, `cat` von 100 MB im Terminal ohne eingefrorene Oberfläche, AI-Zeitleiste mit 1 000 Karten
  flüssig (nur sichtbare Karten werden gebaut), feste Obergrenzen für Scrollback und Verlauf. Hintergrund-Warteschlangen
  mit Rückstaugrenze (splats `Lane` ist unbegrenzt).
- **E26 Tastatur und Einstellungen** *(Review, neu)*. Eine zentrale Kürzeltabelle mit Nutzer-Überschreibungen;
  feste Regeln, welche Tasten das Terminal bekommt (⌃C, Esc, ⌘K) und welche die App. Fokuswechsel Editor/Terminal/
  Browser/AI per Tastatur. Einstellungen in zwei Ebenen (Nutzer, Projekt) mit Versionsnummer und Migration.

### Terminal, Browser, LSP
- **E8 Terminal** *(bestätigt)* auf `alacritty_terminal` 0.26: Core-Text-Zeichnen, eigene Tastaturkodierung inkl.
  Kitty-Keyboard, `NSTextInputClient`, OSC 52. *Review:* Shell-Integration (OSC 133 Befehlsgrenzen, OSC 7
  Arbeitsordner) per Login-Shell-Einbindung selbst, für Vordergrundprozess, Ordner und Dev-Server-Erkennung.
  VoiceOver-Zugang zum Raster (Zeilen als Text).
- **E9 Bild-Paste im Terminal** *(bestätigt)*. Läuft ein bekannter Agent im Vordergrund → Ctrl+V an den PTY, der Agent
  liest die Zwischenablage selbst (Zeds Lösung). Sonst PNG im App-Cache, Pfad per Bracketed Paste. *Review:* Erkennung
  über den Prozessbaum der Vordergrund-Prozessgruppe mit vollständigen Argumenten (`claude`/`codex` laufen oft als `node`
  oder unter `npx`), nicht nur über den Prozessnamen. SSH-Sitzung: Hinweis statt eines lokalen Pfads, der entfernt nicht
  existiert. Abnahme mit echten Bildanhängen in allen vier CLIs.
- **E17 Browser-Tab und „+“** *(gewünscht)*. „+“ am Ende jeder Tableiste und ⌘T: Neue Datei, Terminal, Browser,
  AI-Thread ▸, CLI-Thread ▸. Browser = `WKWebView` mit Adressfeld, Zurück/Vor/Neu laden, Suche, Web-Inspector; zugleich
  Dev-Server-Vorschau (I7). Downloads in den Download-Ordner mit Rückfrage, keine Dateizugriffe aus Webseiten.
- **E10 LSP** *(bestätigt: nachfragen + in den App-Ordner installieren)*: gopls, rust-analyzer, TypeScript.
  *Review:* TypeScript per Sondierung statt fest verdrahtet — kann das `tsc` im Projekt `--lsp`, wird es genutzt, sonst
  vtsls; vtsls/npm-Installationen setzen Node voraus (Hinweis, falls es fehlt). Alle Prozesse mit der Umgebung der
  Login-Shell inkl. `SSH_AUTH_SOCK`, über den Supervisor (E23), nur in vertrauten Ordnern (E22).

### AI-Coder
- **E4 AI wie Zed.** Ein Thread-Modell für alle Agenten, geschnitten wie ACP v2: Thread → Subagenten → Hintergrund-
  Einträge mit Status, Laufzeit, Tokens, letzter Aktion, Ausgabe-Ende, Stopp. *Review:* Auf der Leitung ACP v1 (stabil),
  Fähigkeiten je Agent ausgehandelt; unbekannte Werte werden angezeigt, keine Fähigkeit wird erfunden. Crate-Version wird
  zu Beginn der ACP-Phase auf eine erprobte Version festgelegt (5.1.0 ist vom 2026-11-09), gekapselt in `ide-acp`.
- **E21 Claude direkt wie das offizielle Plugin, ACP für alle anderen** *(entschieden)*. `claude` mit Stream-JSON
  (`--input-format/--output-format stream-json`, `--permission-prompt-tool stdio`, `--include-partial-messages`,
  `--include-hook-events`). Regeln von Anthropic: `claude` bleibt unverändert, Anmeldung nur über `claude` selbst,
  keine Ausgabe als VS-Code-Plugin, Menüname „Claude“. Nicht möglich (nur private Schnittstellen): Reiter „Web“,
  „Remote Control“. *Review — Funktionsvertrag in zwei Stufen* (geprüft gegen die öffentliche
  `@anthropic-ai/claude-agent-sdk@0.3.319/sdk.d.ts` und das installierte Plugin 2.1.319):
  - *Kern (Abnahme hängt daran):* Prompt und Streaming, Denken, Werkzeug-Karten, Freigaben über `can_use_tool`
    (inkl. `updatedInput` = Änderung vor dem Annehmen bearbeiten), Stopp (`interrupt`), Modell/Modus, Subagenten über
    `parent_tool_use_id` und `task_started/progress/updated/notification`, Hintergrund-Aufgaben über
    `background_tasks_changed`, `get_task_output`, `stop_task`, Verdichtung, `rate_limit_event`, `api_retry`,
    Sitzungen fortsetzen.
  - *Best effort, ausdrücklich experimentell:* Nutzungsbalken über `get_usage` (in der SDK als „Experimental — the
    response shape may change“ markiert), `/context`-Aufschlüsselung, `rewind_files` (erfasst nur Datei-Werkzeuge,
    keine Bash- und Subagenten-Änderungen — maßgeblich ist deshalb I6), Hook-Ausgabe, Sitzungsablage
    `~/.claude/projects` (internes Format). Fällt etwas weg, blendet die Oberfläche das Element aus.
  - Unbekannte Freigabe-Anfragen werden abgelehnt, nicht durchgewunken. Zu Phase 4 gehört eine Abbildungstabelle
    Stream-JSON → Thread-Modell; sie ist zugleich Spezifikation der Mitschnitt-Tests.
- **E5 Agenten-Register** *(bestätigt)* aus der ACP-Registry (*Review:* 45 Einträge). Vorbelegt: Claude (E21; Rückfall
  `claude-agent-acp`), Codex (`codex-acp`), Gemini CLI (`gemini --acp`), Antigravity (`agy acp`, offizieller Weg;
  Registry-Binary als Alternative), OpenCode. *Review:* Installiert werden nur erprobte Kombinationen aus Adapter-,
  Laufzeit- und Agent-Version (kein unversioniertes `npx`), Updates mit Rückweg. Registry-Einträge ohne Prüfsumme
  nur nach ausdrücklicher Bestätigung.
- **E6 Eigener Agent** *(bestätigt)*: Anthropic, OpenAI, Google, Ollama, OpenRouter, LM Studio, OpenAI-/Anthropic-
  kompatibel; Keys im Schlüsselbund; Werkzeuge Datei lesen/bearbeiten, suchen, Terminal-Befehl mit Freigabe,
  LSP-Diagnosen, Web-Abruf; Projektregeln aus AGENTS.md/CLAUDE.md; Profile „Nur fragen“ / „Schreiben“.
- **E19 Sandbox für Agenten-Befehle, standardmäßig aus** *(entschieden)*, pro Thread einschaltbar. *Review:* Je
  Register-Eintrag hinterlegte Sandbox-Argumente (Claude-Einstellung `sandbox`, Codex `-s workspace-write`, Gemini
  `--sandbox`); die Oberfläche zeigt die tatsächliche Wirkung je Agent (Claudes Sandbox umfasst nur Shell-Befehle,
  Geminis Standardprofil erlaubt breites Lesen und Netz). Beim eigenen Agenten gelten die Grenzen auch für seine Datei-
  und HTTP-Werkzeuge, nicht nur für Shell-Befehle.
- **E27 Freigabemodell** *(Review, neu)*. Neue Threads starten im Modus „Manuell“. „Alles erlauben“ ohne Sandbox zeigt
  einen dauerhaften Warnhinweis im Thread. Gespeicherte Regeln gelten je Projekt und Agent und sind in den Einstellungen
  einsehbar und löschbar. Zerstörerische Git-Befehle (`push --force`, `reset --hard`, `clean -fd`) brauchen immer eine
  Freigabe, auch bei gespeicherten Regeln.

### Git, CI, Diktat
- **E11 Git** *(entschieden: mit Abschnitts-Staging)* über das `git`-Programm (nur in vertrauten Ordnern, E22);
  Abschnitte stagen/verwerfen über `git apply --cached`/`--reverse`. Randmarken gegen HEAD über `splat-git` (bewusst wie
  splat; VS Code vergleicht gegen den Index).
- **E12 CI** *(entschieden: plus Browser-Anmeldung)*: GitHub Actions und GitLab CI inkl. selbst gehostet. Anmeldung:
  Schlüsselbund, `gh`, `glab`, Umgebung, Token; github.com per Device Flow, gitlab.com per OAuth/PKCE, selbst gehostet
  mit eingetragener Application-ID. Tokens strikt je Host und Konto. OAuth-Apps registriere ich erst nach deiner Freigabe.
- **E13 Diktat über lowspeak** *(entschieden: fn, wenn Lowspeak.app nicht läuft)*. Mikrofon-Knopf und Kürzel immer; fn
  über `hotkey::Hotkey` nur solange `com.lowspeak.app` nicht läuft (Bedienungshilfen-Freigabe nötig). *Review:*
  `Hotkey::stop()` ist endgültig, nach einem Lowspeak-Ende startet eine neue Instanz; Aufnahme und Abbruch hängen am
  Zielfeld. API-Keys: Dango liest Lowspeaks Schlüsselbund-Einträge nicht stillschweigend (jeder Zugriff löst einen
  Systemdialog aus, „Immer erlauben“ würde Lowspeaks Eintrag ändern), sondern bietet beim ersten Diktat an, den Key
  einmalig zu übernehmen und als eigenen Eintrag zu speichern. Modelle und Einstellungen (Suite `com.lowspeak.app`)
  werden mitgenutzt; eigenes Log-Verzeichnis.

## Aufbau
```
vibeedit/
  Cargo.toml            Workspace + App-Paket, Profile, comrak-Patch, Feature `dictation`
  product.toml          Anzeigename, URL-Schema, DMG-/Cask-Name (E18)
  build.rs              Produktkonstanten; clang-Laufzeit für whisper.cpp
  .cargo/config.toml    MACOSX_DEPLOYMENT_TARGET=14.0, LOWSPEAK_LOG_DIR, xtask-Alias
  crates/               alles ohne Fenster, mit `cargo test` prüfbar, ohne Netz
    ide-core           Einstellungen (2 Ebenen), Pfade, Login-Shell-Umgebung, Kürzeltabelle, Hintergrund-Threads
    ide-process        Supervisor (E23): Prozessgruppen, Ausgabegrenzen, Beenden, Aktivitätsquelle
    ide-document       Dokumentmodell (E24), Zeilen-Lexerzustand (E7)
    ide-project        Ordnerbaum, .gitignore, Beobachtung, Schnellöffnen, Projektsuche, Projektvertrauen (E22)
    ide-terminal       alacritty_terminal-Hülle, Tastaturkodierung, Shell-Integration, Paste-Regeln, Prozessbaum
    ide-lsp            JSON-RPC-Client, Server-Register, Sondierung und Installation
    ide-git            git-Aufrufe, Diff/Abschnitte, Checkpoints (I6)
    ide-ci             GitHub-/GitLab-API, Remote-Erkennung, Anmeldung
    ide-thread         gemeinsames Thread-Modell (E4), Verlauf auf Platte
    ide-claude         Claude-Anschluss (E21): Nachrichten, Steuerbefehle, Abbildung aufs Thread-Modell
    ide-acp            ACP-Anschluss, Register, Installation
    ide-agent          eigener Agent: Anbieter-Clients, Werkzeuge, Regeln, Sandbox-Profil
  src/                  AppKit: main, app, menu, settings/, window/, sidebar/, editor/, terminal/, browser/,
                        preview/, agent/, activity/, diff/
  resources/<35>.lproj  Localizable.strings, InfoPlist.strings
  xtask/                bundle, sign, universal, dmg, notarize, Sparkle, Cask, Acknowledgements
  docs/development.md  ACCESSIBILITY.md  README.md
```

## Wiederverwendung
- **splat** (`/Users/Sven/Repos/splat`): `splat-i18n` (`install`, `t`, `tf`, `n`); `splat-markdown` (`reader::parse`,
  `code::tokens`, `palette::color`); `splat-preview` (`layout::layout`, `NoImages`); `splat-git` (`find`,
  `Repository::head_version`, `diff::changes`); `splat-file-state` (Kodierung, Format, `Stamp`);
  `splat-editor-state::lanes`. Muster: `src/editor/controller/window.rs` (Split-View, Toolbar), `sidebar.rs` +
  `outline.rs` (`NSOutlineView`, `RowItems`), `changes.rs` (Änderungsbalken), `toolbar.rs`, `views.rs`
  (`text_system_2`), `src/util/appkit.rs` + `lanes.rs` (`guarded`, `main_soon`, `ns`, `item`, `editing`),
  `xtask/src/bundle.rs`, `tests/translations.rs`.
- **lowspeak** (`/Users/Sven/Repos/lowspeak`): `audio::recorder::{Recorder, request_mic_permission}`,
  `transcription::{service::run, local::LocalTranscriber, model_store::Disk, shutdown}`, `hotkey::Hotkey`,
  `config::settings::Settings::load_from`, `config::vocabulary::parse`, `i18n::texts`; Signatur/Notarisierung aus
  `xtask/src`.

## Oberfläche
- **Fenster:** `NSSplitViewController` mit Sidebar-Item (Symbolspalte + Panel) und Inhalts-Item (Tableiste + Tab).
  Toolbar: Seitenleiste, Zurück/Vor, Projektname mit Branch, Aktivität (Zähler), Neues Terminal, Neuer AI-Thread.
- **Symbolspalte:** Dateien, AI, Git (Badge), CI (Badge-Farbe), unten Einstellungen; ⌘1–⌘4; Klick aufs aktive Symbol
  klappt ein. *(Umsetzung: fünf Panels mit Suche, ⌘1–⌘5, siehe E55.)*
- **Dateien:** `NSOutlineView` Source-List, Git-Farben, Kontextmenü (Neu, Umbenennen, In Papierkorb, Im Finder zeigen,
  Pfad kopieren, Im Terminal öffnen, An Agent anhängen).
- **AI-Panel nach Screenshot 2:** *Konto & Nutzung* (Konto; Nutzungsbalken 5 h / 7 Tage mit Rücksetzzeit, experimentell
  nach E21; beim eigenen Agenten Kosten je Anbieter), *Sitzungen* („+ Neue Sitzung“ mit Agentwahl, „Aktiv · N“, Suche,
  Filter, Gruppen per Drag, Titel, Alter, Statuspunkt, Umbenennen, Archiv), *Laufend* (Baum der aktiven Sitzungen mit
  Subagenten und Hintergrund-Einträgen).
- **Aktivität** *(gewünscht)*: Toolbar-Symbol mit Zähler, Übersicht über alles, was läuft — aus dem Supervisor (E23)
  und den Agenten-Meldungen: Sitzungen mit Status, Subagenten (Laufzeit, Tokens, letzte Aktion), Hintergrund-Shells
  und Monitore (letzte Ausgabe, ganze Ausgabe im Tab), Terminal-Vordergrundprozesse, Dev-Server, Language Server.
  Jeder Eintrag mit Stopp, „Zeigen“, Laufzeit, Herkunft. Claude vollständig; ACP-Agenten so weit, wie sie melden.
- **AI-Tab** (Detailtiefe des Plugins; Abnahme gegen den Kern-Funktionsvertrag aus E21 und eingefrorene Mitschnitte,
  dazu ein Sichtvergleich mit dem Plugin):
  - *Kopf/Leerzustand:* Agent-Symbol und Titel (automatisch, umbenennbar), Lesezeichen, Verlauf, Neu; Logo und Tipp.
  - *Zeitleiste:* Linie mit Punkten, Zeitstempel; Nutzernachricht im Rahmen; Antworten als natives Markdown (Tabellen,
    Code, Formeln); Kopieren; Fokus-Ansicht ohne Werkzeuge/Denken.
  - *Denken* einklappbar mit Dauer. *Werkzeug-Karten* mit Statuspunkt und Live-Laufzeit: Bash (Beschreibung, IN/OUT,
    „Im Hintergrund ausführen“), Read (Pfad, Zeilen), Edit/Write (Diff, vor dem Annehmen bearbeitbar, im Editor
    öffnen), Grep/Glob, WebFetch/WebSearch, NotebookEdit, MCP, Hook-Ausgabe.
  - *Subagenten* als aufklappbare Karten mit eigenen Werkzeug-Karten und Stopp; *Agenten-Übersicht* des Threads
    (`/tasks`); Esc stoppt den Zug, Hintergrundarbeit läuft weiter.
  - *To-dos*, *Plan* als Dokument mit Inline-Kommentaren, *Rückfragen* mit Freitext, *Freigaben* mit Regel und
    Gültigkeitsbereich (E27).
  - *Status:* Kontext, Kosten, „Verdichtet“, „Wiederhole…“, Fehler. *Zurückspulen* je Nachricht (Unterhaltung, Code,
    beides) über I6.
  - *Eingabekarte* wie Screenshot: ⌘Esc-Hinweis, Mikrofon, `+`, `/`-Menü, `@datei`, `@datei#5-10`, `@terminal`,
    `@browser`, Auswahl-Chip, Modell-/Denktiefe-Pill, Modus-Pill, Sandbox-Schalter, Senden ↔ Stopp, Warteschlange.
  - Mehrere Sitzungen in Tabs mit Statuspunkt; Farbe je Agent (Claude Terrakotta), sonst Systemakzent.
- **Tableiste:** Symbol je Art, Änderungspunkt, Schließen, Drag, Vorschau-Tab (kursiv), „+“ (E17), ⌘⇧[ ⌘⇧] ⌘W.
- **I10 Panels** *(entschieden)*: Standard Tabs; Rechtsklick → „In eigenem Panel öffnen (rechts / unten)“, beliebig
  viele, verschachtelte `NSSplitView`s, Tabs zwischen Panels ziehbar, Layout je Projekt gespeichert.
- **I6 Änderungen prüfen und zurückspulen** *(bestätigt; Review präzisiert)*: Vor jedem Prompt ein Checkpoint — auch
  bei Claude, weil `rewind_files` Bash- und Subagenten-Änderungen nicht erfasst. Erfasst werden versionierte und
  unversionierte (nicht ignorierte) Dateien, ungesicherte Puffer und der Index; Nicht-Git-Ordner über eine eigene
  Schnappschuss-Ablage. Versteckte Refs bleiben lokal (nie gepusht). Wiederherstellen erkennt zwischenzeitliche
  Nutzeränderungen und fragt. Agent-Änderungen als Diff mit Annehmen/Verwerfen je Abschnitt; klare Regel je Anschluss,
  ob eine Änderung schon auf der Platte liegt („Verwerfen“ = zurücknehmen) oder erst vorgeschlagen ist.
- **I1** Worktree-Threads, **I3** Kontext/Kosten, **I4** Secrets-Schutz (gitleaks), **I5** CLI-Threads, **I7**
  Dev-Server → Browser-Tab, **I8** CI-Fehler an Agent (mit Prompt-Injection-Hinweis, E22), **I9** Mitteilungen und
  Dock-Zahl — alle *bestätigt*, Einordnung in die Phasen unten.

## Phasen
*Review:* neu geschnitten — Grundlagen, die andere brauchen, kommen vor ihren Nutzern (Dokumentmodell vor Agenten,
LSP vor dem eigenen Agenten, Diff und Checkpoints vor dem Review); große Blöcke sind geteilt. Der Umfang von Version 1
bleibt unverändert *(bestätigt nach den Reviews: alles behalten)*. Jede Phase endet mit lauffähiger App, grünem `make check`, Leistungsbudgets (E25) und
VoiceOver-Prüfung der neuen Ansichten.

- **Phase 0 — Fundament:** Workspace, `product.toml`, Lints, xtask (bundle, sign), i18n mit Test, Login-Shell-
  Umgebung, Supervisor (E23), Einstellungen und Kürzeltabelle (E26), Fenster mit Symbolspalte und Tableiste mit „+“,
  Menüleiste, Einstellungsfenster, Selftest-Gerüst wie splat. TOML in splats Tokenizer.
- **Phase 1 — Dokumentmodell, Dateien, Editor:** E24, Projektvertrauen (E22), Dateibaum mit Beobachtung,
  `dango`-Befehl für das Terminal, Editor mit inkrementellen Farben (E7), Zeilennummern, Git-Marken, Suchen/Ersetzen,
  Schnellöffnen ⌘P, Projektsuche ⌘⇧F. Editor-Grundlagen: Auto-Einrückung, Tabs/Leerzeichen + `.editorconfig`,
  Klammerpaare und -schließen, Kommentar umschalten, Mehrfach-Cursor, Spaltenauswahl, Faltung, Gehe zu Zeile,
  Leerzeichen-Anzeige, Umbruch, Nur-lesen.
- **Phase 2 — Terminal und Browser:** E8, E9, E17, I7.
- **Phase 3 — LSP:** E10 mit Diagnosen, Hover, Vervollständigung, Definition, Referenzen, Umbenennen, Formatieren,
  Semantic Tokens, Faltung, Gliederung.
- **Phase 4 — Claude-Kern:** Kern-Funktionsvertrag (E21) mit Abbildungstabelle, AI-Tab, AI-Panel, Aktivität, Diff-Ansicht
  und Checkpoints (I6), Freigabemodell (E27), I9.
  **→ Meilenstein „Eigennutzung“: Editor, Terminal, Browser, LSP und Claude sind täglich benutzbar.**
- **Phase 5 — Panels und weitere Agenten:** I10, ACP-Agenten mit Register (E4/E5), I5, E19.
- **Phase 6 — Claude-Vollausbau:** experimentelle Teile (Nutzung, `/context`, Hook-Ausgabe), Plan-Kommentare, I3.
- **Phase 7 — Eigener Agent:** E6 mit Sandbox-Profil (E19).
- **Phase 8 — Git und CI:** Git-Panel, Abschnitts-Staging (E11), CI mit Anmeldung (E12), I1, I4, I8.
- **Phase 9 — Vorschau und Diktat:** splat-Eingriff `splat-render` (*Review:* betrifft ~4 000 Zeilen splat-Kern —
  `render.rs`, `js.rs`, `images.rs`, `remote.rs`, `env.rs`, `webkit.rs`, `controller/layers.rs`; zuerst Schnittstellen
  für Ressourcen, Render-Aufträge, Invalidierung und Abbruch festlegen), Markdown-Vorschau mit Diagrammen/Formeln,
  Bilder, PDF, HTML im Browser-Tab; lowspeak-Eingriff (`dictation`-Modul, `build.rs`) und Diktat (E13). Bis dahin
  rendern AI-Antworten über `splat_preview::layout` mit eigener kleiner Formatierung, Formeln/Diagramme als Code.
- **Phase 10 — Release:** 35 Sprachen, Test scharf, Barrierefreiheits-Abnahme, README, Signatur, Notarisierung, DMG,
  Sparkle, Homebrew Cask; Test auf macOS 14 und Intel mit signiertem Build.
- **Danach:** Store-Lite (E20), I2.

## Risiken
- **R1 Claude im AI-Tab zählt als Agent-SDK-Nutzung**; die angekündigte getrennte Abrechnung ist seit 2026-06-15
  pausiert. CLI-Threads (I5) bleiben unberührt.
- **R2 ACP-Crate bricht fast monatlich**; Version erst zur ACP-Phase festlegen, gekapselt.
- **R3 Eingriffe in splat und lowspeak** betreffen ausgelieferte Apps; deshalb spät (Phase 9), eigener Branch, deren
  Testsuites plus Tests für die neue Fremdnutzung.
- **R4 Schlüsselbund:** jeder Zugriff auf fremde Einträge löst Systemdialoge aus; deshalb einmalige Übernahme (E13).
- **R5 Kein Sandbox** — getragen von E22, E23, E27, E19.
- **R6 Build und Größe:** whisper.cpp (cmake, CLT, ~6 MB) nur mit Feature `dictation`; ~14 MB JS-Bibliotheken (E1).
- **R7 Claudes Protokoll ohne Versionsgarantie**, einzelne Teile ausdrücklich experimentell (E21). Abhilfe:
  Zwei-Stufen-Vertrag, `init.capabilities`, unbekannte Nachrichten ignorieren, nächtlicher Test gegen die neueste CLI,
  Rückfall auf `claude-agent-acp`.
- **R8 Lieferkette der Agenten** *(Review)*: npx/uvx-Pakete und Registry-Binaries ändern sich ohne Zutun; abgefangen
  durch erprobte Versionskombinationen und Prüfsummenregel (E5).

## Verifikation
- `make check`: fmt, clippy `-D warnings`, `cargo test --workspace` (ohne Netz, ohne Feature `dictation`).
- Mitschnitt-Tests: echte Claude-Sitzungen (Subagenten, Hintergrund-Aufgaben, Verdichtung, Freigaben, Abbruch,
  Fortsetzen, Fehler) werden in `ide-claude` abgespielt und gegen das Thread-Modell geprüft; nächtlich gegen die neueste
  CLI (R7). Gleiches für ACP-Agenten mit einem Test-Agenten im Repo.
- `make test-integration` (lokal, mit Logins): echte Language Server (Go, Rust, TS mit und ohne `--lsp`), echtes `claude`,
  `gemini --acp`, `agy acp`, `codex-acp`; Bild-Paste mit echtem Anhang in allen vier CLIs; git auf Temp-Repo inkl.
  Checkpoint/Wiederherstellen mit zwischenzeitlicher Nutzeränderung; CI lesend gegen github.com und GitLab self-managed.
- `cargo xtask selftest` (wie splat): Szenarien für Editor (Tippen, Sichern, externe Änderung bei geändertem Puffer,
  Kodierungen/CRLF), Terminal, Bild-Paste-Verzweigungen, AI-Tab gegen Mitschnitte, Aktivität, Beenden mit laufenden
  Agenten; Fensterfotos hell/dunkel/Rechts-nach-links; Eingabemethoden (Japanisch).
- Leistungsmessungen nach E25 je Phase; VoiceOver je Phase; i18n-Test; Abnahme je Anforderung von Hand.

## Ideen
- In Version 1: I1, I3, I4, I5, I6, I7, I8, I9, I10 (Phasen oben). Später: I2 Bildschirmausschnitt-Knopf.

## Entscheidungen während der Umsetzung
Getroffen nach der Freigabe des Plans; sie ergänzen die Entscheidungen oben und sind ebenso verbindlich.

- **E28 Unterhaltung zurückspulen** nur vor einer eigenen Nachricht, immer mit der Schutzprüfung der CLI
  (`--resume-drops-turn`); lehnt die CLI ab, läuft die Sitzung ohne Rücksprung weiter, kein zweiter Versuch.
- **E29 Terminal ohne Vertrauensprüfung:** Es ist die eigene Shell des Nutzers; E22 gilt für das, was der Ordner
  selbst einrichtet.
- **E30 ⌘K im Terminal** leert immer den Verlauf, den sichtbaren Bereich nur bei einer Shell im Vordergrund.
- **E31 Erfolgreiche Hooks** erscheinen als eine eingeklappte Zeile pro Antwort.
- **E32 Nach dem Zurückspulen** bleibt die ursprüngliche Sitzung als eigener Verlaufseintrag erhalten.
- **E33 „Immer erlauben“** gibt es bei zerstörerischen Git-Befehlen nicht (E27).
- **E34 Prompt nach dem Checkpoint:** Ein Prompt geht erst nach dem Checkpoint (I6) raus; die Eingabe zeigt
  „Sicherungspunkt wird angelegt …“.
- **E35 Register-Vorgaben** gelten erst nach `make test-integration` als erprobt; bis dahin fragt jede Installation.
- **E36 Eigener Agent, OpenAI** über Chat Completions; eine Anbindung für OpenAI, OpenRouter, Ollama, LM Studio und
  kompatible Endpunkte.
- **E37 Keine eingebauten Preise** für OpenAI und Gemini; Kosten erst mit vom Nutzer eingetragenem Preis.
- **E38 Anthropic-Ausweichstrategie** bei Ablehnungen standardmäßig an, mit Hinweis im Verlauf.
- **E40 Code zurückspulen** auch für ACP-Agenten über die eigenen Checkpoints; die Unterhaltung nur bei Claude.
- **E41 Texte des eigenen Agenten** laufen über die Übersetzung; der Baustein liefert strukturierte Meldungen.
- **E42 Verdichtung** für den eigenen Agenten ab etwa 85 % Kontext, dazu `/compact`.
- **E43 Gelöschte Datei schließen** fragt immer (Sichern / Verwerfen / Abbrechen), auch ohne eigene Änderungen
  (ersetzt E39).
- **E44 Eigener Decoder-Dienst** (XPC, von launchd gestartet) für Bilder, Formeln und Diagramme; Abweichung von E23.
- **E45 Keine Netzbilder** in der Vorschau nicht vertrauter Ordner; Platzhalter mit „Bilder aus dem Netz laden“.
- **E46 Diktat als Build-Feature** `dictation`: im Release an, in `make check` aus, eigenes `make check-dictation`.
- **E47 Diktat-Kürzel** ⌃⌥D.
- **E48 fn-Taste** standardmäßig an (E13); die Bedienungshilfen-Freigabe wird erst beim ersten Diktat erfragt.
- **E49 fn-Taste** wirkt nur, solange Dango die aktive App ist; sonst würde sie in jeder App abgefangen.
- **E50 Bedienungshilfen-Freigabe** für fn wird nach dem ersten Diktat erfragt, nicht während des Sprechens.
- **E51 Eigener Sparkle-Schlüssel** für Dango, getrennt von splat und lowspeak.
- **E52 Lizenz** MIT OR Apache-2.0 wie splat und lowspeak.
- **E53 `make test-integration`** startet bezahlte Gruppen (claude, paste, acp) nur, wenn sie ausdrücklich genannt oder mit
  `--paid` angefordert werden.
- **E54 ⌘1–⌘5** setzen den Tastaturfokus in das gewählte Panel.
- **E55 Suche als eigenes Panel** *(Abweichung von der Symbolspalte mit vier Panels)*: fünf Panels — Dateien, Suche, AI,
  Git, CI — auf ⌘1–⌘5; ⌘⇧F öffnet die Suche. Sie steht wie VS Codes Suchansicht unter eigenem Symbol statt über dem
  Dateibaum, mit Ersetzen: alle Treffer nach Rückfrage, die einer Datei oder einer Zeile über Knopf bei Mauszeiger oder
  Kontextmenü. Geöffnete Dateien ersetzen über das Dokumentmodell als ein Widerrufschritt je Datei (E24), geschlossene
  werden wie beim Sichern geschrieben, mit Kodierung, BOM und Zeilenenden. Die Dateiliste bekommt eine Kopfzeile mit
  Ordnername, Neue Datei, Neuer Ordner und Alle einklappen; die Symbolspalte ist 44 pt breit.
- **E56 Ordner öffnen ersetzt den Ordner des Fensters:** Ein Ordner, der in einem Fenster mit Ordner geöffnet wird
  (Menü, Willkommen-Tab, Zuletzt geöffnet, Befehlszeile), tritt an dessen Stelle; ist er schon in einem anderen Fenster
  offen, kommt dieses nach vorn. Gingen dabei in einem Tab der Tabgruppe des Ordners ungesicherte Änderungen oder
  laufende Agenten und Terminalprogramme verloren, fragt ein Sheet: „In neuem Fenster öffnen“ (Standard), „Alles
  schließen“, „Abbrechen“. Fenster verschiedener Ordner werden nie zu einer Tabgruppe zusammengelegt (eine
  Tab-Kennung je Ordner, ein Fenster ohne Ordner hat seine eigene).
- **E57 Kosten nur, wo sie anfallen.** Läuft Claude über ein Abo (Anmeldung ohne API-Key, `subscriptionType` im
  Konto von `initialize`), zeigt die Statuszeile statt Dollarbeträgen die Limits des Plans (5 Stunden / 7 Tage), und
  die Sitzungen im AI-Panel tragen keine Kosten; mit API-Key oder über einen Cloud-Anbieter bleiben die Kosten. ACP
  sagt nichts über Abos: Kosten eines ACP-Agenten erscheinen, wie er sie meldet. Der eigene Agent läuft immer über
  API-Keys oder lokale Modelle.
- **E58 Agentenliste in den Einstellungen.** Status, Installation und Register aller Agenten stehen im Tab „Agenten“
  der Einstellungen, nicht im AI-Panel; der Tab des eigenen Agenten heißt „Eigener Agent“. Ein Adapter, dessen
  Agenten-Programm da ist, heißt „… vorhanden, Adapter fehlt“; Claude über ACP ist der Ausweichweg und nie „Claude
  fehlt“. „+ Neue Sitzung“ bietet nur Agenten an, die bereit sind, und „Agenten verwalten …“.
- **E59 Markdown-Komfort und „In Splat öffnen“** (Wahl des Eigentümers): Eine Markdown-Datei öffnet ihre Vorschau
  automatisch daneben (Einstellung, Standard an; ⇧⌘V blendet sie aus und ein), ein Format-Menü mit splats Kürzeln
  (Link ⇧⌘K, weil ⌘K im Terminal leert, E30), Einsetzen aus Word und dem Web als Markdown (⌥⇧⌘V rein),
  Export als PDF, HTML und Word über splats Crates, „In Splat öffnen“, wenn Splat installiert ist. splat selbst bleibt
  unverändert.
- **Tableiste** nach dem Vorbild von Xcode, angedockt über der Inhaltsfläche (macOS hat kein Standardelement dafür).
- **Änderungen an splat und lowspeak** bleiben in deren Repos auf `vibeedit/*`-Branches, uncommittet bis zur Übernahme.

Neue Risiken: **R9** Last durch parallele Agenten (Zeitmessungen nur über `make perf`), **R11** Nutzungslimit des
Claude-Kontos (keine Live-Läufe echter Agenten-CLIs in Tests), **R12** Selbsttest-Fenster dürfen nie den Fokus nehmen
(Wächter im Testlauf), **R13** Dango baut gegen den Arbeitsstand von `../splat` und `../lowspeak`.
