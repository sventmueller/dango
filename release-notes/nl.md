<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Dit is de eerste release van Dango, een native macOS-editor om met codeeragents te werken. Editor, terminals, een
browser, Git, CI, een debugger en AI-threads delen één venster per map. De app is geschreven in Rust op AppKit; WebKit
komt alleen voor in het browsertabblad. De interface is beschikbaar in 35 talen.

## Vereisten
- macOS 14 of nieuwer, op Apple silicon of Intel.
- Voor de AI-threads de agent die je kiest: Claude heeft het commando `claude` nodig, ingelogd; Codex heeft Node.js
  nodig, en de app installeert het vastgezette pakket als je daarom vraagt; Antigravity en OpenCode gebruiken je eigen
  `agy` of `opencode`, of een vastgezette download; de ingebouwde agent heeft een API-sleutel nodig, of Ollama of LM
  Studio op deze Mac.
- Taalservers en debugadapters worden geïnstalleerd als je daarom vraagt. gopls en `dlv` hebben Go nodig; de servers
  voor TypeScript, Python, JSON, YAML, CSS, HTML en shell en de JavaScript-debugger hebben Node.js nodig; clangd,
  sourcekit-lsp en `lldb-dap` komen mee met de Xcode Command Line Tools.
- Voorspellingen tijdens het typen vereisen een GitHub Copilot-abonnement (het gratis abonnement volstaat), of een
  fill-in-the-middle-model in Ollama of LM Studio.

## Venster
- Elk bestand, elke terminal, browserpagina, AI-thread, verschillenweergave en voorvertoning is een macOS-tabblad in het
  venster van de map. De '+' aan het eind van de tabbladbalk en ⌘T bieden een nieuw bestand, een terminal, de browser en
  elke geïnstalleerde agent aan, ook in een eigen Git-worktree.
- De navigatiekolom wisselt tussen Bestanden, Zoeken, Agents, Git, Debug en CI (⌘1–⌘6); met een klik met de
  rechtermuisknop verberg je wat je niet gebruikt. In de knoppenbalk staan terug en vooruit langs de plekken die je hebt
  bewerkt (⌃- en ⌃⇧-), de branch, 'Open snel' en het overzicht 'Activiteit'.
- 'Open ernaast' toont een tweede document naast het eerste. Na een herstart of een crash komt elk venster terug met
  zijn tabbladen, en editors met niet-bewaarde tekst komen met die tekst terug.
- Afbeeldingen, audio, video, lettertypen en Office-documenten openen in een tabblad via Quick Look, pdf's via PDFKit.

## Editor
- Syntaxiskleuren, regelnummers, wijzigingsmarkeringen ten opzichte van de laatste commit, zoeken en vervangen, 'Open
  snel' (⌘P) en zoeken in het project (⇧⌘F).
- Automatisch inspringen, tabs of spaties volgens `.editorconfig`, haakjesparen, opmerkingen aan- en uitzetten,
  invouwen, ga naar regel, zichtbare witruimte en doorlopende regels.
- Meerdere invoegpunten: ⌥-klik, ⌥-sleep voor een kolom, ⌘D voor de volgende instantie.
- Bestanden behouden hun codering, byte order mark en regeleinden. Een bestand dat op schijf is gewijzigd, wordt opnieuw
  geladen als de editor geen niet-bewaarde tekst heeft; anders biedt een balk de verschillen en samenvoegen aan.
- **Voorspellingen tijdens het typen:** terwijl je typt, verschijnt een voorstel in grijs; Tab neemt het over.
  Voorstellen komen van GitHub Copilot of van een lokaal model in Ollama of LM Studio, wat je in de instellingen kiest;
  met een lokaal model blijft je code op deze Mac.

