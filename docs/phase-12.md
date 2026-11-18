# Dango — Phase 12: Lücken zu VS Code und Zed

**Freigegeben am 2026-11-13.** Die Entscheidungen E104–E140 und die Risiken R23–R39 stehen als Kurzfassung in
[plan.md](plan.md); die Einzelheiten hier sind ebenso verbindlich. Die Kürzel B25–B29
stammen aus dem Vergleich mit Zed und VS Code vom 2026-11-13. Im Gespräch hießen sie B2, B3, B6, B8 und B9; sie wurden
verschoben, weil plan.md und [phase-11.md](phase-11.md) B1–B24 schon vergeben haben. B30 ist ein Befund aus dem
Review. B31–B36 stammen aus einem Abgleich mit den meistinstallierten VS-Code-Erweiterungen (Sitzung dango-7a,
2026-11-13, dort G1–G4 und zwei kleine Lücken); der Owner hat alle sechs übernommen. *Review:* Am 2026-11-13 haben
zwei Reviews den Entwurf gegen Code und Plan geprüft: Codex mit 27 Befunden (C19–C45, die Zählung setzt die aus Phase
11 fort) und Fable 5.2 mit 39 Befunden (F1–F39). Alle sind eingearbeitet und an der jeweiligen Stelle markiert.
B31–B36 kamen danach dazu; Fable 5.2 hat sie in einem dritten Review geprüft, seine 34 Befunde (G5–G38, G1–G4 waren
die Vorschläge selbst) sind ebenfalls eingearbeitet und markiert.

Umfang: B26 Vim-Modus mit `modalkit` · B27 Test-Explorer · B28 Einstellungen über iCloud abgleichen · B29 Minimap,
Sticky Scroll und Emmet · B30 ungesicherte Puffer im Checkpoint · B32 ESLint und Prettier · B33 Docker-Panel · B35 Diagnosen am
Zeilenende · B36 Live-Vorschau statischer Seiten.

