<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Dette er den første udgave af Dango, en native macOS-editor til arbejdet med kodeagenter. Editor, terminaler, en
browser, Git, CI, en fejlfinder og AI-tråde deler ét vindue pr. mappe. Appen er skrevet i Rust oven på AppKit; WebKit
findes kun i browserfanen. Brugerfladen findes på 35 sprog.

## Krav
- macOS 14 eller nyere, på Apple silicon eller Intel.
- Til AI-trådene den agent, du vælger: Claude kræver kommandoen `claude`, og at du er logget ind; Codex kræver Node.js,
  og appen installerer dens fastlåste pakke, når du beder om det; Antigravity og OpenCode bruger din egen `agy` eller
  `opencode` eller en fastlåst download; appens egen agent kræver en API-nøgle eller Ollama eller LM Studio på denne
  Mac.
- Sprogservere og debug-adaptere installeres, når du beder om det. gopls og `dlv` kræver Go; serverne til TypeScript,
  Python, JSON, YAML, CSS, HTML og shell og JavaScript-fejlfinderen kræver Node.js; clangd, sourcekit-lsp og `lldb-dap`
  følger med Xcode Command Line Tools.
- Forudsigelser under redigering kræver et GitHub Copilot-abonnement (det gratis er nok) eller en
  fill-in-the-middle-model i Ollama eller LM Studio.

## Vindue
- Hvert arkiv, hver terminal, hver browserside, hver AI-tråd, hver diff og hvert eksempel er en macOS-fane i mappens
  vindue. “+” i slutningen af fanelinjen og ⌘T tilbyder et nyt arkiv, en terminal, browseren og hver installeret agent,
  også i sin egen Git-worktree.
- Indholdsoversigten skifter mellem Arkiver, Søgning, Agenter, Git, Fejlfind og CI (⌘1–⌘6); med et højreklik skjuler du
  dem, du ikke bruger. Værktøjslinjen rummer tilbage og frem gennem de steder, du har redigeret (⌃- og ⌃⇧-), branchen,
  Åbn hurtigt og oversigten Aktivitet.
- “Åbn ved siden af” viser et andet dokument ved siden af det første. Efter en genstart eller et nedbrud vender hvert
  vindue tilbage med sine faner, og editorer med ikke-gemt tekst vender tilbage med teksten.
- Billeder, lyd, video, skrifter og Office-dokumenter åbner i en fane via Quick Look, PDF-dokumenter via PDFKit.

## Editor
- Syntaksfarver, linjenumre, ændringsmærker i forhold til seneste commit, søg og erstat, Åbn hurtigt (⌘P) og søgning i
  projektet (⇧⌘F).
- Automatisk indrykning, tabulatorer eller mellemrum efter `.editorconfig`, parenteser i par, slå kommentarer til og
  fra, foldning, gå til linje, synlige mellemrum og linjeombrydning.
- Flere indsættelsesmærker: ⌥-klik, ⌥-træk for en kolonne, ⌘D for den næste forekomst.
- Arkiver beholder deres tegnkodning, byte order mark og linjeskift. Et arkiv, der er ændret på disken, indlæses igen,
  når editoren ikke har ikke-gemt tekst; ellers tilbyder en bjælke forskellene og en merge.
- **Forudsigelser under redigering:** Mens du skriver, vises et forslag med gråt; Tab accepterer det. Forslagene kommer
  fra GitHub Copilot eller fra en lokal model i Ollama eller LM Studio, alt efter hvad du vælger i indstillingerne; en
  lokal model holder din kode på denne Mac.

## Sprogservere
Go, Rust, TypeScript og JavaScript, Python, C, C++ og Objective-C, Swift, shellscripts, JSON, YAML, CSS, HTML, TOML,
Markdown og Lua samt dine egne servere fra indstillingerne. Diagnosticering, hover, kodefuldførelse, signaturhjælp, gå
til definition, typedefinition og implementering, referencer, kald- og typehierarkier, omdøbning, formatering,
semantiske farver, foldning og dispositionen; typer og parameternavne inde i linjen; hurtige rettelser og
refaktoreringer bag en pære (⌥⌘A); projektets symboler med ⌥⌘O eller `#` i Åbn hurtigt. Et TypeScript-projekts egen
`tsc` bruges, når den kan fungere som sprogserver.

## Fejlfinder
Brudpunkter, også med betingelser, trinvis kørsel, tråde med deres kaldstakke, variabler og overvågede udtryk i
Inspektør (⌥⌘I), værdier ved siden af koden, mens programmet er standset, og en fejlfindingskonsol, der evaluerer i den
standsede frame. Rust, C, C++ og Swift via `lldb-dap`, Go via `dlv`, JavaScript og TypeScript via
JavaScript-fejlfinderen, Python via debugpy. Et projekts `.vscode/launch.json` bliver læst. Menuen Fejlfind bruger
Xcodes tastaturgenveje.

## Terminal
Terminaler i faner med login-shellens miljø, tegnet med Metal. Indsætter du et billede eller trækker det ind, går det
til en kodeagent, der kører i terminalen, ellers indsættes stien til en gemt kopi. ⌘-klik åbner links og arkivnavne. En
udviklingsserver, som terminalen starter, kan åbnes i browserfanen.

## Markdown
Et Markdown-arkiv åbner med sit eksempel ved siden af (⇧⌘V), med formler, Mermaid-, Graphviz- og PlantUML-diagrammer og
mappens billeder. En Format-menu til fed, kursiv, links, overskrifter, lister, citater og kodeblokke; tekst, der
indsættes fra Word eller en webside, ankommer som Markdown. **Arkiv › Eksporter** skriver PDF, et enkelt HTML-arkiv
eller et Word-dokument. Hvor Splat er installeret, åbner **Arkiv › Åbn i Splat** dokumentet der.

