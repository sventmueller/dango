# Dango — Phase 14: Terminal aus Tapa (P7)

**Freigegeben am 2026-11-25** mit E199. Tapas Plan sieht diesen Schritt als P7 vor („Dango ersetzt `ide-terminal`, `ide-ui-terminal`, seine Terminalbereiche und das Vim-Modell durch diese Crates und bekommt Eingabefeld, Vorschläge und Blöcke“). Der Owner hat dazu am 2026-11-25 entschieden: Tapas Terminal wird eingebettet statt nachgebaut; dazu gehören Eingabefeld mit Farben, Vorschläge und Menü, Befehlsblöcke mit Übergabe an einen Agenten, Suche im Verlauf, Mitteilungen und Fortschritt, Smart Selection, Paste-Schutz und Secure Keyboard Entry. Tapa darf dafür um T1 und T2 erweitert werden, direkt auf seinem `main`.

Nicht Teil dieser Phase: der Terminalinhalt nach einem Neustart (Tapas F71), Splits im Terminal, das Quick Terminal und entfernte Arbeitsbereiche über SSH.

## Reihenfolge

| Stufe | Inhalt | Gate |
|---|---|---|
| 1 | Tapa: T1 Dev-Server an den Wirt, T2 Kindprozesse der Vorschläge über den Spawner des Wirts | Tapas `make check` grün, eigener Commit in `tapa` |
| 2 | Fundament: Abhängigkeiten, Identität, Spawner, Host, Einstellungen, Verlauf, Fassade `src/terminal`, alte Crates raus | `make check`; die Szenarien `terminal`, `terminal-view` und `paste-cli` laufen auf Tapas Ansicht |
| 3 | Befehle und Tasten | Selbsttest: jeder Menüpunkt erreicht die Ansicht im Vordergrund, kein Kürzel nimmt dem Editor eines weg |
| 4 | Einstellungsseite „Terminal“ | Selbsttest: ein Schalter wirkt sofort auf offene Terminals |
| 5 | Mitteilungen, Fortschritt, Glocke | Selbsttest mit OSC 9, OSC 777 und OSC 9;4 |
| 6 | Block an Agent | Selbsttest: die Ausgabe des Blocks steht als Text von außen im AI-Tab |
| 7 | Texte | `tests/translations.rs` prüft auch Tapas Schlüssel |
| 8 | Selbsttests der neuen Funktionen | neues Szenario `terminal-tapa` |
| 9 | Aufräumen und Doku | `make check`, `cargo xtask selftest probe` und die Terminal-Szenarien |

## Stufe 1 — Tapa

- **T1:** `term-ui` meldet einen erkannten Dev-Server mit `Change::DevServer(url)` an den Host, statt ihn zu verwerfen; Tapas eigene App ignoriert ihn wie bisher.
- **T2:** Die dynamischen Quellen von `term-suggest` (docker, podman, kubectl) starten ihre Programme über einen vom Host übergebenen Weg mit dessen Umgebung, nicht mehr über `std::process::Command` mit dem PATH der App. Tapas App behält ihr bisheriges Verhalten.

## Stufe 2 — Fundament

- **Abhängigkeiten:** `term-ui`, `term-base`, `term-config`, `term-session` und `term-suggest` aus `../tapa` (E14), danach `cargo hakari generate` und `manage-deps` (E194). Tapas `term-workspace-hack` kommt dabei mit; das ist seit `term-vt` schon so (Tapas E24).
- **Identität:** `term_base::set_identity` mit Dangos neutraler Bundle-Id aus `ide_core::paths::identity()` (E18), vor jedem anderen Aufruf der Crates; im Selbsttest `term_base::set_home` neben `ide_core::paths::set_home`. Damit landen Shell-Integration und eingefügte Bilder in denselben Ordnern wie bisher. Das alte `ide-shell-integration.fish` wird beim Start gelöscht, sonst lädt fish beide Skripte.
- **Spawner:** Ein `term_base::Spawner` über `ide_process::Supervisor` mit `Kind::Terminal`, der nicht selbst wartet (die Sitzung wartet auf ihr Kind), mit Auflegen wie bisher (E10, E23).
- **Host:** `term_ui::Host` in `src/host.rs` nach dem Muster von E99/E143; `Opened::Url` öffnet den Browser-Tab, `Opened::File` den Editor (`editor::reveal_position`), `Opened::TerminalHere` einen Terminal-Tab, `Change::DevServer` den Hinweis mit „Im Browser-Tab öffnen“.
- **Einstellungen:** Dango baut aus seinen eigenen Einstellungen ein `term_config::Config` (Schriftgröße, Wahltaste als Meta und die neuen Schalter aus Stufe 4) und ruft nach jeder Änderung `apply_config` an allen Ansichten auf. Themes: Tapas eingebaute „Tapa Light“ und „Tapa Dark“.
- **Verlauf:** `term_suggest::History` in `data_dir()/terminal-history.sqlite`; aufgezeichnet wird beim Ende eines Befehls mit Ordner, Branch, vorigem Befehl und Exit-Code wie in Tapa.
- **Fassade `src/terminal`:** `open_tab`, `open_running` (`runInTerminal` des Debuggers), `open_passing` (Logs und Shells der Container, G30), `running_in`, `agents_running`, `tab_of_process`, `recent_output` (`@terminal`) und das Wiederherstellen (`KeptTerminal`, nur der Ordner, nur in vertrauten Ordnern, E64) behalten ihr Verhalten; eine neu geöffnete Ansicht wird über ihre Id gefunden, nicht über „die vorderste“.
- **Raus:** `crates/ide-terminal`, `crates/ide-ui-terminal`, ihre Fuzz-Ziele, Property-Tests, Perf-Einträge und Zeilen in `docs/thread-locals.md`; die Ratchets in `xtask/src/rules.rs` sinken entsprechend.