## Taalservers
Go, Rust, TypeScript en JavaScript, Python, C, C++ en Objective-C, Swift, shellscripts, JSON, YAML, CSS, HTML, TOML,
Markdown en Lua, plus eigen servers via de instellingen. Diagnostiek, hover, aanvulling, signatuurhulp, ga naar
definitie, typedefinitie en implementatie, verwijzingen, aanroep- en typehiërarchie, hernoemen, opmaak, semantische
kleuren, invouwen en de structuur; typen en parameternamen in de regel; snelle oplossingen en refactorings achter een
gloeilamp (⌥⌘A); de symbolen van het project met ⌥⌘O of `#` in 'Open snel'. De eigen `tsc` van een TypeScript-project
wordt gebruikt als die als taalserver kan dienen.

## Debugger
Breakpoints, ook met voorwaarden, stap voor stap uitvoeren, threads met hun aanroepstack, variabelen en bewaakte
expressies in de Inspecteur (⌥⌘I), waarden naast de code terwijl het programma onderbroken is, en een debugconsole die
evalueert in het onderbroken frame. Rust, C, C++ en Swift via `lldb-dap`, Go via `dlv`, JavaScript en TypeScript via de
JavaScript-debugger, Python via debugpy. De `.vscode/launch.json` van een project wordt gelezen. Het menu 'Debug'
gebruikt de toetscombinaties van Xcode.

## Terminal
Terminals in tabbladen met de omgeving van de inlogshell, getekend met Metal. Een geplakte of hierheen gesleepte
afbeelding gaat naar een codeeragent die in de terminal draait, of anders wordt het pad van een bewaarde kopie geplakt.
⌘-klik opent links en bestandsnamen. Een ontwikkelserver die de terminal start, kun je openen in het browsertabblad.

## Markdown
Een Markdown-bestand opent met de voorvertoning ernaast (⇧⌘V), met formules, diagrammen in Mermaid, Graphviz en PlantUML
en de afbeeldingen uit de map. Een menu 'Opmaak' voor vet, cursief, links, koppen, lijsten, citaten en codeblokken;
tekst die je uit Word of een webpagina plakt, komt aan als Markdown. **Archief › Exporteer** schrijft pdf, één
HTML-bestand of een Word-document. Als Splat is geïnstalleerd, opent **Archief › Open in Splat** het document daar.

## Browser
Een browsertabblad met adresveld, terug en vooruit, zoeken, downloads met eerst een vraag, en de webinspector.

## Git
Een Git-paneel met stagen en weggooien van hele bestanden of losse hunks, commits, committen en pushen of
synchroniseren, branches, stashes, samenvoegconflicten en een grafiek van de recente commits. Blame toont wie de regel
onder de cursor het laatst heeft gewijzigd, of een kolom voor het hele bestand. Een knop schrijft het commitbericht met
AI; de diff gaat pas de deur uit als gitleaks er geen geheim in heeft gevonden, en elke commit wordt ook op geheimen
gecontroleerd. Een AI-thread kan in een eigen Git-worktree werken; het paneel toont wat hij heeft gewijzigd en neemt het
werk over met samenvoegen of cherry-pick.

## CI
GitHub Actions en GitLab CI, ook GitHub Enterprise Server en zelfbeheerd GitLab: runs, jobs en logs van de huidige
branch; mislukte jobs, één job of (op GitHub) een hele run opnieuw uitvoeren, handmatige jobs starten of wachtende
deployments goedkeuren, een nieuwe pipeline of een handmatig te starten workflow starten, en annuleren. De open merge
requests of pull requests en issues staan in hetzelfde paneel en openen in het browsertabblad. Het log van een mislukte
job kan naar een agent gaan, gemarkeerd als externe inhoud. Het token komt uit de sleutelhanger, van `gh` of `glab`, uit
de omgeving van de inlogshell, of je voert het zelf in.