## Browser
En browserfane med adressefelt, tilbage og frem, søgning, overførsler, der spørger først, og Web Inspector.

## Git
Et Git-panel med staging og kassering af hele arkiver eller enkelte blokke, commits, commit og push eller
synkronisering, branches, stashes, merge-konflikter og en graf over de seneste commits. Blame viser, hvem der sidst
ændrede linjen ved markøren, eller en kolonne for hele arkivet. En knap skriver commit-beskeden med AI; diffen sendes
først, når gitleaks ikke har fundet nogen secrets i den, og hvert commit kontrolleres også for secrets. En AI-tråd kan
arbejde i sin egen Git-worktree; panelet viser, hvad den har ændret, og overfører arbejdet med merge eller cherry-pick.

## CI
GitHub Actions og GitLab CI, også GitHub Enterprise Server og selvadministreret GitLab: kørsler, jobs og logs for den
aktuelle branch; kør mislykkede jobs, et enkelt job eller (på GitHub) en hel kørsel igen, start manuelle jobs eller
godkend ventende udrulninger, start en ny pipeline eller et workflow, der kan startes manuelt, og annuller. De åbne
merge requests eller pull requests og issues vises i samme panel og åbner i browserfanen. Loggen fra et mislykket job
kan gives til en agent, markeret som fremmed tekst. Tokenet kommer fra nøgleringen, `gh` eller `glab`, login-shellens
miljø eller et, du indtaster.

## AI
Hver agent arbejder i en AI-fane med samme tidslinje: svar, værktøjskald med deres output, anmodninger om tilladelse,
spørgsmål, planer til godkendelse og kommentarer, gøremål, subagenter og baggrundsopgaver; to knapper i fanens top viser
de kørende subagenter og baggrundsprocesser. Før hver prompt gemmer appen et kontrolpunkt; ændringer kan gennemgås og
spoles tilbage pr. besked. Nye tråde starter i tilstanden “Manuel”, og destruktive Git-kommandoer tillades aldrig
permanent. Oversigten Aktivitet viser alt, der kører. En grøn prik på fanen viser en session i gang, en rød en session,
der venter på dig, og en notifikation fortæller, når den er færdig eller venter, mens du kigger et andet sted.
- **Claude** via kommandoen `claude`, uændret, med sessioner, subagenter, baggrundsopgaver, kontekst og forbrug.
- **ACP-agenter:** Codex, Antigravity og OpenCode samt flere fra ACP-registret, når du beder om det; installerede
  agenter er låst til testede versioner og kontrolleres mod deres kontrolsummer.
- **Egen agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio og servere, der taler OpenAI's eller
  Anthropics API, med værktøjer til arkiver, søgning, kommandoer, diagnosticering og webanmodninger; nøgler bliver i
  nøgleringen.
- En sandbox-kontakt pr. tråd begrænser, hvad en agents kommandoer må gøre, så vidt agenten understøtter det.
- Fremmed tekst, såsom terminaloutput, en webside eller en CI-log, går til en agent markeret som ikke-betroet.

## Diktering
Diktering i AI-beskedfeltet og i editoren via Lowspeaks talegenkendelse: mikrofonknappen, ⌃⌥D eller fn-tasten, mens
Dango er den aktive app. Den bruger Lowspeaks indstillinger og modeller; onlinetransskription kræver en nøgle til OpenAI
eller Groq.

## Opdateringer
Appen opdaterer sig selv via Sparkle. Ved anden start spørger den, om den selv må søge efter opdateringer; indtil du
siger ja, sender den ingen forespørgsler. **Dango › Søg efter opdateringer…** søger med det samme.

## Tilgængelighed
VoiceOver-navne til indholdsoversigten, faner, kortene i AI-tidslinjen, Git- og CI-rækker, fejlfinderen og terminalens
tekst; fuld tastaturstyring med genveje, du kan ændre i `keybindings.json`; Forøg kontrast og Reducer bevægelse;
kodevisninger forbliver venstre mod højre i brugerflader, der skrives fra højre mod venstre. Detaljer findes i
ACCESSIBILITY.md.

## Privatliv
Ingen telemetri, ingen analyse og ingen nedbrudsrapporter. Appen opretter kun forbindelse for det, du selv starter: de
agenter, du bruger, de overførsler, du beder om, de CI-værter, du logger ind på, forudsigelser fra Copilot, hvis du har
valgt dem, onlinetransskription, hvis du har valgt den, de sider, du åbner, og søgningen efter opdateringer, når du har
tilladt den. Nøgler og tokens bliver i macOS-nøgleringen.

## Kendte begrænsninger
- Login til CI via browseren er slået fra; brug `gh`, `glab`, et token eller miljøet.
- Appen kører uden App Sandbox, da terminaler, Git, sprogservere og agenter ville arve den; du skal stole på en mappe,
  før noget, den konfigurerer, kører. Antigravity og OpenCode har ingen sandbox, som appen kan slå til.
- Claudes forbrugstal og kontekstopdeling bygger på dele af protokollen, der er markeret som eksperimentelle, og
  skjules, når de mangler. Tilbagespoling af samtalen virker kun med Claude; for de andre agenter spoles kun koden
  tilbage.
- Gemini CLI tilbydes ikke: Google har lukket for login med en personlig konto og henviser til Antigravity. Tidligere
  Gemini-tråde kan stadig læses.
