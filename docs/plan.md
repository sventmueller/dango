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
- **E3 Seitenleiste VS-Code-Stil** *(deine Wahl)*: senkrechte Symbolspalte (`NSButton`s, SF Symbols; Badges entfallen, E69), Panel im
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
  *Abweichung (umgesetzt):* Kein `product.toml` und kein `build.rs`; Name, Bundle-ID, URL-Schema, Cask-Name und die
  übrigen Kennungen stehen als Konstanten in `crates/ide-product/src/lib.rs`, das App, Crates und xtask gleichermaßen
  einbinden – eine Stelle zum Ändern, ohne Build-Skript.
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
  - *Performance-Review 2026-11:* 16 ms ist der Frame, nicht das Budget. Die Selbsttests prüfen den Tastendruck bei
    10 000 Zeilen mit 5 ms Median-CPU-Zeit (auch nach einem Sprung), Zuschläge durch Einblendungen und Diagnosen mit
    2 ms, eine fertige Antwortkarte bis ~100 KB mit 200 ms. Grund: ein 10-facher Rückschritt lag unbemerkt knapp
    unter 16 ms.
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
- **E17 Browser-Tab und „+“** *(gewünscht; „+“ seit E60 das native der Tableiste)*. „+“ am Ende jeder Tableiste und
  ⌘T: Neue Datei, Terminal, Browser, AI-Thread ▸, CLI-Thread ▸. Browser = `WKWebView` mit Adressfeld, Zurück/Vor/Neu laden, Suche, Web-Inspector; zugleich
  Dev-Server-Vorschau (I7). Downloads in den Download-Ordner mit Rückfrage, keine Dateizugriffe aus Webseiten.
  *Nachtrag (Eigentümer):* Ein Schlüsselknopf in der Navigationszeile fragt in einem nativen Fenster nach einer
  Anmeldung, die macOS-AutoFill aus Passwörter anbietet, und trägt sie ins Formular der Seite ein, nur solange sie den
  Host zeigt, für den gefragt wurde. Ein `WKWebView` außerhalb von Safari bekommt kein eigenes Passwort-AutoFill;
  Passkeys bräuchten die von Apple vergebene Berechtigung `com.apple.developer.web-browser.public-key-credential`.
- **E10 LSP** *(bestätigt: nachfragen + in den App-Ordner installieren)*: gopls, rust-analyzer, TypeScript.
  *Review:* TypeScript per Sondierung statt fest verdrahtet — kann das `tsc` im Projekt `--lsp`, wird es genutzt, sonst
  vtsls; vtsls/npm-Installationen setzen Node voraus (Hinweis, falls es fehlt). Alle Prozesse mit der Umgebung der
  Login-Shell inkl. `SSH_AUTH_SOCK`, über den Supervisor (E23), nur in vertrauten Ordnern (E22).
  *Speicher:* Ein Server endet fünf Minuten nach dem Schließen des letzten Editors, den er bedient, und startet mit
  dem nächsten wieder; seine Diagnosen verfallen dabei, der eigene Agent meldet dann „kein Sprachserver“ statt „keine
  Probleme“. rust-analyzer läuft ohne Vorab-Indexierung (`cachePriming`): am Repo selbst 1,3 statt 2,4 GB, Spitze
  1,3 statt 3,8 GB.
  *Weitere Sprachen (Owner, 2026-11):* Python (basedpyright, pyright, pylsp), C/C++/Objective-C (clangd), Swift
  (sourcekit-lsp), Shell, YAML, JSON, CSS, HTML, TOML (taplo), Markdown (marksman), Lua — gesucht auf dem PATH der
  Login-Shell, clangd und sourcekit-lsp über `xcrun` aus den Entwicklerwerkzeugen (nie über die Platzhalter in
  `/usr/bin`, die sonst die Installation anstoßen). Die npm-Server (basedpyright, bash-, yaml- und die VS-Code-Server
  für JSON/CSS/HTML) installiert die App auf Nachfrage wie vtsls; für die übrigen nennt die Statusleiste den
  Installationsbefehl. yaml-language-server ohne SchemaStore-Abruf (kein Netz ohne Zutun). Eigene Server legt nur der
  Nutzer im Tab „Sprachserver“ der Einstellungen an (Name, Endungen, Befehl; nie aus dem Projekt, E22); sie gehen den
  eingebauten für ihre Endungen vor. Java, Kotlin, C#, Ruby, PHP usw. laufen über diesen Weg.

### AI-Coder
- **E4 AI wie Zed.** Ein Thread-Modell für alle Agenten, geschnitten wie ACP v2: Thread → Subagenten → Hintergrund-
  Einträge mit Status, Laufzeit, Tokens, letzter Aktion, Ausgabe-Ende, Stopp. *Review:* Auf der Leitung ACP v1 (stabil),
  Fähigkeiten je Agent ausgehandelt; unbekannte Werte werden angezeigt, keine Fähigkeit wird erfunden. Crate-Version wird
  zu Beginn der ACP-Phase auf eine erprobte Version festgelegt (5.1.0 ist vom 2026-11-09), gekapselt in `ide-acp`.
- **E21 Claude direkt wie das offizielle Plugin, ACP für alle anderen** *(entschieden; seit E147: SDK-Protokoll zuerst, ACP als Rückfall)*. `claude` mit Stream-JSON
  (`--input-format/--output-format stream-json`, `--permission-prompt-tool stdio`, `--include-partial-messages`,
  `--include-hook-events`). Regeln von Anthropic: `claude` bleibt unverändert, Anmeldung nur über `claude` selbst,
  keine Ausgabe als VS-Code-Plugin, Menüname „Claude“. Nicht möglich (nur private Schnittstellen): Reiter „Web“.
  „Remote Control“ gibt es auf Entscheidung des Owners trotzdem, als Best effort (siehe unten). *Review —
  Funktionsvertrag in zwei Stufen* (geprüft gegen die öffentliche `@anthropic-ai/claude-agent-sdk@0.3.319/sdk.d.ts`
  und das installierte Plugin 2.1.319):
  - *Kern (Abnahme hängt daran):* Prompt und Streaming, Denken, Werkzeug-Karten, Freigaben über `can_use_tool`
    (inkl. `updatedInput` = Änderung vor dem Annehmen bearbeiten), Stopp (`interrupt`), Modell/Modus, Subagenten über
    `parent_tool_use_id` und `task_started/progress/updated/notification`, Hintergrund-Aufgaben über
    `background_tasks_changed`, `get_task_output`, `stop_task`, Verdichtung, `rate_limit_event`, `api_retry`,
    Sitzungen fortsetzen.
  - *Best effort, ausdrücklich experimentell:* Nutzungsbalken über `get_usage` (in der SDK als „Experimental — the
    response shape may change“ markiert), `/context`-Aufschlüsselung, `rewind_files` (erfasst nur Datei-Werkzeuge,
    keine Bash- und Subagenten-Änderungen — maßgeblich ist deshalb I6), Hook-Ausgabe, Sitzungsablage
    `~/.claude/projects` (internes Format), Remote Control über die interne Steuernachricht `remote_control` und
    `--replay-user-messages` (wie die VS-Code-Erweiterung; die Anmeldung bei claude.ai macht `claude` selbst, der
    Thread folgt `remoteControlAtStartup`, bis der Nutzer ihn selbst verbindet oder trennt). Fällt etwas weg, blendet
    die Oberfläche das Element aus.
  - Unbekannte Freigabe-Anfragen werden abgelehnt, nicht durchgewunken. Zu Phase 4 gehört eine Abbildungstabelle
    Stream-JSON → Thread-Modell; sie ist zugleich Spezifikation der Mitschnitt-Tests.