## AI
Elke agent werkt in een AI-tabblad met dezelfde tijdlijn: antwoorden, toolaanroepen met hun uitvoer,
toestemmingsverzoeken, vragen, plannen om goed te keuren en van commentaar te voorzien, to-do's, subagents en
achtergrondtaken; twee knoppen in de kop van het tabblad tonen de actieve subagents en achtergrondprocessen. Voor elke
prompt maakt de app een herstelpunt; wijzigingen kun je per bericht bekijken en terugspoelen. Nieuwe threads starten in
de modus 'Handmatig', en destructieve Git-commando's worden nooit blijvend toegestaan. Het overzicht 'Activiteit' toont
alles wat draait. Een groene stip op het tabblad toont een sessie die bezig is, een rode een sessie die op jou wacht, en
een melding laat weten wanneer een sessie klaar is of wacht terwijl je met iets anders bezig bent.
- **Claude** via het ongewijzigde commando `claude`, met sessies, subagents, achtergrondtaken, context en gebruik.
- **ACP-agents:** Codex, Antigravity en OpenCode, en meer uit het ACP-register als je daarom vraagt; geïnstalleerde
  agents zijn vastgezet op geteste versies en gecontroleerd aan de hand van hun checksums.
- **Ingebouwde agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio en servers die de API van OpenAI
  of Anthropic spreken, met tools voor bestanden, zoeken, commando's, diagnostiek en webverzoeken; sleutels blijven in
  de sleutelhanger.
- Een sandboxschakelaar per thread beperkt wat de commando's van een agent mogen doen, voor zover de agent dat
  ondersteunt.
- Externe inhoud, zoals terminaluitvoer, een webpagina of een CI-log, gaat naar een agent gemarkeerd als niet-vertrouwd.

## Dicteerfunctie
Dicteren in het berichtveld van het AI-tabblad en in de editor via de spraakherkenning van Lowspeak: de microfoonknop,
⌃⌥D, of de fn-toets terwijl Dango de actieve app is. Dit gebruikt de instellingen en modellen van Lowspeak; online
transcriptie vereist een sleutel voor OpenAI of Groq.

## Updates
De app werkt zichzelf bij via Sparkle. Bij de tweede start vraagt hij of hij zelf naar updates mag zoeken; zolang je
niet akkoord gaat, doet hij geen enkel verzoek. **Dango › Zoek naar updates…** zoekt meteen.

## Toegankelijkheid
VoiceOver-namen voor de navigatiekolom, tabbladen, de kaarten van de AI-tijdlijn, Git- en CI-rijen, de debugger en de
tekst van de terminal; volledige bediening via het toetsenbord met toetscombinaties die je in `keybindings.json` kunt
wijzigen; 'Verhoog contrast' en 'Verminder beweging'; codeweergaven blijven van links naar rechts lopen in interfaces
die van rechts naar links lopen. Details staan in ACCESSIBILITY.md.

## Privacy
Geen telemetrie, geen analyses en geen crashrapporten. De app maakt alleen verbinding voor wat je zelf start: de agents
die je gebruikt, de downloads waar je om vraagt, de CI-hosts waarbij je inlogt, voorspellingen van Copilot als je
daarvoor hebt gekozen, online transcriptie als je daarvoor hebt gekozen, de pagina's die je opent, en het zoeken naar
updates zodra je dat hebt toegestaan. Sleutels en tokens blijven in de macOS-sleutelhanger.

## Bekende beperkingen
- Inloggen bij CI via de browser staat uit; gebruik `gh`, `glab`, een token of de omgeving.
- De app draait zonder de App Sandbox, omdat terminals, Git, taalservers en agents die zouden erven; een map moet
  vertrouwd zijn voordat iets wat die map configureert, wordt uitgevoerd. Antigravity en OpenCode hebben geen sandbox
  die de app kan inschakelen.
- De gebruikscijfers en contextuitsplitsing van Claude steunen op delen van zijn protocol die als experimenteel zijn
  gemarkeerd, en worden verborgen als die ontbreken. Het gesprek terugspoelen werkt alleen bij Claude; bij de andere
  agents wordt alleen de code teruggespoeld.
- Gemini CLI wordt niet aangeboden: Google heeft het inloggen met een persoonlijk account beëindigd en verwijst naar
  Antigravity. Eerdere Gemini-threads blijven leesbaar.
