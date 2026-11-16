# Dango — Phase 13: Leitstand (Software-Factory)

**Freigegeben am 2026-11-16 (ganze Phase).** Die Entscheidungen und Risiken stehen als Kurzfassung in [plan.md](plan.md);
die Einzelheiten hier sind ebenso verbindlich. O4 ist mit der Freigabe entschieden: lokal. Offen bleiben O1 und O2.

- **Kürzel:** B37–B47, A14–A21 und O1–O4 setzen den Plan fort. Entscheidungen und Risiken bilden einen eigenen Block,
  E160–E193 und R60–R70, weil eine parallele Sitzung am 2026-11-16 E148 und R45–R47 vergeben hat. In der Sitzung, aus
  der dieser Entwurf stammt, hießen E160–E164 noch E148–E152 und R60–R66 noch R45–R51. Ein Verzeichnis aller Kürzel
  steht am Ende *(F74)*.
- **Review:** Am 2026-11-16 haben zwei Reviews den Entwurf gegen Code und Plan geprüft, beide mit Zählungen, die die aus
  Phase 12 fortsetzen. Alle Befunde sind eingearbeitet und an der jeweiligen Stelle markiert.
  - Fable 5.2 mit 38 Befunden (F40–F77).
  - Codex mit 27 Befunden (C46–C72) auf dem Stand nach Fable.