## Stufe 3 — Befehle und Tasten

Tapas Ansicht nimmt außer Kopieren, Einsetzen, Alles auswählen und Leeren keine Aktionen über die Responder-Kette an; alles andere ruft die App als Methode der Ansicht auf. Dango bekommt dafür Menüpunkte (Suchen, Prompt- und Lesezeichen-Sprünge, Block auswählen, Ausgabe kopieren und sichern, Block falten, Lesezeichen, Kopieren mit Formatierung, Auswahl mit der Tastatur, Eingabefeld, Secure Keyboard Entry) und Kürzel in der zentralen Tabelle (E26). Kürzel, die im Editor etwas anderes bedeuten (⌘I, ⌘↑/⌘↓, ⇧⌘↑/⇧⌘↓, ⇧⌥⌘C), gelten nur, solange ein Terminal den Fokus hat; ⌘K bleibt „Terminal leeren“ (E30), ⌘F sucht im Terminal, wenn es den Fokus hat.

## Stufe 4 — Einstellungen

Eine Seite „Terminal“ mit den Schaltern aus Tapa: Eingabefeld für neue Terminals, Vim-Tasten im Feld, Vorschläge, Farben der Befehlszeile, Unterstreichen unbekannter Befehle, Befehlsblöcke, Paste-Schutz, Kopieren beim Auswählen, Wahltaste als Meta (zieht von „Allgemein“ hierher um). Standardwerte wie in Tapa.

## Stufe 5 — Mitteilungen, Fortschritt, Glocke

`Change::Notification` wird zur Mitteilung von macOS, wenn das Terminal nicht vorn ist (über Dangos vorhandene Mitteilungen, I9); `Change::Progress` zeigt den Fortschritt am Tab und im Dock; `Change::Bell` wie bisher.

## Stufe 6 — Block an Agent

„Block an Agent übergeben“ im Menü „Bearbeiten“ und im Kontextmenü des Terminals gibt die Ausgabe des aktuellen Blocks (über `with_session` und `block_text`) mit seinem Befehl an einen Agenten, als Text von außen markiert wie ein CI-Log (I8, E22). Für das Kontextmenü bekam Tapas `term_ui::Host` dazu `context_items`; der Eintrag trägt die Id der Ansicht, auf der das Menü aufging.

## Stufe 7 — Texte

Tapas Schlüssel aus `crates/term-ui/strings` und `crates/term-base/strings` kommen in Dangos 34 `Localizable.strings`; „Nicht mehr fragen“ bleibt bei Dangos „Don’t Ask Again“. `tests/translations.rs` liest dafür auch die Quellen dieser beiden Crates. Schlüssel, die nur Dangos altes Terminal nutzte, entfallen.

## Stufe 8 — Selbsttests

Die Szenarien `terminal`, `terminal-view` und `paste-cli` laufen auf `term_ui::inspect`, dessen Methoden denen von Dangos altem `inspect` entsprechen. Neu ist `terminal-tapa`: Eingabefeld, Vorschlag und Menü, Farben, Blöcke, Suche, Mitteilung, Smart Selection, Paste-Schutz und Secure Keyboard Entry.

## Stufe 9 — Aufräumen und Doku

README (Abschnitt Terminal), ACCESSIBILITY.md, docs/development.md (Fuzzing, Property-Tests, hakari-Beispiel), die Crate-Karte in plan.md und die Geschwister-Repos in CLAUDE.md.

## Risiken

- **R71 Kopplung an Tapas Build:** `term-workspace-hack` und Tapas Feature-Auswahl gehen in Dangos Build ein; jede neue Abhängigkeit in Tapa kann Dangos `cargo hakari generate --diff` rot machen, wie am 2026-11-25 mit `term-vt` 1.0.2.
- **R72 Gewicht:** SQLite wird mitgebaut, die Specs von `term-suggest` liegen als rund 10 MB im Programm.
- **R73 Dangos Prüfungen sehen Tapa nicht:** Statics, Lookups, `guarded`, 1000-Zeilen-Regel und Abdeckung gelten nur für `src/` und `crates/`; was in Tapas Crates steckt, prüft Tapa.
