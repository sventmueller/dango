<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Dette er den første utgivelsen av Dango, en native macOS-editor for arbeid med kodeagenter. Editor, terminaler, en
nettleser, Git, CI, en feilsøker og KI-tråder deler ett vindu per mappe. Appen er skrevet i Rust på AppKit; WebKit
brukes bare i nettleserfanen. Grensesnittet finnes på 35 språk.

## Krav
- macOS 14 eller nyere, på Apple-chip eller Intel.
- For KI-trådene, agenten du velger: Claude trenger kommandoen `claude`, pålogget; Codex trenger Node.js, og appen
  installerer den fastlåste pakken når du ber om det; Antigravity og OpenCode bruker din egen `agy` eller `opencode`,
  eller en fastlåst nedlasting; den innebygde agenten trenger en API-nøkkel, eller Ollama eller LM Studio på denne
  Macen.
- Språktjenere og feilsøkingsadaptere installeres når du ber om det. gopls og `dlv` trenger Go; tjenerne for TypeScript,
  Python, JSON, YAML, CSS, HTML og shell og JavaScript-feilsøkeren trenger Node.js; clangd, sourcekit-lsp og `lldb-dap`
  følger med Xcode Command Line Tools.
- Forutsigelser mens du skriver krever et GitHub Copilot-abonnement (gratisabonnementet holder), eller en
  fill-in-the-middle-modell i Ollama eller LM Studio.

## Vindu
- Hver fil, terminal, nettleserside, KI-tråd, diff og forhåndsvisning er en macOS-fane i mappens vindu. «+» på slutten
  av fanelinjen og ⌘T tilbyr en ny fil, en terminal, nettleseren og hver installerte agent, også i en egen Git-worktree.
- Sidepanelet bytter mellom Filer, Søk, Agenter, Git, Feilsøk og CI (⌘1–⌘6); et høyreklikk skjuler dem du ikke bruker.
  Verktøylinjen har tilbake og fremover gjennom stedene du har redigert (⌃- og ⌃⇧-), branchen, Åpne raskt og oversikten
  Aktivitet.
- «Åpne ved siden av» viser et annet dokument ved siden av det første. Etter en omstart, eller et krasj, kommer hvert
  vindu tilbake med fanene sine, og editorer med ulagret tekst kommer tilbake med teksten.
- Bilder, lyd, video, skrifter og Office-dokumenter åpnes i en fane via Quick Look, PDF-er via PDFKit.

## Editor
- Syntaksfarger, linjenumre, endringsmerker mot siste commit, finn og erstatt, Åpne raskt (⌘P) og prosjektsøk (⇧⌘F).
- Automatisk innrykk, tabulatorer eller mellomrom etter `.editorconfig`, parentespar, slå kommentarer av og på,
  bretting, gå til linje, synlige mellomrom og linjebryting.
- Flere innsettingspunkter: ⌥-klikk, ⌥-dra for en kolonne, ⌘D for neste forekomst.
- Filer beholder tegnkoding, byte order mark og linjeskift. En fil som er endret på disken, lastes inn på nytt når
  editoren ikke har ulagret tekst; ellers tilbyr en linje forskjellene og sammenslåing.
- **Forutsigelser mens du skriver:** Mens du skriver, vises et forslag i grått; Tab godtar det. Forslagene kommer fra
  GitHub Copilot eller fra en lokal modell i Ollama eller LM Studio, alt etter hva du velger i innstillingene; en lokal
  modell holder koden din på denne Macen.

## Språktjenere
Go, Rust, TypeScript og JavaScript, Python, C, C++ og Objective-C, Swift, shellskript, JSON, YAML, CSS, HTML, TOML,
Markdown og Lua, og egne tjenere fra innstillingene. Diagnostikk, hover, fullføring, signaturhjelp, gå til definisjon,
typedefinisjon og implementering, referanser, kallhierarki og typehierarki, endre navn, formatering, semantiske farger,
bretting og disposisjonen; typer og parameternavn inne i linjen; hurtigrettinger og refaktoreringer bak en lyspære
(⌥⌘A); prosjektets symboler med ⌥⌘O eller `#` i Åpne raskt. Et TypeScript-prosjekts egen `tsc` brukes når den kan
fungere som språktjener.

## Feilsøker
Brytepunkter, også med betingelser, trinnvis kjøring, tråder med anropsstakkene sine, variabler og overvåkede uttrykk i
Inspektør (⌥⌘I), verdier ved siden av koden mens programmet er pauset, og en feilsøkingskonsoll som evaluerer i den
pausede rammen. Rust, C, C++ og Swift via `lldb-dap`, Go via `dlv`, JavaScript og TypeScript via JavaScript-feilsøkeren,
Python via debugpy. Et prosjekts `.vscode/launch.json` blir lest. Feilsøk-menyen bruker hurtigtastene fra Xcode.

## Terminal
Terminaler i faner med miljøet fra påloggingsskallet, tegnet med Metal. Et bilde du limer inn eller slipper, går til en
kodeagent som kjører i terminalen, ellers limes banen til en lagret kopi inn. ⌘-klikk åpner lenker og filnavn. En
utviklingstjener som terminalen starter, kan åpnes i nettleserfanen.

## Markdown
En Markdown-fil åpnes med forhåndsvisningen ved siden av (⇧⌘V), med formler, Mermaid-, Graphviz- og PlantUML-diagrammer
og bildene i mappen. En Format-meny for fet, kursiv, lenker, overskrifter, lister, sitater og kodeblokker; tekst limt
inn fra Word eller en nettside kommer inn som Markdown. **Fil › Eksporter** skriver PDF, én HTML-fil eller et
Word-dokument. Der Splat er installert, åpner **Fil › Åpne i Splat** dokumentet der.