- **Gestrichen (Owner, 2026-11-16):** das Kanban-Board (F66), B48 Best-of-N und B49 verdeckte Szenarien (F52). E181,
  E182 und A16 sind damit entfallen; Einzelheiten unter [Gestrichen](#gestrichen).

## Ziel

Was sich für den Nutzer ändert:
- Statt einzelner Prompts gibt er **Aufträge**: ein Ziel mit prüfbaren Abnahmekriterien, von Hand oder aus einem Issue.
- Dango plant, baut und prüft jeden Auftrag in einem eigenen Worktree (I1), mit Agenten mehrerer Hersteller in festen
  Rollen: Planer, Builder, Prüfer, Späher.
- Er entscheidet an zwei Stellen: **G1** Spezifikation freigeben, **G2** Nachweis abnehmen. G2 gilt immer, G1 entfällt
  nur nach E178 und E185. Dazwischen meldet sich ein Auftrag nur, wenn er eine Entscheidung braucht.
- Der **Leitstand** zeigt alle Aufträge: wo sie stehen, was sie kosten, worauf sie warten und welcher Agent wie oft im
  ersten Anlauf besteht.

Nicht im Umfang: Übernehmen ohne Abnahme (E164), Läufe auf fremden Rechnern oder in der Cloud der Hersteller (O4),
Telemetrie oder Export nach OpenTelemetry (E16; die GenAI-Konventionen von OpenTelemetry sind noch im Status
„Development“).

## Vorbilder

Recherche vom 2026-11-16, Quellen am Ende. Was keines der Vorbilder bietet und den Kern dieser Phase bildet: lokal,
herstellerübergreifend und mit einer Prüfung über die Herstellergrenze.

| Vorbild | Übernommen | Bewusst nicht übernommen |
|---|---|---|
| OpenAI Symphony (Spezifikation, Apr. 2026) | Zustandsautomat je Lauf, Workflow-Datei im Repo, Erkennung hängender Läufe, Wiederholung mit wachsender Wartezeit, Ende in einem Übergabezustand statt „Done“ | nur Codex; Steuerung über ein externes Board |
| OpenAI Harness Engineering (Feb. 2026) | Regeln als mechanische Prüfungen, Agent prüft Agent, wiederkehrende Aufräum-Aufträge | Verzicht auf menschliche Review |
| Anthropic Harness-Design (März 2026) | Planer → Builder → Prüfer, Abnahmekriterien vor dem Bau, harte Schwelle je Kriterium | — |
| Anthropic Managed Agents (Apr. 2026) | Sitzung als Ereignisprotokoll; Zugangsdaten nie in Reichweite des Modells (E186) | Hände in der Cloud |
| Anthropic C-Compiler (Feb. 2026) | ein Orakel vor jeder Parallelität; getrennte Pfadbereiche, weil sich 16 Agenten am selben Fehler gegenseitig überschrieben | Koordination nur über Git, ohne Orchestrator |
| StrongDM (Feb. 2026) | — | „kein Mensch liest Code“; verdeckte Szenarien (gestrichen, F52) |
| GitHub Agent HQ (Feb. 2026) | — | ein Auftrag an mehrere Hersteller (gestrichen); Läufe in der Cloud |
| Factory 2.0 (Juni 2026) | Routing nach Kontingent und Erfolgsquote | — |

## Ausgangslage im Code

Vorhanden und genutzt:
- **Thread-Modell und Sitzungen:**
  - Ein Thread-Modell für alle fünf Wege (`ide-thread`): Status `Idle`/`Running`/`RequiresAction`, Tokens, Kosten,
    `RateLimit` mit Auslastung und Neustart des Fensters, Subagenten und Hintergrundarbeit (E4, E147).
  - Die Sitzungen selbst sind fensterlos und haben Test-Agenten: `ide_claude::Session`, `ide_acp::Session`,
    `ide_codex::Session`, `ide_opencode::Session`, `ide_agent::Session` *(F40)*.
  - **Unterschiede je Weg** *(C60, C61)*: Wie vollständig Subagenten, Hintergrundarbeit und das Ende eines Zugs
    gemeldet werden, ist je Weg verschieden.
    - Codex und OpenCode zeigen Subagenten teils nur als Werkzeugkarten.
    - Gescheiterte und unterbrochene Züge enden dort wie erfolgreiche in `Idle`.
- **Git:**
  - Worktree-Threads mit Übernahme per Merge oder Cherry-Pick (`ide_ui_git::worktrees`, I1).
  - Checkpoints mit `delete_thread` und `prune`, Secrets-Prüfung (`ide-git`, I6, I4).
  - `Repository::commit` und Merges führen die Hooks des Repos ohne Zeitlimit aus *(C53)*.
  - gitleaks liest seine Konfiguration aus dem geprüften Ordner *(C52)*.
- **Tests und CI:** Test-Adapter (`ide-test`, E110); CI lesen, neu starten und Logs für Agenten zuschneiden (`ide-ci`,
  I8).
- **Freigaben und Sandbox:**
  - Freigabe- und Sandbox-Modi je Weg (`ide_claude::PermissionMode`, `ide_codex::SandboxMode`, `ide_acp::Sandbox`, E19).
  - Ein `sandbox-exec`-Profil im eigenen Agenten (`crates/ide-agent/src/workspace.rs`) *(F45)*.
- **Prozesse:** Supervisor mit Prioritäten je Art (`ide-process/src/priority.rs`), Aktivität (E23), Mitteilungen
  (I9).
- **Siegel:** das Prinzip aus `xtask/src/seal.rs`, ein Nachweis gebunden an den Git-Tree eines Stands.

Lücken:
- **Alles um die Sitzung herum hängt am AI-Tab** *(F40)*.
  - `Source` (`crates/ide-ui-agent/src/source.rs`), der Zustand mit Auswahl, Warteschlange, Checkpoint und Fork, die
    `Inbox`, die Freigabekarten und die Mitteilungen gehören dem `AgentTab`.
  - Gestartet wird je Weg in `tab_session.rs` (Claude), `acp.rs`, `codex.rs`, `opencode.rs` und `own.rs`; Freigaben
    liegen in `requests.rs` und `actions.rs`.
  - Jede Sitzung nimmt genau einen Empfänger für ihre Ereignisse.
  - `ide-ui-agent` hat 28 009 Zeilen; 24 ihrer Dateien greifen an 96 Stellen auf `Source` zu, 26 Dateien in `src/` auf
    `AgentTab` oder die Tab-Aufzählung.
  - Beenden-Frage, Aktivität, Fensterschluss und `working_in` kennen Sitzungen nur über die offenen Tabs *(F41)*.

  Diese Lücke sperrt alles andere (B37).
- **Orchestrator und Regeln:** kein Orchestrator, keine Workflow-Datei, keine Prüfstationen außer `make ci`, das nur
  Dangos eigenes Repo siegelt; kein Routing nach Rollen.
- **Worktrees** *(F49, F50)*:
  - Die Worktree-Logik liegt in der UI-Crate `ide-ui-git`.
  - `take_over` merged in den gerade ausgecheckten Branch.
  - `ide-git` kann nicht rebasen.
  - Das Git-Panel bietet jeden Worktree zur Übernahme an.
- **CI:** Schreibende Aufrufe für Merge Requests und Issues fehlen. `Item` hat weder Text noch Labels *(F64)*.
- **Energie:** Es gibt keine Energie-Zusicherung gegen den Ruhezustand.

## Reihenfolge

| Stufe | Inhalt | Größe | Warum an dieser Stelle |
|---|---|---|---|
| 1 Ein Auftrag | B37a als Sperre, dann B38, B39, B41a, B42, B43, B44a, B45a | XL | Der kürzeste Weg, der einen Auftrag von Hand über Bau und Gates bis zur Übernahme trägt, mit einem Builder. Der AI-Tab bleibt unverändert *(F40, F55)* |
| 2 Mehrere Hersteller | B37b, B40, B41b, B46 | L | Rollen lassen sich erst besetzen, wenn ein Auftrag allein zuverlässig durchläuft. Den Tab auf den Sitzungsbaustein umzustellen braucht erst das Eingreifen (E184) |
| 3 Fließband | B44b, B45b, Parallelität aus E168 | L | Parallelität lohnt erst mit Pfadbereichen und Merge-Queue; vorher überschreiben sich Aufträge (R64) |
| 4 Lernen | B47 | S | Braucht die Protokolle und Kennzahlen aus Stufe 1–3 |

Jede Stufe endet wie die Phasen in plan.md:
- `make check` grün
- `cargo xtask selftest` mit neuen Szenarien. Selftests lösen keine Mitteilung und keine Energie-Zusicherung aus
  (R12) *(F61)*.
- `make perf` (E25), dazu eine feste Messung mit einem laufenden Gate (R9, R68) *(F70)*
- VoiceOver-Prüfung der neuen Ansichten
- neue Texte in `en.lproj` und den 33 weiteren Sprachen nach [glossary.md](glossary.md) (E15); „Leitstand“, „Auftrag“,
  „Nachweis“, „Abnahme“ und die Rollennamen kommen zuerst ins Glossar
- HIG-Abnahme (E101)
- jede neue Crate mit README und Mermaid-Diagramm (E100, `tests/crate_readmes.rs`)

## Stand der Umsetzung

Umgesetzt auf dem Branch `phase-13-leitstand`, mit grünem `make check`: der Orchestrator-Kern `ide-factory` (B38,
B39, die Logik von B40, B41a, B41b, B44a, B45a/b, E175, E180, E183), die Git- und Prozess-Bausteine (B41a, B43, B45a)
und der Sitzungstreiber `ide-session` mit den Auftragsverträgen (B37a, B43). Was dabei anders wurde als oben
beschrieben, gilt so, wie es hier steht:

- **Ort der Worktree-Logik:** `ide_git::thread_trees`, die Varianten für Aufträge in `thread_trees::order`.
  - Die Übernahme eines Auftrags committet keine Reste mehr; sie verlangt den abgenommenen Commit und lehnt ab,
    solange die Basis Commits hat, die dem Auftrag fehlen. Unversionierte Dateien zählen beim Urteil „sauber“ nicht,
    weil git sie ohnehin nicht überschreibt.
  - Zeilen von Auftrags-Worktrees im Git-Panel bieten auch „Thread öffnen“ nicht an: Es öffnete einen normalen Thread,
    der unprotokolliert im Worktree des Auftrags schreiben könnte.
  - Der Ort der Worktrees ist noch nicht wählbar (E188); sie liegen im Datenordner der App.
- **Git ohne Hooks (E192):** `core.hooksPath=/dev/null` wie schon bei den Checkpoints, dazu `core.fsmonitor=false` und
  ein leerer `credential.helper`.
- **Netz (E173, E187):** `sandbox-exec` filtert nicht nach Domains. Gates und der eigene Agent haben das Netz ganz an oder
  ganz aus; eine Liste von Domains lehnen Codex und der eigene Agent ab. Nur Claude setzt die Liste über seinen
  Netz-Proxy durch. Lokales TCP ist in Gates aus, bis die Workflow-Datei es erlaubt.
- **Ein Gate entkommt,** wenn es eine eigene Prozessgruppe oder Sitzung beginnt; das Ende der Gruppe erreicht es dann
  nicht.
- **Codex (E173, A21):** Das stabile Schema des App-Servers hat keine Lese-Wurzeln; ein Codex-Builder oder -Prüfer kann
  außerhalb des Worktrees lesen. Abgeschaltet werden über `-c` Apps, Plugins, Browser, Computer Use, Bildgenerierung,
  Subagenten, Websuche und MCP-Server; ob `mcp_servers={}` vorhandene Server wirklich entfernt, ist offen. Das Schema
  der Antwort geht über `Session::send_with_schema` mit (A19).
- **Claude (E173):** geprüft gegen die Einstellungen von Claude Code 2.1.325.
  - Leser laufen in `dontAsk` ohne Schreibregeln, nicht in `plan`.
  - Beide Lesesperren sind gesetzt (`permissions.blockReadsOutsideWorkingDirectories` und
    `sandbox.filesystem.denyRead`), dazu `blockWritesOutsideWorkingDirectories`, `disableBypassPermissionsMode`,
    `--strict-mcp-config` und `--no-chrome`.
  - 27 Deny-Regeln für Git über den Worktree hinaus.
  - `--json-schema` gibt es nur je Start im Print-Modus, nicht je Zug (A19).
- **Begründete Ablehnungen** erreichen den Agenten nur bei Claude und in der Skript-Sitzung; die übrigen Wege lehnen ohne
  Text ab.
- **Ausnahme für Git im eigenen Worktree (E173, C47):** noch nicht umgesetzt. Der eigene Agent fragt weiter bei
  zerstörerischem Git, und die Regel des Auftrags lehnt dann ab.
- **Ende eines Zugs (E189):** Ein Ergebnis, das kommt, während Subagenten oder Nachrichten der Warteschlange noch laufen,
  wartet auf sie.
  - Endet der Prozess bei offenem Zug ohne Fehler oder Abbruch im Thread, ist das Ergebnis „Verbindung verloren“.
  - Bei OpenCode entscheiden `time.busy` und `time.idle` der Sitzung, ob der Zug noch läuft (C62).
- **Checkpoint:** Scheitert er in einem Auftrag, scheitert der Zug, ohne dass der Prompt hinausgeht.
- **Commits nach einem Zug** tragen `Dango-Operation: <id>` als Trailer; daran erkennt der Wiederanlauf, ob der Commit
  schon geschah (E167). Findet die Secrets-Prüfung etwas, wird nichts committet, und der Builder bekommt den Befund.
- **Budgets (E168):** werden zwischen den Zügen geprüft. Das Budget eines Auftrags aus dem Sheet ersetzt die Vorgabe.
  Die 70-%-Grenze gilt beim Beginn einer Station; Wiederholungen innerhalb einer Station hält nur ein erschöpftes
  Kontingent auf.
- **Urteile (E179):** Ein Urteil, das ein Kriterium doppelt behandelt, gilt wie ein unlesbares und geht an den Nutzer.
- **Agenten-Konfiguration (E172):** `CLAUDE.md` und `AGENTS.md` zählen in jeder Tiefe, dazu `CLAUDE.local.md`.
  Rust-Tests in Quelldateien zählen nicht als Teständerung, weil sie sich am Pfad nicht erkennen lassen.
- **Spätes G1:** In Stufe 1 gilt G1 immer als übersprungen; ein Diff, der hoch gemessen wird, wartet vor der Prüfung.
- **Laufzeit und Oberfläche (B37a, B38, B42, B44a, B45a in der App):** `src/leitstand/` führt je vertrautem Ordner
  eine Fabrik, schreibt das Protokoll vor jeder Wirkung und führt die Wirkungen auf Lanes aus. `ide-ui-factory` zeigt
  Panel, Leitstand-Tab, Auftrag-Tab, Sheet, Einstellungen „Aufträge“ und den Dialog „Leitstand für <Ordner>“.
  `src/leitstand_ui.rs` verbindet beide; Ergebnisse aus Worker-Threads und die Ansichten erreichen den App-Kontext über
  einen gebundenen `Back`, ohne nachzuschlagen (E143).
  - **Teilweise Abnahme (C55):** Die Abschnitte wählt der Nutzer mit Häkchen im Auftrag-Tab ab. „Diff zeigen“ öffnet die
    Zeile des Worktrees im Git-Panel, weil der Diff-Tab keinen Abnahme-Modus hat.
  - **Die Karte einer Anfrage** zeigt den Text der Frage nicht, weil das Modell ihn nicht trägt; sie bietet Antwort,
    „Erlauben“, „Ablehnen“ und „Sitzung öffnen“.
  - **„Weiter“** bestätigt bei geänderten Gate-Befehlen den aktuellen Satz (E190). Fehler der Workflow-Datei,
    unlesbare Protokolle und offene Lockerungen stehen über der Flottenzeile, Lockerungen mit „Bestätigen“.
  - **Einstellungen (E183):** Der Tab „Aufträge“ deckt Stufe 1 ab: Rangliste des Builders, Grenzen, Budgets je
    Auftrag und Tag, Fortsetzen nach dem Neustart, Batterie. Er steht neben „Agenten“; die späteren Tabs rücken um eins.
  - **Projekt-Dialog:** Gate-Befehle nur lesend mit „Workflow-Datei im Editor öffnen“; Risikomuster und prüfrelevante
    Dateien noch nicht im Dialog.
  - **Menüs:** „Ablage ▸ Neuer Auftrag …“, „Ablage ▸ Auftrag“ (mit „Auftrag abbrechen …“ und „Auftrag fortsetzen“,
    weil „Abbrechen“ und „Fortsetzen“ als Schlüssel schon feste Bedeutungen haben), „Darstellung ▸ Leitstand“.
  - **Mitteilungen** schickt nur die Laufzeit, aus der Wirkung der Fabrik; die Ansichten schicken keine.
  - **Ref-Wache:** Als von Dango gemacht gelten bisher nur Checkpoints und die Branches anderer Aufträge. Ein Commit
    des Nutzers im Git-Panel während eines Zugs lässt den Auftrag warten.
  - **Testzahlen der Gates** kommen aus den Zusammenfassungen von libtest, nextest, pytest, Jest und Vitest; andere
    Ausgaben zählen als „unbekannt“.
  - **Der Worktree** beginnt an der Spitze des Basis-Branches, nicht genau am gemerkten Commit.
  - **Das Einfrieren der Agenten-Konfiguration** geht höchstens 6 Ebenen tief und überspringt `node_modules`,
    `target` und Ähnliches.
  - **Eingefrorene Konfiguration je Weg (E172):**
    - Claude startet mit `--setting-sources=user`; die eingefrorene Ablage kommt über `--add-dir` mit
      `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` (CLAUDE.md, `.claude/rules/`). Von ihren Projekt-Einstellungen gelten
      nur Deny- und Ask-Regeln; Hooks, Allow-Regeln, MCP und `CLAUDE.local.md` werden nicht geladen.
    - Codex bekommt `project_doc_max_bytes=0`, beide Ordner als nicht vertraut, und die eingefrorene AGENTS.md als
      `developerInstructions`. Die eingefrorene `.codex/config.toml` gilt nicht, weil sie sich nur über `CODEX_HOME`
      umlenken ließe, das die Anmeldung hält.
    - Der eigene Agent liest seine Regeln aus der eingefrorenen Ablage.
  - **Fortsetzen (A15):** Claude startet unter einer vorab erzeugten `--session-id`, der eigene Agent setzt über seine
    Thread-Id fort.
  - **Stufe 2 (B40, B41b, B46):** Ein Ordner läuft in Stufe 2, solange ein bereiter Builder und ein bereiter Prüfer
    verschiedener Hersteller in den Ranglisten stehen; jeder Auftrag behält die Stufe, in der er entstand. Planer und
    Prüfer laufen als Leser; Codex bekommt das Schema des Urteils, Claude und der eigene Agent antworten mit einem
    JSON-Block. Der eigene Agent ist bereit, sobald ein Endpunkt mit Modell eingerichtet ist; sein Hersteller kommt vom
    Endpunkt. In den vorgegebenen Ranglisten steht er nicht.
  - **Git im eigenen Worktree (C47):** umgesetzt, wirkt aber erst mit O1. Claudes Ask-Regeln greifen nach Präfix, ein
    `git reset -q --hard` rutscht durch; Codex kann solche Befehle nie an den Nutzer geben.
  - **Pfad-Abhängigkeiten (E188):** Neben dem Worktree entsteht für jedes `../name` ein Symlink auf den Ordner neben
    dem Repository. Eine Abhängigkeit zwei Ebenen höher wird übersprungen, weil sie im gemeinsamen Worktree-Ordner läge.
  - **AI-Tab (B37b):** Alle AI-Tabs laufen über das gemeinsame Register (`Source::Session`): Claude, Codex, OpenCode,
    ACP, der eigene Agent und die Mitschnitte der Selbsttests. Die Sitzung eines Auftrags hängt der Tab als
    `Source::Driven` an (A14 bestätigt).
    - Was nur ein Weg kann, geht über `Driver::claude/codex/opencode/acp/own`; Freigaben mit jeder angebotenen Option
      über `Driver::answer_option`.
    - Codex tritt auf Wunsch dem geteilten Server bei (`UserChoices::shared`, E148, nie für Aufträge).
    - Der eigene Agent setzt eine außerhalb des Haupt-Threads gelesene Ablage fort (`AgentPath::OwnStored`).
    - Eine Lücke im OpenCode-Strom hält nur bei Aufträgen Prompts und Zug-Ende an; eine Sitzung des Nutzers liest danach
      nach.
    - Stoppen, während der Checkpoint noch angelegt wird, nimmt die Nachricht zurück in die Eingabe; ist ein Agent
      beschäftigt, versucht der Treiber die Nachricht erneut.
    - Der Treiber kann dafür jetzt Remote Control und Zurückspulen beim Start, Claudes eigene Fehlerarten und Remote
      Control im Feed, Freigaben mit bearbeiteter Eingabe und gespeicherter Regel (E27/E33 weiter ohne „Immer“ bei
      zerstörerischem Git), einen Checkpoint je Nachricht und Nachrichten in den laufenden Zug für den Nutzer, und
      Mitschnitte als eigene Art. Was nur Claude kann (Overlays nach E146, Nutzung, Zurückspulen von Code), geht über
      `Driver::claude()`.
    - Je Flush wird der Thread etwa zweimal statt einmal kopiert; der Zustand „Sicherungspunkt wird angelegt …“ endet
      bis zu 30 ms später; eine mitten im Zug gestoppte Sitzung schließt ihren offenen Checkpoint jetzt ab.
    - `agent-background` scheitert mit gesperrtem Bildschirm vor und nach dem Umzug gleich (Mitscrollen); mit
      entsperrtem Bildschirm noch zu prüfen.
    - Der Tab eines Auftrags trägt das Schild „Auftrag … · Rolle“. Modus, Sandbox und Denktiefe sind gesperrt;
      Nachrichten gehen in die Warteschlange des Treibers und stehen als Eingriff im Protokoll; Schließen beendet nur
      das Abonnement.
    - „Zurück an den Leitstand“ startet Claude unter neuer Sitzung; ein offener Tab zeigt dann die alte als beendet.
  - **G1 (E178):** Die Spezifikation erscheint mit Inline-Kommentaren im Auftrag-Tab, aus dem Text der Fabrik.
    „Verwerfen“ bricht den Auftrag nach dem Sheet ab.
  - **Bilder der Gates** kommen über die Umgebungsvariable `GATE_IMAGES_DIR`.
  - **Aufbewahrung:** Protokolle, Spezifikation, eingefrorene Konfiguration, Bestätigungen, Tageskonto und Kontingente
    liegen im Datenordner unter `leitstand/`; die Vorgaben des Nutzers in der Einstellung `factoryDefaults` (PerMac).
- **Stufe 3 (B44b ohne O2, B45b):**
  - **Eingang aus Issues (E176):** `ide-ci` liest Text und Labels eines Issues, nie Kommentare.
    - „Als Auftrag übernehmen“ im CI-Panel füllt das Sheet; Aufgabenlisten werden einzeln abwählbare Vorschläge.
    - G1 ist Pflicht und lässt sich auch im Sheet nicht abwählen; in Stufe 1 wird der Text des Issues die
      Spezifikation für G1.
    - Automatisch: alle 5 Minuten, nur mit bestätigter Lockerung (E190), nur erlaubte Autoren; ein Issue ohne Autor
      wird nicht genommen. Übernommene Issues stehen vor der Übergabe in `intake.json`, damit nach einem Absturz keines
      doppelt kommt.
  - **Ohne O2 (E177):** „Abnehmen und Branch behalten“ entfernt den Worktree und behält den Branch; „Merge Request
    anlegen“ öffnet die Seite der Forge im Browser-Tab. Über die API wird nichts geschrieben.
  - **Pfadbereiche (E174)** werden beim Beginn des Baus geprüft, nicht beim Eingang, weil der Bereich in Stufe 2 erst
    nach G1 feststeht. Ein Auftrag, der auf G2 wartet, hält keinen Bereich.
  - **Parallelität (E168):** Vorgabe 2, ein Ordner in Stufe 1 bleibt bei 1. Wer schon gespeicherte Einstellungen hat,
    behält 1. Eingereihte Aufträge nennen den Grund und zählen nicht als laufend.
  - **Merge-Queue:** Rejoint ein Auftrag nach seiner Neuprüfung die Schlange, weckt er deren Kopf; vorher blieben ab drei
    Aufträgen alle stehen.
- **Stufe 4 (B47):**
  - **Kennzahlen (E180):** Die Laufzeit rechnet sie auf einer Lane und zeigt sie eingeklappt unter der Flottenzeile.
    Aufträge mit Eingriff, Agenten- oder Modellwechsel oder Arbeit von Hand zählen in der Quote nicht.
  - **Gleichstand (E170):** Ein späterer Eintrag gewinnt nur, wenn er genauso bereit ist und eine echt bessere Quote für
    die Risikoklasse hat, beide auf mindestens 5 Aufträgen. Der Nachweis nennt die Entscheidung.
  - **Regelvorschläge:** nach einem gescheiterten, ausgeschöpften oder unklaren Auftrag, ab 5 geprüften Aufträgen und
    einem Befund, der in mindestens zwei wiederkehrt. Es gibt einen offenen Vorschlag zur Zeit; er ist ein eigener
    Auftrag, beschränkt auf AGENTS.md/CLAUDE.md, der in jeder Stufe an G1 wartet. Ein Schalter in Einstellungen ▸
    Aufträge, standardmäßig an.
  - **Wiederkehrende Aufträge:** `[[recurring]]` in der Workflow-Datei mit `title`, `goal`, `criteria` und `interval`
    („daily“, „weekly“ oder etwa „14d“). Sie brauchen eine Bestätigung je Projekt wie die anderen Lockerungen (E190),
    weil eine geteilte Datei sonst Agenten starten und Budget ausgeben könnte. Geprüft wird alle 5 Minuten, solange
    Dango läuft; nach einer Pause kommt höchstens ein verpasster Termin.
- **Selbsttests:** `leitstand`, `leitstand-order`, `leitstand-attach`, `leitstand-queue` und `leitstand-metrics` laufen
  grün, auch mit gesperrtem Bildschirm.
- **Lücken geschlossen:**
  - **Einwand des Builders (E178):** Der erste Prompt verlangt am Ende `CRITERIA CONFIRMED` oder `CRITERIA OBJECTION`
    mit Gründen. Ein Einwand lässt den Auftrag mit seinem Text warten: „Kriterien ändern …“ fragt den Builder neu,
    „Weiter“ überstimmt, „Auftrag abbrechen …“ beendet. Eine Antwort ohne eine der Zeilen gilt als bestätigt und wird
    als „fehlend“ protokolliert.
  - **Herkunft je Einstellung (E183):** Der Dialog „Leitstand für <Ordner>“ zeigt für jeden Wert „Vorgabe“ oder
    „factory.toml“ mit „Zurücksetzen“. Risikomuster und prüfrelevante Dateien sind dort bearbeitbar, die Ranglisten
    stehen weiter nur in der Datei. Der Tab „Aufträge“ deckt alle vier Rollen mit Modell und Denktiefe, die
    Prüfer-Regel und API-Budgets je Tag für Claude, Codex und den eigenen Agenten ab.
  - **Issue-Änderungen (E176):** Ein geändertes Issue zeigt im Auftrag-Tab „Issue geändert“ mit Zeilen-Diff; erst
    „Änderung übernehmen“ ändert Ziel und Vorschläge aus der Aufgabenliste, eigene Kriterien bleiben. Gelesen wird beim
    5-Minuten-Abruf (nur Ordner mit bestätigtem Eingang) und beim Öffnen des Auftrag-Tabs.
  - **Migration der Workflow-Datei:** Der Haken ist mit einem künstlichen Schritt getestet; mehr gibt es erst mit einer
    echten Version 2.
- **Selbsttests der Agentenwege:** `codex`, `opencode`, `acp`, `acp-actions`, `acp-dialogs`, `own-agent`, `agent`,
  `agent-actions`, `agent-dialogs`, `agent-perf`, `quit` und `leitstand-gaps` laufen grün.
- **A17** ist geprüft (siehe Annahmen).
- **Noch nicht umgesetzt:**
  - der Eingang aus Issues im Leitstand selbst (nur über das CI-Panel);
  - E177 mit Schreibzugriff (O2 entschieden: auf Knopfdruck; in Arbeit);
  - der Modus „Auftrag“ ohne Rückfragen (O1 entschieden: nach dem Testlauf der Gruppe `factory`).

## Stufe 1 — Ein Auftrag

### B37a Sitzungstreiber ohne Tab (Sperre)

- **E165 Sitzungen gehören einem Baustein ohne Fenster** *(F40, F41)*.
  - **Treiber:** Neue Crate `ide-session` ist ein Treiber über den vorhandenen Sitzungstypen. Er startet, sendet,
    beantwortet Freigaben nach einer übergebenen Regel (im Auftrag E173), legt vor jedem Prompt den Checkpoint an (I6,
    E34) und hält Schnappschuss und Warteschlange.
  - **Checkpoints:** Der Name des Checkpoint-Threads ist Auftrags-Id und Rolle *(F67)*.
  - **Verteiler** *(C71)*: Er gibt die Ereignisse einer Sitzung an beliebig viele Ansichten weiter; wer später
    dazukommt, bekommt zuerst den Schnappschuss.
    - Kontrollereignisse gehen vollständig und in Reihenfolge weiter.
    - Invalidierungen fasst er je Ansicht zusammen wie heute die `Inbox`; bei Rückstau ersetzt ein Abgleich mit dem
      Schnappschuss die aufgelaufenen Ereignisse.
    - Das Protokoll schreibt der Orchestrator, nie eine Ansicht. Ansichten melden sich ab, wenn sie schließen.
  - **Abschluss eines Zugs** *(C60)*: Jeder Zug endet mit einem typisierten Abschluss aus Turn-Id und Ergebnis:
    erfolgreich, gescheitert, vom Nutzer abgebrochen, Verbindung verloren. `Idle` allein unterscheidet das nicht.
  - **Lücken im Ereignisstrom** *(C62)*:
    - Reißt der Strom eines Wegs ab, meldet der Treiber die Lücke; der SSE-Strom von OpenCode etwa ist flüchtig.
    - Vor dem nächsten Schritt gleicht er Nachrichten, offene Anfragen und Verbrauch mit dem Server ab.
    - Gelingt das nicht, pausiert der Auftrag.
  - **Threading:** Der Treiber lebt auf dem Haupt-Thread wie heute der Tab; die Ereignisse kommen wie heute von den
    Threads der Sitzungen. Den Sprung auf den Haupt-Thread reicht die App als Schnittstelle hinein, die Crate kennt
    kein AppKit (E142).
  - **Register:** Laufende Sitzungen stehen mit einer `SessionId` im App-Kontext (E97); Antworten aus Worker-Threads
    tragen die Id (E143).
  - **Wer Sitzungen kennt:** Beenden-Frage (`quit::running`), Aktivität, Fensterschluss und `working_in` lesen das
    Register statt der offenen Tabs. Bis B37b führen sie beide Quellen zusammen, Tabs und Register, ohne doppelte
    Einträge; sonst fielen normale AI-Tabs heraus *(C58)*.
  - **Skript-Sitzung:** Für Selftests und Tests bekommt der Treiber eine Sitzung (Feature `selftest`), die je Prompt
    Ereignisse abspielt und Dateien im Worktree ändert. `Replay` gibt es nur für Claude und schreibt keine Dateien
    *(F61)*.
- **In Stufe 1** nutzt nur der Orchestrator den Treiber. Der AI-Tab bleibt, wie er ist, und zeigt eine
  Auftrags-Sitzung nur lesend aus dem Schnappschuss. Die Umstellung des Tabs folgt in B37b.
- **Abnahme:**
  - Ein Selftest startet einen Auftrag mit einer Skript-Sitzung.
  - Er öffnet die Sitzung mitten im Lauf lesend und schließt die Ansicht, ohne dass die Sitzung endet.
  - Der Selftest `quit` läuft mit einem laufenden Auftrag und zugleich einem normalen AI-Tab (C58).
  - Tests ohne Live-CLI:
    - eine langsame Ansicht bei hoher Ereignisrate (C71);
    - ein Mitschnitt, in dem bei OpenCode genau das Abschlussereignis verloren geht (C62);
    - je ein Abschluss jeder Art (C60).

### B38 Orchestrator

- **E160 Zwei neue Crates.**
  - `ide-factory` ohne Fenster: Aufträge, Zustandsautomat, Protokoll, Grenzen, Routing, Prüfstationen und was Panel
    und Leitstand-Tab zeigen (E98).
  - `ide-ui-factory` für Panel, Leitstand-Tab und Auftrag-Tab. Sie erreicht die App über ihren `Host` und Fenster über
    `WindowHost` (E99). Kontext oder Fenster-Id bekommt sie übergeben.
  - **Ratchets** *(F67)*: Sie stehen heute bei 404 (`APP_LOOKUPS`) und 481 (`STATE_LOOKUPS`). Die Zahl der
    `state()`-Zugriffe, die AppKit-Rückrufe der neuen Crate brauchen, wird vor dem Bau genannt und im selben Zug
    anderswo eingespart.
- **E166 Zustandsautomat je Auftrag.**
  - **Stationen:** Eingang → Spezifikation → Bau → Prüfung → Abnahme → Übernahme → Erledigt.
  - **Nebenzustände** je Station: läuft, wartet auf den Nutzer, pausiert (Budget, Kontingent, Ruhezustand, App
    beendet), gescheitert, abgebrochen; jeweils mit Grund.
  - **Lauf einer Station** in Phasen wie bei Symphony: Worktree vorbereiten (E188), Prompt bauen, Agent starten,
    Sitzung öffnen, Zug, Abschluss. Ergebnis: bestanden, gescheitert, Zeit überschritten, hängt, abgebrochen.
  - **Nur der Orchestrator setzt Zustände.** Ein Agent kann eine Station nicht für fertig erklären; fertig ist sie,
    wenn ihr Gate besteht.
  - **Besitz** *(F41)*: Aufträge gehören dem Ordner und stehen im App-Kontext, nicht im Fenster. Schließen des letzten
    Fensters eines Ordners und Ordner ersetzen (E56) fragen bei laufenden Aufträgen wie das Beenden; ohne
    Ordnerfenster pausieren sie.
- **E189 Ende eines Zugs, Rückfragen, Prozesse** *(F58)*.
  - **Zug-Ende** *(C60, C61)*:
    - **Wann:** Ein Zug ist zu Ende, wenn sein typisierter Abschluss da ist (E165) und kein Subagent und kein
      Hintergrundeintrag mehr läuft. Was danach noch läuft, wird vor dem Gate gestoppt und protokolliert.
    - **Wohin:** Nur „erfolgreich“ führt zum Gate. Gescheitert und Verbindung verloren gehen in die Wiederholung nach
      E168, ein Abbruch durch den Nutzer lässt den Auftrag warten.
    - **Fehlende Meldungen:** Meldet ein Weg Subagenten oder Hintergrundarbeit nicht verlässlich (Tabelle in E173),
      sind sie in Auftrags-Sitzungen abgeschaltet. Eine leere Liste gilt nie als Beleg, dass nichts mehr läuft.
  - **Rückfragen** mit Freitext lassen den Auftrag auf den Nutzer warten (roter Punkt). Plan-Anfragen lehnt der Modus
    „Auftrag“ ab.
  - **Prozesse:** Nach dem Ende einer Station endet ihr Prozess. Bei einer Rückschleife setzt die Station ihre Sitzung
    fort (A15).
- **E167 Ereignisprotokoll** *(F42, F60, F68)*.
  - **Ablage:** JSON-Zeilen je Auftrag, nur Anhängen, mit Versionsnummer. Das Protokoll liegt im Datenordner der App
    neben dem Worktree, nie im Repository; es verlässt den Mac nicht (E16). Eine abgeschnittene letzte Zeile wird
    verworfen.
  - **Nebenwirkungen** *(C66)*:
    - Jede Nebenwirkung steht als Paar „begonnen/beendet“ im Protokoll: Worktree anlegen, Branch, Commit, Merge, Push,
      Merge Request, Kommentar, Entfernen.
    - „Begonnen“ trägt eine Operations-Id, den erwarteten Vorzustand und einen nachprüfbaren Zielzustand, etwa den
      Commit mit der Operations-Id in seinen Metadaten oder den Merge Request mit Branch und Marke im Text.
    - Fehlt „beendet“, prüft der Wiederanlauf den Zielzustand, bevor er wiederholt; so wirkt nichts doppelt.
    - Kann das Protokoll nicht geschrieben werden, beginnt keine Nebenwirkung.
  - **Abgleich:** Beim Start gleicht der Orchestrator das Protokoll mit der Wirklichkeit ab: Worktree da, Branch da,
    offener Merge über `ide_git::Operation`.
  - **Beenden:** Vor `Supervisor::shutdown` schreibt er für jeden laufenden Auftrag „pausiert (App beendet)“. Das
    Beenden tötet Builder und Gates; ohne diesen Eintrag zählte es als Fehlschlag.
  - **Nach dem Neustart** bleiben Aufträge pausiert, bis der Nutzer „Fortsetzen“ wählt, einzeln oder für alle. Ein
    Fortsetzen ist ein neuer Prompt, und E64 schickt nach einem Neustart nichts. Automatisches Fortsetzen gibt es nur
    als Einstellung, standardmäßig aus.
  - **Abgeschlossene Sitzungen** zeigt der Auftrag-Tab aus Protokoll und Thread-Ablage, auch nachdem der Worktree
    entfernt ist.
  - **Aufbewahrung** *(F77, C70)*: Protokolle und Nachweise bleiben 90 Tage nach dem Abschluss. Die Checkpoints eines
    Auftrags entfernt `Checkpoints::delete_thread` vor dem Worktree, weil die Instanz im Worktree arbeitet. Ist der
    Worktree schon fort, etwa nach einem Absturz dazwischen, räumt der Orchestrator über das Haupt-Repository auf.
- **E168 Grenzen** (Vorgaben, über E161 änderbar).
  - **Hängende Züge** *(F57)*: Ein Zug hängt, wenn der Status `Running` ist, 5 min kein Ereignis kam und weder ein
    Werkzeugaufruf noch ein Subagent noch ein Hintergrundeintrag läuft. Für einen laufenden Werkzeugaufruf gilt eine
    eigene Frist von 30 min. Ein hängender Zug wird gestoppt; höchstens 2 neue Versuche, dann ist die Station
    gescheitert.
  - **Uhr:** Gemessen wird mit einer monotonen Uhr. Einschlafen und Aufwachen kommen über die
    `NSWorkspace`-Mitteilungen ins Protokoll. Fehler in den ersten 2 min nach dem Aufwachen zählen nicht als Versuch,
    weil dann Verbindungen der Agenten abgerissen sind.
  - **Zug und Station:** Ein Zug dauert höchstens 60 min, eine Station höchstens 20 Züge, eine Prüfung höchstens 3
    Runden *(F74)*.
  - **Gate- und Einrichtungsbefehle** *(C67)*:
    - eigene Frist, Vorgabe 60 min, und eine Ausgabegrenze;
    - bei Ablauf endet die ganze Prozessgruppe, und der Orchestrator wartet, bis der Supervisor das Ende bestätigt;
    - ein hängendes Gate blockiert so weder die Fabrik noch das Wachhalten.
  - **Wiederholung** nach einem Fehler nach 10 s · 2^(n−1), höchstens 5 min Wartezeit, höchstens 3 Versuche.
  - **Budgets in drei Einheiten je Weg** *(F59)*:
    - **Dollar:** wo der Weg Kosten meldet (Claude mit API-Schlüssel, E57) oder ein Preis eingetragen ist (E37).
    - **Tokens:** wo die Zahl verlässlich ist; bei ACP-Agenten ist sie es nicht.
    - **Fensteranteil:** beim Abo. Vorgabe: Kein Auftrag beginnt eine Station über 70 % Auslastung (R11). Die Grenze
      reserviert nichts; eine laufende Station kann das Fenster weiter füllen *(C64)*.
    - Wege ohne verlässliche Zahl begrenzen nur Laufzeit und Züge; der Nachweis sagt das.
    - Das Tageskonto liegt im App-Zustand und gilt über alle Projekte.
  - **Wie hart der Halt ist** *(C64)*: Budgets wirken in drei Stufen.
    - Vorab-Grenzen, die ein Weg kennt, werden beim Start gesetzt.
    - Vor jedem Zug prüft der Orchestrator den Rest.
    - Meldet ein Weg den Verbrauch erst nach dem Zug, kann ein Zug das Budget überschreiten, höchstens um diesen einen
      Zug. Der Nachweis nennt die Überschreitung.
    - Ist ein Budget erreicht, pausiert der Auftrag mit einer Mitteilung (I9) und läuft nie still weiter.
  - **Kontingente** *(C65)*:
    - Der Orchestrator führt sie je Konto, Dienst und Fenster, mit dem Zeitpunkt der Beobachtung. Ein Wert, der älter
      ist als sein Fenster, gilt als unbekannt.
    - Der Hersteller des Modells (E171) und der Dienst, der abrechnet, sind getrennte Größen.
  - **Parallelität:** In Stufe 1 und 2 läuft höchstens 1 Auftrag. Ab Stufe 3 sind es 2 als Vorgabe, höchstens so viele
    wie eingestellt (R9, R68).
- **E169 Wachhalten** *(F57)*.
  - Solange ein Auftrag läuft, hält Dango den Leerlauf-Ruhezustand an, über eine Aktivität mit
    `NSActivityUserInitiated`, die auch App Nap abschaltet (A18). Ohne laufenden Auftrag gibt es keine.
  - Auf Batterie gilt das nur nach Einstellung.
  - Schläft der Mac trotzdem, etwa bei geschlossenem Deckel, pausieren die Aufträge; die Aktivität nennt den Grund.
- **Abnahme:**
  - Tabellentests des Automaten.
  - proptest über an beliebiger Stelle abgeschnittene Protokolle (E96).
  - Hängen, Wiederholung, Schlaf und Budget mit gestellter Uhr und der Skript-Sitzung.
  - git-Tests auf einem Temp-Repo, die den Orchestrator an jeder Grenze zwischen „begonnen“ und „beendet“ abbrechen
    und neu starten *(F60)*, ausdrücklich auch nach erfolgreicher Nebenwirkung vor „beendet“ (C66).
  - Gates, die still hängen, endlos ausgeben oder Kindprozesse mit offenen Pipes hinterlassen (C67).
  - Eine verspätete Verbrauchsmeldung, die das Budget deutlich überschreitet (C64).

### B39 Workflow-Datei

- **E161 `.dango/factory.toml`**, im Ordner aus `ide_product::PROJECT_DIR`.
  - **Eigene Datei neben `.dango/settings.json`** *(F71)*, weil sie Befehle und Vorlagen trägt, kommentiert und im Team
    geteilt wird. Sie hat eine eigene Versionsnummer und Migration; die von E26 gilt für JSON. Der TOML-Leser ist eine
    neue Abhängigkeit der App und kommt in die Acknowledgements.
  - **Inhalt:**
    - Gate-Befehle je Station, etwa `make check`. Ohne Angabe erkennt `ide-test` das Testwerkzeug (E110).
    - Umgebungsvariablen für Gates, etwa ein gemeinsamer Build-Cache *(F70)*.
    - Rollen mit Ranglisten aus Agent, Modell und Denktiefe (E162).
    - Risikoklassen über Pfadmuster und welche Gates je Klasse gelten; Pfadbereiche (E174).
    - Grenzen (E168), Netz-Freigaben (E173, E187), Dateien zum Mitkopieren und Einrichtungsbefehl (E188).
    - Prompt-Vorlagen als Markdown unter `.dango/factory/` mit Platzhaltern für Auftrag, Kriterien und Befunde.
  - **Vorgaben** für Rollen, Budgets und Grenzen stehen in den Nutzereinstellungen (E26, E183).
  - **Vertrauen:** Die Datei gilt nur in vertrauten Ordnern (E22), weil sie Befehle startet.
- **E190 Die Projektdatei verschärft nur** *(F48)*.
  - **Grenzen und Budgets:** Es gilt der strengere Wert aus Nutzer- und Projektwert.
  - **Lockerungen** gelten erst, wenn der Nutzer sie je Projekt auf diesem Mac bestätigt hat: G1-Verzicht, Eingang aus
    Issues, erlaubte Autoren, Prüfer-Regel „bevorzugt“. Die Bestätigung liegt im App-Zustand, nicht im Repository.
  - **Gate-Befehle:** Dango merkt sich den Hash des bestätigten Satzes. Ändert er sich nicht über den Leitstand-Dialog
    (etwa durch `git pull`), fragt Dango vor dem nächsten Gate einmal nach.
  - **Ungültige Datei:** Ist die Datei ungültig, beginnt keine neue Station. Der Leitstand nennt den Fehler in der
    Sprache des Nutzers (E141).
- **E172 Die Prüfregeln gehören dem Hauptstand** *(F47, F56)*.
  - **Herkunft:** Der Orchestrator liest Workflow-Datei, Vorlagen und Gate-Befehle aus der Arbeitskopie des Nutzers,
    nie aus dem Worktree eines Auftrags.
  - **Agenten- und Prüfwerkzeug-Konfiguration** *(C52, C57)*: Diese Dateien bilden eine feste Gruppe, die sich nicht
    abwählen lässt:
    - `CLAUDE.md`, `AGENTS.md`, `.claude/`, `.mcp.json`, `.codex/`;
    - `opencode.json`, `opencode.jsonc`, `.opencode/`;
    - die Projektregeln des eigenen Agenten, `.gitleaks.toml` und `.gitleaksignore`.

    Für sie gilt:
    - **Eingefroren:** Die wirksame Konfiguration wird je Auftrag aus dem Hauptstand eingefroren und außerhalb des
      Bereichs abgelegt, in den der Builder schreiben darf.
    - **Vor jedem Start und Fortsetzen** wird sie erneut durchgesetzt: bei Claude über die Einstellungsquellen, sonst
      indem der Weg auf die eingefrorene Ablage zeigt. Kopien im Worktree lädt keine Auftrags-Sitzung.
    - **Ändert der Diff eine davon**, ist das ein Arbeitsergebnis wie jedes andere, wirkt aber erst nach der Übernahme.
      Der Auftrag wartet vor der Prüfung auf den Nutzer.
  - **Weitere prüfrelevante Änderungen** *(C54)*:
    - **zählen:** jede Änderung an einer bestehenden Testdatei, auch reine Einfügungen wie `return;` oder `#[ignore]`;
      dazu Lint-, CI- und Build-Konfiguration und die Workflow-Datei;
    - **zählen nicht:** neue Testdateien. Eine feinere Ausnahme für neue Testfälle in bestehenden Dateien kommt erst
      mit einer Erkennung je Werkzeug;
    - **erkannt** am Diff-Status je Datei. Die Liste der Pfade steht in der Datei, mit einer Vorgabe je Werkzeug.
    - **Folge:** Solche Änderungen heben die gemessene Risikoklasse auf hoch (E163), und der Nachweis nennt sie zuerst
      (R69).
- **Abnahme:**
  - Parser-Tests mit Fehlertexten in der Sprache des Nutzers (E141).
  - ein Migrationstest.
  - Tabellentests für „verschärft nur“ und für prüfrelevante Änderungen: neue Testdatei, geänderter Test und ein
    bestehender Test, der nur durch eine eingefügte Zeile entwertet wird (C54).
  - Ein Test, in dem der Builder seine eigene Kopie der Agenten-Konfiguration ändert und die nächste Sitzung sie
    trotzdem nicht lädt (C57).

### B41a Prüfstationen und Nachweis

- **E163 Erst automatische Gates, dann der Prüfer-Agent.** Reihenfolge:
  1. Pfadbereich: Der Diff berührt nur erlaubte Pfade. Steht in Stufe 1 kein Pfadbereich im Auftrag, entfällt der
     Schritt, und das Protokoll sagt es *(F54)*.
  2. Secrets-Prüfung des Diffs (`Git::scan_text`, I4).
     - gitleaks läuft mit der eingefrorenen Konfiguration aus dem Hauptstand (`--config`). Ignore-Dateien und
       Allow-Kommentare, die der Auftrag hinzufügt, gelten nicht *(C52)*.
     - Fehlt gitleaks (`SecretScan::NotInstalled`), gilt das als „nicht geprüft“ und steht im Nachweis an erster Stelle
       *(F75)*.
  3. Gate-Befehle aus E161, nach E187.
     - Findet `ide-test` kein Werkzeug und nennt E161 keinen Befehl, beginnt keine Station ohne einen Prüfplan, den der
       Nutzer bestätigt hat.
     - Ein Gate, das null Tests ausführt oder nur übersprungene, ist gescheitert *(C68)*.
  4. Prüfer-Agent (B41b, ab Stufe 2).

  Der erste Fehlschlag stoppt die Reihe. Der Befund geht mit dem zugeschnittenen Log an den Builder, so wie I8 Logs
  zuschneidet.
- **Zwei Risikoklassen** *(F54)*.
  - **Erwartet:** aus der Wahl des Nutzers und dem Vorschlag des Planers.
  - **Gemessen:** aus dem Diff, nach den Pfadmustern und E172.
  - **Nachgeholtes G1:** Liegt „gemessen“ über der Klasse, für die G1 entfiel, oder verlässt der Diff den Pfadbereich,
    wartet der Auftrag vor der Prüfung auf den Nutzer.
- **E187 Gates in der Sandbox** *(F45)*.
  - **Warum:** Der Gate-Befehl kommt aus dem Hauptstand, führt aber aus, was im Worktree steht: `build.rs`, Tests,
    npm-Skripte.
  - **Profil:** Gates laufen deshalb immer im `sandbox-exec`-Profil.
    - Geschrieben werden darf nur in Worktree, Build-Ordner und ein eigenes Temp-Verzeichnis je Lauf.
    - Das Netz ist aus oder auf die Liste der Workflow-Datei beschränkt.
    - Das heutige Profil des eigenen Agenten lässt Lesen überall und alle Unix-Sockets zu. Für Gates kommen dazu: eine
      Lesesperre für das Home-Verzeichnis außer den Werkzeug-Ordnern (darunter `~/.ssh`, Schlüsselbund und Konfiguration
      anderer Programme) und gesperrte Unix-Sockets außer einer Liste in der Workflow-Datei *(C50)*.
  - **Umgebung:** die von E186.
  - **Ort des Profils:** Es zieht aus `ide-agent` nach `ide-process`, damit Orchestrator und eigener Agent dasselbe
    nutzen (A20).
  - **Prozesse:** Gates laufen über den Supervisor als neue Art `Kind::Gate`, sichtbar in der Aktivität und stoppbar,
    mit `AGENT_COMMAND_NICE` *(F70)*.
  - **Sauberer Tree:** Ist `git status` nach einem Gate nicht leer, ist das Gate gescheitert *(F53)*.
- **Commits und gebundener Nachweis** *(F43, F53)*.
  - **Commits:** Der Orchestrator committet nach jedem Zug, nach der Secrets-Prüfung. Rolle, Agent und Modell stehen im
    Protokoll; Agenten committen nicht selbst.
  - **Fingerabdruck:** Tree dieses Commits + Hash des Regelsatzes (E190) + Basis-Commit + ein Manifest der übrigen
    Eingaben *(C69)*.
    - Das Manifest nennt die mitkopierten Dateien (E188) mit Hash und den Stand von Pfad-Abhängigkeiten außerhalb des
      Repos; bei Dango sind das `../splat` und `../lowspeak` (R13).
    - Jede Änderung daran macht den Nachweis ungültig; übernommen wird nur mit gültigem Nachweis.
- **E192 Git-Aufrufe des Orchestrators** *(C53)*.
  - Commit, Merge und die Worktree-Befehle des Orchestrators laufen ohne die Hooks des Repos (`core.hooksPath` auf
    einen leeren Ordner), mit der Umgebung aus E186 und mit Frist.
  - Hooks liefen sonst nach jedem Zug außerhalb der Gate-Sandbox, und ein Builder könnte sie über `core.hooksPath` aus
    dem Worktree setzen.
  - Der heutige Weg mit Hooks bleibt den Commits des Nutzers.
  - **Herkunft:** Herkunft und Nachweis liegen nur im Protokoll, nie in Git-Refs, weil ein Builder Refs des
    Haupt-Repos ändern kann (E173).
- **Nachweispaket**, Abweichungen zuerst.
  - **Abweichungen:**
    - Secrets nicht geprüft;
    - geänderte Agenten-Konfiguration oder prüfrelevante Dateien (E172);
    - Prüfung ohne Herstellerwechsel (E162), Wechsel des Agenten oder Modells (E170, E184), Eingriffe (E184);
    - Wege ohne verlässliche Verbrauchszahl (E168).
  - **Inhalt:**
    - Auftrag und Kriterien, Diff nach Abschnitten;
    - jedes Gate mit Ergebnis, Dauer und Log-Ausschnitt, die Befunde des Prüfers (ab Stufe 2);
    - Bilder, die ein Gate in einen Ordner außerhalb des Worktrees schreibt, den E161 nennt, je Lauf in einem eigenen
      Unterordner (C69);
    - Kosten, Tokens oder Fensteranteil und Laufzeit je Station;
    - Herkunft je Zug.
- **Abnahme:**
  - Gate-Läufer mit Test-Befehlen.
  - Der Nachweis wird ungültig nach einer Änderung am Tree, am Regelsatz, an der Basis oder an einer Eingabe des
    Manifests.
  - Ein Gate, das eine Datei hinterlässt, scheitert; ebenso eines, das null Tests ausführt (C68).
  - Der Builder ergänzt eine Allowlist in `.gitleaks.toml`, und das Secret wird trotzdem gefunden (C52).
  - Ein Hook im Worktree läuft beim Commit des Orchestrators nicht (C53).
  - Ohne Agent-CLI: Ein Gate liest weder eine `.env` außerhalb des Worktrees noch `~/.ssh`, und es erreicht keinen
    lokalen Socket außerhalb der Liste (C50).

### B42 Leitstand

- **E175 Panel, Leitstand-Tab und Auftrag-Tab.**
  - **Panel „Leitstand“** in der Symbolspalte, über E80 ausblendbar *(F65)*.
    - Es steht vor CI, weil CI als letztes steht (E94), und hat das feste Kürzel ⌘9. Dieses Kürzel wird beim Bau gegen
      `conflicts()` geprüft.
    - Tooltip und VoiceOver-Text des Symbols nennen wartende Aufträge, etwa „Leitstand, 1 Auftrag wartet“ (wie E69).
    - Es listet die Aufträge des Ordners nach Station gruppiert: Titel, Station, Agent und Modell, Laufzeit.
    - Ein roter Punkt steht da, solange ein Auftrag auf den Nutzer wartet (wie E71). „+ Neuer Auftrag“ steht im
      Titel.
  - **Tab „Leitstand“** *(F66; Kanban vom Owner gestrichen)*:
    - **Stationsleiste** mit Zählern je Station: Eingang 1 › Spezifikation 1 › Bau 2 … Ein Klick filtert die
      Tabelle.
    - **Tabelle** darunter, eine sortierbare `NSTableView` mit Titel, Station, Agent, Prüfrunde, Verbrauch und „wartet
      seit“. Sortiert nach „wartet seit“ zeigt sie zuerst, was am längsten auf den Nutzer wartet.
    - **Flottenzeile** darüber:
      - laufend, wartet, gescheitert; Parallelität;
      - Verbrauch je Konto und Dienst gegen sein Fenster, aus den Kontingenten nach E168 mit dem Zeitpunkt der
        Beobachtung. Sie liegen im App-Zustand, auch wenn gerade keine Sitzung läuft *(F77, C65)*.
      - Tagesbudget, erledigte Aufträge je Tag, Wartezeit auf den Nutzer als Median;
      - der Platz, den die Worktrees belegen (R68).
  - **Tab je Auftrag:**
    - **Zeitleiste:** die Stationen aus dem Protokoll; je Station die Sitzung (in Stufe 1 lesend, ab B37b als AI-Tab),
      Diff, Gate-Logs und Befunde; darunter das Nachweispaket.
    - **Anfragen**, die auf den Nutzer warten, erscheinen dort als Karte und lassen sich ohne AI-Tab beantworten
      *(F72)*.
    - **Knöpfe:** Freigeben (G1); Abnehmen (G2, öffnet den Diff mit Annehmen/Verwerfen je Abschnitt, I6; bei hoher
      Risikoklasse erst nach dem Öffnen des Diffs); Zurück an den Builder mit Kommentar; Kriterien ändern (E184);
      Abbrechen (entfernt den Worktree, Sheet mit zerstörerischer Aktion).
    - **Teilweise Abnahme** *(C55)*: Verwirft der Nutzer bei G2 Abschnitte, entsteht ein neuer Stand.
      - Der Orchestrator committet ihn, und der Nachweis wird ungültig.
      - Gates und Prüfer laufen für diesen Stand erneut.
      - Danach schließt G2 für genau diesen Stand ab.
  - **AI-Panel** *(F68)*: Sitzungen von Aufträgen öffnen keine Tabs von selbst. Öffnet der Nutzer eine als Tab, steht
    sie wie jeder offene Tab im AI-Panel, mit dem Namen des Auftrags. E68 ändert sich nicht.
  - **Mitteilungen** kommen, wenn ein Auftrag wartet, gescheitert oder abnahmebereit ist; keine Dock-Zahl (E72).
    `notify::post` zieht dafür aus `ide-ui-agent` nach `ide-ui-common` *(F76)*.
  - **Wiederherstellen:** Leitstand-Tab und Auftrag-Tab sind eigene Layout-Arten, der Auftrag-Tab mit der
    Auftrags-Id, damit sie nach E64 wiederkommen *(F76)*.
  - **Menüs** *(F66)*: Es kommt kein dreizehntes Menü dazu.
    - „Ablage ▸ Neuer Auftrag …“ legt einen Auftrag an.
    - „Darstellung ▸ Leitstand“ zeigt den Leitstand.
    - Die Befehle des vorderen Auftrags stehen im Untermenü „Ablage ▸ Auftrag“: Freigeben, Abnehmen, Zurück an den
      Builder, Kriterien ändern, Von Hand weiter, Abbrechen, Leitstand-Einstellungen …
  - **Darstellung:** Zustände sind als Form und Text erkennbar, nicht nur über Farbe (E101).
- **Abnahme:**
  - Ein Selftest führt einen Auftrag mit der Skript-Sitzung durch alle Stationen.
  - Fensterfotos hell, dunkel und Rechts-nach-links; VoiceOver.
  - HIG-Abnahme der Menüs auf 14 Zoll in allen 34 Sprachen.

### B43 Modus „Auftrag“

Hängt an O1. Ohne diese Entscheidung gilt derselbe Vertrag, aber jede Anfrage außerhalb der Grenzen geht an den Nutzer
und wird ein roter Punkt *(C72)*.

- **E173 Ohne Rückfragen, in festen Grenzen** (Vorschlag) *(F43, F44)*.
  - **Ein Vertrag für alle Aufträge** *(C72)*: Es gibt einen Betriebsvertrag, mit oder ohne O1.
    - Ohne O1 gelten dieselben Grenzen, nur dass Anfragen außerhalb davon an den Nutzer gehen statt abgelehnt zu werden.
    - Die Sandbox ist in beiden Fällen Pflicht.
  - **Builder:** Er schreibt nur im Worktree und führt Shell-Befehle nur in der Sandbox aus; E19 ist für
    Auftrags-Sitzungen Pflicht. Er liest nichts außerhalb von Worktree und Build-Ordner, wo der Weg das sperren kann.
    Builder sind nur Wege mit Sandbox; eine Ausnahme mit Rückfragen gibt es nicht.
  - **Planer, Prüfer und Späher** arbeiten nur lesend, und zwar nur auf Wegen, die Schreiben technisch sperren
    *(C48)*.
    - Ein Tree, der nach dem Zug unverändert ist, belegt das nicht: Ignorierte Dateien, Pfade außerhalb und externe
      Dienste bleiben erreichbar.
    - Der Vergleich des Trees bleibt als zusätzliche Kontrolle.
  - **Ablehnen statt durchwinken:** Anfragen außerhalb der Grenzen werden abgelehnt (wie E21) und dem Agenten
    begründet.
  - **Git im eigenen Worktree** *(C47)*:
    - `reset --hard` und `clean -fd` im Worktree des Auftrags verwerfen nur dessen eigene Arbeit; der Checkpoint vor dem
      Zug holt sie zurück. Für sie gilt E27/E33 in Auftrags-Sitzungen nicht. Das ist eine ausdrückliche Ausnahme, die O1
      mit entscheidet.
    - Git-Befehle, die über den Worktree hinausreichen (fremde Refs, `push`, Stash und Branches des Haupt-Repos),
      verhindern Ablehn-Regeln des Wegs, wo er sie kennt; bei Claude sind es Deny-Regeln für die Bash-Befehle.
  - **Ref-Wache** *(C47)*: eine nachträgliche Erkennung, kein Schutz.
    - Nach jedem Zug vergleicht der Orchestrator die Refs des Haupt-Repos: Branches, `refs/ide-checkpoints/*`, im
      eigenen Repo `refs/ci-seals/*`.
    - Änderungen, die Dango selbst in der Zeit vorgenommen hat (Commits des Nutzers, andere Aufträge), ordnet er zu.
    - Jede andere Änderung, auch eine aus dem Terminal, lässt den Auftrag auf den Nutzer warten.
  - **E191 Werkzeuge je Rolle** *(C51)*.
    - MCP-Server, Apps und Connectoren, Browser- und Web-Werkzeuge sind in Auftrags-Sitzungen aus. Sie haben eigene
      Anmeldungen und gehen an der Netz-Sperre der Shell vorbei.
    - Ausnahmen gelten je Werkzeug und Rolle, mit eigener Bestätigung je Projekt (E190).
    - Ein Weg, der ein Werkzeug nicht abschalten kann, übernimmt keine Rolle. Das ist zu prüfen, bevor O1 entschieden
      wird.
  - **E193 Codex über einen eigenen App-Server** *(C46)*.
    - Auftrags-Sitzungen von Codex starten immer einen App-Server eigens für sich, nie den Proxy zum gemeinsamen Dienst
      aus E148; der Umzug nach E148 ist für sie gesperrt.
    - Grund: Der Dienst hat seine eigene Umgebung (E186 greift nicht), und das Ende des Proxys beendet keinen Zug.
    - Abnahme mit einem schon laufenden Dienst: Umgebung, Abbruch und App-Ende wirken.
  - **Claude, ausgeschrieben** (A21):
    - Modus `dontAsk` mit Regeln, die Bearbeiten im Worktree erlauben; so wird abgelehnt statt gefragt.
    - Sandbox mit `failIfUnavailable`, `allowUnsandboxedCommands: false` und einer Lesesperre außerhalb des Worktrees
      (`permissions.blockReadsOutsideWorkingDirectories` oder `filesystem.denyRead`).
    - `network.allowedDomains` aus der Workflow-Datei; der Start setzt `allow_bypass = false`. Heute setzt er nur
      `enabled` und `failIfUnavailable`, und `allow_bypass` immer.
    - Die Dateiwerkzeuge (Read, Edit, Write) begrenzen Regeln auf den Worktree-Pfad, weil Claudes Sandbox nur
      Shell-Befehle umfasst.
  - **Je Weg:**

    | Weg | Builder | Lesende Rollen | Fortsetzen (A15) | Kontingent | Strukturierte Antwort (A19) | Subagenten und Hintergrund (E189) |
    |---|---|---|---|---|---|---|
    | Claude | ja, mit der Einstellung oben | `plan` oder `dontAsk` ohne Schreibregeln, mit Sandbox | `--resume` | `rate_limit_event` | zu prüfen | gemeldet (`task_*`) |
    | Codex | ja, `workspace-write` mit `ApprovalPolicy::Never`, ausdrücklichen Schreib- und Lese-Wurzeln, Netz aus oder nach Liste; fehlt eine Fähigkeit in der erprobten Version, wird abgelehnt *(C59)* | `read-only` | Thread-Id | Nutzungslimits des App-Servers | `outputSchema` in `turn/start`; `Session::send` reicht es heute nicht durch *(C63)* | teils nur Werkzeugkarten: aus |
    | OpenCode | nein, keine Sandbox | nein: Agent `plan` darf Plan-Dateien schreiben, Rechte sind überschreibbar *(C48)* | — | nur OpenCode Go über seine Abfrage *(C65)* | — | — |
    | Antigravity (ACP) | nein, keine Sandbox | nein, nicht erzwingbar | — | — | — | — |
    | Eigener Agent | ja, eigenes `sandbox-exec`-Profil und Grenzen für Datei- und HTTP-Werkzeuge (E19) | Profil „Nur fragen“ (E6) | eigener Verlauf | Antwort der API | über die API des Anbieters, zu prüfen | keine |

    OpenCode und Antigravity übernehmen in Aufträgen keine Rolle, bis ein beschränktes Profil technisch durchgesetzt
    und geprüft ist *(C48, C72)*. Gemini wird nicht angeboten (E67). Der Herstellerwechsel beim Prüfen ist damit Claude
    ↔ Codex, oder der eigene Agent mit einem Modell eines dritten Herstellers, etwa Google über dessen API.
- **E186 Umgebung für Aufträge** *(F46)*.
  - **Variablen:** Auftrags-Sitzungen und Gates bekommen nicht die volle Login-Shell-Umgebung, sondern eine Positivliste
    daraus: PATH, Sprache, Werkzeug-Ordner. Ohne `SSH_AUTH_SOCK` und ohne Variablen nach den Mustern `*_TOKEN`, `*_KEY`,
    `*_SECRET`. Was ein Agent selbst zur Anmeldung braucht, setzt sein Weg gezielt.
  - **Push** *(C49)*:
    - Der Builder kann nicht selbst pushen, weil ihm Zugangsdaten und Netz zum Git-Host fehlen: kein `SSH_AUTH_SOCK`,
      ein für die Sitzung leerer `credential.helper` über `GIT_CONFIG_*`, Netz nach E173.
    - Eine Push-URL ins Leere hielte einen Push an eine ausdrücklich genannte URL nicht auf. Ohne
      `extensions.worktreeConfig` änderte sie zudem die Konfiguration des Nutzers; sie entfällt.
    - Gepusht wird nur nach E177.
  - **Netz** nach E173 und E187.
  - **Erreichbare Zugangsdaten** *(C50)*: Die Umgebung allein hält Zugangsdaten nicht vom Modell fern. Dafür sorgen
    erst die Lesesperren aus E173 und E187, die Sperre der Unix-Sockets (E187) und E188, das Dateien mit Zugangsdaten
    nicht mitkopiert.
- **Abnahme** *(F61)*:
  - Die Startparameter je Weg als Unit-Tests.
  - Die Wirkung in der bezahlten Gruppe `factory` (E53) mit Negativfällen:
    - Schreiben außerhalb des Worktrees;
    - `git push`, auch an eine ausdrücklich genannte URL (C49);
    - `git update-ref -d` auf eine fremde Ref;
    - Lesen von `~/.ssh` und einer `.env` des Hauptstands (C50);
    - Aufruf eines konfigurierten MCP-Werkzeugs (C51).
  - Ein Selftest mit einer abgelehnten Anfrage.

### B44a Neuer Auftrag von Hand

- **E185 Zwei Pflichtfelder, der Rest eingeklappt** *(F55, F73)*.
  - **Sheet „Neuer Auftrag“** (Panel, Menü „Ablage ▸ Neuer Auftrag …“):
    - Felder: Titel, Ziel (mehrzeilig), Kriterien (eine je Zeile).
    - In Stufe 1 sind die Kriterien Pflicht, weil es keinen Planer gibt. Ab Stufe 2 schreibt sie der Planer, wenn sie
      fehlen.
    - Hat die Arbeitskopie uncommittete Änderungen, sagt das Sheet, dass der Auftrag ohne sie bei HEAD beginnt
      *(F51)*.
  - **Optionen**, eingeklappt:
    - **Risikoklasse:** Automatisch, niedrig, mittel oder hoch. Das ist die erwartete Klasse; die gemessene kann sie nur
      anheben (E163).
    - **Pfadbereich** (optional).
    - **Budget:** Vorgabe oder ein eigener Wert. Später darf es nie unter das schon Verbrauchte fallen.
    - **Besetzung je Rolle** (ab Stufe 2): Vorgabe oder ein bestimmter Agent. Widerspricht sie der Regel „anderer
      Hersteller: Pflicht“, sperrt das Sheet den Start. Bei „bevorzugt“ zeigt es vor dem Start „ohne
      Herstellerwechsel“.
    - **Spezifikation ohne Freigabe:** nur bei Risikoklasse niedrig und nur, wenn das Projekt es erlaubt (E190).
  - **Nachträglich ändern:** Optionen bleiben im Auftrag-Tab änderbar, bis ihre Station beginnt; jede Änderung steht im
    Protokoll.
  - **Spezifikation in Stufe 1:** Die Spezifikation ist der Text des Nutzers. G1 entfällt, weil er sie selbst schrieb;
    das Protokoll führt die Station „Spezifikation“ als übersprungen.
- **Abnahme:** Selftest des Sheets mit Pflichtfeldern, gesperrter Besetzung und dem Hinweis auf uncommittete
  Änderungen.

### B45a Worktree und Übernahme

- **E188 Worktree vorbereiten** *(F51, F70)*.
  - **Warum:** Ein frischer Worktree enthält nur Versioniertes. Im eigenen Repo fehlen `CLAUDE.md` (ignoriert) und die
    Pfad-Abhängigkeiten `../splat` und `../lowspeak`. `make check` könnte dort nie grün werden.
  - **Mitkopieren:** eine Liste in der Workflow-Datei, kopiert per `clonefile` aus dem Hauptstand, etwa `node_modules`
    oder lokale Build-Konfiguration.
    - Dateien mit Zugangsdaten wie `.env` werden nur kopiert, wenn die Liste sie einzeln nennt und der Nutzer das je
      Projekt bestätigt hat (E190); der Nachweis vermerkt es *(C50)*.
    - Agenten-Konfiguration kommt nicht über diese Liste, sondern eingefroren nach E172.
  - **Einrichtungsbefehl** (optional), in der Gate-Sandbox (E187).
  - **Ort:** wählbar, entweder der Datenordner der App oder ein Geschwisterordner des Repos, damit `../x` auflöst.
    Worktree-Ordner sind vom Backup ausgenommen (`NSURLIsExcludedFromBackupKey`).
  - **Branch:** eigenes Präfix `order/…` statt `thread/…`.
- **E164 Zwei menschliche Gates, keine Übernahme ohne G2**, auch nicht bei niedriger Risikoklasse. G1 entfällt nur
  nach E178 und E185 *(F74)*. Gepusht wird nie automatisch.
- **Übernahme** *(F49)*.
  - **Ort der Logik:** Die Worktree-Logik (`create`, `list`, `take_over`, `remove`, `commit_pending`) zieht aus
    `ide-ui-git` nach `ide-git`, weil `ide-factory` keine UI-Crate einbinden darf (E99).
  - **Ziel:** Ziel ist der Basis-Branch des Auftrags, nicht der gerade ausgecheckte. Steht die Arbeitskopie auf einem
    anderen Branch oder ist sie unsauber, wartet die Übernahme und nennt den Grund.
  - **Mergen statt rebasen:** Vor der Übernahme wird das Ziel in den Auftrags-Branch gemerged; kein Commit ändert sich.
  - **Gültigkeit** *(C56)*:
    - Jede Änderung der Basis macht den Nachweis ungültig; Gates und Prüfer laufen neu.
    - G2 bleibt nur, wenn der Diff des Auftrags gegen die neue Basis byteweise gleich ist; der Nachweis nennt dann
      „Basis aktualisiert“. Eine Patch-Id reicht nicht, weil sie Leerraum übergeht.
    - Ist der Diff anders, auch ohne Konflikt, wird G2 wiederholt.
    - Bei Konflikten geht der Auftrag zurück an Builder, Prüfer und G2.
  - **Danach** werden Worktree, Build-Ordner und die Checkpoints des Auftrags entfernt; Protokoll und Nachweis bleiben
    (E167).
- **Git-Panel** *(F50)*: `ThreadTree` bekommt ein Feld für den Auftrag, dem er gehört. Solche Zeilen zeigen „Im
  Leitstand zeigen“ statt „Übernehmen …“, „Branch behalten“ und „Entfernen“.
- **Abnahme:**
  - git-Tests auf einem Temp-Repo: Nutzeränderung im Ziel-Branch während des Auftrags, Arbeitskopie auf einem anderen
    Branch, Konflikt nach dem Merge des Ziels.
  - Ein Selftest prüft das Git-Panel mit einem Auftrags-Worktree.

## Stufe 2 — Mehrere Hersteller

### B37b AI-Tab auf dem Sitzungstreiber

- **Umbau je Weg:** Der AI-Tab zieht je Weg auf `ide-session` um: Claude zuerst, dann Codex, OpenCode, ACP, eigener
  Agent. Jeder Schritt einzeln mit `make ci` (R67).
- **Danach** hängt sich der Tab an laufende Sitzungen. Schließt er, endet die Sitzung nur, wenn kein Auftrag sie hält.
- **E184 Mitschreiben erlaubt, aber sichtbar** *(F72)*.
  - **Kennzeichnung:** Der AI-Tab einer Auftrags-Sitzung trägt oben ein Schild, etwa „Auftrag CSV-Export · Builder“.
    Modus- und Sandbox-Pille sind gesperrt (E173).
  - **Nachrichten:** Was der Nutzer schreibt, geht in die Warteschlange, die jetzt im Treiber liegt.
    - Ein Eingriff verlängert die Station; das Gate beginnt erst bei leerer Schlange.
    - Er steht als „Eingriff“ im Protokoll und unter den Abweichungen im Nachweis.
    - Der Prüfer bekommt ihn nicht zu sehen und urteilt nur gegen die Kriterien (E179).
  - **Modellwechsel:**
    - Er ist erlaubt, wird als „Modellwechsel“ protokolliert und ist kein Agentenwechsel nach E170.
    - Der Prüfer muss von einem anderen Hersteller sein als alle, die gebaut haben.
  - **Kriterien ändern:** ein eigener Knopf im Auftrag-Tab, kein Chat. Die Änderung macht den Nachweis ungültig und
    geht an Builder und Prüfer.
  - **Von Hand weiter:** hält den Orchestrator für die Station an; der Nutzer arbeitet wie in einem normalen Thread.
  - **Zurück an den Leitstand:**
    - Modus und Sandbox werden wiederhergestellt; bei Claude heißt das neu starten und fortsetzen.
    - Der Nachweis wird ungültig, und die Station endet über ihr Gate.
- **Abnahme:**
  - Die vorhandenen Selftests und Mitschnitt-Tests des AI-Tabs bleiben grün.
  - Neu ist ein Selftest, der einen Auftrag mitten im Lauf als Tab öffnet, eine Nachricht schickt und den Tab schließt,
    ohne dass die Sitzung endet.

### B40 Rollen und Routing

- **E162 Rollen statt Agenten.** Jede Rolle hat eine Rangliste aus Agent, Modell und Denktiefe:
  - Planer: schreibt die Spezifikation, nur lesend;
  - Builder: schreibt im Worktree;
  - Prüfer: urteilt, nur lesend;
  - Späher: liest und fasst zusammen, günstig oder lokal, etwa der eigene Agent mit Ollama.

  Der Prüfer stammt standardmäßig von einem anderen Hersteller als alle Builder desselben Auftrags. Ist keiner bereit,
  prüft derselbe Hersteller, und der Nachweis sagt „ohne Herstellerwechsel“.
- **E171 Hersteller nach Modell, nicht nach Programm** *(F69)*.
  - Claude zählt als Anthropic, Codex als OpenAI, Antigravity als Google.
  - Bei OpenCode und dem eigenen Agenten zählt der Anbieter des gewählten Modells. Eine Tabelle ordnet Präfixe der
    Modell-Ids einem Hersteller zu; was sie nicht kennt, gilt als „unbekannt“ und nie als Herstellerwechsel.
- **E170 Routing** *(F69)*. Gewählt wird der erste Eintrag der Rangliste, der
  1. bereit ist: installiert (E58), und ein Anmelde- oder Startfehler zählt als „nicht bereit“ und führt mit Vermerk
     zum nächsten Eintrag;
  2. Kontingent hat: nach dem Kontingent seines Kontos und Dienstes und der 70-%-Grenze aus E168 *(C65)*; ohne Meldung
     oder mit einem veralteten Wert gilt es als unbekannt und wird im Nachweis so genannt;
  3. im Budget liegt (E168).

  Ist das Kontingent mitten in einer Station erschöpft, übernimmt der nächste Eintrag die Station ab dem Stand des
  Worktrees. Er bekommt Auftrag, Kriterien und eine Zusammenfassung des Protokolls; der Nachweis vermerkt den Wechsel.
  Ab Stufe 4 entscheidet bei Gleichstand die Erstdurchlaufquote (E180).
- **Abnahme:** Tabellentests mit Kontingent, Budget, Anmeldefehler und dem Ausfall eines Wegs (R65).

### B41b Prüfer und Rückschleife

- **E179 Strukturiertes Urteil** *(F64)*.
  - **Eingabe:** Der Prüfer bekommt Auftrag, Kriterien, Diff und Gate-Ergebnisse, nicht das Gespräch des Builders. Diff,
    Commit-Nachrichten und Gate-Logs sind im Prompt als Fremdtext gekennzeichnet.
  - **Antwort:** je Kriterium erfüllt, nicht erfüllt oder unklar, jeweils mit Begründung, und Befunde aus Datei, Zeile
    und Schwere (A19).
    - Lässt sich die Antwort nicht lesen, zählt die Runde als „unklar“.
    - Kriterien tragen feste Ids. Ein Urteil muss jedes Kriterium genau einmal behandeln; fehlt eines, ist es „unklar“,
      doppelte machen die Runde ungültig *(C63)*.
  - **Codex:** Bei Codex geht das Schema als `outputSchema` in `turn/start`. Dafür bekommt `ide_codex::Session` einen
    eigenen Sende-Aufruf, ein Schritt dieses Bausteins *(C63)*.
  - **Harte Schwelle:** Ein „nicht erfüllt“ oder ein Befund der Schwere hoch geht mit den Befunden zurück an den
    Builder; „unklar“ geht an den Nutzer.
  - **Runden:** höchstens 3 (E168). Danach wartet der Auftrag auf den Nutzer, mit allen Runden im Nachweis.
- **Abnahme:** eine Skript-Sitzung mit zwei Runden und einer Eskalation.

### B46 Planer und G1

- **E178 Spezifikation vor dem Bau** *(F63)*.
  - **Inhalt:** Der Planer schreibt Ziel, prüfbare Abnahmekriterien, Testplan, Pfadbereich und eine vorgeschlagene
    Risikoklasse.
  - **Ablage:** Die Spezifikation ist eine Markdown-Datei im Auftragsordner. Die heutige Plan-Ansicht hängt an
    `AgentTab` und an Claudes Plan-Anfrage (`answer_plan`); Codex und OpenCode stellen keine.
  - **Ansicht:** Die Dokumentansicht mit Inline-Kommentaren aus `plan/doc.rs` wird so gelöst, dass sie eine Datei ohne
    Sitzung zeigt. Eigener Schritt, Größe M.
  - **G1:** Freigeben, Kommentieren (die Kommentare gehen als Text an den Planer) oder Verwerfen.
    - G1 darf bei niedriger Risikoklasse entfallen, wenn das Projekt es erlaubt (E190).
    - Für Aufträge aus Issues ist G1 Pflicht (E176).
  - **Gemeinsame Kriterien:** Builder und Prüfer bekommen dieselbe Kriterien-Datei. Der Builder bestätigt sie im ersten
    Zug; Einwände gehen an den Nutzer (der „Sprint-Vertrag“ nach Anthropic).
- **Abnahme:** Selftest mit einer Spezifikation aus der Skript-Sitzung, einem Kommentar und der Freigabe.

## Stufe 3 — Fließband

### B44b Eingang aus Issues und Rückmeldung

- **E176 Eingang aus Issues** *(F64, F73)*.
  - **Wege:** über das Kontextmenü der Issue-Liste im CI-Panel (E77) und im Leitstand.
  - **Automatisch** nur für Issues mit einem in E161 genannten Label, nur von erlaubten Autoren, nur nach Bestätigung
    je Projekt (E190, R63).
  - **Lesen:** `ide-ci` braucht neue Lese-Aufrufe für Text und Labels.
  - **Einfrieren:** Beim Eingang werden Text und Hash eingefroren. Kommentare werden nie mitgenommen, spätere
    Änderungen am Issue nur auf Klick.
  - **Fremdtext:** Der Issue-Text geht als Fremdtext gekennzeichnet in den Prompt (E22, I8). G1 ist Pflicht.
  - **Vorbefüllung:** Das Sheet übernimmt Titel und Text. Aufgabenlisten des Issues (`- [ ]`) erscheinen als „Vorschlag
    aus dem Issue“, einzeln abwählbar; der Planer formuliert sie prüfbar.
- **E177 Rückmeldung an GitHub/GitLab.** Hängt an O2.
  - **Ablauf:** Nach G2 steht neben „Übernehmen“ (B45a) „Als Merge Request“ bereit: Branch pushen, MR oder PR mit einer
    Zusammenfassung des Nachweises anlegen, das Issue kommentieren.
  - **Nur auf Knopfdruck.** Vorher prüft die Secrets-Prüfung den ausgehenden Text (I4, E88). Fehlt gitleaks, ist das
    hier ein Fehlschlag *(F75)*.
  - **Rechte:** `ide-ci` braucht schreibende Aufrufe, bei GitLab mit dem Bereich `api` (E77), bei GitHub mit
    Schreibrecht für Pull Requests und Issues.
  - **Ohne O2** öffnet „Merge Request anlegen“ den Browser wie heute (E77).

### B45b Pfadbereiche und Merge-Queue

- **E174 Getrennte Pfadbereiche.**
  - Jeder Auftrag hat einen Pfadbereich; der Planer schlägt ihn vor, G1 bestätigt ihn.
  - Aufträge mit überlappendem Bereich laufen nacheinander.
  - Übernahmen in denselben Ziel-Branch stehen in einer Schlange, eine zur Zeit, jede nach E164 und B45a erneut
    geprüft.
- Erst damit gibt E168 die Parallelität frei.
- **Abnahme:** Zwei Aufträge mit überlappendem Bereich laufen nacheinander; eine Übernahme wird nach einem Konflikt
  erneut geprüft.

## Stufe 4 — Lernen

### B47 Kennzahlen und Lernschleife

- **E180 Kennzahlen aus den Protokollen** *(F77)*.
  - **Werte:** Erstdurchlaufquote, Prüfrunden, Kosten und Laufzeit je Rolle, Agent, Modell und Risikoklasse, dazu die
    Wartezeit auf den Nutzer.
  - **Ausnahmen:** Aufträge mit Eingriffen (E184) und mit Agentenwechsel zählen in der Quote nicht mit; sonst misst sie
    den Nutzer oder lernt das Routing Falsches.
  - **Anzeige und Nutzung:** Sie stehen im Leitstand-Tab; das Routing nutzt die Quote bei Gleichstand (E170).
- **Regelvorschläge.** Nach einem eskalierten oder gescheiterten Auftrag fasst ein Späher wiederkehrende Befunde
  zusammen. Er schlägt eine Änderung an AGENTS.md oder CLAUDE.md vor, als eigener Auftrag mit G2. Erst wenn Kennzahlen
  aus echten Läufen vorliegen.
- **Aufräum-Aufträge.** Wiederkehrende Aufträge stehen in der Workflow-Datei, etwa wöchentlich. Sie laufen nur, solange
  Dango läuft; es gibt keinen Hintergrunddienst. Von verpassten Terminen wird höchstens einer nachgeholt.

## Einstellungen

- **E183 Vorgaben in den Einstellungen, Projektwerte im Leitstand** *(F69, F71)*.
  - **Einstellungen ▸ Aufträge**, neben „Agenten“: die Vorgaben des Nutzers für alle Projekte.
    - **Rollen:** je Rolle eine sortierbare Liste aus Agent, Modell und Denktiefe.
      - Angeboten werden installierte Agenten (E58).
      - Die Modelle kommen aus dem zuletzt gesehenen Handschlag des Agenten; ohne einen gibt es Freitext und „Vorgabe
        des Agenten“.
      - Ein Eintrag, der nicht mehr bereit ist, bleibt ausgegraut mit Grund.
    - **Prüfer-Regel:** anderer Hersteller Pflicht oder bevorzugt.
    - **Grenzen** nach E168, **Budgets** je Auftrag, Tag und Anbieter in den Einheiten aus E168.
    - **Abgleich:** Alle neuen Schlüssel werden nicht abgeglichen (`PerMac`, E113) *(F67)*.
  - **Projektwerte:** „Ablage ▸ Auftrag ▸ Leitstand-Einstellungen …“ und ein Zahnrad im Kopf des Leitstand-Tabs öffnen
    „Leitstand für <Ordner>“.
    - Dort stehen dieselben Felder und das, was nur das Projekt betrifft: Gate-Befehle, Risikoklassen mit Pfadmustern,
      prüfrelevante Dateien, Mitkopieren und Einrichtung (E188), Prompt-Vorlagen mit „Im Editor öffnen“.
    - Jeder Wert zeigt seine Herkunft („Vorgabe“ oder „factory.toml“) und lässt sich zurücksetzen.
    - Lockerungen gelten erst nach Bestätigung (E190).
  - **Speichern:** Geschrieben wird `.dango/factory.toml`, mit erhaltenen Kommentaren und erhaltener Reihenfolge.
    - Die Änderung erscheint im Git-Panel, weil das Team die Datei teilt. Ist `.dango/factory.toml` ignoriert, sagt
      der Dialog das.
    - Ist die Datei ungültig, nennt der Dialog den Fehler (E141); nach E190 beginnt keine neue Station.
  - **Stufen:** In Stufe 1 kommen Builder, Grenzen, Budgets, Gates und Mitkopieren, in Stufe 2 die übrigen Rollen.

## Offene Entscheidungen des Owners

- **O1 Freigaben ohne Rückfrage (ändert E27).**
  - Ein unbeaufsichtigter Lauf kann nicht bei jedem Befehl fragen. Vorschlag ist E173 mit E186, E187 und E191.
  - Dazu gehört die Ausnahme von E27 und E33 für Git im eigenen Worktree (E173) *(C47)*.
  - Entscheidungsreif erst, wenn A17 und A21 geklärt sind und je Weg feststeht, welche Werkzeuge sich abschalten lassen
    (E191) *(F62, C51)*.
  - Davon hängt B43 ab und damit jeder Lauf ohne Nutzer.
- **O2 Schreiben bei GitHub/GitLab (ändert E77).** Dort bleiben Bearbeiten, Kommentieren und Mergen heute im Browser.
  Ohne Schreibzugriff endet die Fabrik am lokalen Branch. Davon hängt E177 ab.
- **O3 entfällt** *(F68)*: Das AI-Panel zeigt Auftrags-Sitzungen nur, wenn der Nutzer sie als Tab öffnet (E175); E68
  bleibt.
- **O4 Wo die Hände laufen.**
  - Vorschlag für diese Phase: lokal.
  - Läufe auf evo-1/evo-2 oder in der Cloud der Hersteller hängen an B25 Remote, das zurückgestellt ist.
  - Cloud-Agenten wie Claude Code Projects sieht Dango nur teilweise (Remote Control ist Best effort, E21).
- O5–O7, die Lücken aus der Sitzung, sind durch E183, E184 und E185 geschlossen.

## Änderungen am Plan bei Freigabe

*(F67)*
- **E19:** Die Sandbox ist für Auftrags-Sitzungen Pflicht; für andere Threads bleibt sie standardmäßig aus. Wege ohne
  Sandbox übernehmen in Aufträgen keine Rolle (E173).
- **E27, E33 und E77:** nach O1 und O2. E27 und E33 gelten in Auftrags-Sitzungen nicht für Git im eigenen Worktree
  (E173) *(C47)*.
- **E148:** Auftrags-Sitzungen von Codex laufen nie über den gemeinsamen Dienst (E193) *(C46)*.
- **E22:** Die Workflow-Datei ist eine neue Art projektgesteuerter Ausführung; Lockerungen und Gate-Befehle brauchen
  eine eigene Bestätigung (E190).
- **E11:** Git-Aufrufe des Orchestrators laufen ohne Hooks (E192).
- **E23:** neue Art `Kind::Gate` mit eigenem Sandbox-Profil und Frist (E187, E168).
  - Beenden-Frage und Aktivität lesen das Sitzungsregister (E165).
  - Auftrags-Sitzungen bekommen nur eine Positivliste der Login-Shell-Umgebung (E186).
- **E26:** eine zweite Projektdatei neben `.dango/settings.json` (E161).
- **E34 und I6:** Den Checkpoint vor einem Prompt legt der Sitzungstreiber an, auch ohne Tab (E165).
- **E55, E94, E111, E136:** ein weiteres Panel vor CI mit ⌘9.
- **E56:** Ordner ersetzen fragt bei laufenden Aufträgen (E166).
- **E64:** Aufträge kommen nach einem Neustart pausiert zurück; ihr Pausieren beim Beenden wird noch geschrieben (E167).
- **E99 und I1:** Die Worktree-Logik zieht aus `ide-ui-git` nach `ide-git`; Worktrees startet auch der Orchestrator
  (B45a).
- **E113:** neue Nutzerschlüssel sind `PerMac`.
- **E143:** Ratchet-Budget der neuen UI-Crate (E160).
- **R1, R9, R11** wachsen mit jeder parallelen Sitzung; siehe R60 und R68.

## Risiken

- **R60 Kosten und Kontingente.**
  - Bei Anthropic kostete der volle Aufbau mit Planer und Prüfer 200 $ statt 9 $ für den Einzellauf (22-fach), der
    C-Compiler knapp 20 000 $. StrongDM rechnet mit mindestens 1 000 $ je Ingenieur und Tag.
  - Laut Anthropic erreichen Claude Code Projects die Nutzungslimits schneller.
  - Abgefangen durch Budgets in drei Einheiten und die 70-%-Grenze beim Abo (E168), dazu das Kontingent im Routing
    (E170).
  - Der Halt ist nicht hart: Ein Weg, der erst nach dem Zug meldet, kann um einen Zug überziehen, und die 70-%-Grenze
    reserviert nichts für den Nutzer *(C64)*.
- **R61 Großzügige Prüfer und ausgetrickste Tests.**
  - Anthropics Prüfer „neigt zur Großzügigkeit“; StrongDM fand Tests mit `return true`.
  - Abgefangen durch automatische Gates vor dem Prüfer-Agenten (E163) und einen Prüfer eines anderen Herstellers (E162).
  - Dazu kommen die Erkennung jeder Änderung an bestehenden Tests (E172), Gates, die null Tests nicht bestehen lassen,
    und eine Secrets-Prüfung mit eingefrorenen Regeln (E163) *(C52, C54, C68)*.
- **R62 Unbeaufsichtigte Rechte ohne App-Sandbox** (verschärft R5) *(F43–F46)*.
  - Claudes Sandbox umfasst nur Shell-Befehle und lässt standardmäßig den ganzen Rechner lesen; zwei Wege haben gar
    keine. Gates führen den Code des Agenten aus.
  - Abgefangen durch:
    - E173: Lesesperre, Ablehn-Regeln für Git über den Worktree hinaus, Ref-Wache als Erkennung, nur Wege mit Sandbox;
    - E186: Umgebung ohne Zugangsdaten;
    - E187: Gates in der Sandbox, mit Lese- und Socket-Sperre;
    - E191: Werkzeuge aus;
    - E192: Git ohne Hooks;
    - E193: Codex nie über den gemeinsamen Dienst *(C46–C53)*.
  - Was der Prototyp nicht bestätigt (A21), bleibt offen. Deshalb hängt O1 daran.
- **R63 Prompt-Injection.**
  - Issue-Texte, Kommentare Dritter und Diffs sind Fremdtext.
  - Abgefangen durch eingefrorene Texte ohne Kommentare, Pflicht-G1 für Issues und erlaubte Autoren (E176).
  - Dazu die Kennzeichnung im Prompt (E179) und keine Zugangsdaten in Reichweite des Modells (E186).
- **R64 Parallele Aufträge treten sich auf die Füße**, wie die 16 Agenten des C-Compilers. Abgefangen durch
  Pfadbereiche und die Merge-Queue (E174).
- **R65 Protokolle ohne Garantie** (R7, R40, R41) treffen jetzt die ganze Fabrik. Abgefangen, weil der Orchestrator nur
  das Thread-Modell kennt und bei einem Ausfall der nächste Agent der Rolle übernimmt (E170).
- **R66 Der Mac schläft.** Abgefangen durch E169 und die Regeln für Schlaf und Aufwachen (E168); bei geschlossenem
  Deckel pausieren Aufträge trotzdem.
- **R67 Umbau der Sitzungen ohne neue Funktion** (wie R17) *(F40)*.
  - B37 greift in den meistgenutzten Teil der App ein: 28 009 Zeilen, 96 Zugriffe auf `Source`.
  - Abgefangen durch den Schnitt in B37a, der den Tab nicht anfasst, und B37b mit einem Weg je Schritt und `make ci` je
    Schritt.
- **R68 Platte und Last je Worktree** *(F70)*.
  - Jeder Worktree baut in einen eigenen Build-Ordner, bei Rust-Projekten mehrere GB, und beginnt mit einem Kaltbau.
  - Parallele Builds konkurrieren mit der Oberfläche um die CPU (R9).
  - Abgefangen durch die Grenze von 2 parallelen Aufträgen, `AGENT_COMMAND_NICE` für Gates, Ausnahme vom Backup, einen
    gemeinsamen Cache über die Workflow-Datei und das Entfernen nach der Übernahme.
  - Die Kaltbau-Zeit wird in Stufe 1 gemessen und hier nachgetragen.
- **R69 Agenten ändern ihre eigenen Prüfungen.** Abgefangen durch E172: Regeln und Agenten-Konfiguration kommen aus dem
  Hauptstand, geänderte bestehende Prüfungen heben die gemessene Risikoklasse.
- **R70 Die Abnahme wird zur Routine.** Viele Nachweise verleiten zum Durchwinken. Abgefangen durch Abweichungen am
  Anfang des Nachweises und Abnahme je Abschnitt. Bei hoher Risikoklasse geht „Abnehmen“ erst nach dem Öffnen des
  Diffs. Neue Testdateien zählen nicht als Abweichung, damit die Klasse ihre Aussage behält (E172).

## Annahmen

Die Zählung setzt die aus phase-12 (A5–A13) fort. A16 ist mit B49 entfallen.

- **A14** Start, Ereignisse, Freigaben, Checkpoint vor dem Prompt und Warteschlange lassen sich je Weg in einen Treiber
  ziehen, ohne dass sich das Verhalten des AI-Tabs ändert. Zu prüfen zu Beginn von B37a an Claude.
- **A15** Jeder Weg kann eine Sitzung nach dem Ende ihres Prozesses fortsetzen: Claude über `--resume`, Codex und
  OpenCode über ihre Sitzungs-Ids, ACP-Agenten nur, wo sie es anbieten. Zu prüfen je Weg in B38; ohne Fortsetzen
  beginnt die Station ab ihrem Checkpoint neu.
- **A17** Die Nutzungsbedingungen der Abos erlauben unbeaufsichtigte Läufe der offiziellen CLIs über ihr
  SDK-Protokoll *(F62)*.
  - Zu prüfen vor der Entscheidung über O1, je Hersteller mit Datum und Quelle: Anthropic, OpenAI, Google und die
    Anbieter hinter OpenCode.
  - Fällt ein Abo-Weg weg, bleibt er im Routing nur mit API-Schlüssel.
  - **Geprüft am 2026-11-16:**
    - **Anthropic:** Die Consumer Terms (gültig ab 2025-10-08, §3 Nr. 7) verbieten Zugriff „through automated or
      non-human means … bot, script“ ohne API-Schlüssel oder ausdrückliche Erlaubnis.
      - Die Seite „Legal and compliance“ von Claude Code verbietet Dritten, Abo-Anmeldungen anzubieten, zu
        vermitteln oder Zugangsdaten zu speichern. Ausgenommen ist ausdrücklich ein Nutzer, der sich im
        unveränderten Claude-Code-Programm mit seinem eigenen Abo anmeldet; das ist Dangos Weg (E21).
      - Dieselbe Seite nimmt für die Grenzen der Pläne „ordinary, individual usage“ an. Die Authentifizierungs-Doku
        empfiehlt `claude setup-token` ausdrücklich für Skripte und unbeaufsichtigte Läufe.
      - Ergebnis: vertretbar, solange Dango Zugangsdaten nie berührt und wenige Aufträge parallel laufen. Eindeutig
        abgesichert sind unbeaufsichtigte Läufe nur mit API-Schlüssel.
      - Die angekündigte Trennung von Agent-SDK- und `claude -p`-Nutzung vom Abo (2026-06-15) ist laut
        Sekundärquellen ausgesetzt (R1).
    - **OpenAI:** Die Codex-Doku empfiehlt API-Schlüssel für Automatisierung; ein Verbot unbeaufsichtigter Läufe mit
      ChatGPT-Anmeldung fand sich dort nicht. Die Terms of Use ließen sich nicht abrufen (403); das bleibt offen.
    - **Google:** Antigravity hat in Aufträgen keine Rolle (E173). Google-Modelle laufen nur über den eigenen Agenten
      mit API-Schlüssel und damit unter den API-Bedingungen.
    - **Anbieter hinter OpenCode:** OpenCode hat in Aufträgen keine Rolle (E173).
  - **Folge für O1:** Der Modus „Auftrag“ ist für Claude mit eigenem Abo vertretbar, mit API-Schlüssel eindeutig. Für
    Codex mit ChatGPT-Anmeldung fehlt der Wortlaut der Terms of Use.
- **A18** Eine Aktivität über `NSProcessInfo` mit `NSActivityUserInitiated` hält den Mac im Leerlauf wach und schaltet
  App Nap ab, nicht aber bei geschlossenem Deckel ohne externen Bildschirm. Zu prüfen in B38.
- **A19** Jeder Weg lässt sich zu einer Antwort nach einem festen JSON-Schema bringen, oder wenigstens zu einem
  JSON-Block, der sich prüfen lässt. Zu prüfen zu Beginn von B41b.
- **A20** Übliche Build-Werkzeuge (cargo, go, npm) laufen unter dem `sandbox-exec`-Profil der Gates, mit Schreiben nur in
  Worktree, Build-Ordner und Temp *(F45)*. Zu prüfen zu Beginn von B41a.
- **A21** Wie sich die Sandboxen im verknüpften Worktree verhalten *(F43, F53)*.
  - Claudes Sandbox lässt Schreiben in das gemeinsame `.git` des Haupt-Repos zu, außer `hooks/` und `config`; das sagt
    eine Zusammenfassung der Doku, nicht ihr Wortlaut.
  - Codex' `workspace-write` sperrt `.git` samt aufgelöstem Git-Verzeichnis, auch im verknüpften Worktree. Das ist
    dokumentiert, aber nicht ausprobiert. Der App-Server der erprobten Version nimmt ausdrückliche Lese-Wurzeln an *(C59)*.
  - Ob die Einstellungen aus E173 lesen und schreiben wie beschrieben begrenzen, zeigt erst der Prototyp mit der echten
    CLI vor der Entscheidung über O1.

## Abnahme der Phase

Neben den Kriterien je Stufe läuft eine bezahlte Gruppe `factory` in `make test-integration` (E53), nur wenn sie
ausdrücklich genannt wird:
- ein echter Auftrag auf einem Temp-Repo durch alle Stationen, Builder und Prüfer von zwei Herstellern, mit einer
  Prüfrunde zurück;
- die Negativfälle aus B43;
- ein Auftrag am Dango-Repo selbst, mit Mitkopieren und Geschwisterordner nach E188 *(F51)*.

## Gestrichen

Vom Owner am 2026-11-16 nach dem Review gestrichen; nicht umzusetzen, bis er sie wieder aufnimmt.

- **Kanban-Board** *(F66)*:
  - AppKit hat kein Steuerelement für Spalten mit Karten. Tastatur, VoiceOver und Rechts-nach-links müssten neu gebaut
    werden.
  - Karten laden zum Ziehen ein, das E166 verbietet.
  - Bei höchstens 2 parallelen Aufträgen bleiben die meisten Spalten leer.
  - Ersetzt durch Stationsleiste und Tabelle (E175).
- **B48 Best-of-N (E181):** Zwei Builder verschiedener Hersteller je Auftrag kosten mit doppelter Prüfung etwa das
  Dreifache, für einen Einzelnutzer mit Abo-Grenzen zu viel *(F73)*.
- **B49 Verdeckte Szenarien (E182, A16)** *(F52)*.
  - Im Prompt des Prüfers liefen sie über die Ablage der Gespräche (`~/.claude/projects`, Dangos Thread-Ablage) und
    über die Befunde zum Builder aus.
  - Ein nur lesender Prüfer kann Abläufe aus Nutzersicht nicht ausführen.
  - Bei Wiederaufnahme: Szenarien als Gate-Befehle außerhalb des Repos, die der Orchestrator nur in einer Kopie
    ausführt; an den Builder geht nur „Szenario N nicht bestanden“.

## Verzeichnis der Kürzel

*(F74)*

| Baustein | Stufe | Entscheidungen |
|---|---|---|
| B37a Sitzungstreiber ohne Tab | 1 | E165 |
| B38 Orchestrator | 1 | E160, E166, E167, E168, E169, E189 |
| B39 Workflow-Datei | 1 | E161, E172, E190 |
| B41a Prüfstationen und Nachweis | 1 | E163, E187, E192 |
| B42 Leitstand | 1 | E175 |
| B43 Modus „Auftrag“ | 1 | E173, E186, E191, E193 |
| B44a Neuer Auftrag von Hand | 1 | E185 |
| B45a Worktree und Übernahme | 1 | E164, E188 |
| B37b AI-Tab auf dem Sitzungstreiber | 2 | E184 |
| B40 Rollen und Routing | 2 | E162, E170, E171 |
| B41b Prüfer und Rückschleife | 2 | E179 |
| B46 Planer und G1 | 2 | E178 |
| B44b Eingang aus Issues und Rückmeldung | 3 | E176, E177 |
| B45b Pfadbereiche und Merge-Queue | 3 | E174 |
| B47 Kennzahlen und Lernschleife | 4 | E180 |
| Einstellungen | 1–2 | E183 |
| gestrichen | — | E181, E182 |

## Quellen

- OpenAI: [Harness engineering](https://openai.com/index/harness-engineering/) (für automatische Abrufe gesperrt; Zahlen
  aus [InfoQ](https://www.infoq.com/news/2026/02/openai-harness-engineering-codex) und
  [ZenML](https://www.zenml.io/llmops-database/zero-human-written-code-harness-engineering-for-autonomous-ai-agents-at-scale)),
  [Symphony](https://openai.com/index/open-source-codex-orchestration-symphony/),
  [Symphony SPEC.md](https://github.com/openai/symphony), [Codex-App](https://openai.com/index/introducing-the-codex-app/)
- Anthropic: [C-Compiler](https://www.anthropic.com/engineering/building-c-compiler),
  [Harness-Design](https://www.anthropic.com/engineering/harness-design-long-running-apps),
  [Effective harnesses](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents),
  [Managed Agents](https://www.anthropic.com/engineering/managed-agents),
  [Sandboxing in Claude Code](https://code.claude.com/docs/en/sandboxing),
  [Dynamic Workflows (InfoQ)](https://www.infoq.com/news/2026/06/dynamic-workflows-claude-code/),
  [Claude Code Projects (MarkTechPost, Sekundärquelle)](https://www.marktechpost.com/2026/09/17/anthropic-launches-claude-code-projects-in-beta-parallel-cloud-sessions-that-keep-running-after-you-close-your-laptop/)
- Codex: [openai/codex#14794](https://github.com/openai/codex/issues/14794) (`.git` schreibgeschützt in
  `workspace-write`), [App-Server: Turns](https://learn.chatgpt.com/docs/app-server#turns),
  [Verkehr außerhalb des Netz-Proxys](https://learn.chatgpt.com/docs/agent-approvals-security#traffic-outside-the-command-network-proxy)
- OpenCode: [Agents](https://opencode.ai/docs/agents/)
- git: [git-push](https://git-scm.com/docs/git-push), [git-worktree](https://git-scm.com/docs/git-worktree),
  [git-patch-id](https://git-scm.com/docs/git-patch-id); gitleaks:
  [Konfigurationsauswahl](https://github.com/gitleaks/gitleaks/blob/master/cmd/root.go)
- StrongDM: [Software Factory](https://factory.strongdm.ai/), dazu
  [Simon Willison](https://simonwillison.net/2026/Feb/7/software-factory/)
- Factory: [Factory 2.0](https://factory.ai/news/software-factory)
- GitHub: [Claude und Codex in Agent HQ](https://github.blog/news-insights/company-news/pick-your-agent-use-claude-and-codex-on-agent-hq/)
- OpenTelemetry: [Status der GenAI-Konventionen](https://dev.to/azena-ai/opentelemetrys-genai-semantic-conventions-are-not-stable-yet-heres-what-actually-shipped-in-2026-3mke)
