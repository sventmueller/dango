<p><img src="../../readme-icon.png" alt="Dango-symbool" width="168" height="168"></p>

# Dango

**De native macOS-editor voor werken met coding agents.**

Dango brengt de code, de agents die hem veranderen en alles daaromheen samen in één venster per map: een editor met taalservers en voorspellingen tijdens het typen, terminals, een browser, Git, CI, een debugger en AI-threads met Claude, Codex, Antigravity, OpenCode of een agent die in de app is ingebouwd. Elke agent werkt in dezelfde tijdlijn, vraagt het voordat hij bestanden wijzigt of commando's uitvoert, en laat vóór elke prompt een checkpoint achter, zodat je per bericht kunt nakijken en terugdraaien wat hij heeft gedaan. Niets wat een map instelt, draait voordat je die map vertrouwt, en tekst van buitenaf bereikt een agent gemarkeerd als niet te vertrouwen. Dicteer in de editor en in het berichtveld; de spraakherkenning kan op je Mac blijven. Dango is geschreven in Rust op echte AppKit-bedieningselementen, zonder Electron, spreekt 35 talen en heeft geen telemetrie.

**Nog niet uitgebracht.** De eerste release wordt een schijfkopie, ondertekend met een Developer ID en genotariseerd door Apple, voor macOS 14 of nieuwer op Apple silicon en Intel. Tot die tijd [bouw je Dango vanuit de broncode](#bouwen-vanuit-de-broncode).

**Een fout gevonden?** [Open een issue](https://github.com/sventmueller/dango/issues/new) en vermeld met welke agent en in wat voor project het gebeurde.

[![Rust, editie 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 of nieuwer](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universeel: Apple silicon en Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 talen voor de interface](https://img.shields.io/badge/languages-35-4C9A2A)
![Geen telemetrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Ondertekend met Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licentie: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licentie)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
**Nederlands** ·
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

## Inhoud

- [Functies in één oogopslag](#functies-in-één-oogopslag)
- [Vereisten](#vereisten)
- [Installatie](#installatie)
- [Aan de slag](#aan-de-slag)
- [AI-agents](#ai-agents)
  - [De tijdlijn](#de-tijdlijn)
  - [Toestemmingen en modi](#toestemmingen-en-modi)
  - [Checkpoints en worktrees](#checkpoints-en-worktrees)
- [Editor](#editor)
- [Taalservers](#taalservers)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Markdown](#markdown)
- [Dicteren](#dicteren)
- [Vertrouwen](#vertrouwen)
- [Sandbox](#sandbox)
- [Weergave en toegankelijkheid](#weergave-en-toegankelijkheid)
- [Talen](#talen)
- [Toetscombinaties](#toetscombinaties)
- [Privacy en beveiliging](#privacy-en-beveiliging)
- [Waar gegevens staan](#waar-gegevens-staan)
- [Bekende beperkingen](#bekende-beperkingen)
- [Bouwen vanuit de broncode](#bouwen-vanuit-de-broncode)
- [Licentie](#licentie)

## Functies in één oogopslag

- **Elke agent in één tijdlijn.** Claude via zijn eigen commando `claude`, Codex, Antigravity en OpenCode via het Agent Client Protocol, meer uit het ACP-register, en een in de app ingebouwde agent voor Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama en LM Studio. Antwoorden, toolaanroepen, toestemmingsverzoeken, plannen, to-do's, subagents en achtergrondtaken zien er hetzelfde uit, welke agent er ook draait.
- **Nakijken en terugdraaien.** Vóór elke prompt bewaart Dango een checkpoint. Wat een agent heeft veranderd, kun je per bericht nakijken en terugdraaien, en een thread kan in een eigen Git-worktree werken en zijn werk overdragen via samenvoegen of cherry-pick.
- **Eén venster per map.** Bestanden, terminals, browserpagina's, AI-threads, diffs en voorvertoningen zijn macOS-tabbladen van dat venster. De navigatiekolom bevat Bestanden, Zoeken, Agents, Git, Debug en CI.
- **Een echte code-editor.** Syntaxkleuren, meerdere invoegpunten, invouwen, `.editorconfig`, taalservers voor 17 talen met aanvulling, ga naar definitie, verwijzingen, hernoemen, snelle oplossingen en hiërarchieën, en voorspellingen tijdens het typen van GitHub Copilot of een lokaal model.
- **Debugger, Git en CI ingebouwd.** Breekpunten en stapsgewijs uitvoeren voor Rust, C, C++, Swift, Go, JavaScript, TypeScript en Python; losse hunks stagen, branches, stashes, conflicten en een commitgrafiek; runs, jobs en logs uit GitHub Actions en GitLab CI.
- **Voorzichtig met je mappen.** Een map die je niet vertrouwt, start niets: geen Git-processen, taalservers, hooks of agents. Destructieve Git-commando's worden nooit voorgoed toegestaan, en commits worden op geheimen gecontroleerd.
- **Dicteren.** Spreek in de editor en het berichtveld, online via OpenAI of Groq of lokaal met Whisper en Parakeet.
- **Native tot in de kern.** AppKit-vensters, -tabbladen, -menu's en -knoppenbalken, TextKit 2 voor de editor, Metal voor de terminal, WebKit alleen in het browsertabblad. Donkere modus, Verhoog contrast en VoiceOver werken zoals in elke Mac-app.
- **In jouw taal.** De interface is er in 35 talen: Arabisch, Catalaans, Chinees (vereenvoudigd), Chinees (traditioneel), Kroatisch, Tsjechisch, Deens, Nederlands, Engels, Fins, Frans, Duits, Grieks, Hebreeuws, Hindi, Hongaars, Indonesisch, Italiaans, Japans, Koreaans, Maleis, Noors, Pools, Portugees (Brazilië), Portugees (Portugal), Roemeens, Russisch, Slowaaks, Sloveens, Spaans, Zweeds, Thai, Turks, Oekraïens en Vietnamees.

## Vereisten

- macOS 14 Sonoma of nieuwer, op Apple silicon of Intel. Dango wordt ontwikkeld en getest op macOS 27.
- Voor de AI-threads de agent die je wilt gebruiken; zie [AI-agents](#ai-agents).
- Voor taalservers en debug-adapters, die Dango installeert als je erom vraagt: Go voor gopls en `dlv`; Node.js voor de servers voor TypeScript, Python, JSON, YAML, CSS, HTML en shell en de JavaScript-debugger; de Xcode Command Line Tools voor clangd, sourcekit-lsp en `lldb-dap`. rust-analyzer wordt gedownload van zijn GitHub-releases.
- Voor voorspellingen tijdens het typen een GitHub Copilot-abonnement (het gratis abonnement volstaat), of een fill-in-the-middle-model in Ollama of LM Studio.

## Installatie

**Van de schijfkopie.** Elke release is een schijfkopie, `Dango-<version>.dmg`, met één universele app voor Apple silicon en Intel, ondertekend met een Developer ID en genotariseerd door Apple. Open hem en sleep Dango naar de map 'Apps'. Een Homebrew Cask installeert dezelfde schijfkopie. Er is nog geen release uitgebracht.

**Updates.** Dango van de schijfkopie houdt zichzelf bijgewerkt met [Sparkle](https://sparkle-project.org). Bij de tweede start vraagt het of het zelf naar updates mag zoeken; tot je instemt, doet het geen enkel verzoek. **Dango › Zoek naar updates…** zoekt meteen. Updates zijn ondertekend met de eigen sleutel van de app, die Sparkle controleert voordat het iets installeert. Een zelf gebouwde versie heeft geen updatefunctie.

**Vanuit de broncode.** Zie [Bouwen vanuit de broncode](#bouwen-vanuit-de-broncode) en verplaats daarna `target/release/Dango.app` naar `/Applications`.

**Het terminalcommando.** **Dango › Installeer 'dango'-commando…** schrijft een klein script naar `~/.local/bin/dango`, zonder beheerdersrechten; `~/.local/bin` moet in je `PATH` staan. Daarna opent `dango .` de huidige map en `dango file.rs` een bestand. Dango vervangt daar nooit een bestand dat het niet zelf heeft geschreven.

## Aan de slag

1. **Open een map** met ⌘O of `dango <folder>`. De eerste keer vraagt Dango of je de map vertrouwt (zie [Vertrouwen](#vertrouwen)).
2. **De navigatiekolom** wisselt tussen Bestanden (⌘1), Zoeken (⌘2), Agents (⌘3), Git (⌘4), Debug (⌘5) en CI (⌘6); met de rechtermuisknop verberg je wat je niet gebruikt, en ⌃⌘S klapt de navigatiekolom weg.
3. **Elk document is een tabblad** van het venster van de map, de eigen tabbladen van macOS. De '+' aan het einde van de tabbladbalk en ⌘T bieden een nieuw bestand, een terminal, de browser en elke geïnstalleerde agent aan, ook in een eigen Git-worktree; houd je de '+' ingedrukt, dan verschijnt hetzelfde als menu.
4. **Open snel** met ⌘P vindt bestanden, en met `#` de symbolen van het project; ⇧⌘F zoekt en vervangt in het project.
5. **Praat met een agent** in een AI-tabblad. ⌘Esc zet de toetsenbordfocus in zijn berichtveld en weer terug.
6. **Instellingen** met ⌘, bevatten de tabbladen Algemeen, CI, Agents, Ingebouwde agent, Taalserver, Dicteerfunctie en Voorspelling.

In de knoppenbalk staan terug en vooruit langs de plekken die je hebt bewerkt (⌃- en ⌃⇧-), de branch, Open snel en het activiteitenoverzicht, dat alles toont wat draait. **Open ernaast** in het contextuele menu van een bestand toont een tweede document naast het eerste. Na een herstart of een crash komt elk venster terug met zijn tabbladen, en editors met niet-bewaarde tekst komen terug met die tekst. Afbeeldingen, audio, video, lettertypen en Office-documenten openen in een tabblad via Snelle weergave, pdf's via PDFKit.

## AI-agents

| Agent | Hoe hij draait | Wat hij nodig heeft |
|---|---|---|
| Claude | het commando `claude`, rechtstreeks aangesproken via zijn stream-json-protocol | `claude` geïnstalleerd en ingelogd; Dango zoekt in het `PATH` van de login-shell, in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` en `/usr/local/bin`. Inloggen gebeurt alleen in `claude` zelf |
| Codex | `codex-acp` via het Agent Client Protocol (ACP) | Node.js en npm; Dango installeert het vastgezette pakket als je erom vraagt |
| Antigravity | `agy acp` | je eigen `agy` in het `PATH`, of de vastgezette download, geïnstalleerd als je erom vraagt |
| OpenCode | `opencode acp` | je eigen `opencode` in het `PATH`, of de vastgezette download, geïnstalleerd als je erom vraagt |
| Andere ACP-agents | uit het ACP-register, dat alleen wordt opgehaald als je erom vraagt | wat het item in het register noemt; items zonder checksum worden pas geïnstalleerd nadat je hebt bevestigd |
| Ingebouwde agent | in de app ingebouwd, met tools voor bestanden, zoeken, commando's, diagnoses en webverzoeken | een API-sleutel voor Anthropic, OpenAI, Google Gemini of OpenRouter, bewaard in de sleutelhanger; Ollama of LM Studio op deze Mac zonder sleutel; elke server die de API van OpenAI of Anthropic spreekt |

**Instellingen › Agents** toont welke agents deze Mac heeft, installeert de andere en hun updates, en opent het ACP-register. Agents die Dango installeert, zijn vastgezet op geteste versies en gecontroleerd tegen hun gepubliceerde checksums (npm integrity of SHA-256); Dango voert geen `npx`-pakketten zonder versie uit.

### De tijdlijn

Elke agent werkt in een AI-tabblad met dezelfde tijdlijn: antwoorden, toolaanroepen met hun uitvoer, toestemmingsverzoeken, vragen, plannen om goed te keuren en van commentaar te voorzien (⌥⌘K), to-do's, subagents en achtergrondtaken. Twee knoppen in de kop van het tabblad tonen de lopende subagents en achtergrondprocessen. Noem een bestand, `@terminal` of `@browser` om de agent de inhoud ervan mee te geven. Een groene stip op het tabblad toont een sessie die bezig is, een rode een sessie die op je wacht, en een melding laat weten wanneer een sessie klaar is of wacht terwijl je ergens anders kijkt. Bij Claude toont de statusregel de context en het gebruik.

### Toestemmingen en modi

Nieuwe threads beginnen in de modus 'Handmatig': de agent vraagt het voordat hij bestanden wijzigt of commando's uitvoert. Voor destructieve Git-commando's (`push --force`, `reset --hard`, `clean -fd`) biedt Dango geen 'Sta altijd toe' aan. De ingebouwde agent bewaart zijn 'Sta altijd toe'-regels per project, toont ze in **Instellingen › Ingebouwde agent** en vraagt bij destructieve Git-commando's elke keer opnieuw, wat er ook is opgeslagen; Claude en de ACP-agents bewaren zulke regels in hun eigen configuratie. Met 'Sta alles toe' en zonder sandbox toont de thread een blijvende waarschuwing.

### Checkpoints en worktrees

Vóór elke prompt bewaart Dango een checkpoint, als commit onder `refs/ide-checkpoints/` in de repository van het project, of in een eigen map voor een project zonder Git; een gewone push of fetch neemt ze nooit mee. Wat een agent heeft veranderd, kun je per bericht nakijken en terugdraaien. Een thread kan in een eigen Git-worktree werken: het Git-paneel toont wat hij heeft veranderd en neemt het werk over via samenvoegen of cherry-pick.

## Editor

- **Syntaxkleuren, regelnummers en wijzigingsmarkeringen** ten opzichte van de laatste commit, zoeken en vervangen, Open snel (⌘P) en zoeken in het project (⇧⌘F).
- **Inspringing** volgt het bestand en `.editorconfig`: tabs of spaties, automatisch inspringen, haakjesparen.
- **Meerdere invoegpunten:** ⌥-klik, ⌥-slepen voor een kolom, ⌃⇧↑ en ⌃⇧↓, ⌘D voor de volgende plek waar het voorkomt.
- **Invouwen** (⌥⌘← en ⌥⌘→, met ⇧ voor alles), commentaar aan/uit (⌘/), ga naar regel (⌃G), zichtbare witruimte en regelafbreking (⌥Z).
- **Bestanden behouden hun codering**, byte order mark en regeleinden. Een bestand dat op schijf verandert, wordt opnieuw geladen als de editor geen niet-bewaarde tekst heeft; anders biedt een balk de verschillen en samenvoegen aan.
- **Voorspellingen tijdens het typen.** Terwijl je typt, verschijnt een suggestie in grijs, en Tab neemt hem over. Hij komt van GitHub Copilot of van een lokaal model in Ollama of LM Studio, zoals gekozen in **Instellingen › Voorspelling**; een lokaal model houdt je code op deze Mac.

## Taalservers

Go, Rust, TypeScript en JavaScript, Python, C, C++ en Objective-C, Swift, shellscripts, JSON, YAML, CSS, HTML, TOML, Markdown en Lua, en eigen servers via **Instellingen › Taalserver**. Dango installeert een server als je erom vraagt, in zijn eigen toolsmap, en gebruikt de eigen `tsc` van een TypeScript-project als die als taalserver kan dienen.

Wat ze bieden: diagnoses (⌘' en ⇧⌘' gaan ertussen heen en weer), hover (⌃⌘I), aanvulling (⌥Esc), hulp bij signaturen, ga naar definitie (⌃⌘J), typedefinitie en implementatie, verwijzingen (⌃⇧⌘F), aanroep- en typehiërarchieën (⌃⇧⌘H), hernoemen (⌃⌘E), opmaken (⌥⇧F), semantische kleuren, invouwen en de structuur (⇧⌘O); typen en parameternamen in de regel; snelle oplossingen en refactorings achter een lampje (⌥⌘A); de symbolen van het project met ⌥⌘O.

## Debugger

Breekpunten, ook met voorwaarden en als logpunten, stapsgewijs uitvoeren, threads met hun aanroepstapels, variabelen en bewaakte expressies in de inspector (⌥⌘I), waarden naast de code zolang het programma gepauzeerd is, en een debugconsole die evalueert in het gepauzeerde frame.

| Taal | Debug-adapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` uit de Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | de JavaScript-debugger, op Node.js |
| Python | debugpy |

De `.vscode/launch.json` van een project wordt gelezen. Het menu 'Debug' gebruikt de toetscombinaties van Xcode: ⌘\\ zet een breekpunt, ⌘Y zet ze allemaal aan of uit, ⌃⌘Y gaat verder, F6, F7 en F8 stappen over, in en uit, ⌘. stopt.

## Terminal

Terminals in tabbladen (⌃\`) met de omgeving van de login-shell, getekend met Metal. ⌘K wist het scherm. Een geplakte of erin gesleepte afbeelding gaat naar een coding agent die in de terminal draait, of plakt het pad van een bewaarde kopie. ⌘-klik opent links en bestandsnamen. Een ontwikkelserver die de terminal start, kun je openen in het browsertabblad.

## Browser

Een browsertabblad met adresveld (⌘L), terug en vooruit (⌘[ en ⌘]), opnieuw laden (⌘R), zoeken, downloads die het eerst vragen, en de webinspector. `@browser` geeft de pagina mee aan een agent, gemarkeerd als tekst van buitenaf.

## Git

Het Git-paneel staget en verwerpt bestanden of losse hunks, maakt commits, commit en pusht of synchroniseert, wisselt en maakt branches, bewaart stashes, lost samenvoegconflicten op en tekent een grafiek van de recente commits. Blame toont wie de regel onder de cursor het laatst heeft gewijzigd, of een kolom voor het hele bestand. Een knop schrijft het commitbericht met AI; de diff gaat pas de deur uit nadat gitleaks er geen geheim in heeft gevonden, en elke commit wordt ook op geheimen gecontroleerd.

## CI

GitHub Actions en GitLab CI, ook GitHub Enterprise Server en zelfbeheerde GitLab:

- runs, jobs en logs van de huidige branch;
- mislukte jobs, één job of, op GitHub, een hele run opnieuw starten; handmatige jobs starten of wachtende deployments goedkeuren; een nieuwe pipeline of een workflow die met de hand gestart mag worden, starten; annuleren;
- de open pull requests of merge requests en de issues, die openen in het browsertabblad;
- het log van een mislukte job, doorgegeven aan een agent, gemarkeerd als tekst van buitenaf.

Het token komt uit de sleutelhanger, `gh` of `glab`, de omgeving van de login-shell, of is er een die je invoert in **Instellingen › CI**.

## Markdown

- **Voorvertoning.** Een Markdown-bestand opent met de voorvertoning ernaast; **Weergave › Toon/verberg voorvertoning ernaast** (⇧⌘V) sluit en opent hem, en **Instellingen › Algemeen** zet de automatische voorvertoning uit. De voorvertoning tekent formules, diagrammen in Mermaid, Graphviz en PlantUML en de afbeeldingen van de map; afbeeldingen van het web laden alleen in een vertrouwde map of nadat je er in de voorvertoning om vraagt.
- **Menu 'Opmaak':** vet (⌘B), cursief (⌘I), doorgehaald (⇧⌘X), code (⌥⇧⌘C), link (⇧⌘K), koppen 1–3 (⌥⌘1–⌥⌘3), lijsten met opsommingstekens en genummerde lijsten, taken, citaten en codeblokken. Elk commando schakelt om, op de selectie of de regels, en is één stap om ongedaan te maken.
- **Plakken.** Tekst uit Word of een webpagina komt binnen als Markdown, de afbeeldingen ervan als bestanden naast het document; ⌥⇧⌘V plakt platte tekst.
- **Exporteren.** **Archief › Exporteer** schrijft pdf, één HTML-bestand met de afbeeldingen erin, of een Word-document. Afbeeldingen van het web gaan alleen mee vanuit een vertrouwde map.
- **Open in Splat.** Als [Splat](https://github.com/sventmueller/splat) is geïnstalleerd, openen **Archief › Open in Splat** en de contextuele menu's van de bestandenlijst en de tabbladen het document daar, nadat het is bewaard.

## Dicteren

Dicteer in het berichtveld van een AI-tabblad of in de editor: met de microfoonknop daar, **Wijzig › Start dicteren** (⌃⌥D), of de fn-toets (ingedrukt houden om op te nemen, twee keer tikken om door te blijven opnemen). De spraakherkenning is die van [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak zelf hoeft niet geïnstalleerd te zijn.

- **Instellingen › Dicteerfunctie** stelt de modus en het model in (online met OpenAI of Groq, lokaal met Whisper of Parakeet), de taal, de nabewerking, de microfoon en je eigen woordenlijst.
- **Lokale modellen** worden van Hugging Face gedownload als je er een kiest, met voortgang en een manier om te annuleren. Ze komen in de mappen die Lowspeak gebruikt, zodat elke app gebruikt wat de andere heeft gedownload. Een lokaal model houdt de opname op deze Mac.
- **Online transcriptie** heeft een sleutel voor OpenAI of Groq nodig, die Dango in een eigen onderdeel van de sleutelhanger bewaart.
- **Vanuit Lowspeak.** Is Lowspeak anders ingesteld dan de standaard, dan biedt het tabblad 'Dicteerfunctie' één keer aan om de instellingen over te nemen, en op verzoek de sleutels; macOS vraagt het voordat Dango er een leest (kies 'Sta toe', niet 'Sta altijd toe').
- **Toestemmingen.** macOS vraagt bij het eerste dictaat om toegang tot de microfoon. De fn-toets werkt zolang Dango de actieve app is en Lowspeak niet draait; hij heeft de toestemming voor toegankelijkheid nodig, waar Dango pas na het eerste dictaat om vraagt, nooit bij het starten.

## Vertrouwen

Een map kan programma's laten draaien zodra er iets in wordt gestart: Git-hooks en repository-instellingen zoals `core.fsmonitor`, taalservers uit `node_modules`, agent-hooks, `.mcp.json`. De eerste keer dat je een map opent, vraagt Dango of je hem vertrouwt.

- **Vertrouwd:** Git, taalservers, debuggers, agents en CI draaien voor deze map en alles daaronder.
- **Beperkt:** je kunt lezen en bewerken; de wijzigingsmarkeringen in de editor komen uit het rechtstreeks lezen van `.git`. Niets wat de map instelt, wordt gestart. De panelen voor Git, CI en AI zeggen dat en bieden aan de map te vertrouwen.

Tekst die niet van jou komt, kan instructies bevatten die een agent moeten sturen (prompt injection). De uitvoer van een terminal (`@terminal`), een browserpagina (`@browser`), een CI-log dat aan een agent wordt doorgegeven, een bestand buiten het project en langere tekst die uit een webpagina wordt geplakt, gaan daarom gemarkeerd mee: de chip zegt 'Externe inhoud', en de agent krijgt de tekst met de mededeling dat het niet te vertrouwen gegevens zijn waarvan hij de instructies niet mag volgen. De webpagina's van de ingebouwde agent komen met dezelfde mededeling. Geplakte tekst geldt als afkomstig van een webpagina als de browser dat op het klembord aangeeft, zoals Safari, Chrome en Firefox doen; tekst van elders, en een korte zin van een paar woorden, blijft je eigen tekst.

## Sandbox

Dango zelf draait zonder App Sandbox, omdat elk programma dat het start die zou erven: terminals, Git, taalservers en agents zouden niet kunnen werken. In plaats daarvan heeft elke AI-thread een sandboxschakelaar, die standaard uit staat. Wat hij beperkt, hangt af van de agent, en de tooltip van de schakelaar zegt het:

| Agent | Met de sandbox aan |
|---|---|
| Claude | shellcommando's draaien in Claudes eigen sandbox; zijn tools voor bestanden en web niet |
| Codex | commando's en wijzigingen schrijven alleen in het project, zonder netwerk |
| Ingebouwde agent | bestanden, webverzoeken en commando's blijven binnen het project en de toegestane mappen, zonder netwerk (`sandbox-exec`) |
| Antigravity, OpenCode | geen sandbox die Dango kan aanzetten |

## Weergave en toegankelijkheid

- **Donkere modus** volgt het systeem; **Verhoog contrast** en **Verminder beweging** worden gerespecteerd.
- **VoiceOver** benoemt de navigatiekolom, de tabbladen, de kaarten van de AI-tijdlijn, de rijen in Git en CI, de debugger en de tekst van de terminal. Breekpunten worden nooit alleen door kleur onderscheiden.
- **Toetsenbord.** Elk commando heeft een toetscombinatie, en ze zijn allemaal te wijzigen in `keybindings.json`; ⌥⌘\` gaat naar het volgende paneel.
- **Van rechts naar links.** In het Arabisch en Hebreeuws wordt het venster gespiegeld, terwijl codeweergaven van links naar rechts blijven.

Wat Dango per onderdeel toevoegt aan AppKit, staat in [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Talen

De interface is vertaald in 35 talen: Arabisch, Catalaans, Chinees (vereenvoudigd), Chinees (traditioneel), Kroatisch, Tsjechisch, Deens, Nederlands, Engels, Fins, Frans, Duits, Grieks, Hebreeuws, Hindi, Hongaars, Indonesisch, Italiaans, Japans, Koreaans, Maleis, Noors, Pools, Portugees (Brazilië), Portugees (Portugal), Roemeens, Russisch, Slowaaks, Sloveens, Spaans, Zweeds, Thai, Turks, Oekraïens en Vietnamees.

Dango volgt de taalvolgorde in **Systeeminstellingen › Algemeen › Taal en regio**; daar kun je onder **Apps** ook een taal alleen voor Dango kiezen. Zonder vertaling voor je taal gebruikt het Engels. De termen volgen één woordenlijst, [docs/glossary.md](../../../docs/glossary.md), en de vertalingen van Apple, zodat Dango leest als de rest van je Mac.

## Toetscombinaties

| Actie | Toetscombinatie |
|---|---|
| Map openen, nieuw venster, venster sluiten | ⌘O, ⇧⌘N, ⇧⌘W |
| Nieuw bestand, bewaren, bewaren als | ⌘N, ⌘S, ⇧⌘S |
| Nieuw tabblad, tabblad sluiten, volgend en vorig tabblad | ⌘T, ⌘W, ⇧⌘] en ⇧⌘[ |
| Open snel, zoeken in het project | ⌘P, ⇧⌘F |
| Bestanden, Zoeken, Agents, Git, Debug, CI | ⌘1 tot ⌘6 |
| Navigatiekolom tonen of verbergen, volgend paneel | ⌃⌘S, ⌥⌘\` |
| Terug en vooruit langs bewerkte plekken | ⌃- en ⌃⇧- |
| Commentaar aan/uit, ga naar regel | ⌘/, ⌃G |
| Volgende plek waar het voorkomt, invoegpunt erboven en eronder | ⌘D, ⌃⇧↑ en ⌃⇧↓ |
| Invouwen, uitvouwen, alles invouwen, alles uitvouwen | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Regelafbreking, inspector | ⌥Z, ⌥⌘I |
| Aanvulling, hover, ga naar definitie | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Verwijzingen, aanroephiërarchie, hernoemen | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symbolen in het bestand, in het project | ⇧⌘O, ⌥⌘O |
| Snelle oplossing, opmaken | ⌥⌘A, ⌥⇧F |
| Volgend en vorig probleem | ⌘' en ⇧⌘' |
| Markdown-voorvertoning, plakken als platte tekst | ⇧⌘V, ⌥⇧⌘V |
| Vet, cursief, doorgehaald, code, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Kop 1 tot 3 | ⌥⌘1 tot ⌥⌘3 |
| Nieuwe terminal, terminal wissen | ⌃\`, ⌘K |
| Focus op het berichtveld van de agent, commentaar op een plan | ⌘Esc, ⌥⌘K |
| Adresveld, opnieuw laden, terug, vooruit (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Dicteren starten of stoppen | ⌃⌥D |
| Breekpunt, alle breekpunten aan of uit | ⌘\\, ⌘Y |
| Doorgaan, stap over, in, uit, stoppen | ⌃⌘Y, F6, F7, F8, ⌘. |
| Instellingen | ⌘, |

`keybindings.json` in de gegevensmap van de app (zie [Waar gegevens staan](#waar-gegevens-staan)) wijzigt elk daarvan.

## Privacy en beveiliging

- **Geen telemetrie,** geen analyse, geen crashrapporten die ergens heen gaan. Fouten gaan naar het systeemlogboek (Console, onder de naam van de app).
- **Het netwerk alleen voor wat je zelf start:** de agents die je gebruikt, elk met zijn eigen dienst; het ACP-register en de downloads van agents, taalservers, debug-adapters en dicteermodellen als je erom vraagt; de CI-hosts waarop je inlogt; voorspellingen van Copilot als je die hebt gekozen; de aanbieders van de ingebouwde agent; online transcriptie als je die hebt gekozen; de pagina's die je in het browsertabblad opent; en het zoeken naar updates zodra je dat hebt toegestaan.
- **Code verlaat de Mac** alleen waar je er een dienst voor hebt gekozen: een agent, de voorspellingen van Copilot, een commitbericht geschreven met AI. De diff voor een commitbericht gaat pas de deur uit nadat gitleaks er geen geheim in heeft gevonden.
- **Sleutels en tokens** blijven in de macOS-sleutelhanger, nooit in een instellingenbestand. Sleutels van andere apps, zoals die van Lowspeak, worden alleen gelezen als je instemt, en maar één keer.
- **Vastgezette downloads.** Agents, taalservers en debug-adapters die Dango installeert, zijn vastgezet op geteste versies en gecontroleerd tegen hun checksums.
- **Codeondertekening** met de Hardened Runtime; updates worden alleen met de eigen handtekening van de app geïnstalleerd.

## Waar gegevens staan

`<id>` is de bundle-ID, `local.agentide` voor de release (`local.agentide.debug` voor debugbuilds). Hij bevat de weergavenaam niet, zodat een naamswijziging alle gegevens behoudt.

| Plek | Wat |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (de lijst van het AI-paneel), `agent-threads/` (gesprekken van de ingebouwde agent), `layouts/` (tabbladen per project), `recovery/` (niet-bewaarde editors na een crash), `checkpoints/` (voor mappen zonder Git), `worktrees/`, `agents/` en `tools/` (geïnstalleerde agents, taalservers en debug-adapters), `task-output/` |
| `~/Library/Caches/<id>/` | geplakte afbeeldingen, scripts voor de shell-integratie, het ACP-register; kan worden verwijderd |
| `~/Library/Logs/<id>/dictation/` | de logbestanden van het dicteren |
| `~/Library/Application Support/Lowspeak` en `FluidAudio` | lokale dicteermodellen, gedeeld met Lowspeak |
| `~/Library/WebKit/<id>/` | cookies en websitegegevens van het browsertabblad |
| `<project>/.dango/settings.json` | de eigen instellingen van het project |
| de Git-repository van het project | checkpoints, als commits onder `refs/ide-checkpoints/`; een gewone push of fetch neemt ze nooit mee |
| sleutelhanger, dienst `<id>` | API-sleutels van de ingebouwde agent en van het dicteren, CI-tokens |
| `~/.local/bin/dango` | het terminalcommando, als je het hebt geïnstalleerd |

Claude bewaart zijn eigen sessies onder `~/.claude/projects`; Dango leest ze om een sessie te hervatten, maar verandert ze niet.

## Bekende beperkingen

- **Geen App Sandbox** voor de app zelf, omdat terminals, Git, taalservers en agents die zouden erven; een map moet vertrouwd zijn voordat iets wat hij instelt, draait. Antigravity en OpenCode hebben geen sandbox die Dango kan aanzetten.
- **Gebruikscijfers en de uitsplitsing van de context van Claude** steunen op delen van zijn protocol die als experimenteel zijn gemarkeerd, en worden verborgen als die ontbreken.
- **Het gesprek terugdraaien** werkt alleen bij Claude; bij de andere agents wordt alleen de code teruggedraaid.
- **Inloggen bij CI via de browser** staat uit; gebruik `gh`, `glab`, een token of de omgeving.
- **Gemini CLI wordt niet aangeboden:** Google heeft het inloggen met een persoonlijk account beëindigd en verwijst naar Antigravity. Eerdere Gemini-threads blijven leesbaar.

## Bouwen vanuit de broncode

Je hebt nodig:

- macOS 14 of nieuwer met de Xcode Command Line Tools (`xcode-select --install`); Xcode zelf is niet nodig.
- Rust zoals `rust-toolchain.toml` het noemt. De `rust` van Homebrew bouwt voor de processor van deze Mac; de universele build van een release heeft rustup nodig met de targets `aarch64-apple-darwin` en `x86_64-apple-darwin`.
- Checkouts van [splat](https://github.com/sventmueller/splat) en [lowspeak](https://github.com/sventmueller/lowspeak) naast deze (`../splat`, `../lowspeak`), allebei op hun branch `main`; de workspace haalt crates daaruit via een pad.
- cmake, voor elke build met dicteren (`make run`, `make build`, `make release`), die whisper.cpp bouwt. `make check` bouwt zonder.

| Commando | Wat het doet |
|---|---|
| `make run` | bouwt de debug-app (`target/debug/Dango Debug.app`) en opent hem |
| `make build` | bouwt de release-app (`target/release/Dango.app`), ondertekend met een lokaal certificaat dat de eerste build in de inlogsleutelhanger aanmaakt; verhoogt het buildnummer in `VERSION` |
| `make lint` | opmaak en clippy; waarschuwingen laten hem mislukken |
| `make check` | `lint`, daarna alle tests; heeft geen sleutelhanger, venster of netwerk nodig |
| `make test` | `check`, daarna de zelftest `probe` en de toegankelijkheidscontrole, van links naar rechts en van rechts naar links, in de echte app |
| `make perf` | de tijdsbudgetten, in een release-build; alleen zinvol op een verder ongebruikte Mac |
| `make release` | een proefrun van de release naar `target/release-dry-run/`, ondertekend met het lokale certificaat; verstuurt niets |
| `make release-notarized` | `check`, daarna de release zelf naar `dist/`: Developer ID-handtekening, notarisatie, de updatefeed ondertekend met de eigen sleutel van de app; verhoogt het buildnummer |

Debug en release zijn aparte apps met hun eigen instellingen en gegevens. Bouwen, testen, de zelftests, de release en de regels die de code volgt, staan in [docs/development.md](../../../docs/development.md), de beslissingen erachter in [docs/plan.md](../../../docs/plan.md). De release-opmerkingen zijn [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), vertaald naast elke README in `assets/readme/<language>/`.

## Licentie

Dango is gelicenseerd onder een van de volgende twee licenties

- Apache-licentie, versie 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-licentie ([LICENSE-MIT](../../../LICENSE-MIT))

naar jouw keuze.

Tenzij je uitdrukkelijk anders aangeeft, wordt elke bijdrage die je opzettelijk indient voor opname in Dango, zoals gedefinieerd in de Apache-2.0-licentie, onder dubbele licentie geplaatst zoals hierboven, zonder aanvullende voorwaarden.