## Nettleser
En nettleserfane med adressefelt, tilbake og fremover, søk, nedlastinger som spør først, og Webinspektør.

## Git
Et Git-panel med staging og forkasting av hele filer eller enkeltblokker, commits, commit og push eller synkronisering,
brancher, stasher, sammenslåingskonflikter og en graf over de siste commitene. Blame viser hvem som sist endret linjen
under markøren, eller en kolonne for hele filen. En knapp skriver commit-meldingen med KI; diffen sendes bare ut når
gitleaks ikke har funnet noen hemmelighet i den, og hver commit sjekkes også for hemmeligheter. En KI-tråd kan arbeide i
en egen Git-worktree; panelet viser hva den har endret, og overtar arbeidet med sammenslåing eller cherry-pick.

## CI
GitHub Actions og GitLab CI, også GitHub Enterprise Server og selvdriftet GitLab: kjøringer, jobber og logger for
gjeldende branch; kjøre mislykkede jobber på nytt, én jobb eller (på GitHub) en hel kjøring, starte manuelle jobber
eller godkjenne ventende utrullinger, starte en ny pipeline eller en arbeidsflyt som kan startes manuelt, og avbryte.
Åpne merge requests eller pull requests og issues vises i samme panel og åpnes i nettleserfanen. Loggen fra en mislykket
jobb kan sendes til en agent, merket som eksternt innhold. Tokenet hentes fra nøkkelringen, `gh` eller `glab`, miljøet
fra påloggingsskallet, eller du skriver det inn selv.

## KI
Hver agent arbeider i en KI-fane med den samme tidslinjen: svar, verktøykall med utdata, forespørsler om tillatelse,
spørsmål, planer å godkjenne og kommentere, gjøremål, underagenter og bakgrunnsoppgaver; to knapper i fanens topptekst
viser underagentene og bakgrunnsprosessene som kjører. Før hver prompt lagrer appen et sjekkpunkt; endringer kan
gjennomgås og spoles tilbake per melding. Nye tråder starter i modusen «Manuell», og destruktive Git-kommandoer blir
aldri tillatt for godt. Oversikten Aktivitet viser alt som kjører. En grønn prikk på fanen viser en økt som arbeider, en
rød en økt som venter på deg, og et varsel sier fra når en økt er ferdig eller venter mens du ser et annet sted.
- **Claude** via kommandoen `claude`, uendret, med økter, underagenter, bakgrunnsoppgaver, kontekst og forbruk.
- **ACP-agenter:** Codex, Antigravity og OpenCode, og flere fra ACP-registeret når du ber om det; installerte agenter er
  låst til testede versjoner og kontrollert mot sjekksummene sine.
- **Innebygd agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio og tjenere som snakker API-et til
  OpenAI eller Anthropic, med verktøy for filer, søk, kommandoer, diagnostikk og nettforespørsler; nøkler blir i
  nøkkelringen.
- En sandkassebryter per tråd begrenser hva en agents kommandoer kan gjøre, så langt agenten støtter det.
- Eksternt innhold, som terminalutdata, en nettside eller en CI-logg, går til en agent merket som ikke klarert.

## Diktering
Diktering i meldingsfeltet i KI-fanen og i editoren via talegjenkjenningen i Lowspeak: mikrofonknappen, ⌃⌥D eller
fn-tasten mens Dango er den aktive appen. Den bruker innstillingene og modellene i Lowspeak; nettbasert transkripsjon
krever en nøkkel for OpenAI eller Groq.

## Oppdateringer
Appen oppdaterer seg selv via Sparkle. Ved andre oppstart spør den om den kan søke etter oppdateringer på egen hånd; før
du sier ja, sender den ingen forespørsler. **Dango › Søk etter oppdateringer…** søker med en gang.

## Tilgjengelighet
VoiceOver-navn for sidepanelet, faner, kortene i KI-tidslinjen, Git- og CI-rader, feilsøkeren og terminalens tekst; full
tastaturstyring med hurtigtaster du kan endre i `keybindings.json`; Øk kontrast og Reduser bevegelse; kodevisninger går
fra venstre mot høyre også i grensesnitt som går fra høyre mot venstre. Detaljer står i ACCESSIBILITY.md.

## Personvern
Ingen telemetri, ingen analyse og ingen krasjrapporter. Appen kobler seg bare til for det du starter: agentene du
bruker, nedlastingene du ber om, CI-vertene du logger på, forutsigelser fra Copilot hvis du har valgt det, nettbasert
transkripsjon hvis du har valgt det, sidene du åpner, og søket etter oppdateringer når du har tillatt det. Nøkler og
tokener blir i macOS-nøkkelringen.

## Kjente begrensninger
- Pålogging til CI via nettleseren er slått av; bruk `gh`, `glab`, et token eller miljøet.
- Appen kjører uten App Sandbox, siden terminaler, Git, språktjenere og agenter ville arve den; du må stole på en mappe
  før noe den konfigurerer, kjører. Antigravity og OpenCode har ingen sandkasse appen kan slå på.
- Claudes forbrukstall og kontekstfordeling bygger på deler av protokollen som er merket som eksperimentelle, og skjules
  når de mangler. Å spole tilbake samtalen fungerer bare for Claude; for de andre agentene spoles bare koden tilbake.
- Gemini CLI tilbys ikke: Google har avsluttet påloggingen med personlig konto og viser til Antigravity. Tidligere
  Gemini-tråder kan fortsatt leses.