- **E5 Agenten-Register** *(bestätigt; SDK-Wege mit erprobtem Versionsbereich seit E147)* aus der ACP-Registry (*Review:* 45 Einträge). Vorbelegt: Claude (E21; Rückfall
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
- **E27 Freigabemodell** *(Review, neu)*. Neue Threads starten mit Modell, Denktiefe und Modus, die der Nutzer für
  den Agenten zuletzt gewählt hat, auch nach einem Neustart (Vorgabe des Owners); beim ersten Mal im Modus „Manuell“.
  „Alles erlauben“ und „Umgehen“ ohne Sandbox zeigen ausdrücklich keinen Warnkasten im Thread (Vorgabe des Owners); den
  Modus nennt die Modus-Pille.
  Gespeicherte Regeln gelten je Projekt und Agent und sind in den Einstellungen einsehbar und löschbar (seit E146 über
  den Schreibweg der CLI, denselben wie `/permissions`). Zerstörerische
  Git-Befehle (`push --force`, `reset --hard`, `clean -fd`) brauchen immer eine Freigabe, auch bei gespeicherten Regeln.

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
  werden mitgenutzt; eigenes Log-Verzeichnis. *Geändert durch E103:* Die Einstellungen sind Dangos eigene.

## Aufbau
```
vibeedit/
  Cargo.toml            Workspace + App-Paket, Profile, comrak-Patch, Feature `dictation`
  (kein product.toml, kein build.rs: Produktkonstanten in crates/ide-product, E18; die clang-Laufzeit für
   whisper.cpp bindet lowspeaks build.rs ein, docs/dictation.md)
  .cargo/config.toml    MACOSX_DEPLOYMENT_TARGET=14.0, LOWSPEAK_LOG_DIR, xtask-Alias
  crates/               alles ohne Fenster, mit `cargo test` prüfbar, ohne Netz
    ide-product        Anzeigename, Kennungen, URL-Schema, Cask-Name (E18)
    ide-core           Einstellungen (2 Ebenen), Pfade, Login-Shell-Umgebung, Kürzeltabelle, Hintergrund-Threads
    ide-process        Supervisor (E23): Prozessgruppen, Ausgabegrenzen, Beenden, Aktivitätsquelle
    ide-rpc            Transport unter ide-lsp und ide-dap: Frames, Anfragen mit Frist, Antwortpflicht
    ide-net            einziger Weg ins Netz: HTTPS, Downloads mit Prüfsumme, Fortschritt, Abbruch
    ide-secrets        Schlüsselbund für CI-Token, Anbieter- und Diktatschlüssel
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
    ide-predict, ide-dap  Vorhersage-Anbieter (B4), Debug Adapter Protocol (B1)
    ide-ui-common      AppKit-Helfer, Diagnose, Farbregeln (E66), Fenster-Handle und Register; Grundlage der UI-Crates (E99)
    ide-ui-icons, ide-ui-export, ide-ui-browser, ide-ui-terminal, ide-ui-dictation, ide-ui-debug, ide-ui-git,
    ide-ui-agent, ide-ui-lsp  vom App-Crate abgetrennte Oberflächenteile (E99); Zustand nur über ihren `Host`, Fenster
                       nur über `ide_ui_common::window` (Handle nach Id, `WindowHost`); `ide-ui-lsp` erreicht den
                       Editor nur über seine Schnittstelle `Editor` (E144)
  src/                  AppKit: main, app, menu, settings/, window/, sidebar/, editor/, preview/, activity/, diff/;
                        `host.rs` setzt `WindowHost` und die `Host`s der UI-Crates um; worktrees/, debug/ (mit den
                        Teilen im Editor), terminal/, browser.rs und util/ halten die App-Seite ihrer UI-Crates, alle
                        übrigen erreicht der Code über ihren eigenen Pfad
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
- **Fenster:** `NSSplitViewController` mit Sidebar-Item (Symbolspalte + Panel) und Inhalts-Item (Tableiste + Tab);
  seit E60 je Tab ein `NSWindow` in der Tabgruppe des Ordners.
  Toolbar: Seitenleiste, Zurück/Vor, Projektname, Branch, Schnell öffnen, Aktivität (Zähler im Tooltip); siehe E73.
- **Symbolspalte:** Dateien, AI, Git, CI (Zahl und Zustand im Tooltip, E69), unten Einstellungen; ⌘1–⌘4; Klick aufs aktive Symbol
  klappt ein. *(Umsetzung: fünf Panels mit Suche, ⌘1–⌘5, siehe E55; das Einklappen per Klick entfällt, siehe E61.)*
- **Dateien:** `NSOutlineView` Source-List, Git-Farben, Kontextmenü (Neu, Umbenennen, In Papierkorb, Im Finder zeigen,
  Pfad kopieren, Im Terminal öffnen, An Agent anhängen).
- **AI-Panel** *(E68 ersetzt die Gliederung nach Screenshot 2)*: Titel „Agenten“ mit „+ Neue Sitzung“ (Agentwahl),
  darunter eine Liste aller Sitzungen des Ordners, AI und CLI gleich, mit Kreis (grün gefüllt, wenn offen), Titel und
  Alter; Kontextmenü mit Umbenennen, Lesezeichen, Archiv.
- **Aktivität** *(gewünscht)*: Toolbar-Symbol ohne Badge, Zähler im Tooltip, Übersicht über alles, was läuft — aus dem Supervisor (E23)
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
- **Tableiste:** Symbol je Art, Änderungspunkt, Schließen, Drag, Vorschau-Tab (kursiv), „+“ (E17), ⌘⇧[ ⌘⇧] ⌘W;
  seit E60 die native von macOS; den Vorschau-Tab gibt es seit E62 nur noch auf Wunsch.
- **I10 Panels** *(entschieden; durch E60 ersetzt)*: Standard Tabs; Rechtsklick → „In eigenem Panel öffnen (rechts /
  unten)“, beliebig viele, verschachtelte `NSSplitView`s, Tabs zwischen Panels ziehbar, Layout je Projekt gespeichert.
- **I6 Änderungen prüfen und zurückspulen** *(bestätigt; Review präzisiert)*: Vor jedem Prompt ein Checkpoint — auch
  bei Claude, weil `rewind_files` Bash- und Subagenten-Änderungen nicht erfasst. Erfasst werden versionierte und
  unversionierte (nicht ignorierte) Dateien, ungesicherte Puffer und der Index; Nicht-Git-Ordner über eine eigene
  Schnappschuss-Ablage. Versteckte Refs bleiben lokal (nie gepusht). Wiederherstellen erkennt zwischenzeitliche
  Nutzeränderungen und fragt. Agent-Änderungen als Diff mit Annehmen/Verwerfen je Abschnitt; klare Regel je Anschluss,
  ob eine Änderung schon auf der Platte liegt („Verwerfen“ = zurücknehmen) oder erst vorgeschlagen ist.
- **I1** Worktree-Threads, **I3** Kontext/Kosten, **I4** Secrets-Schutz (gitleaks), **I5** CLI-Threads *(entfallen, E78)*, **I7**
  Dev-Server → Browser-Tab, **I8** CI-Fehler an Agent (mit Prompt-Injection-Hinweis, E22), **I9** Mitteilungen und
  Dock-Zahl — alle *bestätigt*, Einordnung in die Phasen unten.

## Phasen
*Review:* neu geschnitten — Grundlagen, die andere brauchen, kommen vor ihren Nutzern (Dokumentmodell vor Agenten,
LSP vor dem eigenen Agenten, Diff und Checkpoints vor dem Review); große Blöcke sind geteilt. Der Umfang von Version 1
bleibt unverändert *(bestätigt nach den Reviews: alles behalten)*. Jede Phase endet mit lauffähiger App, grünem `make check`, Leistungsbudgets (E25) und
VoiceOver-Prüfung der neuen Ansichten.

- **Phase 0 — Fundament:** Workspace, Produktkonstanten (`crates/ide-product`, statt `product.toml`, E18), Lints,
  xtask (bundle, sign), i18n mit Test, Login-Shell-Umgebung, Supervisor (E23), Einstellungen und Kürzeltabelle (E26),
  Fenster mit Symbolspalte und Tableiste mit „+“, Menüleiste, Einstellungsfenster, Selftest-Gerüst wie splat. TOML in splats Tokenizer.
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
- **Phase 11 — Zed-Lücken und Architektur** *(freigegeben 2026-11-12, Einzelheiten in [phase-11.md](phase-11.md))*:
  Stufe 1 Absicherung (B20 Fuzzing und Property-Tests, B23 Doctests und Crate-READMEs, B24 Subagenten und
  Hintergrundprozesse im Sitzungskopf), Stufe 2 Editor (Prototyp E81 als Sperre, dann B14, B15, B16), Stufe 3 Struktur
  (B22, B21), Stufe 4 Vorhersage und Debugger (B4, B1), Stufe 5 Build (B19 als Teilschnitt).
- **Phase 12 — Lücken zu VS Code und Zed** *(freigegeben 2026-11-13; Einzelheiten in
  [phase-12.md](phase-12.md))*: Stufe 1 Komfort (B30 Puffer im Checkpoint, B29 Minimap und Sticky Scroll, B35
  Diagnosen am Zeilenende, B36 Live-Vorschau, B32a Prettier, B28 Einstellungen über iCloud), Stufe 2 Editor (B32b
  ESLint mit mehreren Servern je Dokument, B29c Emmet, B26 Vim-Modus mit Prototyp als Sperre), Stufe 3 B27
  Test-Explorer, Stufe 4 B33 Docker-Panel. Zurückgestellt und nicht umzusetzen: B25 Remote-Entwicklung, B31
  Jupyter-Notebooks, B34 Debugger für C# und Java; ihre Ausarbeitung bleibt im Dokument. Von Codex (C19–C45) und
  Fable 5.2 (F1–F39, G5–G38) geprüft, alle Befunde eingearbeitet.
- **Danach:** Store-Lite (E20), I2.

## Risiken
- **R1 Claude im AI-Tab zählt als Agent-SDK-Nutzung**; die angekündigte getrennte Abrechnung ist seit 2026-06-15
  pausiert.
- **R2 ACP-Crate bricht fast monatlich**; Version erst zur ACP-Phase festlegen, gekapselt. *(Seit 5.0 folgt die stabile v1-API semver; Dango nutzt 5.2.0, siehe docs/acp.md.)*
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
- **E46 Diktat als Build-Feature** `dictation`: im Release und im Debug-Build (`cargo xtask run`) an, in `make check` aus, eigenes `make check-dictation`.
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
  Ordnername, Neue Datei, Neuer Ordner und Alle einklappen; die Symbolspalte ist 44 pt breit. *(Seit E63 steht der
  Ordnername als „Dateien: <Ordner>“ in der Titelzeile des Panels, die Knöpfe rechts daneben.)*
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
- **E60 Native Fenstertabs** *(Entscheidung des Eigentümers, Abweichung von I10 und E17)*: Jedes Dokument —
  Datei, Terminal, Browser, AI- und CLI-Thread, Diff, Vorschau, PDF — öffnet als eigener macOS-Tab, ein `NSWindow` in
  der Tabgruppe des Ordners, mit einem kleinen Symbol vor dem Titel; die Tableiste ist immer sichtbar, gleich breite
  Tabs, Ziehen, Herauslösen und „Fenster zusammenführen“ wie in Safari. „+“ und ⌘T öffnen eine Neuer-Tab-Seite mit
  dem Angebot aus E17, die der gewählten Sache Platz macht; ⌘W schließt das Dokument mit der üblichen Rückfrage, ⇧⌘W
  alle Tabs des Fensters. Geteilt wird nur nebeneinander: „Daneben öffnen“ (Kontextmenü der Dateiliste) und die
  Vorschau (E59) zeigen ein zweites Dokument im selben Tab, jedes dann mit einer Kopfzeile (seit E63 ein Schild);
  schließt eines, bleibt das andere allein. Die Seitenleiste gibt es je Ordner einmal; sie wandert in den Tab im Vordergrund und bleibt
  dabei, wie sie ist. Die Ansicht je Projekt (E24) speichert Tabs und Teilungen. Ersetzt die eigene Tableiste nach
  dem Vorbild von Xcode und die Panels aus I10. Seit E145 teilt sich ein Tab in Bereiche.
- **E61 Das aktive Symbol klappt nicht ein** *(Entscheidung des Eigentümers, Abweichung von der Symbolspalte oben)*:
  Ein Klick auf das Symbol des Panels, das schon zu sehen ist, lässt die Seitenleiste offen. Ein- und ausgeklappt wird
  sie über den Seitenleisten-Knopf der Toolbar oder ⌃⌘S; ein Symbol bei eingeklappter Seitenleiste klappt sie auf.
- **E62 Jede Datei in einem eigenen Tab** *(Entscheidung des Eigentümers, Abweichung vom Vorschau-Tab der Tableiste)*:
  Ein Klick auf eine Datei, einen Diff oder ein PDF öffnet einen neuen Tab, oder holt den Tab nach vorn, der die
  Sache schon zeigt. Der Vorschau-Tab, den der nächste Klick ersetzt, ist eine Einstellung, die standardmäßig aus ist.
- **E63 Seitenleiste und Teilung, aufgeräumt** *(Entscheidungen des Eigentümers)*: Die Titelzeile des Dateien-Panels
  lautet „Dateien: <Ordner>“, die Knöpfe Neue Datei, Neuer Ordner und Alle einklappen stehen rechts in derselben
  Zeile; die eigene Ordnerzeile entfällt, ebenso der Trennstrich neben der Symbolspalte. Symbolspalte und Titel
  beginnen auf der Höhe der Tab-Oberkante, der Inhalt hält von den Symbolen denselben Abstand wie die Symbole vom
  Fensterrand. Zwei Dokumente nebeneinander teilen sich den Tab zur Hälfte; das Verhältnis bleibt bei jeder
  Größenänderung, nur Ziehen am Trennstrich ändert es. Kopfzeilen über den beiden entfallen, der Trennstrich reicht
  bis an die Tableiste; das zweite Dokument trägt oben rechts ein Schild mit Symbol, Namen (bei der Vorschau
  „Vorschau“) und ×, das nur dieses Dokument schließt; das erste schließt über den Tab.
- **E64 Alles kommt nach einem Neustart wieder** *(Entscheidung des Eigentümers, ergänzt E24 und E60)*: Jedes
  Ordnerfenster kommt mit seinen Tabs zurück, die Markdown-Vorschau mit ihrem Editor, AI-Threads als ihre Sitzung und
  CLI-Threads, die ihre Sitzung fortsetzen: Claude Code die eigene (`--resume`, die Sitzung erkennt Dango an ihrem
  Protokoll), Codex und Gemini die letzte des Ordners (`resume --last`, `--resume latest`); eine CLI ohne solche
  Option kommt nicht wieder. Wie bei Terminals nur in einem vertrauten Ordner (E22), und keinem wird dabei etwas
  geschickt. Je Ordner bleiben Fensterrahmen, Breite der Seitenleiste, das gezeigte Panel, ob es offen ist und ob die
  Seitenleiste eingeklappt ist; ein Ordner ohne eigene Werte übernimmt die des zuletzt gesicherten Fensters, ein
  Stück versetzt, und ein Rahmen, den kein Bildschirm mehr zeigt, rückt auf den Hauptbildschirm. Ab dem Beenden wird
  nichts mehr nachgeschrieben, damit endende Terminals und Agenten den gesicherten Stand nicht verändern.
- **E65 Das „+“ der Tableiste: Klick oder Halten** *(Entscheidung des Eigentümers, ergänzt E17 und E60)*: Ein Klick
  öffnet die Neuer-Tab-Seite wie ⌘T. Wird das „+“ länger als 0,35 s gehalten oder mit gedrückter Taste verlassen,
  öffnet sich darunter dasselbe Angebot als Menü; der gewählte Eintrag öffnet sofort in einem neuen Tab, ohne die
  Seite, und wer nichts wählt, öffnet nichts. Das Menü wie die Seite bieten unter „AI-Thread“ (auch „In eigenem
  Worktree“) nur Agenten an, die installiert und eingerichtet sind: Claude, wenn sein Programm da ist, ACP-Agenten,
  die starten, statt sie erst zu installieren, und den eigenen Agenten zuletzt, sobald ein Anbieter ein Modell
  anbietet. Installiert wird über „Agenten verwalten …“ (E58).
- **E66 Farben nach Apples Richtlinien, Anbieterfarbe nur in der Tableiste** *(Entscheidung des Eigentümers)*: Der
  Chat, das AI-Panel und die Aktivitätsübersicht nutzen die Farben des Systems: die eingestellte Akzentfarbe für
  Aktives und Bedienbares (offene Fragen und Pläne, laufende Aufgabe, Fokus, Senden, Hauptknöpfe, Laufpunkte,
  Lesezeichen), Statusfarben für Erfolg, Warten und Fehler, neutrale Töne für schmückende Symbole. Nur in der
  Tableiste unterscheidet sich jeder Anbieter: das Symbol eines AI- oder CLI-Tabs trägt seine Farbe (der Punkt folgt E71) –
  Claude Terrakotta, Codex Blau, Googles Agenten (Antigravity, Gemini) die vier Farben des Gemini-Sterns, OpenCode Dunkelgrau,
  der eigene Agent Grün, andere ACP-Agenten ihre Farbe aus dem Register, sonst die Systemakzentfarbe –, so angepasst an Hell,
  Dunkel und erhöhten Kontrast, dass sie 3:1 zum Hintergrund halten, mit erhöhtem Kontrast 4,5:1.
- **E67 Gemini wird nicht mehr angeboten** *(Entscheidung des Eigentümers)*: Google hat die Anmeldung der Gemini CLI
  mit einem privaten Google-Konto beendet und verweist auf Antigravity; ohne API-Schlüssel startet Gemini keine
  Sitzung mehr. Gemini erscheint deshalb weder im „+“-Menü, auf der Neuer-Tab-Seite, im Worktree- und AI-Panel-Menü
  noch in der Agentenliste der Einstellungen oder als CLI-Thread. Die Definition bleibt, damit vorhandene Gemini-
  Verläufe lesbar bleiben; Antigravity übernimmt.
- **E68 Das AI-Panel ist eine Liste** *(Entscheidung des Eigentümers, ersetzt die Gliederung nach Screenshot 2)*:
  Unter dem Titel „Agenten“, der rechts „+ Neue Sitzung“ anbietet, steht eine einzige Liste aller Sitzungen des
  Ordners, ohne Abschnittsüberschriften: Lesezeichen zuerst, sonst die neueste oben. AI- und CLI-Sitzungen (offene
  CLI-Threads und im Terminal gespeicherte Claude-Sitzungen) sehen gleich aus: vorn ein Kreis, grün gefüllt, wenn die
  Sitzung offen ist, sonst leer; dahinter Titel und Alter, kein Anbietername, keine Kosten. Eine Sitzung, die ein
  AI-Tab fortsetzt, steht nur einmal da. Ein Klick öffnet jede Sitzung. Umbenennen, Lesezeichen und Archivieren
  bleiben im Kontextmenü, dort auch „Archiv zeigen“. Gruppen, Suche, „Aktiv · N“ und der Baum „Laufend“ entfallen;
  was läuft, zeigt die Aktivitätsübersicht. Die Nutzung des Abos steht in der Statuszeile des Claude-Tabs: links der
  Kontext, rechts Abo oder Kosten mit Wochenlimit und 5-Stundenlimit.
- **E69 Keine Badges in der Symbolspalte** *(Entscheidung des Eigentümers, ändert E3)*: Die Symbole der Seitenleiste
  tragen weder Zahl noch Punkt. Die Zahl der Git-Änderungen und der Zustand des letzten CI-Laufs stehen nur noch im
  Tooltip und in der VoiceOver-Beschriftung des Symbols („Git, 3 Änderungen“, „CI: fehlgeschlagen“).
- **E71 Der Punkt im Tab: grün, rot oder keiner** *(Entscheidung des Eigentümers, ändert E66)*: Ein AI- oder CLI-Tab
  zeigt rechts einen grünen Punkt, solange die Sitzung arbeitet, einen roten, solange sie auf eine Aktion des Nutzers
  wartet (Freigabe, Frage, Plan; bei einer CLI die Glocke), und keinen, wenn sie untätig ist. „Fertig, noch nicht
  angesehen“ zeigt keinen Punkt mehr; das meldet die Mitteilung. Die Anbieterfarbe bleibt beim Symbol des Tabs.
- **E72 Keine Zahl am Dock-Symbol, ein Ordner ein Eintrag** *(Entscheidung des Eigentümers, ändert I9)*: Das
  Dock-Symbol trägt keine Zahl; dass eine Sitzung wartet oder fertig ist, melden der rote Punkt im Tab (E71) und die
  Mitteilung. Fenstermenü und Dock-Menü nennen jeden Ordner einmal, mit dem Tab, der vorn liegt; ein herausgezogener
  Tab in einem eigenen Fenster bleibt dort genannt.
- **E73 Toolbar mit Verlauf, Branch und Schnellöffnen** *(Entscheidung des Eigentümers, ergänzt die Fensterbeschreibung)*:
  Vor dem Titel stehen Zurück und Vorwärts (⌃- und ⌃⇧-, auch im Menü „Gehe zu“): Sie führen durch die Stellen in
  Editoren, die der Nutzer für eine andere Datei oder einen Sprung über mehr als zehn Zeilen verlassen hat, je Fenster.
  Rechts stehen der Branch als Auswahl mit allen Branches und „Neuer Branch …“ (nur wo git läuft, E22), die Lupe für
  „Schnell öffnen“ und die Aktivität. „Neues Terminal“ und „Neuer AI-Thread“ entfallen; das „+“ der Tableiste deckt
  sie ab (E65).
- **E70 Datei-Icons nach macOS-Art** *(Entscheidung des Eigentümers, ersetzt VS Codes Seti)*: Dateibaum, Suche,
  Open Quickly und Tabs zeigen für die häufigen Sprachen das Logo aus Simple Icons (einfarbig, `assets/simple-icons`),
  für alles andere ein SF Symbol der Dateiart, beide in Apples Systemfarben mit 3:1 Kontrast (E66). Logos mit
  nicht-kommerzieller Lizenz (Sass, Vue) entfallen; Ordner behalten ihren Pfeil statt eines Icons.
- **E74 Git-Panel nach VS-Code-Art** *(Entscheidung des Eigentümers, ergänzt E11)*: Abschnitte heißen in normaler
  Schreibung mit der Zahl als Pille rechts; Dateien stehen nicht eingerückt, ihr Datei-Icon (E70) bündig mit dem Pfeil
  ihres Abschnitts, der Statusbuchstabe rechts. „Letzte Commits“ wird zum „Graph“: HEAD und sein Upstream, 50 Commits
  und „50 weitere laden …“, je Commit ein Punkt in seiner Spur unter dem Pfeil des Abschnitts, Linien zu den Eltern in
  Spurfarben (Akzentfarbe zuerst, E66), HEAD als Ring, Merges als Ring mit Punkt, Branches, Remote-Branches und Tags
  als Pillen. Ein Klick klappt den Commit auf und listet seine Dateien, deren Klick den Diff des Commits zeigt; der
  Tooltip nennt Autor, Datum, die ganze Nachricht und die geänderten Dateien und Zeilen. Das Kontextmenü kopiert
  Commit-ID und Nachricht und legt dort einen Branch an. Neben „Commit“ bietet ein Menü „Committen und pushen“ und
  „Committen und synchronisieren“ (Pull, dann Push).
- **E75 Dokumente in Quick Look** *(Wunsch des Eigentümers, ergänzt Phase 9 und E62)*: Bilder, Audio, Video,
  Schriften und Office-/iWork-Dokumente öffnen wie PDFs in einem eigenen Tab, gezeigt von Quick Look
  (`QLPreviewView`) wie im Finder; PDFs bleiben bei PDFKit. Welche Dateien das sind, entscheidet eine feste Liste
  von Endungen, nicht der UTType, weil macOS `.ts`/`.mts` als Video und `.key` als Keynote führt. SVG bleibt Text
  im Editor; „Als Vorschau öffnen“ im Kontextmenü der Dateiliste zeigt die Zeichnung in einem eigenen Tab
  „Vorschau: <Datei>“.
- **E76 Terminal zeichnet mit Metal** *(Entscheidung des Eigentümers, ändert E8)*: Das Terminal setzt jedes Bild
  auf der GPU aus Bausteinen zusammen (Schriftzeichen, Rahmenzeichen, Unterstreichungen), die Core Graphics einmal
  zeichnet; Schrift und Farben bleiben damit wie bei Core Text, ein Test vergleicht beide Wege Pixel für Pixel. Pro
  Bild kostet das den Hauptthread rund 0,7 ms statt 2–17 ms (200 × 60 Zeichen, M3), unabhängig von der Zahl der
  Farbwechsel. Der Core-Graphics-Weg bleibt für Bilder der Ansicht (`cacheDisplayInRect`) und als Rückfall ohne Metal
  oder mit `IDE_TERMINAL_CPU`. Ein von anderen Fenstern verdecktes Fenster zeichnet erst, wenn es wieder sichtbar ist.
- **E77 CI-Aktionen, Merge Requests und Issues** *(Entscheidung des Eigentümers, ergänzt E12)*: Das CI-Panel
  startet außer fehlgeschlagenen Jobs einer Pipeline auch einen einzelnen beendeten Job neu, einen wartenden Job
  (GitLab: manueller Job; GitHub: Freigabe der Deployments seines Laufs, die auf dieses Konto warten) und einen neuen
  Lauf für den Branch (GitLab: neue Pipeline; GitHub: ein Workflow des Arbeitsstands mit `workflow_dispatch`, per
  Auswahl). GitHub startet zusätzlich alle Jobs eines Laufs neu. Ein Umschalter im Panel zeigt statt der Läufe die
  offenen Merge Requests (GitHub: Pull Requests) oder Issues, zuletzt geänderte zuerst, je höchstens 50; ein
  Doppelklick öffnet sie im Browser, „+“ legt im Browser einen neuen für den Branch oder ein neues Issue an.
  Bearbeiten, Kommentieren und Mergen bleiben im Browser. Schreibende Aufrufe brauchen bei GitLab den Bereich `api`.
  *Nachtrag (Eigentümer):* Seiten des CI-Panels und des Log-Tabs öffnen im Browser-Tab (E17) des Fensters, nicht im
  Standardbrowser; die Anmeldung (E12) bleibt im Standardbrowser, weil OAuth eingebettete Web-Views ausschließt (RFC 8252).
- **E78 Agenten direkt im „+“-Menü, keine CLI-Threads** *(Entscheidung des Eigentümers, ändert E17, E60, E64, E68 und
  ersetzt I5)*: Das „+“-Menü und die Neuer-Tab-Seite nennen die installierten Agenten direkt, jeden als eigenen
  Eintrag bzw. Knopf untereinander, den eigenen Agenten mit seinen Endpunkten als Untermenü; darunter „In eigenem
  Worktree ▸“ und „Agenten verwalten …“. Das Untermenü „AI-Thread“ entfällt. CLI-Threads (I5) entfallen ganz: kein
  Start mehr, keine Zeilen in AI-Panel und Aktivitätsübersicht, kein Statuspunkt. Ein CLI-Thread aus einem früher
  gesicherten Fensterlayout öffnet als Terminal im Projektordner.
- **E79 Keine Neuer-Tab-Seite, „+“ und ⌘T öffnen das Menü** *(Entscheidung des Eigentümers, ändert E60, E65 und
  E78)*: Die Neuer-Tab-Seite entfällt. Ein Druck auf das „+“, kurz oder gehalten, öffnet darunter das Menü aus E17
  und E78; ein Klick lässt es offen, ein gehaltener Druck zieht hinein. ⌘T und „Ablage ▸ Neuer Tab …“ öffnen
  dasselbe Menü am „+“. Ein Tab entsteht erst mit dem gewählten Eintrag; wer nichts wählt, öffnet nichts.
- **E80 Symbole der Seitenleiste ein- und ausblenden** *(Entscheidung des Eigentümers, ergänzt E3)*: Ein Rechtsklick
  in die Symbolspalte öffnet ein Menü mit jedem Panel, angehakt, solange sein Symbol zu sehen ist; ein Klick blendet
  es aus oder wieder ein, in allen Fenstern und über den Neustart hinaus (Einstellung `hiddenSidebarPanels`). Das
  letzte sichtbare Symbol bleibt. Zeigt die Seitenleiste gerade das ausgeblendete Panel, rückt das erste sichtbare
  nach. Menüleiste und Tastenkürzel öffnen ein ausgeblendetes Panel weiterhin.

Phase 11 *(freigegeben 2026-11-12)*: Kurzfassung; die Einzelheiten in [phase-11.md](phase-11.md) sind ebenso
verbindlich. E89 und E90 sind mit dem gestrichenen MCP-Server (B11) entfallen.

- **E81 Einblendungen:** ein gemeinsamer Baustein für Text, der nicht in der Datei steht (Inlay Hints, Ghost Text,
  Blame, Inline-Werte), außerhalb von Undo und Sichern; Prototyp mit Prüfliste als Sperre vor Stufe 2.
- **E82 Code Actions:** Glühbirne in der Randspalte, ⌥⌘A und „Editor ▸ Schnellkorrektur …“; Workspace-Edits an die
  Revision gebunden (E24).
- **E83 Inlay Hints:** Standard an, je Sprache abschaltbar, über E81.
- **E84 Signaturhilfe:** kleines Fenster über dem Cursor, nimmt nicht den Fokus, „Editor ▸ Parameter zeigen“.
- **E85 LSP-Navigation:** Projekt-Symbolsuche, Implementierung, Typdefinition, Deklaration, Aufrufhierarchie.
- **E86 Blame:** am Zeilenende (Standard aus) und als Spalte, nur in vertrauten Ordnern.
- **E87 Stash:** Abschnitt im Git-Panel; Löschen zerstörerisch mit Rückfrage, auch für den eigenen Agenten (E27, E33).
- **E88 Commit-Message per AI:** genau der mit gitleaks geprüfte Text geht hinaus, sonst nichts.
- **E91 Edit Prediction mit Copilot und lokal** (Ollama, LM Studio) hinter einer Schnittstelle.
- **E92 Verhalten der Vorhersage:** nach Tipp-Pause, an die Revision gebunden, verworfen statt geraten.
- **E93 Crate `ide-dap`:** Debug Adapter Protocol über den Supervisor, nur in vertrauten Ordnern.
- **E94 Debugger:** Panel „Debuggen“ über CI, das als letztes steht (⌘5, CI ⌘6; Abweichung von E55), Inspektor, Menü
  „Debuggen“.
- **E95 Fuzzing mit cargo-fuzz** (`make fuzz`, nightly, außerhalb von `make check`), Funde werden Regressionstests.
- **E96 proptest in `make check`** mit fester Fallzahl und festem Seed.
- **E97 Ein App-Kontext statt Registern:** `thread_local!` nach Zustand und Lebensdauer aufgeteilt.
- **E98 Model ohne AppKit, View ohne Logik** je Panel und Tab.
- **E99 UI-Crates als Teilschnitt:** nur kreisfrei abtrennbare Module werden eigene Crates.
- **E100 Doctests und Crate-READMEs** mit Mermaid-Diagramm des Kontrollflusses, ein Test prüft beides.
- **E101 Apples Human Interface Guidelines als Abnahme** (ergänzt E3): Menüleiste für jeden Befehl, keine
  HIG-Standardkürzel umbelegt, ausgegraut statt ausgeblendet, Barrierefreiheit, Kennzeichnung von AI.
- **E102 Subagenten und Hintergrundprozesse im Sitzungskopf** *(umgesetzt)*: zwei Knöpfe (`person.2`,
  `terminal`) statt „Aufgaben dieser Sitzung“, Akzentfarbe solange etwas läuft, Zahlen im Tooltip; beendete
  Hintergrundprozesse bleiben in der Liste.
- **E103 Diktat-Einstellungen in Dango** *(Entscheidung des Eigentümers, ändert E13)*: Modus und Modell (online
  OpenAI und Groq, lokal Whisper und Parakeet), Sprache, Nachbearbeitung, Mikrofon (Liste der Eingabegeräte) und
  Vokabular stellt der Nutzer in Einstellungen ▸ Diktat ein, ohne dass Lowspeak installiert sein muss. Sie stehen im
  Eintrag `dictation` der versionierten `settings.json` (E26) mit lowspeaks gespeicherten Namen; ein unbekannter Name
  fällt einzeln auf Lowspeaks Vorgabe zurück, und weil der Eintrag nur hinzukommt, braucht die Datei keine Migration.
  Jedes Diktat liest sie beim Start. Weichen Lowspeaks Einstellungen (Suite `com.lowspeak.app`) von den Vorgaben ab,
  bietet der Tab einmal an, sie zu übernehmen; danach liest Dango die Suite nicht mehr. Fehlt das gewählte lokale
  Modell, lädt der Tab es mit lowspeaks `model_store::download` von Hugging Face in die Ordner, die auch Lowspeak
  nutzt, mit Fortschritt und Abbrechen. API-Schlüssel lassen sich eingeben, ändern, entfernen und aus Lowspeak
  übernehmen (R4 wie in E13).

Risiken aus Phase 11: **R14** Einblendungen über das Kern-Attribut, **R15** Code verlässt den Mac (Copilot,
Commit-Messages), **R16** Lieferkette der Debug-Adapter, **R17** Umbau ohne neue Funktion, **R18** nightly für
Fuzzing, **R19** Indexierung bei der Projekt-Symbolsuche, **R21** veraltete Vorschläge und Aktionen, **R22**
undokumentierte Meldungen von Claude zu Subagenten und Hintergrundarbeit.

Phase 12 *(freigegeben 2026-11-13)*: Kurzfassung; die Einzelheiten in [phase-12.md](phase-12.md) sind ebenso
verbindlich. Zurückgestellt und erst bei Wiederaufnahme gültig: B25 Remote (E115–E129), B31 Notebooks (E137–E139),
B34 C#- und Java-Debugger (E134, E135).

- **E104 Vim über `modalkit`** als Kopie unter `vendor/` (ohne `ropey`-Standard-Features), Crate `ide-vim`;
  Prototyp mit Variante A (Schattenkopie mit `EditBuffer`) als Sperre.
- **E105 Tastenverteilung im Vim-Modus:** alle Modi über den Eingabekontext, Esc schließt nur das Erste, was offen ist.
- **E106 Umfang der ersten Vim-Ausgabe:** Normal, Einfügen, Visual, Ersetzen, eigene Suche, wenige Ex-Befehle;
  Block-Cursor als eigene Sperre.
- **E107 Minimap** neben dem Editor, Standard aus *(umgesetzt)*.
- **E108 Sticky Scroll,** Standard an, höchstens 5 Kopfzeilen, aus Dokument-Symbolen oder Einrückungs-Faltungen.
- **E109 Emmet** als zweiter Server je Dokument über `@olrtg/emmet-language-server`.
- **E110 Crate `ide-test`** mit einem Adapter je Werkzeug (cargo test, nextest, Go, pytest, Vitest, Jest).
- **E111 Panel „Tests“** vor „Debuggen“ (Tests ⌘5, Debuggen ⌘6, CI ⌘7; ändert E94), Randspalte, ⌃⌥⌘U und ⌃⌥⌘G.
- **E112 Einstellungen über einen Ordner in iCloud Drive,** nicht CloudKit; vorher ein Besitzer der Einstellungen.
- **E113 Abgleich je Schlüssel festgelegt,** `sync` als Pflichtargument der Konstruktoren.
- **E114 Eine Datei je Mac,** zusammengeführt als Maximum einer logischen Uhr je Schlüssel.
- **E130 Diagnosen am Zeilenende** über die Einblendungen (E81), Standard aus.
- **E131 Statische Seiten im Browser-Tab** über einen eigenen Nur-GET-Server auf 127.0.0.1 mit Cookie, nur in
  vertrauten Ordnern.
- **E132 ESLint als zweiter Server je Dokument,** aus einem festen Tag von `microsoft/vscode-eslint`.
- **E133 Prettier als Formatierer** neben dem Sprachserver, an die Revision gebunden.
- **E136 Docker-Panel** über die `docker`-CLI.
- **E140 Mehrere Sprachserver je Dokument,** Slots nach Register-Eintrag und Wurzel.

Code-Review 2026-11-14 *(nichtfunktional, vom Eigentümer beauftragt)*:

- **E141 Fehler in der Sprache des Nutzers:** Jeder Fehler, den der Nutzer liest, sagt über `ide_core::explain::Explain`
  in seiner Sprache, was schiefging; das `Debug`-Format bleibt dem Systemprotokoll. App und UI-Crates führen keine
  Fehler als bloßen `String`, sondern als Typ oder als schon übersetztes `Explained`; `cargo xtask rules` prüft beides.
- **E142 Schreiben und Hintergrundarbeit über Lanes:** ⌘S schreibt auf der Disk-Lane und kehrt sofort zurück;
  Schließen und Beenden warten auf ihren Schreibvorgang. Keine Arbeit auf der Disk-Lane wartet auf den Haupt-Thread
  (Workspace-Edits haben eine eigene Lane). App und UI-Crates starten Threads nur über `util::spawn` oder eine Lane;
  `cargo xtask rules` prüft das.
- **E143 Zustand wird übergeben, nicht nachgeschlagen:** Neuer Code bekommt den App-Kontext, das Objekt mit dem
  Zustand oder eine Fenster-ID übergeben. Nachgeschlagen wird nur dort, wo AppKit oder ein Worker-Thread mit einer ID
  zurückruft (E97). Zwei Ratchets in `cargo xtask rules` zählen die Zugriffe über `app()`/`try_app()` und über
  `state()` der UI-Crates; die Zahl sinkt, steigt aber nie. Geheimnisse liest die App nur aus dem übergebenen Speicher;
  vorher lehnt der Zugriff ab, statt still in den Arbeitsspeicher auszuweichen.
- **E144 Sprachserver als eigene UI-Crate `ide-ui-lsp`** *(erweitert E99)*: Sie erreicht einen Editor nur über die
  Schnittstelle `Editor` (Textansicht, Dokument, Ordner, Fenster-ID und die wenigen Dinge, die eine Serverantwort im
  Editor ändert) und die App über ihren `Host`; Einblendungshinweise liefert sie als eigene Daten, die der Editor in
  seine Einblendungen (E81) übersetzt. Der Editor führt seine Nummer selbst und beobachtet sein Scrollen selbst.
  Selbsttest-Funktionen liegen in den `selftest_probe`-Modulen ihrer Datei, nie im Produktivcode.
- **E145 Bereiche nach dem Vorbild von Xcode** *(Entscheidung des Eigentümers, ändert E60, E62 und E63)*:
  - **Teilen:** Ein Fenster teilt sich in Bereiche, rechts oder unten und beliebig verschachtelt.
    - Werkzeugleiste (Klick rechts, mit ⌥ unten), Menü Darstellung › Bereiche, Kontextmenü eines Bereichs-Tabs, Ziehen eines Tabs an den rechten oder unteren Rand.
  - **Echte Teilung:** Beim Teilen ziehen alle Tabs des Fensters in den ersten Bereich, und die native Tableiste verschwindet.
    - Jeder Bereich trägt oben seine eigene Leiste im Look der nativen, mit × links zum Schließen des Bereichs. Die Trennlinie reicht bis unter die Werkzeugleiste.
    - Bleibt nur ein Bereich übrig, bekommt jedes Dokument wieder einen eigenen nativen Tab (E60).
  - **Öffnen:** Ungeteilt öffnet eine Datei wie bisher einen eigenen Tab (E62); geteilt geht sie in den aktiven Bereich.
    - Eine Datei ist je Fenster höchstens einmal offen; eine offene kommt nach vorn, wo sie ist.
    - Die Vorschau neben ihrem Editor (E59) ist keine Teilung. Sie folgt ihrem Editor und behält ungeteilt ihr Schild (E63).
  - **Befehle:**
    - Teilen ⌃⌘T rechts und ⌃⌥⌘T unten. Hält der Bereich mehrere Dokumente, wandert das vordere in den neuen Bereich; sonst öffnet er leer und nimmt das nächste Dokument auf.
    - Bereich schließen ⌃⇧⌘W, Bereich maximieren ⌃⇧⌘↩.
    - Nächster und vorheriger Tab im Bereich ⌃⌥⌘] / ⌃⌥⌘[ (⌃⇥ bleibt dem Fokus vorbehalten, E101).
    - ⌥⌘` geht zum nächsten Bereich.
    - In der Dateiliste: ⌥-Klick und „Daneben öffnen“ für rechts, „Darunter öffnen“ für unten, ⌥⇧-Klick für die Wahl des Bereichs.
  - **Speichern:** Die Ansicht je Ordner speichert den Baum mit Anteilen und dem vorderen Dokument je Bereich (Layout-Version 4).
  - **Nicht übernommen:** Assistent-Modus, Sprungleiste und Verlauf je Bereich.
- **E146 Overlays für die Einstellungen der Agenten** *(Entscheidung des Eigentümers, ändert E27)*:
  - **Was:** Was die CLI eines Agenten nur im Terminal als Dialog zeigt, zeigt Dango als Overlay im AI-Tab: `/mcp`, `/plugins`, `/permissions`, `/hooks`, `/memory`, `/config`, `/output-style`, `/btw`, `/export`, `/status`, `/skills`, `/logout`, `/bug`.
  - **Gleiche Funktion, gleiche Oberfläche:** ein Overlay je Funktion über einem neutralen Datenmodell, für Claude, ACP-Agenten, Codex und OpenCode gleich. Unterschiede zeigen sich nur als ausgegraute Aktion mit Begründung, nie als anderer Aufbau.
  - **Geschrieben wird über die Schreibwege des Agenten**, nie direkt in seine Dateien:
    - Claude: Steuernachrichten und die Unterbefehle `edit-permission-rules`, `edit-hook`, `edit-memory-settings` und `edit-skill-overrides`, die auch die VS-Code-Erweiterung nutzt;
    - Codex: `config/*` des App-Servers.
  - **Ändert E27:** Die Seite „Gespeicherte Regeln“ entfernt über `edit-permission-rules`, mit Rückfall auf `rules.rs`, wenn Claude Code fehlt oder den Schreiber nicht kennt. Sie liest weiter die Dateien selbst: Sie listet die Regeln aller Projekte, und `list_permission_rules` braucht eine laufende Sitzung im jeweiligen Projekt. `/permissions` zeigt die Regeln der Sitzung im Overlay. Ein Schreibweg, zwei Ansichten.
  - **Grenzen:** Kann eine CLI-Version eine Funktion nicht, zeigt das Overlay das und bietet „Im Terminal öffnen“ an. Die Steuernachrichten gehören in E21 zur Stufe „Best effort“ (siehe docs/claude-mapping.md).
- **E147 SDK zuerst, ACP als Rückfall** *(Entscheidung des Eigentümers, ändert E21 und E5)*:
  - **Was:** Jeder Agent läuft über das offizielle SDK-Protokoll seines Herstellers, wo es eines gibt; ACP bleibt der Rückfall:
    - Claude: Stream-JSON, wie bisher;
    - Codex: `codex app-server`, die Grundlage des Codex SDK;
    - OpenCode: `opencode serve`, die Grundlage von `@opencode-ai/sdk`.
  - **Antigravity bleibt bei ACP:** `agy acp` ist der offizielle Weg; das SDK `google.antigravity` ist eine Bibliothek zum Bauen eigener Agenten, keine Fernsteuerung.
  - **Ändert E21:** aus „Claude direkt, ACP für alle anderen“ wird „das offizielle SDK-Protokoll, wo vorhanden, sonst ACP“.
  - **Ändert E5:** Je SDK-Weg gilt ein erprobter Versionsbereich (Codex app-server ab 0.167, OpenCode ab 2.2.6). Außerhalb davon, oder wenn der Handschlag scheitert, startet der Tab über den ACP-Adapter desselben Agenten und sagt warum.
  - **Fortsetzen:** Eine fortgesetzte Sitzung behält ihren Weg.
  - **Umsetzung Codex (abweichend vom ersten Entwurf):** kein eigener Agent-Typ, sondern ein zweiter Weg derselben Codex-Auswahl (Preset `codex-acp`): Beim Start prüft der Tab `codex --version` und nimmt bei 0.167 oder neuer den App-Server, sonst `codex-acp`. Modell, Aufwand und Modus erscheinen im Composer wie bei einem ACP-Agenten. Details in [codex.md](codex.md).
  - **Umsetzung OpenCode:** ebenso ein zweiter Weg der OpenCode-Auswahl (Preset `opencode`). 2.2.6 bringt eine neue v2-API unter `/api/…` statt der v1-Routen; `opencode serve --stdio` öffnet trotz des Namens einen HTTP-Server auf 127.0.0.1 mit Einmal-Passwort je Start. Details in [opencode.md](opencode.md).
- **E148 Remote Control für Codex über den gemeinsamen App-Server** *(Entscheidung des Eigentümers, weicht von E10 und E23 ab)*:
  - **Was:** Das Handy im Kopf des KI-Tabs gibt es auch für Codex. Das Handy erreicht nur Codex' gemeinsamen App-Server, den Dienst, den `codex app-server daemon` verwaltet. Ist Remote Control an, verbindet sich ein Codex-Tab über `codex app-server proxy` mit diesem Dienst, statt einen eigenen App-Server zu starten.
  - **Für den ganzen Mac, nicht je Sitzung:** Remote Control gehört bei Codex dem Dienst. Einschalten (`codex remote-control start`) und Beenden (`remoteControl/disable`) gelten für alle Codex-Sitzungen des Macs und bleiben für den nächsten Start gespeichert. Das unterscheidet sich von Claude, wo Remote Control je Sitzung gilt.
  - **Umzug:** Ein Tab mit eigenem App-Server wechselt nach dem Einschalten auf den Dienst, sobald der Agent fertig ist, und setzt seinen Thread dort fort. Neue Tabs starten gleich dort, solange die gespeicherte Wahl an ist; startet der Dienst nicht, bleibt der eigene App-Server.
  - **Fremde Threads:** Am Dienst hört jede Verbindung auch die Threads der anderen Clients. Dango zeigt und beantwortet nur den eigenen Thread und dessen Unteragenten.
  - **Weicht ab von E10 und E23:** Der Dienst ist kein Kindprozess unter Dangos Supervisor. Startet Dango ihn, erbt er die Login-Shell-Umgebung; läuft er schon, weil die Codex-App oder ein Terminal ihn startete, bekommen die Befehle, die Codex ausführt, dessen Umgebung. Unter dem Supervisor laufen der Proxy und die Aufrufe der Codex-CLI.
  - **Experimentelle API:** Nur die Verbindung zum Dienst meldet sich dafür an, weil Codex das Abschalten nur dort anbietet. Details in [codex.md](codex.md).

Risiken aus Phase 12: **R23** `modalkit` 0.0.x mit einem Betreuer, **R24** Vim-Nutzer messen am Original, **R25**
iCloud Drive langsam und nicht steuerbar, **R28** Emmet-Server kaum gepflegt, **R29** Testausgaben ohne festen
Vertrag, **R32** Feature-Vereinigung von Abhängigkeiten, **R33** iCloud-Erlaubnis gilt für die ganze App, **R37**
Docker-Socket entspricht Root-Rechten, **R38** ESLint-Server aus einem Upstream-Tag. Zu den zurückgestellten
Bausteinen gehören R26, R27, R30, R31, R34, R35, R36 und R39.

- **Änderungen an splat und lowspeak** bleiben in deren Repos auf `vibeedit/*`-Branches, uncommittet bis zur Übernahme.

Risiken aus E146 und E147:
- **R40** Interne Schnittstellen von Claude: die Steuernachrichten mit `@internal` und die `edit-*`-Unterbefehle können sich mit jeder Version ändern. Abhilfe: nachsichtige Typen, Fixtures je Version, Rückfall aufs Terminal.
- **R41** Unfertige SDK-Protokolle: Codex' App-Server ist „experimental“, OpenCodes API ändert sich je Release. Abhilfe: erprobter Bereich (E147), Schemata als Fixtures, Rückfall auf ACP.
- **R42** Geheimnisse in MCP-Definitionen: Tokens nie in Prozessnamen oder Protokollen. Dangos eigene Liste hält sie im Schlüsselbund. `.mcp.json` wird geschrieben wie eingegeben, mit Hinweis.
- **R43** Daten nach außen: `/bug` und Feedback senden an den Hersteller, nur nach einer Bestätigung, die nennt, was gesendet wird.
- **R44** Umfang: Codex und OpenCode sind je ein neuer Agentenweg in der Größe von `ide-claude`. Sie kommen nach den Overlays für Claude und ACP, jeweils als eigene Phase.

Risiken aus E148:
- **R45** Remote Control von Codex ist experimentell: Dienst, Proxy und `remoteControl/*` können sich je Release ändern, und das Koppeln hakt laut offenen Issues bei Codex. Abhilfe: Startet der Dienst nicht, bleibt der eigene App-Server; Fehler zeigt das Handy-Symbol.
- **R46** Fremde Umgebung: Läuft der Dienst schon, bekommen Codex' Befehle dessen Umgebung statt der Login-Shell (E10, E23).
- **R47** Tab und Dienst trennen sich: Schließen eines Tabs beendet nur Dangos Verbindung; was der Dienst mit einem laufenden Turn tut, entscheidet Codex.

Neue Risiken: **R9** Last durch parallele Agenten (Zeitmessungen nur über `make perf`), **R11** Nutzungslimit des
Claude-Kontos (keine Live-Läufe echter Agenten-CLIs in Tests), **R12** Selbsttest-Fenster dürfen nie den Fokus nehmen
(Wächter im Testlauf), **R13** Dango baut gegen den Arbeitsstand von `../splat` und `../lowspeak`.
