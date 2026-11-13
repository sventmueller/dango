<p><img src="../../readme-icon.png" alt="Dango-symbol" width="168" height="168"></p>

# Dango

**Den native macOS-editor til arbejde med kodeagenter.**

Dango samler koden, de agenter, der ændrer den, og alt omkring dem i ét vindue pr. mappe: en editor med sprogservere og forudsigelser, mens du skriver, terminaler, en browser, Git, CI, en debugger og AI-tråde med Claude, Codex, Antigravity, OpenCode eller en agent, der er bygget ind i appen. Hver agent arbejder i den samme tidslinje, spørger, før den ændrer arkiver eller kører kommandoer, og efterlader et checkpoint før hver prompt, så du kan gennemgå og spole tilbage, hvad den har gjort, besked for besked. Intet, som en mappe konfigurerer, kører, før du stoler på den, og tekst udefra når frem til en agent mærket som upålidelig. Dikter ind i editoren og beskedfeltet, og talegenkendelsen kan blive på din Mac. Dango er skrevet i Rust på ægte AppKit-betjeningselementer, uden Electron, taler 35 sprog og har ingen telemetri.

**Endnu ikke udgivet.** Den første udgivelse bliver et diskbillede, signeret med et Developer ID og notariseret af Apple, til macOS 14 eller nyere på Apple Silicon og Intel. Indtil da [bygger du Dango fra kildekoden](#byg-fra-kildekoden).

**Har du fundet en fejl?** [Opret et issue](https://github.com/sventmueller/dango/issues/new), og skriv, med hvilken agent og i hvilken slags projekt den opstod.

[![Rust, 2024-udgaven](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 eller nyere](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universel: Apple Silicon og Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 sprog i brugerfladen](https://img.shields.io/badge/languages-35-4C9A2A)
![Ingen telemetri](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signeret med Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licens: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licens)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
**Dansk** ·
[Deutsch](../de/README.md) ·
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

## Indhold

- [Funktioner i overblik](#funktioner-i-overblik)
- [Krav](#krav)
- [Installation](#installation)
- [Kom i gang](#kom-i-gang)
- [AI-agenter](#ai-agenter)
  - [Tidslinjen](#tidslinjen)
  - [Tilladelser og tilstande](#tilladelser-og-tilstande)
  - [Checkpoints og worktrees](#checkpoints-og-worktrees)
- [Editor](#editor)
- [Sprogservere](#sprogservere)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Markdown](#markdown)
- [Diktering](#diktering)
- [Tillid](#tillid)
- [Sandbox](#sandbox)
- [Udseende og tilgængelighed](#udseende-og-tilgængelighed)
- [Sprog](#sprog)
- [Tastaturgenveje](#tastaturgenveje)
- [Privatliv og sikkerhed](#privatliv-og-sikkerhed)
- [Hvor data ligger](#hvor-data-ligger)
- [Kendte begrænsninger](#kendte-begrænsninger)
- [Byg fra kildekoden](#byg-fra-kildekoden)
- [Licens](#licens)

## Funktioner i overblik

- **Alle agenter i én tidslinje.** Claude via sin egen kommando `claude`, Codex, Antigravity og OpenCode via Agent Client Protocol, flere fra ACP-registret og en agent bygget ind i appen til Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama og LM Studio. Svar, værktøjskald, anmodninger om tilladelse, planer, opgaver, underagenter og baggrundsopgaver ser ens ud, uanset hvilken agent der kører.
- **Gennemgå og spol tilbage.** Før hver prompt gemmer Dango et checkpoint. Det, en agent har ændret, kan gennemgås og spoles tilbage for hver besked, og en tråd kan arbejde i sit eget Git-worktree og aflevere sit arbejde med merge eller cherry-pick.
- **Ét vindue pr. mappe.** Arkiver, terminaler, browsersider, AI-tråde, forskelle og eksempler er macOS-faner i det vindue. Indholdsoversigten rummer Arkiver, Søgning, Agenter, Git, Fejlfind og CI.
- **En rigtig kodeeditor.** Syntaksfarver, flere indsætningsmærker, foldning, `.editorconfig`, sprogservere til 17 sprog med fuldførelse, gå til definition, referencer, omdøbning, hurtige rettelser og hierarkier, og forudsigelser, mens du skriver, fra GitHub Copilot eller en lokal model.
- **Debugger, Git og CI indbygget.** Breakpoints og trinvis kørsel til Rust, C, C++, Swift, Go, JavaScript, TypeScript og Python; stage enkelte blokke, branches, stashes, konflikter og en commitgraf; kørsler, jobs og logge fra GitHub Actions og GitLab CI.
- **Forsigtig med dine mapper.** En mappe, du ikke stoler på, starter intet: ingen Git-processer, sprogservere, hooks eller agenter. Destruktive Git-kommandoer tillades aldrig for altid, og commits tjekkes for secrets.
- **Diktering.** Tal ind i editoren og beskedfeltet, online via OpenAI eller Groq eller lokalt med Whisper og Parakeet.
- **Helt igennem native.** AppKit-vinduer, -faner, -menuer og -værktøjslinjer, TextKit 2 til editoren, Metal til terminalen, WebKit kun i browserfanen. Mørk tilstand, Øg kontrast og VoiceOver virker som i enhver Mac-app.
- **På dit sprog.** Brugerfladen findes på 35 sprog: arabisk, catalansk, kinesisk (forenklet), kinesisk (traditionelt), kroatisk, tjekkisk, dansk, hollandsk, engelsk, finsk, fransk, tysk, græsk, hebraisk, hindi, ungarsk, indonesisk, italiensk, japansk, koreansk, malajisk, norsk, polsk, portugisisk (Brasilien), portugisisk (Portugal), rumænsk, russisk, slovakisk, slovensk, spansk, svensk, thai, tyrkisk, ukrainsk og vietnamesisk.

## Krav

- macOS 14 Sonoma eller nyere, på Apple Silicon eller Intel. Dango udvikles og testes på macOS 27.
- Til AI-trådene den agent, du vil bruge; se [AI-agenter](#ai-agenter).
- Til sprogservere og debugadaptere, som Dango installerer, når du beder om det: Go til gopls og `dlv`; Node.js til serverne til TypeScript, Python, JSON, YAML, CSS, HTML og shell og JavaScript-debuggeren; Xcode Command Line Tools til clangd, sourcekit-lsp og `lldb-dap`. rust-analyzer hentes fra dets GitHub-udgivelser.
- Til forudsigelser, mens du skriver, et GitHub Copilot-abonnement (det gratis er nok) eller en fill-in-the-middle-model i Ollama eller LM Studio.

## Installation

**Fra diskbilledet.** Hver udgivelse er et diskbillede, `Dango-<version>.dmg`, med én universel app til Apple Silicon og Intel, signeret med et Developer ID og notariseret af Apple. Åbn det, og træk Dango til mappen Programmer. En Homebrew Cask installerer det samme billede. Der er endnu ikke udgivet nogen version.

**Opdateringer.** Dango fra diskbilledet holder sig selv opdateret med [Sparkle](https://sparkle-project.org). Ved anden start spørger det, om det selv må søge efter opdateringer; indtil du siger ja, sender det ingen forespørgsler. **Dango › Søg efter opdateringer…** søger med det samme. Opdateringer er signeret med appens egen nøgle, som Sparkle tjekker, før det installerer noget. En version, du selv har bygget, har ingen opdateringsfunktion.

**Fra kildekoden.** Se [Byg fra kildekoden](#byg-fra-kildekoden), og flyt derefter `target/release/Dango.app` til `/Applications`.

**Terminalkommandoen.** **Dango › Installer kommandoen “dango”…** skriver et lille script til `~/.local/bin/dango` uden administratorrettigheder; `~/.local/bin` skal være i din `PATH`. Derefter åbner `dango .` den aktuelle mappe og `dango file.rs` et arkiv. Dango erstatter aldrig et arkiv dér, som det ikke selv har skrevet.

## Kom i gang

1. **Åbn en mappe** med ⌘O eller `dango <folder>`. Første gang spørger Dango, om du stoler på den (se [Tillid](#tillid)).
2. **Indholdsoversigten** skifter mellem Arkiver (⌘1), Søgning (⌘2), Agenter (⌘3), Git (⌘4), Fejlfind (⌘5) og CI (⌘6); et højreklik skjuler dem, du ikke bruger, og ⌃⌘S folder indholdsoversigten væk.
3. **Hvert dokument er en fane** i mappens vindue, macOS' egne faner. “+” for enden af fanelinjen og ⌘T tilbyder et nyt arkiv, en terminal, browseren og hver installeret agent, også i sit eget Git-worktree; holder du “+” nede, vises det samme som en menu.
4. **Åbn hurtigt** med ⌘P finder arkiver og med `#` projektets symboler; ⇧⌘F søger og erstatter i projektet.
5. **Tal med en agent** i en AI-fane. ⌘Esc flytter tastaturfokus til dens beskedfelt og tilbage.
6. **Indstillinger** med ⌘, rummer fanerne Generelt, CI, Agenter, Egen agent, Sprogserver, Diktering og Forudsigelse.

Værktøjslinjen rummer tilbage og frem gennem de steder, du har redigeret (⌃- og ⌃⇧-), branchen, Åbn hurtigt og aktivitetsoversigten, der viser alt, hvad der kører. **Åbn ved siden af** i et arkivs kontekstmenu viser et andet dokument ved siden af det første. Efter en genstart eller et nedbrud kommer hvert vindue tilbage med sine faner, og editorer med ikke-arkiveret tekst kommer tilbage med teksten. Billeder, lyd, video, skrifter og Office-dokumenter åbner i en fane via Kvikvisning, PDF'er via PDFKit.

## AI-agenter

| Agent | Sådan kører den | Det har den brug for |
|---|---|---|
| Claude | kommandoen `claude`, talt direkte til over dens stream-json-protokol | `claude` installeret og logget ind; Dango leder i login-shellens `PATH`, i `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` og `/usr/local/bin`. Login sker kun i `claude` selv |
| Codex | `codex-acp` via Agent Client Protocol (ACP) | Node.js og npm; Dango installerer den fastlåste pakke, når du beder om det |
| Antigravity | `agy acp` | din egen `agy` i `PATH`, eller den fastlåste download, installeret når du beder om det |
| OpenCode | `opencode acp` | din egen `opencode` i `PATH`, eller den fastlåste download, installeret når du beder om det |
| Andre ACP-agenter | fra ACP-registret, der kun hentes, når du beder om det | det, registrets post angiver; poster uden kontrolsum installeres først, når du har bekræftet |
| Egen agent | bygget ind i appen, med værktøjer til arkiver, søgning, kommandoer, diagnoser og webanmodninger | en API-nøgle til Anthropic, OpenAI, Google Gemini eller OpenRouter, gemt i nøgleringen; Ollama eller LM Studio på denne Mac uden nøgle; enhver server, der taler OpenAI's eller Anthropics API |

**Indstillinger › Agenter** viser, hvilke agenter denne Mac har, installerer de andre og deres opdateringer og åbner ACP-registret. Agenter, som Dango installerer, er fastlåst til afprøvede versioner og tjekket mod deres offentliggjorte kontrolsummer (npm integrity eller SHA-256); Dango kører ingen `npx`-pakker uden version.

### Tidslinjen

Hver agent arbejder i en AI-fane med den samme tidslinje: svar, værktøjskald med deres output, anmodninger om tilladelse, spørgsmål, planer til godkendelse og kommentarer (⌥⌘K), opgaver, underagenter og baggrundsopgaver. To knapper i fanens top viser de kørende underagenter og baggrundsprocesser. Nævn et arkiv, `@terminal` eller `@browser` for at give agenten indholdet. En grøn prik på fanen viser en session i arbejde, en rød en session, der venter på dig, og en meddelelse fortæller, når den er færdig eller venter, mens du kigger et andet sted. For Claude viser statuslinjen konteksten og forbruget.

### Tilladelser og tilstande

Nye tråde starter i tilstanden “Manuel”: Agenten spørger, før den ændrer arkiver eller kører kommandoer. Til destruktive Git-kommandoer (`push --force`, `reset --hard`, `clean -fd`) tilbyder Dango ingen “Tillad altid”. Den egne agent gemmer sine “Tillad altid”-regler pr. projekt, viser dem i **Indstillinger › Egen agent** og spørger ved destruktive Git-kommandoer hver gang, uanset hvad der er gemt; Claude og ACP-agenterne gemmer den slags regler i deres egen konfiguration. Med “Tillad alt” og uden sandbox viser tråden en permanent advarsel.

### Checkpoints og worktrees

Før hver prompt gemmer Dango et checkpoint som et commit under `refs/ide-checkpoints/` i projektets repository eller i sin egen mappe for et projekt uden Git; en almindelig push eller fetch tager dem aldrig med. Det, en agent har ændret, kan gennemgås og spoles tilbage for hver besked. En tråd kan arbejde i sit eget Git-worktree: Git-panelet viser, hvad den har ændret, og overtager arbejdet med merge eller cherry-pick.

## Editor

- **Syntaksfarver, linjenumre og ændringsmærker** i forhold til det seneste commit, søg og erstat, Åbn hurtigt (⌘P) og søgning i projektet (⇧⌘F).
- **Indrykning** følger arkivet og `.editorconfig`: tabulatorer eller mellemrum, automatisk indrykning, parenteser i par.
- **Flere indsætningsmærker:** ⌥-klik, ⌥-træk for en kolonne, ⌃⇧↑ og ⌃⇧↓, ⌘D for den næste forekomst.
- **Foldning** (⌥⌘← og ⌥⌘→, med ⇧ for alt), slå kommentar til/fra (⌘/), gå til linje (⌃G), synlige mellemrum og linjeombrydning (⌥Z).
- **Arkiver beholder deres tegnkodning**, byte order mark og linjeskift. Et arkiv, der ændres på disken, indlæses igen, når editoren ikke har ikke-arkiveret tekst; ellers tilbyder en bjælke forskellene og en sammenfletning.
- **Forudsigelser, mens du skriver.** Mens du skriver, vises et forslag i gråt, og Tab tager det. Det kommer fra GitHub Copilot eller fra en lokal model i Ollama eller LM Studio, som valgt i **Indstillinger › Forudsigelse**; en lokal model holder din kode på denne Mac.

## Sprogservere

Go, Rust, TypeScript og JavaScript, Python, C, C++ og Objective-C, Swift, shellscripts, JSON, YAML, CSS, HTML, TOML, Markdown og Lua samt dine egne servere fra **Indstillinger › Sprogserver**. Dango installerer en server, når du beder om det, i sin egen værktøjsmappe og bruger et TypeScript-projekts egen `tsc`, når den kan fungere som sprogserver.

Det giver de: diagnoser (⌘' og ⇧⌘' skifter mellem dem), hover (⌃⌘I), fuldførelse (⌥Esc), signaturhjælp, gå til definition (⌃⌘J), typedefinition og implementering, referencer (⌃⇧⌘F), kalde- og typehierarkier (⌃⇧⌘H), omdøbning (⌃⌘E), formatering (⌥⇧F), semantiske farver, foldning og dispositionen (⇧⌘O); typer og parameternavne inde i linjen; hurtige rettelser og refaktoreringer bag en pære (⌥⌘A); projektets symboler med ⌥⌘O.

## Debugger

Breakpoints, også med betingelser og som logpoints, trinvis kørsel, tråde med deres kaldestakke, variabler og overvågede udtryk i inspektøren (⌥⌘I), værdier ved siden af koden, mens programmet er sat på pause, og en fejlfindingskonsol, der evaluerer i den pausede ramme.

| Sprog | Debugadapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` fra Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScript-debuggeren, på Node.js |
| Python | debugpy |

Et projekts `.vscode/launch.json` læses. Menuen Fejlfind bruger Xcodes genveje: ⌘\\ sætter et breakpoint, ⌘Y slår dem alle til eller fra, ⌃⌘Y fortsætter, F6, F7 og F8 træder over, ind og ud, ⌘. stopper.

## Terminal

Terminaler i faner (⌃\`) med login-shellens miljø, tegnet med Metal. ⌘K rydder skærmen. Et indsat eller trukket billede går til en kodeagent, der kører i terminalen, eller indsætter stien til en arkiveret kopi. ⌘-klik åbner links og arkivnavne. En udviklingsserver, som terminalen starter, kan åbnes i browserfanen.

## Browser

En browserfane med adressefelt (⌘L), tilbage og frem (⌘[ og ⌘]), genindlæs (⌘R), søgning, overførsler, der spørger først, og webinspektøren. `@browser` giver siden til en agent, mærket som tekst udefra.

## Git

Git-panelet stager og kasserer arkiver eller enkelte blokke, laver commits, committer og pusher eller synkroniserer, skifter og opretter branches, gemmer stashes, løser merge-konflikter og tegner en graf over de seneste commits. Blame viser, hvem der sidst ændrede linjen under markøren, eller en kolonne for hele arkivet. En knap skriver commitbeskeden med AI; forskellen sendes først, når gitleaks ikke har fundet et secret i den, og hvert commit tjekkes også for secrets.

## CI

GitHub Actions og GitLab CI, også GitHub Enterprise Server og selvadministreret GitLab:

- kørsler, jobs og logge for den aktuelle branch;
- genkør mislykkede jobs, et enkelt job eller, på GitHub, en hel kørsel; start manuelle jobs eller godkend ventende deployments; start en ny pipeline eller et workflow, der må startes manuelt; annuller;
- de åbne pull requests eller merge requests og issues, der åbner i browserfanen;
- et mislykket jobs log givet til en agent, mærket som tekst udefra.

Tokenet kommer fra nøgleringen, `gh` eller `glab`, login-shellens miljø eller er et, du indtaster i **Indstillinger › CI**.

## Markdown

- **Eksempel.** Et Markdown-arkiv åbner med sit eksempel ved siden af; **Oversigt › Vis/skjul eksempel ved siden af** (⇧⌘V) lukker og åbner det, og **Indstillinger › Generelt** slår det automatiske eksempel fra. Eksemplet tegner formler, diagrammer i Mermaid, Graphviz og PlantUML og mappens billeder; billeder fra nettet indlæses kun i en mappe, du stoler på, eller når du beder om dem i eksemplet.
- **Menuen Format:** fed (⌘B), kursiv (⌘I), gennemstreget (⇧⌘X), kode (⌥⇧⌘C), link (⇧⌘K), overskrift 1–3 (⌥⌘1–⌥⌘3), punktopstillinger og nummererede lister, opgaver, citater og kodeblokke. Hver kommando slår til og fra, på markeringen eller linjerne, og er ét fortryd-trin.
- **Indsæt.** Tekst fra Word eller en webside kommer ind som Markdown, dens billeder som arkiver ved siden af dokumentet; ⌥⇧⌘V indsætter ren tekst.
- **Eksport.** **Arkiv › Eksporter** skriver PDF, et enkelt HTML-arkiv med billederne indeni eller et Word-dokument. Billeder fra nettet kommer kun med fra en mappe, du stoler på.
- **Åbn i Splat.** Hvor [Splat](https://github.com/sventmueller/splat) er installeret, åbner **Arkiv › Åbn i Splat** og kontekstmenuerne i arkivlisten og fanerne dokumentet dér, efter at det er arkiveret.

## Diktering

Dikter ind i en AI-fanes beskedfelt eller editoren: med mikrofonknappen dér, **Rediger › Start diktering** (⌃⌥D) eller fn-tasten (hold nede for at optage, tryk to gange for at blive ved med at optage). Talegenkendelsen er [Lowspeaks](https://github.com/sventmueller/lowspeak); Lowspeak selv behøver ikke at være installeret.

- **Indstillinger › Diktering** indstiller tilstand og model (online med OpenAI eller Groq, lokalt med Whisper eller Parakeet), sproget, efterbehandlingen, mikrofonen og dit eget ordforråd.
- **Lokale modeller** hentes fra Hugging Face, når du vælger en, med status og mulighed for at annullere. De lander i de mapper, Lowspeak bruger, så hver app bruger det, den anden har hentet. En lokal model holder optagelsen på denne Mac.
- **Onlinetransskription** kræver en nøgle til OpenAI eller Groq, som Dango gemmer i sit eget emne i nøgleringen.
- **Fra Lowspeak.** Er Lowspeak indstillet anderledes end standarden, tilbyder fanen Diktering én gang at overtage dens indstillinger og på opfordring dens nøgler; macOS spørger, før Dango læser en (vælg “Tillad”, ikke “Tillad altid”).
- **Tilladelser.** macOS spørger om mikrofonen ved første diktering. fn-tasten virker, mens Dango er den aktive app, og Lowspeak ikke kører; den kræver tilladelsen til tilgængelighed, som Dango først beder om efter den første diktering, aldrig ved start.

## Tillid

En mappe kan få programmer til at køre, så snart noget startes i den: Git-hooks og repository-indstillinger som `core.fsmonitor`, sprogservere fra `node_modules`, agent-hooks, `.mcp.json`. Første gang du åbner en mappe, spørger Dango, om du stoler på den.

- **Betroet:** Git, sprogservere, debuggere, agenter og CI kører for denne mappe og alt under den.
- **Begrænset:** Du kan læse og redigere; ændringsmærkerne i editoren kommer fra at læse `.git` direkte. Intet, som mappen konfigurerer, startes. Panelerne til Git, CI og AI siger det og tilbyder at stole på mappen.

Tekst, der ikke kommer fra dig, kan indeholde instruktioner, der skal styre en agent (prompt injection). Outputtet fra en terminal (`@terminal`), en browserside (`@browser`), en CI-log givet til en agent, et arkiv uden for projektet og længere tekst indsat fra en webside sendes derfor mærket: Chippen siger “Fremmed indhold”, og agenten får teksten med besked om, at det er upålidelige data, hvis instruktioner den ikke må følge. Den egne agents websider kommer med samme besked. Indsat tekst regnes for at komme fra en webside, når browseren angiver det på udklipsholderen, som Safari, Chrome og Firefox gør; tekst fra andre steder og en kort sætning på få ord forbliver din egen.

## Sandbox

Dango selv kører uden App Sandbox, fordi alle programmer, det starter, ville arve den: Terminaler, Git, sprogservere og agenter kunne ikke fungere. I stedet har hver AI-tråd en sandbox-kontakt, der som standard er slået fra. Hvad den begrænser, afhænger af agenten, og kontaktens værktøjstip siger det:

| Agent | Med sandboxen slået til |
|---|---|
| Claude | shellkommandoer kører i Claudes egen sandbox; dens værktøjer til arkiver og web gør ikke |
| Codex | kommandoer og ændringer skriver kun i projektet, uden netværk |
| Egen agent | arkiver, webanmodninger og kommandoer holder sig inden for projektet og de tilladte mapper, uden netværk (`sandbox-exec`) |
| Antigravity, OpenCode | ingen sandbox, som Dango kan slå til |

## Udseende og tilgængelighed

- **Mørk tilstand** følger systemet; **Øg kontrast** og **Reducer bevægelse** respekteres.
- **VoiceOver** navngiver indholdsoversigten, fanerne, kortene i AI-tidslinjen, rækkerne i Git og CI, debuggeren og terminalens tekst. Breakpoints skelnes aldrig alene ved farve.
- **Tastatur.** Hver kommando har en genvej, og dem alle kan ændres i `keybindings.json`; ⌥⌘\` går til næste panel.
- **Højre mod venstre.** På arabisk og hebraisk spejles vinduet, mens kodevisninger forbliver fra venstre mod højre.

Hvad Dango tilføjer oven på AppKit, område for område, står i [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Sprog

Brugerfladen er oversat til 35 sprog: arabisk, catalansk, kinesisk (forenklet), kinesisk (traditionelt), kroatisk, tjekkisk, dansk, hollandsk, engelsk, finsk, fransk, tysk, græsk, hebraisk, hindi, ungarsk, indonesisk, italiensk, japansk, koreansk, malajisk, norsk, polsk, portugisisk (Brasilien), portugisisk (Portugal), rumænsk, russisk, slovakisk, slovensk, spansk, svensk, thai, tyrkisk, ukrainsk og vietnamesisk.

Dango følger sprogrækkefølgen i **Systemindstillinger › Generelt › Sprog & område**; dér kan du også vælge et sprog til Dango alene under **Apps**. Findes der ingen oversættelse til dit sprog, bruger det engelsk. Termerne følger én ordliste, [docs/glossary.md](../../../docs/glossary.md), og Apples egne oversættelser, så Dango læses som resten af din Mac.

## Tastaturgenveje

| Handling | Genvej |
|---|---|
| Åbn mappe, nyt vindue, luk vindue | ⌘O, ⇧⌘N, ⇧⌘W |
| Nyt arkiv, arkiver, arkiver som | ⌘N, ⌘S, ⇧⌘S |
| Ny fane, luk fane, næste og forrige fane | ⌘T, ⌘W, ⇧⌘] og ⇧⌘[ |
| Åbn hurtigt, søg i projektet | ⌘P, ⇧⌘F |
| Arkiver, Søgning, Agenter, Git, Fejlfind, CI | ⌘1 til ⌘6 |
| Vis eller skjul indholdsoversigten, næste panel | ⌃⌘S, ⌥⌘\` |
| Tilbage og frem gennem redigerede steder | ⌃- og ⌃⇧- |
| Slå kommentar til/fra, gå til linje | ⌘/, ⌃G |
| Næste forekomst, indsætningsmærke over og under | ⌘D, ⌃⇧↑ og ⌃⇧↓ |
| Fold, fold ud, fold alt, fold alt ud | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Linjeombrydning, inspektør | ⌥Z, ⌥⌘I |
| Fuldførelse, hover, gå til definition | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referencer, kaldehierarki, omdøb | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboler i arkivet, i projektet | ⇧⌘O, ⌥⌘O |
| Hurtig rettelse, formatér | ⌥⌘A, ⌥⇧F |
| Næste og forrige problem | ⌘' og ⇧⌘' |
| Markdown-eksempel, indsæt som ren tekst | ⇧⌘V, ⌥⇧⌘V |
| Fed, kursiv, gennemstreget, kode, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Overskrift 1 til 3 | ⌥⌘1 til ⌥⌘3 |
| Ny terminal, ryd terminal | ⌃\`, ⌘K |
| Fokus på agentens beskedfelt, kommentér en plan | ⌘Esc, ⌥⌘K |
| Adressefelt, genindlæs, tilbage, frem (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Start eller stop diktering | ⌃⌥D |
| Breakpoint, alle breakpoints til eller fra | ⌘\\, ⌘Y |
| Fortsæt, træd over, ind, ud, stop | ⌃⌘Y, F6, F7, F8, ⌘. |
| Indstillinger | ⌘, |

`keybindings.json` i appens datamappe (se [Hvor data ligger](#hvor-data-ligger)) ændrer enhver af dem.

## Privatliv og sikkerhed

- **Ingen telemetri,** ingen analyse, ingen nedbrudsrapporter sendt nogen steder hen. Fejl går til systemloggen (Konsol, under appens navn).
- **Netværket kun til det, du starter:** de agenter, du bruger, hver med sin egen tjeneste; ACP-registret og overførsler af agenter, sprogservere, debugadaptere og dikteringsmodeller, når du beder om dem; de CI-værter, du logger ind på; forudsigelser fra Copilot, hvis du har valgt det; den egne agents udbydere; onlinetransskription, hvis du har valgt det; de sider, du åbner i browserfanen; og søgningen efter opdateringer, når du har tilladt den.
- **Kode forlader kun din Mac**, hvor du har valgt en tjeneste til det: en agent, Copilots forudsigelser, en commitbesked skrevet med AI. Forskellen til en commitbesked sendes først, når gitleaks ikke har fundet et secret i den.
- **Nøgler og tokens** bliver i macOS-nøgleringen, aldrig i et indstillingsarkiv. Andre apps' nøgler, som Lowspeaks, læses kun, når du siger ja, og kun én gang.
- **Fastlåste overførsler.** Agenter, sprogservere og debugadaptere, som Dango installerer, er fastlåst til afprøvede versioner og tjekket mod deres kontrolsummer.
- **Kodesignering** med Hardened Runtime; opdateringer installeres kun med appens egen signatur.

## Hvor data ligger

`<id>` er bundle-id'et, `local.agentide` for udgivelsen (`local.agentide.debug` for debug-builds). Det indeholder ikke det viste navn, så en omdøbning beholder alle data.

| Sted | Hvad |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (AI-panelets liste), `agent-threads/` (den egne agents samtaler), `layouts/` (faner pr. projekt), `recovery/` (ikke-arkiverede editorer efter et nedbrud), `checkpoints/` (til mapper uden Git), `worktrees/`, `agents/` og `tools/` (installerede agenter, sprogservere og debugadaptere), `task-output/` |
| `~/Library/Caches/<id>/` | indsatte billeder, scripts til shell-integration, ACP-registret; kan slettes |
| `~/Library/Logs/<id>/dictation/` | dikteringens logarkiver |
| `~/Library/Application Support/Lowspeak` og `FluidAudio` | lokale dikteringsmodeller, delt med Lowspeak |
| `~/Library/WebKit/<id>/` | cookies og webstedsdata fra browserfanen |
| `<project>/.dango/settings.json` | projektets egne indstillinger |
| projektets Git-repository | checkpoints, som commits under `refs/ide-checkpoints/`; en almindelig push eller fetch tager dem aldrig med |
| nøglering, tjeneste `<id>` | API-nøgler til den egne agent og til diktering, CI-tokens |
| `~/.local/bin/dango` | terminalkommandoen, hvis du har installeret den |

Claude gemmer sine egne sessioner under `~/.claude/projects`; Dango læser dem for at genoptage en session, men ændrer dem ikke.

## Kendte begrænsninger

- **Ingen App Sandbox** til selve appen, da terminaler, Git, sprogservere og agenter ville arve den; en mappe skal være betroet, før noget, den konfigurerer, kører. Antigravity og OpenCode har ingen sandbox, som Dango kan slå til.
- **Claudes forbrugstal og opdeling af konteksten** bygger på dele af protokollen, der er markeret som eksperimentelle, og skjules, når de mangler.
- **At spole samtalen tilbage** virker kun for Claude; for de andre agenter spoles kun koden tilbage.
- **Login til CI via browseren** er slået fra; brug `gh`, `glab`, et token eller miljøet.
- **Gemini CLI tilbydes ikke:** Google har stoppet login med en personlig konto og henviser til Antigravity. Tidligere Gemini-tråde kan stadig læses.

## Byg fra kildekoden

Du skal bruge:

- macOS 14 eller nyere med Xcode Command Line Tools (`xcode-select --install`); Xcode selv er ikke nødvendigt.
- Rust, som `rust-toolchain.toml` angiver det. Homebrews `rust` bygger til denne Macs processor; det universelle build af en udgivelse kræver rustup med målene `aarch64-apple-darwin` og `x86_64-apple-darwin`.
- Checkouts af [splat](https://github.com/sventmueller/splat) og [lowspeak](https://github.com/sventmueller/lowspeak) ved siden af dette (`../splat`, `../lowspeak`), begge på deres branch `main`; workspacet tager crates fra dem via sti.
- cmake til hvert build med diktering (`make run`, `make build`, `make release`), som bygger whisper.cpp. `make check` bygger uden.

| Kommando | Hvad den gør |
|---|---|
| `make run` | bygger debug-appen (`target/debug/Dango Debug.app`) og åbner den |
| `make build` | bygger release-appen (`target/release/Dango.app`), signeret med et lokalt certifikat, som det første build opretter i login-nøgleringen; hæver buildnummeret i `VERSION` |
| `make lint` | formatering og clippy; advarsler får den til at fejle |
| `make check` | `lint`, derefter alle tests; kræver ingen nøglering, intet vindue og intet netværk |
| `make test` | `check`, derefter selvtesten `probe` og tilgængelighedstjekket, fra venstre mod højre og fra højre mod venstre, i den rigtige app |
| `make perf` | tidsbudgetterne i et release-build; giver kun mening på en Mac, der ellers er i ro |
| `make release` | en prøvekørsel af udgivelsen til `target/release-dry-run/`, signeret med det lokale certifikat; sender intet nogen steder hen |
| `make release-notarized` | `check`, derefter selve udgivelsen til `dist/`: Developer ID-signatur, notarisering, opdateringsfeedet signeret med appens egen nøgle; hæver buildnummeret |

Debug og release er separate apps med deres egne indstillinger og data. Bygning, test, selvtestene, udgivelsen og de regler, koden følger, er beskrevet i [docs/development.md](../../../docs/development.md), beslutningerne bag i [docs/plan.md](../../../docs/plan.md). Udgivelsesnoterne er [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), oversat ved siden af hver README i `assets/readme/<language>/`.

## Licens

Dango er licenseret under en af følgende

- Apache-licensen, version 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-licensen ([LICENSE-MIT](../../../LICENSE-MIT))

efter eget valg.

Medmindre du udtrykkeligt angiver andet, skal ethvert bidrag, som du bevidst indsender til optagelse i Dango, som defineret i Apache-2.0-licensen, have dobbeltlicens som ovenfor, uden yderligere vilkår eller betingelser.