**Zurückgestellt (Owner, 2026-11-13):** B25 Remote-Entwicklung, B31 Jupyter-Notebooks und B34 Debugger für C# und Java
werden in dieser Phase nicht umgesetzt. Ihre ausgearbeiteten Entscheidungen, Risiken und Annahmen stehen im Kapitel
[Zurückgestellt](#zurückgestellt) und gelten erst, wenn der Owner sie wieder aufnimmt. B29c Emmet war kurz
zurückgestellt und ist wieder im Umfang, als zweiter Nutzer von E140 nach ESLint.

## Reihenfolge

| Stufe | Inhalt | Größe | Warum an dieser Stelle |
|---|---|---|---|
| 1 Komfort | B30, B29 Minimap und Sticky Scroll, B35, B36, B32a Prettier, B28 | L | Unabhängig von allem. B30 behebt eine Lücke von heute. B35 nutzt die Einblendungen aus E81. Prettier braucht den zweiten Server nicht *(G37)*. B28 beginnt mit A5 und dem Besitzer der Einstellungen (F9) |
| 2 Editor | B32b ESLint mit E140 (mehrere Server je Dokument), B29c Emmet, B26 | XL | *(F30)* Beginnt mit den Prototypen von E104 und E106 (Block-Cursor) als Sperre. Emmet folgt ESLint als zweiter Nutzer von E140. ESLint, Emmet und Vim greifen alle in den Editor ein und werden zusammen abgenommen |
| 3 Tests | B27 | L | Startet Prozesse über den Supervisor und erweitert `ide-dap` um das Debuggen einzelner Tests |
| 4 Docker | B33 | M | *(G37)* Eigene Stufe, unabhängig von Stufe 3 |

Jede Stufe endet wie die Phasen in plan.md:
- `make check` grün
- `cargo xtask selftest` mit neuen Szenarien
- `make perf` (E25)
- VoiceOver-Prüfung der neuen Ansichten
- neue Texte in `en.lproj` und den 33 weiteren Sprachen nach [glossary.md](glossary.md) (E15). Neue Begriffe wie
  „Minimap“ und „Sticky Scroll“ kommen zuerst ins Glossar *(F36)*.
- HIG-Abnahme (E101): Jeder neue Befehl steht in der Menüleiste, kein HIG-Standardkürzel wird umbelegt.

## Stufe 1 — Komfort

### B30 Ungesicherte Puffer im Checkpoint

*(C45, F37)* `crates/ide-ui-agent/src/checkpoint.rs` übergibt heute keine Puffer (`create(.., &[])`), entgegen I6.
Ungesicherte Änderungen fehlen deshalb im Checkpoint vor einem Agenten-Zug, und Zurückspulen stellt sie nicht wieder
her. B30 übergibt die ungesicherten Puffer aller offenen Dokumente des Ordners. Ein Selftest spult mit einem
ungesicherten Puffer zurück.

### B29a Minimap

- **E107 Minimap als eigene Ansicht neben dem Editor, Standard aus.** `editor_container` (`src/editor/mod.rs`) bekommt
  rechts neben dem `NSScrollView` eine weitere Spalte. Der Rollbalken der Scroll-Ansicht liegt damit links von ihr.
  - Sie zeichnet je logischer Zeile Farbblöcke aus den Tokens des Zeilen-Lexers (E7) und den Semantic Tokens, keine
    Glyphen (wie VS Codes „Zeichen nicht darstellen“). Umbrochene Zeilen bleiben eine Zeile.
  - Dazu kommen ein Rahmen für den sichtbaren Bereich und Marken für Git-Änderungen, Diagnosen und die angehaltene
    Debug-Zeile. Suchtreffer entfallen *(F11)*: Die Systemsuchleiste des Editors liefert keine Trefferliste.
  - Klick springt, Ziehen scrollt. Gefaltete Zeilen fehlen auch in der Minimap.
  - Dateien, die länger sind als die Ansicht hoch ist, verschiebt die Minimap anteilig mit (Verhalten von VS Code).
  - Unter einer Editorbreite von 400 pt blendet sie sich aus, etwa bei geteilten Tabs.
  - Ein- und Ausschalten über Editor ▸ Minimap (Menü wie in Xcode, *F36*) und den Schlüssel `minimap`.
  - Für VoiceOver unsichtbar; die Position liefert der Rollbalken.
- **Neu zeichnen** *(C24, F31)*: Die Minimap zeigt nur so viele Zeilen, wie in ihre Höhe passen. Sie zeichnet diese
  bei jedem Neuzeichnen direkt aus `Document::spans(line)`, ohne Kachel-Cache. `setNeedsDisplay` lösen aus:
  - Textänderung (`changed()` in `src/editor/view.rs`)
  - Farben des Lexers (`highlight::step`)
  - Semantic Tokens (`decorate::set_tokens`)
  - Diagnosen (`decorate::set_diagnostics`)
  - Faltung, Git-Marken, Debug-Zeile und Scrollen
- **Abnahme:** Das Budget „Tastendruck < 16 ms bei 10 000 Zeilen“ (E25) gilt mit eingeschalteter Minimap; `make perf`
  bekommt dafür einen eigenen Fall. Ein Selftest fotografiert die Minimap hell, dunkel und mit erhöhtem Kontrast.

### B29b Sticky Scroll

- **E108 Sticky Scroll, Standard an.** Oben im Editor stehen die Kopfzeilen der Bereiche, in denen die oberste
  sichtbare Zeile liegt (Funktion, Klasse, `impl`-Block), höchstens 5 Zeilen (Schlüssel `stickyScrollLines`, 0 schaltet
  ab). Ein Klick springt zur Kopfzeile. Die Kopfzeilen tragen ihre Syntaxfarben.
  - Datenquelle zuerst die Dokument-Symbole des Sprachservers. `src/lsp/outline.rs` behält dafür künftig die Zeile von
    `selection_range` je Symbol; heute geht sie beim Abflachen in `Crumb` verloren.
  - Ohne Sprachserver dienen die Einrückungs-Faltungen (`Document::folds`) als Rückfall; die Kopfzeile ist dann die
    erste Zeile der Faltung. Für Markdown und reinen Text gilt der Rückfall nicht *(F28)*.
  - Gezeichnet wird in einer Ansicht über dem Clip-View, nicht als Unteransicht des Textes. Sie scrollt nicht mit,
    die Verzögerung aus `inset_fragment.rs` betrifft sie deshalb nicht. Der Text der Zeile wird mit Core Text
    gezeichnet, ohne TextKit 1 (E7). Sie überdeckt auch Randspalte und Blame-Spalte.
  - **Sichtbar machen** *(F28)*: Ein gemeinsamer Weg „Bereich sichtbar machen“ zieht die Höhe der Kopfzeilen ab. Alle
    Scroll-Aufrufe in `src/editor` und `src/lsp` (Cursor, Sprungziele, Suchtreffer) gehen über ihn, damit nichts
    unter den Kopfzeilen landet.
  - Für VoiceOver unsichtbar; dieselben Zeilen stehen im Text.
  - Editor ▸ Sticky Scroll schaltet um.
- **Abnahme:** Ein Selftest scrollt durch eine Rust-Datei mit verschachtelten Blöcken und prüft die Kopfzeilen und
  einen Sprung zu einer Zeile direkt unter ihnen.

### B35 Diagnosen am Zeilenende

- **E130 Diagnosen am Zeilenende über die Einblendungen, Standard aus.** Bisher zeigt der Editor eine Diagnose als
  Punkt in der Randspalte mit Tooltip (`src/editor/ruler.rs`).
  - Neue Ebene `Diagnostics` in E81 (`src/editor/insets`) mit `Place::LineEnd`. Sie zeigt die Meldung mit der höchsten
    Schwere der Zeile, auf eine Zeile gekürzt; weitere Meldungen der Zeile stehen als Zähler dahinter, der Tooltip
    zeigt alle.
  - **Erweiterung des Bausteins** *(G31)*:
    - `Inset` bekommt eine Schwere mit Symbol und Farbe; bisher zeichnet der Baustein jeden Kasten in Sekundärfarbe
      (`insets/draw.rs`). Die Schwere ist am Symbol erkennbar, nicht nur an der Farbe (E101).
    - Am Zeilenende gilt eine feste Rangfolge: Vorschlag der Vorhersage (E92), Debug-Wert (E94), Diagnose, Blame
      (E86). Reicht der Platz nicht, entfallen die hinteren, und die Diagnose wird mit „…“ gekürzt.
    - Bei umbrochenen Zeilen steht die Meldung am Ende der letzten Bildschirmzeile.
    - Die Prüfliste aus E81 (Caret, Auswahl, Klick, Kopieren, VoiceOver, Fotos) läuft für die neue Ebene erneut.
  - **Pause** *(G31)*: Diagnosen kommen heute sofort an (`src/lsp/mod.rs`, `diagnostics_arrived`). Eine Zeile zeigt
    ihre Meldung erst, wenn sie 1 s nicht bearbeitet wurde; die Cursorzeile zeigt sie während des Tippens nie.
  - Schlüssel `diagnosticsAtLineEnd`, Darstellung ▸ Diagnosen am Zeilenende, neben Blame und Inlay Hints *(G32)*.
  - VoiceOver: Der Rotor „Hinweise“ (phase-11) liest die Meldungen und sagt die Schwere mit an *(G32)*.
- **Abnahme:** Selftest-Foto mit Fehler, Warnung und Hinweis in hell, dunkel und erhöhtem Kontrast, dazu eine Zeile
  mit Blame und Debug-Wert. `make perf` mit 500 Diagnosen in einer Datei von 10 000 Zeilen (E25).

### B36 Live-Vorschau statischer Seiten

- **E131 Statische Seiten im Browser-Tab, neu geladen beim Sichern.** Für HTML, CSS und JS ohne eigenen Dev-Server.
  - Rechtsklick auf eine HTML-Datei oder Editor ▸ „Im Browser-Tab öffnen“ startet einen kleinen HTTP-Server in der
    App. Er liefert den Projektordner an der Wurzel aus, damit Adressen wie `/css/app.css` stimmen *(G33)*.
  - HTTP statt `file://`, weil ES-Module und `fetch` unter `file://` scheitern; aus demselben Grund nutzt die
    Erweiterung Live Server einen Server.
  - **Server** *(G36)*: eigener Nur-GET-Server über `std::net`, wie der Loopback-Listener in `crates/ide-ci/src/oauth.rs`.
    Keine neue Abhängigkeit; `tiny_http` ist seit 2022 ohne Release.
  - **Zugriff** *(G33, G34)*:
    - Er lauscht nur auf 127.0.0.1 mit zufälligem Port und prüft den `Host`-Header.
    - Vor dem Laden setzt die App ein zufälliges Cookie in den `WKHTTPCookieStore` des Tabs; Anfragen ohne Cookie
      lehnt der Server ab. Andere Prozesse des Macs kommen so nicht an die Dateien.
    - Versteckte und ignorierte Dateien (`.env`, `.git`) liefert er nicht aus. Symlinks, die aus dem Ordner
      hinausführen, lehnt er ab.
    - Skript der Seite kann über HTTP jede ausgelieferte Projektdatei lesen und ins Netz schicken; unter `file://`
      konnte es das nicht. Deshalb nur in vertrauten Ordnern (E22).
  - **Neu laden** *(G35)*: über die Beobachtung von `ide-project`. Ändert sich eine nicht ignorierte Datei unter dem
    ausgelieferten Ordner, lädt der Tab nach 200 ms Ruhe neu. Die App steuert den Tab selbst; ein eingeschleustes
    Skript wie bei Live Server ist nicht nötig.
  - **Wiederherstellen** *(G35)*: Der Tab wird als Vorschau der Datei gespeichert, nicht als Adresse (neue Art in
    `Tab` von `crates/ide-core/src/layout.rs`). Nach einem Neustart (E64) startet der Server neu, und die Vorschau
    lädt mit dem neuen Port.
  - Der Server endet mit dem letzten Tab, der ihn nutzt. Projekte mit eigenem Dev-Server bleiben bei I7.
- **Abnahme:** Selftest mit einer Seite, die ein ES-Modul und eine wurzelrelative CSS-Datei lädt, Sichern und
  Neuladen. Test, dass eine Anfrage ohne Cookie, eine auf `.env` und eine über einen Symlink abgelehnt werden.

### B32a Prettier

- **E133 Prettier als Formatierer neben dem Sprachserver.** Prettier hat keinen Sprachserver und braucht den Umbau aus
  E140 nicht; es kommt deshalb in Stufe 1 *(G37)*.
  - Neuer Formatierer-Weg: Die App ruft Prettier mit `--stdin-filepath` über den Supervisor auf.
    - Gesucht wird `node_modules/.bin/prettier` vom Ordner der Datei aufwärts bis zur Projektwurzel, sonst auf dem
      PATH der Login-Shell.
    - Arbeitsordner ist der Ordner mit diesem `node_modules`, damit `.prettierignore` gefunden wird *(G21)*.
    - Eine ignorierte Datei kommt unverändert zurück; Exit-Code 2 heißt Fehler.
  - Hat das Projekt eine Prettier-Konfiguration (`.prettierrc*`, `prettier.config.*`, Schlüssel `prettier` in
    `package.json`), formatiert Prettier die Sprachen, die es kennt; sonst bleibt der Sprachserver zuständig. Der
    Schlüssel `formatter` je Sprache (`server`, `prettier`) überschreibt das.
  - **An die Revision gebunden** *(G21)*: Das Ergebnis wird nur angewendet, wenn die Revision des Dokuments noch die
    vom Aufruf ist (E24, E92, R21). Sonst und nach der Frist von 5 s wird ohne Formatieren gesichert, mit Hinweis in der
    Statusleiste.
  - Angewendet wird als zeilenweiser Unterschied (`Document::replace_all`) in einem Undo-Schritt, damit Cursor und
    Faltungen erhalten bleiben.
  - „Formatieren beim Sichern“ (`formatOnSave`) nutzt denselben Weg.
  - Nur in vertrauten Ordnern (E22), weil Prettier Plugins aus dem Projekt lädt.
- **Abnahme:** Test der Formatierer-Wahl und der Revisionsbindung; Selftest mit Prettier beim Sichern in einem
  Testprojekt unter `tests/fixtures`.

### B28 Einstellungen über iCloud

- **Voraussetzung: ein Besitzer der Einstellungen** *(F9)*. `Store::set` schreibt heute den beim Laden gehaltenen Stand
  als Ganzes; ein Import zwischen `load` und `set` ginge verloren. Es gibt kein Entfernen eines Schlüssels und keine
  Änderungsmeldung.
  - Zuerst bekommt der `AppContext` (E97) einen Besitzer der Nutzer-Einstellungen. Er schreibt als Lesen-Ändern-Schreiben
    unter einer Sperre, kann Schlüssel entfernen und meldet Änderungen an Fenster, Menü und die erlaubten Caches aus
    [thread-locals.md](thread-locals.md).
  - Erst darauf baut der Abgleich.
- **E112 Abgleich über einen Ordner in iCloud Drive, nicht über CloudKit.**
  - Gegen CloudKit spricht: Es bräuchte eine bei Apple registrierte App-ID mit iCloud-Container, ein Profil in
    `xtask sign` und einen Container, der an der Bundle-ID hängt. Die heutige Kennung `local.agentide`
    (`crates/ide-product`) ist dafür nicht gedacht.
  - **Zugriff** *(C25, F16)*: Auch ohne Sandbox fragt macOS beim ersten Zugriff auf iCloud Drive nach Erlaubnis. Das
    ist der Regelweg.
    - `Info.plist` und `InfoPlist.strings` bekommen den passenden Begründungstext; welcher Schlüssel greift, klärt A5.
    - Lehnt der Nutzer ab, ist der Schalter ausgegraut und nennt den Grund.
    - Als Ausweg wählt der Nutzer den Ordner einmal im Öffnen-Dialog selbst.
  - Der Ordner heißt wie die neutrale Kennung (E18), also getrennt für Debug- und Release-Build.
  - Ist iCloud Drive aus, ist der Schalter ausgegraut und nennt den Grund.
- **E113 Was abgeglichen wird, legt jeder Schlüssel selbst fest** *(F18)*.
  - `sync` wird Pflichtargument der Konstruktoren `Key::user` und `Key::project` in `crates/ide-core/src/settings.rs`.
  - Schlüssel von `state.json` bekommen einen eigenen Typ, der nie abgeglichen wird.
  - Ein Test zählt alle Konstruktoren auf und schlägt fehl, wenn ein Schlüssel in dieser Tabelle fehlt.

  | Klasse | Schlüssel (Stand 2026-11-13, 39 insgesamt) |
  |---|---|
  | abgleichen (25) | `agentEffort`, `agentModel`, `agentPrices`, `blameAtLineEnd`, `ciHosts`, `ciOAuthClients`, `commitMessageModel`, `dictation` (ohne Mikrofon, siehe unten), `dictationWithFn`, `editPrediction`, `editPredictionExclude`, `editPredictionOffFor`, `fontSize`, `formatOnSave`, `hiddenSidebarPanels`, `inlayHints`, `inlayHintsOffFor`, `interfaceFontSize`, `markdownPreviewBeside`, `optionAsMeta`, `previewTabs`, `reopenProjects`, `searchEngine`, `showIgnoredFiles`, `showWhitespace` |
  | je Mac, Pfade und lokale Adressen (6) | `agentAllowedFolders`, `agentEndpoints`, `agentRules`, `editPredictionLmStudio`, `editPredictionOllama`, `languageServers` |
  | je Mac, Zustimmungen und Merker (3) | `commitMessageConsent`, `editPredictionConsent`, `dictationLowspeakOffered` |
  | Zustand, nie (6) | `agentChoices`, `claudeRuleFolders`, `dictationAccessibilityAsked`, `openProjects`, `openWindows`, `recentProjects` |

  - Neue Schlüssel dieser Phase werden abgeglichen *(G22)*: `minimap`, `stickyScrollLines`, `emmet`, `vimMode`,
    `vimClipboard`, `diagnosticsAtLineEnd`, `eslintFixOnSave`, `formatter`.
  - `keybindings.json` wird je Befehl abgeglichen, auch entfernte Überschreibungen *(C28)*.
  - Beim Eintrag `dictation` (E103) bleibt das Mikrofon beim Import lokal, alle anderen Felder werden abgeglichen
    *(C28)*. E103 bleibt unverändert, der Eintrag wird nicht aufgeteilt.
  - Nie abgeglichen werden außerdem Layouts, Verläufe, Checkpoints, Wiederherstellungsdaten, das Projektvertrauen
    (E22), installierte Werkzeuge und Schlüsselbund-Einträge.
- **E114 Eine Datei je Mac, zusammengeführt als Maximum je Schlüssel** *(C26, C27, F17)*.
  - Jeder Mac schreibt nur seine eigene Datei `<geräte-id>.json` und liest die aller anderen. Weil keine Datei zwei
    Schreiber hat, entstehen keine Konfliktkopien; `NSFileVersion` und Dateien wie „settings 2.json“ entfallen.
    Gelesen und geschrieben wird trotzdem über `NSFileCoordinator`, damit iCloud keine halben Dateien liefert.
  - Je Schlüssel stehen in der Datei Wert oder Löschmarke und eine logische Uhr (Zähler plus Wanduhr, damit
    Uhrsprünge die Reihenfolge nicht umdrehen). Zusammengeführt wird als Maximum der Uhr je Schlüssel; bei Gleichstand
    entscheidet die Geräte-ID, so kommt jeder Mac zum selben Ergebnis.
  - Lokale Änderungen ergeben sich aus dem Unterschied zwischen `settings.json` und der eigenen Gerätedatei. Das
    erfasst auch Änderungen von Hand.
  - Beim ersten Einschalten bekommen die vorhandenen lokalen Werte die Uhr 0. Werte anderer Macs gewinnen damit, ein
    Sheet ist nicht nötig.
  - Jede Gerätedatei trägt eine Schemaversion. Importierte Werte laufen durch dieselben Migrationen wie die eigene
    Datei. Eine Gerätedatei aus einer neueren App-Version wird nicht übernommen; die Einstellungen zeigen „Abgleich
    pausiert, Dango auf diesem Mac aktualisieren“.
  - Gelesen wird auf einem Hintergrund-Thread mit Frist, weil iCloud ausgelagerte Dateien erst beim Zugriff lädt.
    Angewendet wird über den Besitzer der Einstellungen, also nie über einen veralteten Stand. Änderungen meldet ein
    `NSFilePresenter`.
  - Schalter in Einstellungen ▸ Allgemein, Standard aus.
- **Abnahme:** Tests für gleichzeitige Änderungen an verschiedenen und am selben Schlüssel, Zurücksetzen, Uhrsprung,
  späte alte Stände, Änderung von Hand und eine Gerätedatei aus einer neueren Version.

## Stufe 2 — Editor

### B32b ESLint

- **E132 ESLint als zweiter Server je Dokument.** Er läuft über den Umbau aus E140 neben dem Hauptserver, für JS, TS,
  JSX und TSX. Vue und Svelte entfallen in der ersten Ausgabe, weil Dango für sie keinen Hauptserver kennt
  (`Language::of_path`) *(G20)*.
  - **Server** *(G17)*: Das npm-Paket `vscode-langservers-extracted` enthält einen Stand vom Mai 2024 (4.10.0) ohne
    `eslint.config.ts`. Dango baut den Server stattdessen wie Zed aus einem festen Tag von `microsoft/vscode-eslint`,
    mit Prüfsumme (R8), nach Nachfrage einmal mit npm in den Werkzeugordner. Ein Update ist ein neuer Tag mit neuer
    Prüfsumme (R38).
  - Er startet nur, wenn das Projekt ESLint nutzt: eine Datei `eslint.config.*` oder `.eslintrc*`, oder `eslint` in
    `package.json`.
  - Er lädt ESLint und dessen Konfiguration aus dem Projekt und führt damit Projektcode aus; nur in vertrauten
    Ordnern (E22).
  - **Einstellungen** *(G18)*: Der Server fragt mit `section: ""` und braucht ein vollständiges Objekt; bei leeren
    Werten prüft er nichts. Die App berechnet es je Wurzel:
    - `validate: "on"`, `run: "onType"`
    - `workingDirectory: {mode: "auto"}`, damit Monorepos stimmen
    - `workspaceFolder` mit URI und Name, `nodePath` aus der Login-Shell
    - `format: false` (Formatieren macht E133 oder der Hauptserver), `codeActionOnSave`, `problems`,
      `rulesCustomizations`
    - `useFlatConfig` je Projekt aus der installierten ESLint-Version und der gefundenen Konfigurationsdatei (wie Zed)
  - **Diagnosen per Pull** *(G19)*: Der Server meldet `diagnosticProvider` und schickt nie `publishDiagnostics`. Pull
    und `workspace/diagnostic/refresh` laufen deshalb je Slot (E140), nicht nur für den Hauptserver.
  - **Eigene Anfragen** *(G19)*: `eslint/noLibrary`, `eslint/noConfig`, `eslint/probeFailed` und `eslint/openDoc`
    beantwortet Dango; heute bekämen sie `METHOD_NOT_FOUND` (`crates/ide-lsp/src/incoming.rs`). Die Statusleiste
    zeigt sie als Zustand („ESLint im Projekt nicht installiert“).
  - Diagnosen und Schnellkorrekturen erscheinen über die zusammengeführten Diagnosen und Code Actions (E140, E82).
  - „Beim Sichern beheben“ führt `source.fixAll.eslint` vor dem Formatieren aus; Schlüssel `eslintFixOnSave`,
    Standard aus. Reihenfolge beim Sichern: Fix-All, dann Formatieren (E133), dann Sichern, als ein Undo-Schritt.
    Beide Schritte sind an die Revision gebunden und haben eine Frist; danach wird ohne sie gesichert *(G21)*.
- **E140 Mehrere Sprachserver je Dokument** *(C23, F15)*. Bisher war dieser Umbau Teil von E109 (Emmet); er gilt
  für ESLint und Emmet. `ViewState.slot` wird zu einer Liste.
  - Schlüssel eines Slots ist der Register-Eintrag plus Wurzel, nicht die Sprache. Ein Zweitserver bedient so alle
    Sprachen eines Ordners mit einem Prozess.
  - Jeder Server bekommt `didOpen`, `didChange` und `didClose` mit eigener Versionszählung, und der Lebenszyklus aus
    E10 (Ende fünf Minuten nach dem letzten Editor) gilt je Server.
  - Vervollständigung: Ein Auslösezeichen geht nur an die Server, die es angemeldet haben. Jeder Vorschlag trägt
    seinen Herkunftsserver, und `resolve` geht an diesen Server statt an `server_of`. Unvollständige Listen und
    Abbruch gelten je Server, die Listen werden zusammengeführt.
  - Diagnosen und Code Actions aller Server werden zusammengeführt; jede Diagnose trägt ihre Quelle, und jeder Server
    ersetzt nur seine eigenen (E132). Hover, Navigation und Formatieren bleiben beim Hauptserver.
  - Zweitserver werden von der Ausleihe der Projekt-Symbolsuche (phase-11, C17) nicht mitgestartet.
- **Abnahme:** Ein Test prüft, dass ein Auslösezeichen nur die Server erreicht, die es angemeldet haben, und dass
  Diagnosen zweier Server getrennt ersetzt werden. Mitschnitt-Test der Konfigurationsanfragen, des Pulls und der vier eigenen Anfragen; Test gegen
  ESLint 9 und die aktuelle Hauptversion, auch mit `eslint.config.ts` (A11). Selftest mit einem ESLint-Fehler und
  seiner Schnellkorrektur.

### B29c Emmet

- **E109 Emmet über `@olrtg/emmet-language-server` als zweiter Server je Dokument.**
  - Der Server läuft für HTML, CSS/SCSS/Less, JSX und TSX neben dem Hauptserver.
  - Die Kürzel erscheinen als Vorschläge in der normalen Vervollständigung (wie in Zed), nicht über eine eigene
    Tab-Taste.
  - Installation über den bestehenden npm-Weg (`Install::Npm`). Gefragt wird einmal für die App, nicht je Datei, mit
    „Nicht mehr fragen“ *(F15)*. Die Statusleiste zeigt „Emmet fehlt“ mit Installieren.
  - Der Schlüssel `emmet` steht standardmäßig auf an und schaltet ab.
  - Nur in vertrauten Ordnern (E22), wie jeder Sprachserver.
- Emmet kommt nach ESLint, als zweiter Nutzer von E140 *(Owner, 2026-11-13)*. Er verhält sich anders als ESLint:
  andere Auslösezeichen, keine Diagnosen, sechs Sprachen in einem Prozess. Damit prüft er, ob E140 allgemein gebaut ist
  und nicht auf ESLint zugeschnitten. Größe S, weil der Umbau aus E140 schon steht.
- **Abnahme:** Ein Selftest tippt in einer HTML-Datei ein Kürzel und übernimmt den Vorschlag. Ein Test prüft, dass
  ein Emmet-Auslösezeichen den Hauptserver nicht erreicht.

### B26 Vim-Modus

- **E104 Tastenfolgen und Bewegungen aus `modalkit`; wer ausführt, entscheidet der Prototyp** *(F2)*.
  - `modalkit` 0.0.32 (Apache-2.0, mit `keybindings` und `editor-types` aus demselben Repo) wird auf eine Version
    festgelegt.
  - **Feature-Vereinigung** *(F1)*: `modalkit` zieht `ropey` mit Standard-Features, also `unicode_lines`. Cargo
    vereinigt Features je Version, damit zählte `ide-document` (bewusst nur `cr_lines`) auch U+2028, U+0085, VT und
    FF als Zeilenende; LSP-Positionen verschöben sich, und `text.rs` verlangt das Gegenteil im Test. `modalkit` kommt
    deshalb als Kopie unter `vendor/` mit `ropey` ohne Standard-Features und wird über `[patch.crates-io]`
    eingebunden, wie `comrak` (E14). Ein Test in `ide-document` hält die Zeilenregel fest.
  - `VimMachine` übersetzt Tasten in Aktionen vom Typ `modalkit::actions::Action` *(C19)*.
  - Neue Crate `ide-vim` ohne AppKit (E99), die einzige Stelle, die `modalkit` kennt. Sie liefert Änderungen als
    `ide_document::Edit` und Cursor-Positionen zurück; das Editor-Modul wendet sie an.
  - Mit `modalkit` kommt `crossterm` als Abhängigkeit mit, nur übersetzt, nie benutzt.
  - **Zwei Wege, Variante A zuerst:**
    - **A:** `modalkit`s `EditBuffer` führt auf einer Schattenkopie aus und bringt Operatoren, Register, Marken,
      Einfügeformen, Visual-Operationen und Makros mit. Die Ersetzungen gehen als `ide_document::Edit` in die Ansicht.
    - **B:** eigener Ausführer, nur für die Bewegungen von `EditRope` über `CursorMovements` und `CursorSearch`.
    - Der Prototyp baut A. Nur wenn A an Undo, Eingabemethoden oder am Abgreifen der Ersetzungen scheitert, folgt B,
      und dann mit kleinerem Umfang (E106).
- **Schattenkopie** *(C21, F13)*:
  - `ide-document` hält den Text schon als `ropey::Rope` (`text.rs`). `EditRope` lässt sich nicht um eine vorhandene
    Rope legen (privates Feld). Deshalb hält `ide-vim` eine eigene Kopie, nur für Editoren mit eingeschaltetem
    Vim-Modus.
  - Sie folgt dem Verteiler `changed()`, der auch Änderungen aus dem Modell erfasst (`apply_model_changes`). Nach
    `resync` und bei abweichender Revision wird sie neu aufgebaut.
  - Die Kopie hat nur LF als Zeilenende. Adressiert wird über (Zeile, Spalte in Zeichen) und in UTF-16 für AppKit
    umgerechnet. So treffen `$` und `x` in CRLF-Dateien nie das `\r`; der Puffer behält CRLF (E24).
  - Was `EditRope` nicht rechnen kann, rechnet der Editor aus TextKit 2: Bildschirmzeilen (`gj`, `gk`, `g0`, `g$`),
    `H`, `M`, `L`, Seitenbewegungen und gefaltete Zeilen (`j` springt über eine Faltung).
- **Einfügen, Wiederholen, Makros, Undo** *(C20, F4, F14)*:
  - Aufgezeichnet wird der Netto-Text eines Einfüge-Durchgangs: was nach dem Durchgang im Puffer steht, einschließlich
    Einfügen aus der Zwischenablage, übernommener Vorschläge, Auto-Klammern und Auto-Einrückung. `.` und Makros fügen
    ihn wörtlich ein, ohne Auto-Klammern und Einrückung ein zweites Mal anzuwenden.
  - Ein Einfüge-Durchgang von `i` bis Esc ist ein Undo-Schritt, mit einer eigenen Undo-Gruppe über mehrere
    Tastaturereignisse. Die Gruppe schließt bei Esc, Fokusverlust, Wechsel des Dokuments und externer Änderung.
  - `apply_model_changes` schließt eine offene Vim-Gruppe, bevor es eine Agenten- oder Datei-Änderung anwendet. Ein `u`
    nimmt so nie Vim- und Agenten-Änderung zusammen zurück (E24). `undo_step` wird für den Fall „fremde Gruppe offen“
    erweitert und getestet.
  - Der Prototyp prüft `ciw…Esc` gefolgt von `.`, ein Makro mit Einfügen, mehrzeiliges Einfügen, das ein `u`
    vollständig zurücknimmt, und eine Agenten-Änderung während eines Einfüge-Durchgangs.
- **Sperre:** Ein Prototyp mit `w`, `b`, `dd`, `ciw`, `.`, `u`, `/suche`, Visual-Modus und einer 10-MB-Datei muss
  zeigen:
  - Die Schattenkopie hält das Budget (E25).
  - Variante A trägt, oder es ist klar, warum B nötig ist.
  - Die Fälle aus C20, C21, F3 und F12 laufen.
  - Fällt er durch, wird neu entschieden, bevor weitergebaut wird.
- **E105 Wer welche Taste bekommt** *(F3, F4, F12)*.
  - **Alle Modi laufen durch den Eingabekontext.** `keyDown:` reicht jede Taste an `interpretKeyEvents`. Im Normal-
    und Visual-Modus geben `insertText:` und `doCommandBySelector:` das Ergebnis an die Maschine statt in den Text.
    - So funktionieren Tottasten der deutschen Tastatur (`^` für den Zeilenanfang, `` ` `` für Marken), wie im
      Terminal (`crates/ide-ui-terminal/src/input.rs`).
    - Bei nicht-lateinischen Belegungen (Russisch, Griechisch, Hebräisch) übersetzt der Normal-Modus über den
      Tastencode und die ASCII-fähige Belegung des Systems, damit `j` auch dort `j` ist.
  - Der Einfüge-Modus läuft wie heute über `insertText:`, damit Eingabemethoden wie Japanisch funktionieren.
  - **Esc** schließt in dieser Reihenfolge immer nur das Erste, was offen ist:
    1. Komposition der Eingabemethode
    2. Vervollständigungsliste
    3. Signaturhilfe (E84)
    4. Vorschlag der Vorhersage (E92)
    5. Diktat
    6. weitere Cursor
    7. dann erst der Wechsel in den Normal-Modus
  - **Tastenverteilung:**

    | Taste | Normal, Visual | Einfügen |
    |---|---|---|
    | ⌘-Kürzel | App | App |
    | ⌃R, ⌃V, ⌃O, ⌃D, ⌃U, ⌃F, ⌃B, ⌃E, ⌃Y, ⌃A, ⌃X, ⌃G | Vim | ⌃O, ⌃R, ⌃W, ⌃U: Vim; sonst App wie heute |
    | ⌃[ und ⌃C | wie Esc | wie Esc |
    | Tab, Return, Pfeile | Vim | wie heute (Vervollständigung, Snippets, Einrückung) |

  - ⌃-Kürzel nimmt der Editor über `performKeyEquivalent:` vor der Menüleiste an sich (A6). Das gilt nur, wenn
    `window.firstResponder` der Editor selbst ist; sonst behielte bei „Daneben öffnen“ (E60) ein zweiter Editor ⌃R,
    obwohl das Terminal den Fokus hat.
  - Ein Test listet die Kürzel der Kürzeltabelle, die der Vim-Modus überdeckt (heute nur `edit.goToLine` auf ⌃G). Das
    Menü zeigt sie weiter.
  - Mehrfach-Cursor: Der Normal-Modus wirkt auf den Haupt-Cursor, weitere Cursor fallen beim Wechsel in den
    Normal-Modus weg. Visual-Block (⌃V) nutzt die vorhandene Spaltenauswahl (`src/editor/multi.rs`).
  - Der Vim-Modus gilt nur im Editor, nicht im Terminal, in der Eingabekarte der Agenten oder im Browser.
- **E106 Umfang der ersten Ausgabe.**
  - Normal, Einfüge, Visual (Zeichen, Zeile, Block) und Ersetzen.
  - Zähler, Operatoren mit Bewegung und Textobjekt, `.`, unbenanntes Register und Zwischenablage.
  - Bei Variante A zusätzlich benannte Register, Makros (`q`, `@`) und Marken. Bei Variante B fallen sie aus der ersten
    Ausgabe *(F39)*; Xcodes Vim-Modus, das Vorbild nach E101, hat sie auch nicht.
  - `u` und ⌃R gehen auf den Undo-Verlauf von AppKit.
  - Das unbenannte Register ist nicht die Zwischenablage (Vim-Verhalten); `"+` und `"*` sind sie. Der Schlüssel
    `vimClipboard` legt beide zusammen.
  - **Eigene Suche** *(F11)*: Der Editor sucht heute über die Suchleiste des Systems (`setUsesFindBar`), die weder
    Trefferliste noch Muster liefert. `/`, `?`, `n`, `N`, `*`, `:s`, `:%s` und `:noh` bekommen deshalb eine eigene
    Suche mit Hervorhebung in `ide-vim`, mit Rust-Regex. Wo sich diese von Vims Muster unterscheidet (`\v`, `\<`),
    steht in der Hilfe; siehe R24.
  - Ex-Befehle: `:w`, `:q`, `:wq`, `:x`, `:<zeile>`, `:s`, `:%s`, `:noh`. `modalkit` bringt `:x`, `:wq` und `:noh`
    nicht mit, `ide-vim` ergänzt sie. Alles andere meldet „Befehl nicht unterstützt“ in der Statusleiste.
  - Kein `.vimrc` und keine eigenen Belegungen in der ersten Ausgabe.
  - Modus in der Statusleiste des Editors; VoiceOver sagt den Moduswechsel an.
  - Einschalten über Editor ▸ Vim-Modus (Menü wie in Xcode, *F36*) und den Schlüssel `vimMode`, Standard aus.
  - **Block-Cursor als eigene Sperre** *(C22)*: Seit macOS 14 zeichnet `NSTextView` die Einfügemarke über
    `NSTextInsertionIndicator`, und die alten Zeichenmethoden greifen unter TextKit 2 nicht verlässlich.
    - Ein Prototyp zeigt auf macOS 14 und dem aktuellen System, wie die Systemmarke unterdrückt wird.
    - Den Block zeichnet das vorhandene Overlay, das schon weitere Cursor zeichnet (`overlay.rs`), ohne
      `layoutManager` (E7).
    - Scheitert der Prototyp, bleibt die Einfügemarke ein Strich.
- **Abnahme:** In `ide-vim` laufen Tabellentests (Ausgangstext, Tasten, Ergebnis, Cursor), mindestens je Operator,
  Bewegung und Textobjekt, mit LF- und CRLF-Text. Ein Selftest-Szenario prüft im echten Fenster Tippen, Undo, die
  Eingabemethode (Japanisch), `^` auf deutscher Belegung und ⌃R bei geteiltem Fenster.

## Stufe 3 — Tests

### B27 Test-Explorer

- **E110 Neue Crate `ide-test` mit einem Adapter je Werkzeug** *(C29, C30, F10, F27)*.
  - Ein Adapter legt Versionen, Befehle, Ergebnisdateien und Rückfälle fest.
  - Testidentität (Paket, Datei, Name, Parameter) und Quellstelle sind getrennt. Ein Test ohne Quellstelle ist erlaubt
    und steht im Panel ohne Sprung.
  - Nur stabile Ausgaben werden gelesen, kein nightly; einzige Ausnahme ist das libtest-JSON von nextest, das nextest
    noch experimentell nennt (`NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1`):

  | Werkzeug | Finden | Ergebnis | „Läuft“ je Test |
  |---|---|---|---|
  | cargo test | `cargo test -- --list`, nur auf Wunsch, weil es alle Testprogramme baut und die Cargo-Sperre hält; vorher ohne Bau aus den Quellen: Standard-Mitglieder des Workspace, Ziele nach Cargos Layout und Manifest, Modulbaum ab der Crate-Wurzel mit `#[path]`, ohne Module hinter einem Feature | Textausgabe von libtest, mit eingefrorenen Mitschnitten je Version, Ergebnisse je Test sofort | nein; libtest meldet auf stable keinen Start, das Testprogramm läuft |
  | cargo-nextest (wenn auf dem PATH) | `cargo nextest list --message-format json`, wie bei cargo test nur auf Wunsch, vorher aus den Quellen | `--message-format libtest-json-plus` laufend gelesen; die Binary-ID ergibt sich aus Name und Art der Suite | ja |
  | Go | `go test -json -list . ./...`; Untertests erst aus dem Lauf | `go test -json` | ja |
  | pytest | mitgeliefertes Plugin, meldet Collection, Start und Ergebnis je Test als JSON-Zeilen mit Node-ID | dasselbe Plugin | ja |
  | Vitest | `vitest list --json` | mitgelieferter Reporter neben dem eigenen, meldet Start und Ergebnis je Test als JSON-Zeilen; Namensfilter `-t` über den Namen mit ` > ` | ja |
  | Jest | nur Dateien (`--listTests`); Testfälle erst aus dem Lauf | mitgelieferter Reporter neben dem eigenen, wie bei Vitest; Namensfilter über die mit Leerzeichen verbundenen Teile | ja |

  - Parametrisierte und dynamisch erzeugte Tests bekommen ihre Identität aus dem Lauf.
  - Ohne Ereignisse je Test läuft im Panel die Datei, das Paket oder das Testprogramm; Einzelergebnisse erscheinen am
    Ende.
  - Die Stelle eines Tests liefern rust-analyzer (`experimental/runnables`) und für Go die Funktionen `Test*` aus den
    Dokument-Symbolen (die Test-Lens von gopls ist standardmäßig aus). Sonst werden statische Namen über die
    Dokument-Symbole gesucht.
  - Prozesse laufen über den Supervisor mit der neuen Art `Kind::Test` (in der Aktivität sichtbar, stoppbar).
  - Nur in vertrauten Ordnern (E22), denn Tests führen Projektcode aus.
- **E111 Panel „Tests“ und Randspalte.**
  - Neues Panel in der Seitenleiste vor „Debuggen“: Tests ⌘5, Debuggen ⌘6, CI ⌘7. Das ändert E94 und mit ihm die Zahl
    der Panels aus E55 und phase-11 C18 *(F35)*. Es lässt sich über E80 ausblenden.
  - Das Panel zeigt einen Baum aus Paket, Datei und Test mit Zustand (nicht gelaufen, läuft, bestanden,
    fehlgeschlagen, übersprungen). Der Zustand ist als Form erkennbar, nicht nur über Farbe (E101).
  - Der Baum steht schon vor dem ersten Lauf da, jeder Test als „nicht gelaufen“; ein Lauf schaltet jeden Test live
    um, soweit das Werkzeug Start und Ergebnis je Test meldet. Ein vollständiger Lauf entfernt Tests, die nur die
    Quellen zeigten und der Lauf nicht nannte.
  - Aktionen: Ausführen, Debuggen, Ausgabe zeigen, Filter „nur fehlgeschlagene“.
  - Randspalte: Symbol an jedem Test mit Quellstelle (`ruler.rs`). Klick führt aus, ⌥-Klick debuggt, Rechtsklick
    öffnet das Menü.
  - Alle Befehle stehen im Menü „Debuggen“ unter dem Abschnitt Tests *(F35)*. „Test an der Einfügemarke ausführen“
    bekommt ⌃⌥⌘U, „Letzten Testlauf wiederholen“ ⌃⌥⌘G (Vorbild Xcode). ⌘U bleibt frei, weil es nach der HIG
    „Unterstreichen“ ist. Der HIG-Test und `conflicts()` prüfen beide Kürzel.
  - Fehlgeschlagene Asserts mit Datei und Zeile erscheinen als Marke an der Zeile. Sie haben eine eigene Quelle neben
    `set_diagnostics`, das die Marken des Sprachservers setzt, damit beide sich nicht überschreiben.
- **Einzelne Tests debuggen** *(C31, F27)*. Jeder Adapter liefert ein eigenes Debug-Ziel; `current_program` startet für
  Python und JS heute nur die Datei selbst. Laufzeit, Arbeitsordner und Umgebung sind dieselben wie beim normalen
  Lauf. Rust baut dabei wie heute im Profil `ide-debug` in einen eigenen Ordner.
  - **Rust:** das Testprogramm des Ziels mit `<name> --exact`.
  - **Go:** Paket mit `-test.run '^Name$'`, Sonderzeichen maskiert.
  - **pytest:** als Modul (`python -m pytest <node-id>`) über debugpy.
  - **Vitest und Jest:** der aufgelöste Einstieg des Runners mit dem Testfilter über js-debug. Jest kommt damit neu
    in die Debug-Liste.
- **Abnahme:** Mitschnitt-Tests je Adapter und Version in `ide-test`. Ein Selftest führt einen Test über die
  Randspalte aus und prüft Panel und Marke.

## Stufe 4 — Docker

### B33 Docker-Panel

- **E136 Container verwalten über die `docker`-CLI.** Gemeint ist das Verwalten von Containern, Images und Compose;
  das Arbeiten in einem Container gehört zu B25 (zurückgestellt).
  - **Lesen** *(G27)*: überall `--format '{{json .}}'` als JSON-Zeilen (`docker ps -a`, `docker images`,
    `docker volume ls`), live über `docker events --format '{{json .}}'` als langlaufender Prozess unter dem
    Supervisor.
    - Compose-Projekte und ihre Dienste ergeben sich aus den Labels `com.docker.compose.project` und
      `com.docker.compose.service`, nicht aus `docker compose ls`; das zeigt ohne `--all` nur laufende Projekte, und
      `compose ps` hat sein Format in v2.21.0 gewechselt.
    - Alle Werte kommen als Strings, Labels als mit Komma verketteter Text.
    - Mindestversion Docker 24.0.6, weil `docker events` vorher kein JSON liefert (A10).
  - **Kontext** *(G28)*: Der Docker-Kontext wird angezeigt und je Fenster gewählt, über `--context` in jedem Aufruf.
    `docker context use` ändert ihn global für alle Shells des Nutzers und wird nie aufgerufen.
  - **Daemon läuft nicht** *(G28)*: Endet `docker events` mit Fehler, zeigt das Panel „Docker läuft nicht“ und
    versucht es mit wachsender Wartezeit erneut.
  - **Panel** *(G29)*: Neues Panel „Container“ hinter CI: Tests ⌘5, Debuggen ⌘6, CI ⌘7, Container ⌘8.
    - Das ändert E55 und E94; nach E94 stand CI zuletzt.
    - Das Panel ist immer vorhanden und zeigt ohne `docker` einen Leerzustand mit Hinweis. Ausgeblendet wird es nur
      über E80.
  - Ansicht als Baum: Compose-Projekte mit ihren Diensten, einzelne Container, Images, Volumes. Zustand als Form und
    Text, nicht nur Farbe (E101).
  - Aktionen:
    - Starten, Stoppen, Neu starten
    - Logs in einem Terminal-Tab (`docker logs -f`)
    - Shell in einem Terminal-Tab (`docker exec -it`)
    - veröffentlichten Port im Browser-Tab öffnen
  - Terminal-Tabs für Logs und Shell kommen nach einem Neustart nicht wieder (Ausnahme von E64); sie wären sonst eine
    leere Shell im Projektordner *(G30)*.
  - Löschen von Containern, Images und Volumes sowie `prune` sind zerstörerisch: Sheet mit `hasDestructiveAction`,
    nie Standardknopf (wie `src/git_panel/sheets.rs`).
  - Compose-Dateien des Projekts erscheinen auch ohne laufende Container. `up`, `down` und `pull` nur in vertrauten
    Ordnern (E22), weil Compose Dockerfiles und Befehle aus dem Projekt ausführt.
  - Alle Befehle stehen auch in einem neuen Menü „Container“ zwischen „Debuggen“ und „Fenster“ (E101), ausgegraut,
    wenn `docker` fehlt.
- **Abnahme:** Mitschnitt-Tests der JSON-Ausgaben der Mindest- und der aktuellen Version in einer neuen Crate
  `ide-docker` ohne AppKit (E99). Ein Selftest mit einem Fake-`docker` auf dem PATH prüft Panel, Aktionen, Leerzustand,
  „Docker läuft nicht“ und das Sheet beim Löschen.

## Risiken

- **R23 `modalkit` ist Version 0.0.x mit im Wesentlichen einem Betreuer.** Abgefangen durch die feste Version als
  eigene Kopie (E104) und die Crate `ide-vim` als einzige Stelle, die `modalkit` kennt. Im schlimmsten Fall werden
  Bewegungen selbst gebaut, die Oberfläche bleibt.
- **R24 Vim-Nutzer messen am Original.** Lücken fallen sofort auf, auch Unterschiede der Suchmuster (Rust-Regex statt
  Vim-Regex). Abgefangen durch einen klaren Umfang (E106), eine Meldung bei nicht unterstützten Befehlen statt
  stillem Ignorieren und Tabellentests.
- **R25 iCloud Drive ist langsam und nicht steuerbar.** Abgleich nach Sekunden bis Minuten, Auslagerung von Dateien,
  der Ordner ist für den Nutzer sichtbar. Für Einstellungen genügt das; der Ordner bekommt eine kurze Liesmich-Datei.
- **R28 `@olrtg/emmet-language-server` wird kaum gepflegt** (letzte Version 2.8.0 vom 2025-08-08). Zed nutzt denselben
  Server. Fällt er aus, entfällt nur Emmet.
- **R29 Testausgaben sind kein fester Vertrag.** Das gilt vor allem für die Textausgabe von libtest. Abgefangen durch
  eingefrorene Mitschnitte je Werkzeug und Version in `ide-test`, und JUnit oder JSON, wo es das gibt.
- **R32 Feature-Vereinigung von Abhängigkeiten** *(F1)*. Eine neue Crate kann über Standard-Features das Verhalten
  einer vorhandenen ändern, ohne dass ein Build fehlschlägt. Abgefangen bei `ropey` durch die eigene Kopie von
  `modalkit` und einen Test der Zeilenregel. Bei jeder neuen Abhängigkeit wird `cargo tree -e features` auf
  geteilte Crates geprüft.
- **R33 Die iCloud-Erlaubnis gilt für die ganze App** *(F16)*. Hat der Nutzer sie erteilt, erreichen auch Terminals
  und Agenten, die Dango startet, iCloud Drive. Das gehört zu R5; die Einstellungen sagen es beim Einschalten.
- **R37 Der Docker-Socket entspricht Root-Rechten** *(G30)*. Ein Fehlklick kann Daten in Volumes löschen. Abgefangen
  durch zerstörerische Sheets und Aktionen nur auf ausdrücklichen Klick. Agenten bekommen kein Docker-Werkzeug, können
  `docker` aber über ihr Shell-Werkzeug aufrufen; das unterliegt E19 und E27.
- **R38 Der ESLint-Server kommt aus einem Upstream-Tag** *(G17)*. Das gebündelte npm-Paket ist seit Mai 2024 stehen
  geblieben. Abgefangen durch einen festen Tag mit Prüfsumme (R8) und Tests gegen ESLint 9 und die aktuelle
  Hauptversion (A11); jedes Update ist eine bewusste Änderung.

Risiken der zurückgestellten Features, gültig erst bei Wiederaufnahme:

- **R26 Umbau von B25 Teil 1 ohne neue Funktion** (wie R17): Hunderte Signaturen und Dateizugriffe, dazu die Umstellung
  synchroner Aufrufer auf asynchrone. Fehler zeigen sich als Verhaltensänderung lokal. Abgefangen durch die Sperre
  nach Stufe 3, volles `make ci` je Teilschritt und Umstellung crate-weise.
  *Zurückgestellt mit B25.*
- **R27 Linux-Build.** macOS-Eigenheiten stecken in mehreren Crates (Liste in Stufe 3). Jede neue macOS-Abhängigkeit in
  einer Dienst-Crate bricht künftig den Linux-Build in `make ci`; das ist gewollt, kostet aber dauerhaft Disziplin.
  Dazu kommen die Grenze der inotify-Watches bei großen Repos und der Allokator unter musl.
  *Zurückgestellt mit B25.*
- **R30 Dango legt ein Programm auf fremde Rechner.** Nur mit Zustimmung beim ersten Verbinden je Host, mit Version
  im Pfad, und „Vom Server entfernen“ in Ablage ▸ Verbindungen. Portweiterleitungen nur auf 127.0.0.1, der Socket
  des Dienstes nur für den Nutzer.
  *Zurückgestellt mit B25.*
- **R31 Zustand auf zwei Seiten** *(C35, C36, C45)*. Puffer, Checkpoints und laufende Prozesse leben teils im Mac,
  teils auf dem Host. Jeder Abbruch zwischen beiden ist ein Fall für Datenverlust. Abgefangen durch Operations-IDs,
  Fingerprints, Instanz-IDs und gemeinsame Checkpoint-IDs.
  *Zurückgestellt mit B25.*
- **R34 Notebooks sind der größte Einzelposten außerhalb von Remote.** Viele Editoren in einer Ansicht, ein eigener
  Undo-Stapel und ein Dateiformat, das je Werkzeug anders geschrieben wird. Abgefangen durch die Undo-Sperre (E137),
  byte-genaue Roundtrip-Tests und den Verzicht auf Sprachserver, Widgets und HTML-Ausgaben in der ersten Ausgabe.
  *Zurückgestellt mit B31.*
- **R35 `netcoredbg` hat für macOS je Architektur einen anderen Stand** *(G23)*. Intel bekommt ein älteres Release
  (3.1.3-1062); der arm64-Build ist nur ad hoc signiert und „community supported“. Prüfsummen gibt es nur als
  Konstanten in Dango. Dazu die Lieferkette nach R16 für `netcoredbg` und das `java-debug`-Plugin.
  *Zurückgestellt mit B34.*
- **R36 Aktive Inhalte in Notebooks** *(G38)*. HTML- und JS-Ausgaben sind Code aus der Datei. Abgefangen, indem sie nie im
  Notebook laufen, sondern nur auf Knopfdruck im Browser-Tab eines vertrauten Ordners (E139).
  *Zurückgestellt mit B31.*
- **R39 Der Port von `java-debug` ist offen und ohne Anmeldung** *(G25)*. Das Plugin lauscht auf allen Schnittstellen.
  Abgefangen nur dadurch, dass Dango sich sofort nach der Antwort verbindet; wer schneller ist, bekommt eine Debug-Sitzung des Projekts.
  *Zurückgestellt mit B34.*

## Annahmen

Die Zählung setzt die aus phase-11 (A1–A4) fort *(F37)*.

- **A5** Welcher Begründungstext in `Info.plist` die iCloud-Abfrage für eine App ohne Sandbox trägt, und dass
  ausgelagerte Dateien beim Zugriff nachgeladen werden. Zu prüfen am Anfang von Stufe 1, mit signiertem
  Developer-ID-Build, frischem Benutzerkonto und verweigerter Zustimmung, vor dem Bau von B28.
- **A6** AppKit gibt ⌃-Kürzel zuerst an `performKeyEquivalent:` der Ansichten im Schlüsselfenster und erst danach an
  die Menüleiste. Von Codex und Fable gegen Apples Dokumentation bestätigt; der Prototyp von E104 prüft es im echten
  Fenster, auch bei geteiltem Fenster.
- **A10** Die JSON-Ausgaben von `docker` ab 24.0.6 und Compose v2 sind über Versionen hinweg stabil genug für
  Mitschnitt-Tests *(G27)*. Zu prüfen zu Beginn von B33 gegen die Mindest- und die aktuelle Version.
- **A11** Der ESLint-Server aus dem gewählten Tag arbeitet mit ESLint 9 und der aktuellen Hauptversion, auch mit
  `eslint.config.ts` *(G17)*. Zu prüfen zu Beginn von B32b.

Annahmen der zurückgestellten Features:

- **A7** Ein statischer Linux-Dienst mit den nötigen Crates hat eine Größe im niedrigen zweistelligen MB-Bereich je
  Architektur. Wird in Stufe 3 gemessen; ist er deutlich größer, wird die Mitlieferung im Bundle neu entschieden.
  *Zurückgestellt mit B25.*
- **A8** `devcontainer exec` reicht Größenänderungen und Signale der PTY weiter. Zu prüfen mit einem Prototyp zu
  Beginn von Stufe 8; sonst laufen Terminals im Container über die Brücke und `docker exec -it` mit den von der CLI
  ausgewerteten Werten. *Zurückgestellt mit B25.*
- **A9** jdtls nimmt das `java-debug`-Plugin über `initializationOptions.bundles` an und antwortet auf
  `vscode.java.startDebugSession` mit einem Port. Zu prüfen zu Beginn von B34. *Zurückgestellt mit B34.*
- **A12** `dotnet msbuild -getProperty:TargetPath` liefert den Pfad der gebauten DLL. Zu prüfen zu Beginn von B34.
  *Zurückgestellt mit B34.*
- **A13** Wo `ipykernel` installiert ist, ist auch `jupyter_client` vorhanden. Zu prüfen zu Beginn von Stufe 6; sonst
  nennt die Statusleiste den Installationsbefehl. *Zurückgestellt mit B31.*

## Zurückgestellt

Vom Owner am 2026-11-13 zurückgestellt; in dieser Phase nicht umzusetzen. Die Abschnitte bleiben, wie sie nach den
Reviews standen, damit eine spätere Wiederaufnahme nicht von vorn beginnt. Verweise auf Stufen beziehen sich auf die
Reihenfolge vor der Rückstellung: Linux-Machbarkeit, Host-Baustein und Remote kamen nach den Tests, Notebooks nach
Docker. Bei einer Wiederaufnahme wird die Reihenfolge neu festgelegt.

### B34 Debugger für C# und Java

Heute kennt `ide-dap` die Adapter `Lldb`, `Go`, `JsDebug` und `Python` (`crates/ide-dap/src/adapter.rs`). C# und Java
laufen nur über eigene Sprachserver des Nutzers (E10).

- **E134 C# über `netcoredbg`.**
  - `netcoredbg` (Samsung, MIT) läuft mit `--interpreter=vscode`. Der Microsoft-Debugger `vsdbg` scheidet aus: Seine
    Lizenz erlaubt ihn nur in Microsofts eigenen Produkten.
  - **Je Architektur eine feste Version** *(G23)*: Das neueste Release 3.2.0-1092 hat für macOS nur einen Build für
    Apple Silicon, die Releases davor nur einen für Intel.
    - Apple Silicon: 3.2.0-1092.
    - Intel: 3.1.3-1062.
    - Samsung veröffentlicht keine Prüfsummen; die Summen sind Konstanten in Dango wie in
      `crates/ide-dap/src/install.rs`.
    - Geladen wird über `ide-net` nach Nachfrage in den Werkzeugordner (wie E10).
  - Neue Art `AdapterKind::NetCore`. `launch.json` vom Typ `coreclr` (eine Konvention von VS Code) wird darauf
    abgebildet.
  - Ohne Konfiguration sucht `current_program` die `.csproj` der Datei, baut mit `dotnet build -c Debug` über den
    Supervisor und startet die DLL aus `dotnet msbuild -getProperty:TargetPath` (A12). Ein Projekt mit mehreren
    Zielframeworks fragt nach dem Framework.
  - Der Sprachserver für C# bleibt ein eigener Server des Nutzers; der Debugger hängt nicht von ihm ab.
- **E135 Java über `java-debug` in jdtls.**
  - `java-debug` ist ein Plugin für den Sprachserver jdtls; jdtls wird deshalb ein eingebauter Sprachserver, gesucht
    auf dem PATH der Login-Shell (`brew install jdtls`, das ein JDK mitbringt; die Statusleiste nennt den Befehl).
    Eine feste Java-Version schreibt der Plan nicht vor, sie folgt jdtls *(G26)*.
  - Hat der Nutzer einen eigenen Java-Server eingetragen (E10, `language.rs` nennt jdtls als Beispiel), geht der vor;
    Java-Debuggen ist dann ausgegraut und nennt den Grund *(G26)*.
  - Das Plugin-JAR `com.microsoft.java.debug.plugin` (EPL-1.0) kommt in der neuesten Version auf Maven Central (0.53.1)
    mit Prüfsumme, nach Nachfrage, und geht über `initializationOptions.bundles` an jdtls (A9). Dango liefert es nicht
    mit.
  - **Ablauf** *(G24)*: `src/debug` ruft über jdtls `vscode.java.resolveMainClass`, `vscode.java.resolveClasspath` und
    `vscode.java.startDebugSession` auf. Letzteres liefert einen Port, mit dem `ide-dap` sich über das vorhandene
    `dial` (`crates/ide-dap/src/session/start.rs`) verbindet. `console` wird ausdrücklich gesetzt (`internalConsole`),
    die Vorgabe des Plugins ist ein Terminal.
  - **Sitzung ohne eigenen Adapterprozess** *(G24)*: Der Adapter ist jdtls und gehört nicht der Debug-Sitzung.
    - `ide-dap` bekommt die Startart „mit vorhandenem Port verbinden“.
    - Stoppen heißt `terminate`, dann `disconnect`, dann Socket schließen, nie ein Signal an jdtls.
    - Die Frist von fünf Minuten aus E10 ruht, solange eine Sitzung läuft.
    - Abweichung von E94 und phase-11 C7 („der Adapter gehört immer Dango, zuletzt beendet der Supervisor seine
      Gruppe“).
  - **Offener Port** *(G25)*: Das Plugin lauscht auf allen Schnittstellen ohne Anmeldung. Dango verbindet sich sofort
    nach der Antwort; das Restrisiko steht in R39.
  - `launch.json` vom Typ `java` wird auf diesen Ablauf abgebildet.
- Beide Adapter laufen nur in vertrauten Ordnern (E93), über den Supervisor und den Host-Baustein (E116). Ihre
  Lieferkette fällt unter R16.
- **Abnahme:** Mitschnitt-Tests der Startbefehle beider Adapter in `ide-dap`. `make test-integration` debuggt ein
  kleines C#- und ein Java-Programm bis zu einem Haltepunkt und stoppt die Java-Sitzung, ohne jdtls zu beenden.

### B31 Jupyter-Notebooks

Heute kennt Dango `.ipynb` nur als Ziel der Werkzeuge `NotebookRead` und `NotebookEdit` von Claude
(`crates/ide-claude/src/tools.rs`).

- **E137 Notebook als eigener Dokumenttyp.**
  - **Lesen und Schreiben** *(G12)*: Neue Crate `ide-notebook` ohne AppKit, mit eigenem Lesen und Schreiben auf
    `serde_json::Value` mit `preserve_order`. Das Crate `nbformat` sortiert immer und lehnt 4.0–4.4 ab und taugt
    deshalb nicht.
    - Übernommen wird aus der Datei: Einrückung, Schlüsselreihenfolge, Zeilenumbruch am Ende, `nbformat_minor` und
      die Schreibweise von `source` (Text oder Zeilenliste). Jupyter schreibt mit einem Leerzeichen und sortierten
      Schlüsseln, Colab mit zwei Leerzeichen.
    - Unbekannte Felder und Metadaten bleiben erhalten.
    - Zell-IDs nur ab nbformat 4.5; in ältere Dateien fügt Dango keine ein.
  - **Ein Puffer je Datei** *(G11)*: Das Notebook-Modell ist der eine Puffer der Datei im Sinn von E24. Seine
    Serialisierung liefert Absturzkopie, Checkpoint-Puffer (B30) und Fingerprint. Jede Zelle hat ein `Document` für
    Farben und Positionen. Das ist eine Abweichung von E24, das einen Puffer je Datei vorsieht; die Zell-Dokumente
    sind Teile davon.
  - `.ipynb` öffnet in einem Notebook-Tab (E62). Über Ablage ▸ „Als Text öffnen“ gibt es die JSON-Ansicht, nur solange
    kein Notebook-Tab der Datei offen ist. Ein Treffer der Projektsuche im JSON springt in die passende Zelle;
    „Ersetzen in geöffneten Dateien“ (E55) lässt Notebooks aus.
  - Nur sichtbare Zellen bekommen einen vollen Editor, damit 1 000 Zellen flüssig bleiben.
  - **Undo** *(G10)*: AppKit hält das Undo des Textes, und ein abgebauter Editor verlöre seine Einträge
    (`crates/ide-document/src/lib.rs`). Das Notebook-Modell führt deshalb einen eigenen Undo-Stapel aus umgekehrten
    Änderungen und Zelloperationen (einfügen, löschen, verschieben, Typ ändern). Die Zell-Editoren geben über
    `undoManagerForTextView:` einen Manager zurück, der dorthin weiterreicht. **Sperre:** ein Prototyp zu Beginn von
    Stufe 6, wie bei E81 und E104.
  - Markdown-Zellen werden außerhalb der Bearbeitung über die Vorschau gezeigt (E1, Formeln über E44).
  - **Änderungen von außen** *(G15)*: Ändert ein Agent die Datei (`NotebookEdit`), führt Dango auf Zellebene zusammen,
    über die Zell-ID, ohne IDs über Position und Inhalt.
    - Zellen, die nur eine Seite geändert hat, werden übernommen. Nur Zellen, die beide geändert haben, zeigen den
      Konfliktbalken (E24).
    - Kernel, Ausführungszähler und laufende Ausgaben überleben das Neuladen.
    - Der eigene Agent (E6) hat kein Notebook-Werkzeug und schreibt rohes JSON; das gilt als Änderung von außen.
  - Sprachserver in Zellen gibt es in der ersten Ausgabe nicht; Vervollständigung und Hilfe kommen vom Kernel
    (`complete_request`, `inspect_request`). Syntaxfarben über den Lexer (E7).
  - **Befehle** im Menü Editor ▸ Notebook *(G16)*:
    - ⇧↩ Ausführen und weiter, ⌃↩ Ausführen, wie in Jupyter und VS Code
    - Alle ausführen, Kernel unterbrechen, Kernel neu starten
    - Zelle einfügen, löschen, verschieben, Typ ändern

    Die Kürzel wirken nur, wenn ein Notebook-Tab den Fokus hat, nie in der Eingabekarte der Agenten.
  - Nach einem Neustart (E64) kommt der Notebook-Tab wieder, der Kernel nicht *(G16)*.
  - VoiceOver: Zellen als Liste, Rotor „Zellen“, Ansage bei Ende einer Ausführung.
- **E138 Kernel über eine Python-Brücke statt eigenem ZeroMQ-Client** *(G5–G9)*.
  - Dango liefert ein kleines Python-Skript mit, wie das pytest-Plugin aus E110. Es läuft mit dem Python der
    Kernel-Umgebung, nutzt `jupyter_client` und spricht mit der App JSON-Zeilen über stdio.
  - Verworfen: die Rust-Crates `jupyter-zmq-client` (Nachfolger von `runtimelib`) und `zeromq`. Sie bräuchten eine
    Async-Laufzeit (tokio oder smol), die Dango nicht hat, und auf einem Host fünf weitergeleitete Ports samt
    Schlüssel.
  - Mit der Brücke entfallen ZeroMQ, Laufzeit, HMAC und Ports. Unterbrechen nach `interrupt_mode` erledigt
    `jupyter_client`: SIGINT an den Kernel, `interrupt_request` nur bei `interrupt_mode: message` *(G9)*.
  - Kernel-Specs aus `jupyter kernelspec list --json` über die Login-Shell; bevorzugt wird die `.venv` des Projekts mit
    `ipykernel`.
  - Preis: Kernel ohne Python-Umgebung mit `jupyter_client` werden nicht unterstützt (A13). Fehlt `ipykernel`, nennt die
    Statusleiste den Installationsbefehl; Dango installiert nicht selbst in die Umgebung des Projekts.
  - Die Brücke startet über den Supervisor mit der neuen Art `Kind::Kernel` und über den Host-Baustein (E116), nur in
    vertrauten Ordnern (E22), weil sie Projektcode ausführt.
  - Ausführen, Unterbrechen, Neustart, Beenden; Ausführungszähler und Zustand (bereit, beschäftigt) in der Kopfzeile.
    Eingabeanfragen (`input_request`) erscheinen als Feld unter der Zelle.
- **E139 Ausgaben ohne JavaScript, kein drittes WebKit** *(G13, G14)*.
  - Unterstützt:
    - Streams (mit ANSI-Farben wie im Terminal)
    - Fehler mit Traceback
    - `text/plain`, `image/png`, `image/jpeg`
    - `image/svg+xml`, `text/markdown` und `text/latex` über den Decoder-Dienst (E44)
  - `text/html` wird nicht als Webseite gezeigt; E1 erlaubt WebKit nur im Browser-Tab und im unsichtbaren Renderer.
    - Angezeigt wird die andere Form desselben Ausgabebündels: `image/png` oder `text/plain`, wie sie etwa pandas und
      Plotly mitliefern.
    - Fehlt beides, steht dort „HTML-Ausgabe“ mit dem Knopf „Im Browser-Tab öffnen“. Er schreibt die Ausgabe in den
      Cache-Ordner und öffnet sie dort, nur in vertrauten Ordnern (E22).
    - Das ersetzt das „Ausgaben vertrauen“ des Entwurfs; ein eigener Speicher für vertraute Notebooks entfällt.
  - ipywidgets brauchen einen Kanal zum Kernel und sind nicht in der ersten Ausgabe; sie zeigen „Widget nicht
    unterstützt“.
  - Lange Ausgaben werden gekürzt (E25); die ganze Ausgabe öffnet in einem Tab.
- **Abnahme:**
  - Roundtrip-Tests in `ide-notebook` mit Notebooks aus Jupyter, VS Code und Colab: Die Datei ist nach Laden und
    Sichern byte-gleich, auch bei Zahlenformaten, Escapes und nbformat 4.0–4.5.
  - Tests der Brücke mit einem Fake-Kernel; `make test-integration` führt ein Notebook mit `ipykernel` aus.
  - Ein Selftest prüft Ausführen, Unterbrechen, Ausgaben, Undo über abgebaute Editoren und ein `NotebookEdit` bei
    offenem Tab.
  - `make perf` bekommt einen Fall mit 1 000 Zellen: Scrollen und Tastendruck im Budget von E25 *(G16)*.

### B25 Remote-Entwicklung

#### Linux-Machbarkeit (vorher Stufe 3)

*(C34, F33)* Für Linux baut heute nur `cargo xtask seal check` im GitHub-Workflow. Bekannte macOS-Bindungen der Crates,
die der Dienst braucht:
- `ide-agent` hängt über `ide-secrets` an `security-framework`.
- `ide-core::paths` legt alles unter `~/Library/...` ab; jede Crate nutzt es.
- `ide-terminal` fragt Prozesse über `proc_info` ab (`sys.rs`, `foreground.rs`).
- Die Beobachtung nutzt nur das FSEvents-Backend von `notify`; dazu `ide-project/src/tree.rs`.
- `xcrun` in `ide-dap`, `ide-git/src/run.rs` und `ide-lsp/src/registry.rs`.
- Der eigene Agent startet über `sandbox-exec`.
- `ide-git`, `ide-lsp` und `ide-project` hängen an `splat-i18n`. Der Dienst bleibt sprachneutral und liefert
  strukturierte Meldungen, die die App übersetzt (Muster von E41).
- `ide-document` zieht `splat-markdown` mit `comrak` und `splat-ooxml`.

Ablauf:
- Ein minimaler Dienst mit dem Abhängigkeitsgraphen, den Stufe 7 und 8 brauchen, wird für Linux x86_64 und aarch64
  (statisch, musl, `cargo-zigbuild`) gebaut, gelinkt und auf einem echten Host gestartet.
- Gemessen werden Größe (A7) und Laufverhalten. Geprüft werden außerdem die Grenze der inotify-Watches bei einem
  großen Repo und der Allokator unter musl.
- Schlüsselbund und lokale Anbieter werden dabei vom Teil getrennt, der auf dem Host läuft.
- `cargo check` allein genügt nicht, weil es keinen fertigen statischen Dienst prüft.
- Ergebnis: eine Liste der Crates, die für den Dienst umgebaut werden müssen, und eine neue Schätzung für Stufe 4, 7 und 8.
- Ohne bestandene Stufe 3 beginnt Stufe 4 nicht.

#### Host-Baustein (vorher Stufe 4)

#- **E116 Ein asynchroner Baustein für vollständige Vorgänge** *(C32, F5)*. Heute ist die Projektwurzel überall ein
  `PathBuf`. Prozesse starten über `Supervisor::spawn` mit einem lokalen `Command`, und viele Zugriffe sind synchron
  auf dem Hauptthread:
  - `Settings::in_project` liest `.dango/settings.json`.
  - `Document::open` ist synchron.
  - `paths::project_dir` fragt `exists()`.
- **Zählung** *(C33, F32)*, Stand 2026-11-13. Befehl:
  `rg -c -t rust -g '!**/tests/**' -g '!**/selftest/**' -g '!**/selftest*.rs' '<muster>' src crates`. Inline-Testmodule
  sind mitgezählt, App-Daten nicht herausgerechnet.
  - 505 Aufrufe von `fs::…`, `File::open`, `File::create` und `OpenOptions` in 86 Dateien
  - 168 Abfragen über Pfad-Methoden (`exists`, `is_dir`, `is_file`, `canonicalize`, `metadata`, `read_dir`)
  - 250 Signaturen mit `root` oder `folder` als Pfad
  - 32 `Command::new`
- Neue Crate `ide-host` mit `Location` (E122) und einem `Host` auf der Ebene ganzer Vorgänge. Antworten kommen
  asynchron mit Kennungen (E97), nie blockierend auf dem Hauptthread. Die Vorgänge:
  - Dateien lesen und atomar schreiben, mit erwartetem Fingerprint (E115)
  - Baum mit Ignorier-Regeln, Beobachtung mit Meldung verpasster Ereignisse (E24)
  - Schnellöffnen-Index und Projektsuche als Strom von Treffern
  - git-Aufrufe
  - Start von Sprachservern, Debug-Adaptern, Agenten und Testläufen
  - Werkzeugprozesse mit Ausgabestrom, Signal und Abbruch: Notebook-Brücke, `docker`, Prettier, `dotnet build`
    *(G8)*
- Jeder Vorgang hat Abbruch und Rückstaugrenze (E25). Lokal führt der heutige Code die Vorgänge aus, die Aufrufer
  werden auf die asynchrone Form umgestellt. Projekt-Einstellungen, `.editorconfig` und `launch.json` werden je
  Fenster zwischengespeichert und über die Beobachtung erneuert.
- Was nur App-Daten betrifft, bleibt lokal und synchron: Installationen, Stores, Wiederherstellung und die
  Nutzer-Einstellungen (`Settings::load_user`). Die Stores bekommen aber die Identität aus E122.
- Der Baustein erfüllt den Teil von E20, der Prozessstarts und Projektzugriffe bündelt. Die Ordnerfreigaben im Stil
  von splats `grants.rs` für die Store-Ausgabe bleiben offen *(C32)*.
- Nach dieser Stufe verhält sich die App genau wie vorher. Abgenommen wird mit dem vollen `make ci`.
- **E122 Ort und Identität getrennt** *(C37, F8)*. `Location` trägt eine Host-ID und den Pfad.
  - Die Host-ID erzeugt der Dienst beim ersten Start. Er legt sie in seinem Zustandsordner auf dem Host ab, je Host
    und Nutzer. Sie bleibt gleich, wenn sich Alias, Adresse oder Sprungrechner in `~/.ssh/config` ändern. Die Prüfung
    des Host-Schlüssels bleibt bei `ssh`.
  - Ein Container ist Host-ID plus lokaler Ordner plus Pfad der `devcontainer.json`, nicht die Container-ID. So
    übersteht er jedes Neuerstellen.
  - Die Identität gilt in:
    - `project_key` und den Layouts (E64)
    - der privaten `Description` in `crates/ide-document/src/recovery.rs`
    - dem `TrustStore` (E22)
    - dem Verlauf der Sitzungen und der Zuordnung offener Dokumente
  - Damit bekommen `/work/app` auf zwei Hosts verschiedene Einträge.
  - Bestehende Einträge werden als lokaler Host übernommen (Migration der Layouts und `state.json`).

#### Remote-Kern und Remote komplett (vorher Stufe 7 und 8)

- **E115 Modell wie Zed: dauerhafter Dienst, kurzlebige Brücke** *(C35, C36, F7, F22)*.
  - Auf dem entfernten Rechner läuft ein Dienst ohne Oberfläche mit den Vorgängen aus E116.
  - Er lauscht auf einem Unix-Socket in seinem Zustandsordner unter dem Home (Rechte 0700), nicht im Laufzeitordner.
    `/run/user/<uid>` fehlt in vielen SSH-Sitzungen und wird mit der letzten Sitzung gelöscht. Die Pfadlänge wird gegen
    die Grenze von `sun_path` (108 Bytes) geprüft.
  - Beendet systemd-logind Prozesse beim Abmelden (`KillUserProcesses=yes`), sagt Dango das beim Verbinden.
  - Jede Verbindung startet eine kurzlebige Brücke zwischen SSH und Socket (E126).
  - Reißt die Verbindung, endet nur die Brücke. Der Dienst hält Sitzungen und laufende Prozesse.
    - Prozessausgaben gehen nicht verloren: Bis zu einer Grenze schreibt der Dienst sie auf Platte. Darüber hinaus
      blockiert der Rückstau das Kind, statt Daten zu verwerfen. Das gilt auch für das Stream-JSON von `claude`.
    - Die App meldet sich mit Sitzungs-ID und letzter Ereignisnummer zurück und setzt dort fort.
  - Jede Instanz des Dienstes trägt eine Instanz-ID. Nach einem Absturz oder Neustart des Hosts erkennt die App so,
    dass ihre Sitzungen beendet sind, und zeigt das.
  - Unterschiedliche Regeln:
    - App beenden: Agenten-Hinweis wie E23, dann Dienst-Prozesse stoppen.
    - Bewusst trennen: fragen, ob laufende Prozesse weiterlaufen sollen.
    - Netzverlust: alles läuft weiter.
  - Ein Dienst ohne Verbindung endet nach einer Frist von 24 Stunden ohne laufende Prozesse. Das ist eine
    Abweichung von E23, weil der Supervisor des Macs diese Prozesse nicht selbst hält; die Aktivität zeigt sie
    trotzdem mit Herkunft „Host“.
  - **Schreiben:** Jeder Schreibvorgang trägt eine Operations-ID und den erwarteten Fingerprint der Datei.
    - Eine wiederholte Operation nach verlorener Antwort erkennt der Dienst und antwortet mit dem ersten Ergebnis.
    - Nach einem Neustart des Dienstes gilt: Gleicht der Fingerprint der Datei schon dem des zu schreibenden Inhalts,
      ist das Schreiben erledigt, ohne Konflikt.
    - Weicht er ab, folgt der Konfliktbalken aus E24.
    - Nach Lücken der Beobachtung wird neu abgeglichen.
  - Ungesicherte Puffer bleiben in der App. Sichern ohne Verbindung schlägt sichtbar fehl, nichts geht verloren.
  - **Versionswechsel:** Ein kleines Protokoll, das sich über Versionen nicht ändert (Hallo, Sitzungen auflisten,
    beenden), erreicht nach einem App-Update auch den Dienst der alten Version. Die App bietet dann an, ihn zu
    beenden, und räumt alte Versionen im Cache des Hosts auf.
  - Dateizugriffe einzeln über SSH (Weg von SSHFS) scheiden aus: Projektsuche, Beobachtung und Sprachserver wären über
    jede Leitung zu langsam.
- **E126 Eine Brücke je Host mit eigenen Strömen** *(F6)*.
  - sshd erlaubt standardmäßig 10 Sitzungen je Verbindung (`MaxSessions`). Deshalb läuft je Host genau eine Brücke,
    und darin sind alle Vorgänge gebündelt: Dateien, Suche, Sprachserver, Agenten, Weiterleitungen.
  - Jeder Strom hat ein eigenes Fenster für die Flusskontrolle, damit ein großer Lesevorgang den Verkehr des
    Sprachservers nicht blockiert.
  - Das erweitert `ide-rpc`, das heute nur Anfrage und Antwort mit einer Schreibschlange kennt; die Rahmung bleibt.
  - Terminals (E118) brauchen je eine eigene Sitzung. Lehnt der Host eine weitere ab, öffnet Dango einen zweiten Master
    und sagt das in der Aktivität.
- **E117 Verbindung über das `ssh` des Systems** *(C41, F19)*.
  - Kein eigener SSH-Stack. Damit gelten `~/.ssh/config`, ssh-agent, ProxyJump, Hardware-Schlüssel und `known_hosts`
    unverändert.
  - Der Master läuft im Vordergrund (`ssh -M -N`) unter dem Supervisor (E23), mit `ServerAliveInterval` und eigenem
    `ControlPath` im Cache-Ordner von Dango. Dango beendet nur Master, die es selbst gestartet hat; einen vorhandenen
    fremden Master nutzt es nicht.
  - Brücken starten mit `-T -o RemoteCommand=none`. Die Brücke schickt zuerst eine feste Marke, weil Ausgaben aus
    `.bashrc` sonst den Strom verschmutzen.
  - **Abfragen:** `SSH_ASKPASS` mit `SSH_ASKPASS_REQUIRE=force`. Der Helfer ist das App-Programm in einem eigenen
    Modus.
    - Er erreicht die laufende App über einen lokalen Socket und ein Einmal-Token in der Umgebung des
      `ssh`-Prozesses.
    - Die Art der Abfrage kommt aus `SSH_ASKPASS_PROMPT`: Passwort und 2FA (auch mehrere hintereinander) oder
      `confirm` für einen unbekannten Host-Schlüssel, mit Fingerprint.
    - Ein geänderter Host-Schlüssel ist keine Frage: `ssh` bricht ab, Dango zeigt die Meldung und den Befehl
      `ssh-keygen -R`.
  - Automatisch neu verbinden nur mit `BatchMode=yes`. Braucht die Verbindung eine Eingabe, zeigt das Fenster einen
    Knopf „Verbinden“.
  - Einstieg: Ablage ▸ „Mit Server verbinden …“ mit den Hosts aus `~/.ssh/config`. Remote-Projekte stehen in der Liste
    der letzten Projekte mit Host.
- **E129 Remote-Zustand in der Oberfläche** *(F25)*.
  - Die Toolbar zeigt bei Remote-Fenstern Host und Zustand (verbunden, verbindet, getrennt) mit Text und Symbol, nicht
    nur über Farbe. Das ergänzt E73.
  - Menü Ablage ▸ Verbindungen: Liste der Hosts mit Zustand, Trennen, Neu verbinden, „Vom Server entfernen“.
  - Nach einem Neustart (E64) kommen Remote-Fenster getrennt zurück, mit „Verbinden“ statt Passwortabfragen beim
    Start. Das ist eine Ergänzung zu E64.
- **E119 Der Dienst kommt aus der App und passt genau zu ihrer Version** *(F23, F34)*.
  - Unterstützte Hosts: Linux x86_64 und aarch64 (statisch, musl, über `cargo-zigbuild` in `xtask`) sowie macOS
    (Universal, E16). Kein Windows, BSD oder 32-Bit-ARM. Erkannt wird das System über `uname -sm`.
  - Die Programme liegen im App-Bundle unter `Contents/Resources`, nicht `Contents/MacOS`.
  - Hochgeladen wird bei der ersten Verbindung über die Brücke mit `cat >`, ohne sftp, in den Zustandsordner unter der
    neutralen Kennung und der Version. Ist der Ordner `noexec`, sagt Dango das. Kein Download vom Server aus, kein
    eigener Download-Dienst.
  - Prüfung: `make check` bleibt bei macOS. `make ci` baut den Dienst für beide Linux-Ziele und lässt seine Tests in
    einem Linux-Container laufen (aarch64 nativ, x86_64 emuliert). Das gehört zum Siegel; GitHub überspringt
    versiegelten Inhalt und würde einen reinen Linux-Job sonst nie ausführen.
  - Neue Voraussetzungen der Entwicklungsumgebung: zig, `cargo-zigbuild`, zwei rustup-Ziele, Docker. Sie kommen mit Stufe 7
    nach [development.md](development.md).
- **E123 Vertrauen auf dem Host** *(C38)*.
  - Die Zustimmung, den Dienst auf einen Host zu legen, ist kein Projektvertrauen.
  - Projektvertrauen (E22) gilt je `Location` (E122).
  - Vor dem Vertrauen startet der Dienst keine Prozesse im Projekt. Er liest Dateien und die Git-Marken prozesslos
    mit `splat-git` wie lokal; `splat-git` hat keine macOS-Abhängigkeit, Stufe 3 bestätigt den Linux-Build.
- **E118 Terminal über eine lokale PTY mit `ssh -tt`, ohne Fortsetzung** *(C42, F20)*.
  - Das Terminal startet lokal `ssh -tt` über den geteilten Master. Auf dem Host wechselt es in den Projektordner und
    startet die Login-Shell des Nutzers mit der Shell-Integration (OSC 133, OSC 7). Die Skripte legt der Dienst ab;
    Argumente und Umgebung je Shell übernimmt er aus `integration.rs`.
  - Die bestehende Sitzung auf `alacritty_terminal` bleibt unverändert. Größenänderungen gehen über SSH.
  - Stoppen beendet den lokalen `ssh`-Prozess. sshd schickt der Shell dann SIGHUP; der Dienst kennt ihre PID nicht.
  - Bild-Einfügen folgt E9: Hinweis statt lokalem Pfad (`ssh` ist in `foreground.rs` schon erkannt).
  - Terminals sind ausdrücklich von der Fortsetzung aus E115 ausgenommen: Reißt die Verbindung, endet die Shell. Wer
    das nicht will, nutzt tmux; die Terminal-Ansicht sagt das beim Abbruch.
- **E127 Portweiterleitung durch den Dienst** *(C44, F21)*.
  - Weitergeleitet wird als Strom durch den Dienst (E126), einheitlich für SSH-Host, lokalen Container und Container
    auf einem SSH-Host. Der Container-Port muss dafür nicht vom Host aus erreichbar sein.
  - Dev-Server, die im entfernten Terminal auf localhost erkannt werden, bekommen zuerst dieselbe Portnummer auf dem
    Mac. So funktionieren HMR von Vite, OAuth-Weiterleitungen und CORS unverändert. Ist der Port belegt, gibt es
    einen freien Port, und der Browser-Tab nennt den Unterschied (I7).
  - Die Ports aus `devcontainer.json` (`forwardPorts`) werden zusätzlich angeboten.
  - Weiterleitungen lauschen nur auf 127.0.0.1 und enden beim Trennen und beim Neuerstellen des Containers.
- **E128 Sprachserver und Debugger auf dem Host** *(F24)*.
  - URIs und Pfade sind die des Hosts; die App rechnet sie über `Location` um.
  - Dateien außerhalb der Wurzel, etwa aus der Cargo-Registry oder Systemheader, öffnet die App nur lesend über den
    Dienst.
  - `didChangeWatchedFiles` kommt aus der Beobachtung des Dienstes.
  - `runInTerminal` von Debug-Adaptern startet ein Terminal auf dem Host (E118). Die Prozessauswahl zum Anhängen
    listet die Prozesse des Hosts.
  - Lokal bleiben gitleaks (E88, R15) und der Copilot-Server (E91): Commit-Messages und Vorhersagen werden auf dem Mac
    erzeugt.
- **Weitere Vorgänge auf dem Host** *(G8)*.
  - Notebooks: Die Brücke aus E138 läuft auf dem Host als Werkzeugprozess über die Brücke des Dienstes (E126), wie ein
    Sprachserver; es gibt keine Kernel-Ports weiterzuleiten.
  - Docker: `docker` läuft auf dem Host; das Panel zeigt dessen Container.
  - Java-Debuggen: Der Port von `java-debug` liegt auf dem Host. `ide-dap` verbindet sich über einen Strom des
    Dienstes (E127), nicht über das Netz; R39 wiegt dort schwerer, weil der Port auf allen Schnittstellen des Hosts
    offen ist.
  - Live-Vorschau: siehe E131.
- **E121 Agenten laufen auf dem Host, Schlüssel bleiben auf dem Mac.**
  - `claude` und ACP-Agenten startet der Dienst auf dem Host. Dort müssen sie installiert und angemeldet sein; die
    Anmeldung macht der Nutzer im entfernten Terminal (E21).
  - Der eigene Agent ruft seine Anbieter weiter vom Mac aus auf, mit den Schlüsseln aus dem lokalen Schlüsselbund;
    nur seine Werkzeuge laufen über den Host.
  - **Checkpoints** *(C45)*: Ein Remote-Checkpoint besteht aus zwei Teilen unter einer gemeinsamen ID:
    - dem Git-Teil auf dem Host (Arbeitsbaum und Index; Nicht-Git-Ordner über die Schnappschuss-Ablage im Dienst)
    - den Revisionen der ungesicherten Puffer in der App (B30)

    Wiederherstellen setzt beide Seiten zurück. Ein Prompt geht erst raus, wenn der Dienst den Checkpoint bestätigt hat
    (E34). Ohne Verbindung gibt es keinen Prompt.
- **E124 Sandbox auf dem Host** *(C39, F29)*. `sandbox-exec` gibt es nur auf macOS.
  - Auf Linux-Hosts ist nur die Shell-Sandbox ausgegraut und sagt „auf diesem Host nicht verfügbar“. Es gibt keinen
    stillen Rückfall auf unbeschränkte Ausführung. Das ist eine Abweichung von E19 für Remote.
  - Die Grenzen des eigenen Agenten für Datei- und HTTP-Werkzeuge setzt Dangos Code selbst durch; sie gelten weiter.
  - Die Sandbox-Argumente der CLIs (Claude, Codex) gelten, soweit der Agent sie selbst umsetzt. Die Oberfläche zeigt
    die Wirkung je Host.
- **E125 Werkzeuge je Ausführungsort** *(C40, F34)*.
  - Suche und Installation von Sprachservern, Debug-Adaptern und Test-Werkzeugen laufen dort, wo das Projekt liegt:
    mit der Login-Shell-Umgebung des Hosts, die der Dienst einmal erfasst (wie `src/environment.rs` lokal), und mit
    einem Werkzeugordner auf dem Host. Das ändert E10 und E93 („in den App-Ordner“) für Remote.
  - Die Nachfrage vor einer Installation nennt den Host.
  - Hat der Host kein Internet, lädt die App das Paket auf dem Mac über `ide-net` mit Prüfsumme und lädt es über die
    Brücke hoch.
  - LLDB auf Linux: `lldb-dap` vom PATH des Hosts statt `xcrun`. Das ist eine Plattformausnahme zu E93, die die
    Debug-Einstellungen nennen.
- **E120 Dev Containers über die offizielle `devcontainer`-CLI** *(C43, F38)*.
  - Erstellen, Starten und Neuerstellen laufen über die CLI (`@devcontainers/cli`, nach Nachfrage installiert, braucht
    Node ≥ 20), nicht über einen eigenen Nachbau von `devcontainer.json`.
  - Dienst und Terminals starten über `devcontainer exec`, nicht über `docker exec`. Damit gelten `remoteUser`,
    `remoteEnv` und `userEnvProbe`.
  - Der Projektordner ist `remoteWorkspaceFolder`. Compose-Dienste und die Lifecycle-Befehle übernimmt die CLI.
  - Die Architektur des Containers kann von der des Hosts abweichen; der Dienst wird im Container über `uname -sm`
    neu bestimmt.
  - Terminals im Container bekommen ihre PTY über `devcontainer exec` mit Terminal (A8).
  - Liegt Docker auf einem SSH-Host, laufen CLI, Docker und die Installation dort, über den Dienst aus E115.
  - Ein Container erbt das Vertrauen des Ordners, der ihn definiert (E122). Ohne Vertrauen startet er nicht, denn
    `initializeCommand` läuft auf dem Host und gehört damit unter E22.
- **Leistung und Tests** *(F26)*.
  - Remote-Budgets ergänzen E25:
    - Tastendruck unverändert lokal (< 16 ms)
    - eine 1-MB-Datei bei 100 ms Laufzeit der Leitung in < 1 s geöffnet
    - erste Einträge des Dateibaums in < 1 s, der Rest als Strom
    - Projektsuche liefert erste Treffer in < 1 s
  - Selftests und `make perf` nutzen einen Transport, der die Brücke als Kindprozess startet, ohne Netz, mit
    eingespielter Verzögerung und Abbrüchen.
  - Echtes `ssh` und Docker laufen nur in `make test-integration`.
  - Die Abnahme von Stufe 7 und 8 enthält Abbrüche mitten im Sichern, im Checkpoint und im Agenten-Zug (R31).
