<p><img src="../../readme-icon.png" alt="Dango-symbol" width="147" height="147"></p>

# Dango

**Den native macOS-editoren for arbeid med kodeagenter.**

Dango samler koden, agentene som endrer den, og alt rundt dem i ett vindu per mappe: en editor med språktjenere og forutsigelser mens du skriver, terminaler, en nettleser, Git, CI, en debugger og KI-tråder med Claude, Codex, Antigravity, OpenCode eller en agent som er bygd inn i appen. Hver agent arbeider i den samme tidslinjen, spør før den endrer filer eller kjører kommandoer, og etterlater et sjekkpunkt før hver ledetekst, slik at du kan gå gjennom og spole tilbake det den har gjort, melding for melding. Ingenting en mappe konfigurerer, kjører før du stoler på den, og tekst utenfra når en agent merket som ikke klarert. Dikter inn i editoren og meldingsfeltet, og talegjenkjenningen kan bli værende på Macen din. Dango er skrevet i Rust på ekte AppKit-kontroller, uten Electron, snakker 35 språk og har ingen telemetri.

[![Last ned Dango.dmg](https://img.shields.io/badge/Last_ned_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Last ned Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: signert og notarisert av Apple, for macOS 14 eller nyere på Apple Silicon og Intel. Lenken blir tilgjengelig med den første utgivelsen.

**Har du funnet en feil?** [Opprett et issue](https://github.com/sventmueller/dango/issues/new), og skriv hvilken agent og hva slags prosjekt det gjaldt.

[![Rust, 2024-utgaven](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 eller nyere](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universell: Apple Silicon og Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 språk i grensesnittet](https://img.shields.io/badge/languages-35-4C9A2A)
![Ingen telemetri](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signert med Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lisens: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lisens)

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
[Nederlands](../nl/README.md) ·
**Norsk** ·
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

## Innhold

- [Funksjoner i korte trekk](#funksjoner-i-korte-trekk)
- [Krav](#krav)
- [Installering](#installering)
- [Kom i gang](#kom-i-gang)
- [KI-agenter](#ki-agenter)
  - [Tidslinjen](#tidslinjen)
  - [Tillatelser og moduser](#tillatelser-og-moduser)
  - [Sjekkpunkter og worktrees](#sjekkpunkter-og-worktrees)
- [Editor](#editor)
- [Språktjenere](#språktjenere)
- [Tester](#tester)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Nettleser](#nettleser)
- [Git](#git)
- [CI](#ci)
- [Containere](#containere)
- [Markdown](#markdown)
- [Diktering](#diktering)
- [Tillit](#tillit)
- [Sandkasse](#sandkasse)
- [Utseende og tilgjengelighet](#utseende-og-tilgjengelighet)
- [Språk](#språk)
- [Tastatursnarveier](#tastatursnarveier)
- [Personvern og sikkerhet](#personvern-og-sikkerhet)
- [Hvor dataene ligger](#hvor-dataene-ligger)
- [Kjente begrensninger](#kjente-begrensninger)
- [Bygg fra kildekoden](#bygg-fra-kildekoden)
- [Lisens](#lisens)

## Funksjoner i korte trekk

- **Alle agenter i én tidslinje.** Claude via sin egen kommando `claude`, Codex, Antigravity og OpenCode via Agent Client Protocol, flere fra ACP-registeret og en agent bygd inn i appen for Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama og LM Studio. Svar, verktøykall, forespørsler om tillatelse, planer, gjøremål, underagenter og bakgrunnsoppgaver ser like ut uansett hvilken agent som kjører.
- **Gå gjennom og spol tilbake.** Før hver ledetekst lagrer Dango et sjekkpunkt. Det en agent har endret, kan gås gjennom og spoles tilbake per melding, og en tråd kan arbeide i sitt eget Git-worktree og levere arbeidet sitt med sammenslåing eller cherry-pick.
- **Ett vindu per mappe.** Filer, terminaler, nettsider, KI-tråder, differ og forhåndsvisninger er macOS-faner i det vinduet. Sidepanelet inneholder Filer, Søk, Agenter, Git, Tester, Feilsøk, CI og Containere.
- **En ekte kodeeditor.** Syntaksfarger, flere innsettingspunkter, bretting, `.editorconfig`, språktjenere for 17 språk med fullføring, gå til definisjon, referanser, endre navn, hurtigrettinger og hierarkier, og forutsigelser mens du skriver fra GitHub Copilot eller en lokal modell.
- **Debugger, Git og CI innebygd.** Stoppunkter og trinnvis kjøring for Rust, C, C++, Swift, Go, JavaScript, TypeScript og Python; stage enkeltblokker, brancher, stasher, konflikter og en commitgraf; kjøringer, jobber og logger fra GitHub Actions og GitLab CI.
- **Forsiktig med mappene dine.** En mappe du ikke stoler på, starter ingenting: ingen Git-prosesser, språktjenere, hooks eller agenter. Destruktive Git-kommandoer tillates aldri for godt, og commits sjekkes for hemmeligheter.
- **Diktering.** Snakk inn i editoren og meldingsfeltet, på nett via OpenAI eller Groq eller lokalt med Whisper og Parakeet.
- **Native helt inn til beinet.** AppKit-vinduer, -faner, -menyer og -verktøylinjer, TextKit 2 for editoren, Metal for terminalen, WebKit bare i nettleserfanen. Mørk modus, Øk kontrast og VoiceOver fungerer som i enhver Mac-app.
- **På ditt språk.** Grensesnittet finnes på 35 språk: arabisk, katalansk, kinesisk (forenklet), kinesisk (tradisjonelt), kroatisk, tsjekkisk, dansk, nederlandsk, engelsk, finsk, fransk, tysk, gresk, hebraisk, hindi, ungarsk, indonesisk, italiensk, japansk, koreansk, malayisk, norsk, polsk, portugisisk (Brasil), portugisisk (Portugal), rumensk, russisk, slovakisk, slovensk, spansk, svensk, thai, tyrkisk, ukrainsk og vietnamesisk.

## Krav

- macOS 14 Sonoma eller nyere, på Apple Silicon eller Intel. Dango utvikles og testes på macOS 27.
- For KI-trådene agenten du vil bruke; se [KI-agenter](#ki-agenter).
- For språktjenere og feilsøkingsadaptere, som Dango installerer når du ber om det: Go for gopls og `dlv`; Node.js for tjenerne for TypeScript, Python, JSON, YAML, CSS, HTML og shell og JavaScript-debuggeren; Xcode Command Line Tools for clangd, sourcekit-lsp og `lldb-dap`. rust-analyzer lastes ned fra GitHub-utgivelsene sine.
- For forutsigelser mens du skriver, et GitHub Copilot-abonnement (det gratis holder) eller en fill-in-the-middle-modell i Ollama eller LM Studio.

## Installering

**Fra diskbildet.** Hver utgivelse er et diskbilde, `Dango-<version>.dmg`, med én universell app for Apple Silicon og Intel, signert med en Developer ID og notarisert av Apple. Åpne det og flytt Dango til Programmer-mappen. En Homebrew Cask installerer det samme bildet. Ingen utgivelse er publisert ennå.

**Oppdateringer.** Dango fra diskbildet holder seg selv oppdatert med [Sparkle](https://sparkle-project.org). Ved andre oppstart spør appen om den selv kan se etter oppdateringer; til du sier ja, sender den ingen forespørsler. **Dango › Søk etter oppdateringer…** ser etter med en gang. Oppdateringer er signert med appens egen nøkkel, som Sparkle kontrollerer før den installerer noe. En versjon du har bygd selv, har ingen oppdateringsfunksjon.

**Fra kildekoden.** Se [Bygg fra kildekoden](#bygg-fra-kildekoden), og flytt deretter `target/release/Dango.app` til `/Applications`.

**Terminalkommandoen.** **Dango › Installer kommandoen «dango»…** skriver et lite skript til `~/.local/bin/dango`, uten administratorrettigheter; `~/.local/bin` må være i `PATH`. Deretter åpner `dango .` den gjeldende mappen og `dango file.rs` en fil. Dango erstatter aldri en fil der som den ikke har skrevet selv.

## Kom i gang

1. **Åpne en mappe** med ⌘O eller `dango <folder>`. Første gang spør Dango om du stoler på den (se [Tillit](#tillit)).
2. **Sidepanelet** veksler mellom Filer (⌘1), Søk (⌘2), Agenter (⌘3), Git (⌘4), Tester (⌘5), Feilsøk (⌘6), CI (⌘7) og Containere (⌘8); et høyreklikk skjuler dem du ikke bruker, og ⌃⌘S fjerner sidepanelet.
3. **Hvert dokument er en fane** i mappens vindu, macOS’ egne faner. «+» på slutten av fanelinjen og ⌘T tilbyr en ny fil, en terminal, nettleseren og hver installerte agent, også i et eget Git-worktree; holder du inne «+», vises det samme som en meny.
4. **Åpne raskt** med ⌘P finner filer, og med `#` prosjektets symboler; ⇧⌘F søker og erstatter i prosjektet.
5. **Snakk med en agent** i en KI-fane. ⌘Esc flytter tastaturfokuset til meldingsfeltet og tilbake.
6. **Innstillinger** med ⌘, inneholder fanene Generelt, CI, Agenter, Innebygd agent, Språktjener, Diktering og Forutsigelse.

Verktøylinjen har tilbake og fram gjennom stedene du har redigert (⌃- og ⌃⇧-), branchen, Åpne raskt og aktivitetsoversikten, som viser alt som kjører. **Åpne ved siden av** i kontekstmenyen til en fil viser et nytt dokument ved siden av det første. Etter en omstart eller et krasj kommer hvert vindu tilbake med fanene sine, og editorer med ulagret tekst kommer tilbake med teksten. Bilder, lyd, video, skrifter og Office-dokumenter åpnes i en fane via Hurtigvisning, PDF-er via PDFKit.

## KI-agenter

| Agent | Slik kjører den | Dette trenger den |
|---|---|---|
| Claude | kommandoen `claude`, snakket direkte med via stream-json-protokollen | `claude` installert og pålogget; Dango leter i `PATH` for påloggingsskallet, i `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` og `/usr/local/bin`. Pålogging skjer bare i `claude` selv |
| Codex | `codex-acp` via Agent Client Protocol (ACP) | Node.js og npm; Dango installerer den fastlåste pakken når du ber om det |
| Antigravity | `agy acp` | din egen `agy` i `PATH`, eller den fastlåste nedlastingen, installert når du ber om det |
| OpenCode | `opencode acp` | din egen `opencode` i `PATH`, eller den fastlåste nedlastingen, installert når du ber om det |
| Andre ACP-agenter | fra ACP-registeret, som bare hentes når du ber om det | det registeroppføringen oppgir; oppføringer uten sjekksum installeres først når du har bekreftet |
| Innebygd agent | bygd inn i appen, med verktøy for filer, søk, kommandoer, diagnostikk og nettforespørsler | en API-nøkkel for Anthropic, OpenAI, Google Gemini eller OpenRouter, lagret i nøkkelringen; Ollama eller LM Studio på denne Macen uten nøkkel; enhver tjener som snakker API-et til OpenAI eller Anthropic |

**Innstillinger › Agenter** viser hvilke agenter denne Macen har, installerer de andre og oppdateringene deres, og åpner ACP-registeret. Agenter Dango installerer, er låst til testede versjoner og kontrollert mot de publiserte sjekksummene (npm integrity eller SHA-256); Dango kjører ingen `npx`-pakker uten versjon.

### Tidslinjen

Hver agent arbeider i en KI-fane med den samme tidslinjen: svar, verktøykall med utdataene, forespørsler om tillatelse, spørsmål, planer til godkjenning og kommentering (⌥⌘K), gjøremål, underagenter og bakgrunnsoppgaver. To knapper i toppen av fanen viser underagentene og bakgrunnsprosessene som kjører. Nevn en fil, `@terminal` eller `@browser` for å gi agenten innholdet. En grønn prikk på fanen viser en økt i arbeid, en rød en økt som venter på deg, og en varsling sier fra når den er ferdig eller venter mens du ser et annet sted. For Claude viser statuslinjen konteksten og forbruket.

### Tillatelser og moduser

Nye tråder starter i modusen «Manuell»: Agenten spør før den endrer filer eller kjører kommandoer. For destruktive Git-kommandoer (`push --force`, `reset --hard`, `clean -fd`) tilbyr Dango ingen «Tillat alltid». Den innebygde agenten lagrer «Tillat alltid»-reglene sine per prosjekt, viser dem i **Innstillinger › Innebygd agent** og spør ved destruktive Git-kommandoer hver gang, uansett hva som er lagret; Claude og ACP-agentene lagrer slike regler i sin egen konfigurasjon. Med «Tillat alt» og uten sandkasse viser tråden en varig advarsel.

### Sjekkpunkter og worktrees

Før hver ledetekst lagrer Dango et sjekkpunkt, som en commit under `refs/ide-checkpoints/` i prosjektets repository, eller i en egen mappe for et prosjekt uten Git; en vanlig push eller fetch tar dem aldri med. Det en agent har endret, kan gås gjennom og spoles tilbake per melding. En tråd kan arbeide i et eget Git-worktree: Git-panelet viser hva den har endret, og tar over arbeidet med sammenslåing eller cherry-pick.

## Editor

- **Syntaksfarger, linjenumre og endringsmerker** mot siste commit, søk og erstatt, Åpne raskt (⌘P) og søk i prosjektet (⇧⌘F).
- **Innrykk** følger filen og `.editorconfig`: tabulatorer eller mellomrom, automatisk innrykk, parentespar.
- **Flere innsettingspunkter:** ⌥-klikk, ⌥-dra for en kolonne, ⌃⇧↑ og ⌃⇧↓, ⌘D for neste forekomst.
- **Bretting** (⌥⌘← og ⌥⌘→, med ⇧ for alt), slå kommentar av/på (⌘/), gå til linje (⌃G), synlige mellomrom og linjebryting (⌥Z).
- **Filer beholder tegnkodingen**, byte order mark og linjeskiftene sine. En fil som endres på disken, lastes inn på nytt når editoren ikke har ulagret tekst; ellers tilbyr en linje forskjellene og en sammenslåing.
- **Forutsigelser mens du skriver.** Mens du skriver, vises et forslag i grått, og Tab tar det. Det kommer fra GitHub Copilot eller fra en lokal modell i Ollama eller LM Studio, som valgt i **Innstillinger › Forutsigelse**; en lokal modell holder koden din på denne Macen.
- **Minikart og festet rulling** (Editor-menyen): et smalt bilde av hele filen ved siden av teksten, og overskriftslinjene til blokkene den øverste linjen står i, holdt fast over den.
- **Diagnostikk på linjeslutt** (Vis-menyen): den alvorligste meldingen for hver linje etter et sekunds ro, med et symbol for alvorlighetsgraden.
- **Live-forhåndsvisning av statiske sider:** Editor › Åpne i nettleserfane lar Dango selv levere sidene i en mappe du stoler på, og laster inn fanen på nytt når en fil lagres.
- **Prettier** formaterer JavaScript, TypeScript, CSS, HTML, Markdown og mer der prosjektet setter det opp, med prosjektets egen `node_modules/.bin/prettier`; innstillingen `formatter` velger formateringsverktøyet per språk.
- **Vim-modus** (Editor › Vim-modus): modusene Normal, Sett inn, Visuell og Erstatt, antall, operatorer med bevegelser og tekstobjekter, `.`, registre, merker og makroer. `u` og ⌃R bruker Angre i editoren, og alt du setter inn i én omgang, er ett steg. `"+` og `"*` er utklippstavlen; innstillingen `vimClipboard` gjør også det navnløse registeret til utklippstavlen. `/`, `?`, `n`, `N`, `*`, `:s` og `:%s` søker med Rusts regulære uttrykk, ikke med Vims mønstre: ingen `\v`, og `\b` i stedet for `\<` og `\>`. Ex-kommandoer er `:w`, `:q`, `:wq`, `:x`, `:noh` og `:<line>`. I Normal-modus tar Vim ⌃G fra gå til linje.
- **Synkronisering av innstillinger** via en mappe i iCloud Drive (**Innstillinger › Generelt**): innstillingene og tastatursnarveiene dine følger deg til de andre Mac-maskinene dine; stier, lokale adresser og samtykker blir værende på hver av dem.

## Språktjenere

Go, Rust, TypeScript og JavaScript, Python, C, C++ og Objective-C, Swift, shellskript, JSON, YAML, CSS, HTML, TOML, Markdown og Lua, og egne tjenere fra **Innstillinger › Språktjener**. Dango installerer en tjener når du ber om det, i sin egen verktøymappe, og bruker et TypeScript-prosjekts egen `tsc` når den kan fungere som språktjener.

Dette gir de: diagnostikk (⌘' og ⇧⌘' går mellom dem), hover (⌃⌘I), fullføring (⌥Esc), signaturhjelp, gå til definisjon (⌃⌘J), typedefinisjon og implementering, referanser (⌃⇧⌘F), kall- og typehierarkier (⌃⇧⌘H), endre navn (⌃⌘E), formatering (⌥⇧F), semantiske farger, bretting og disposisjonen (⇧⌘O); typer og parameternavn inne i linjen; hurtigrettinger og refaktoreringer bak en lyspære (⌥⌘A); prosjektets symboler med ⌥⌘O.

I tillegg til filens egen tjener sjekker ESLint JavaScript og TypeScript i prosjekter som bruker det, med hurtigrettingene sine, og forkortelsene til Emmet vises i fullføringen for HTML, CSS og JSX. Dango installerer begge når du ber om det; **Innstillinger › Generelt** retter ESLint-problemer ved lagring hvis du vil.

## Tester

Tester-panelet viser testene i en mappe etter pakke og fil med tilstanden deres: `cargo test` eller cargo-nextest, Go, pytest, Vitest og Jest. Et symbol ved hver test i margen kjører den, ⌥-klikk feilsøker den, og en mislykket assert markerer linjen sin. Feilsøk-menyen kjører testen ved innsettingspunktet (⌃⌥⌘U), de siste testene på nytt (⌃⌥⌘G) eller alle, og feilsøker testen ved innsettingspunktet. Tester kjører bare i mapper du stoler på, siden de kjører koden i prosjektet; `cargo test` lister testene sine først når du ber om det, fordi listingen bygger hvert testprogram.

## Debugger

Stoppunkter, også med betingelser og som loggpunkter, trinnvis kjøring, tråder med kallstablene sine, variabler og overvåkede uttrykk i inspektøren (⌥⌘I), verdier ved siden av koden mens programmet er satt på pause, og en feilsøkingskonsoll som evaluerer i den pausede rammen.

| Språk | Feilsøkingsadapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` fra Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScript-debuggeren, på Node.js |
| Python | debugpy |

Et prosjekts `.vscode/launch.json` leses. Feilsøk-menyen bruker snarveiene fra Xcode: ⌘\\ setter et stoppunkt, ⌘Y slår alle av eller på, ⌃⌘Y fortsetter, F6, F7 og F8 går over, inn og ut, ⌘. stopper.

## Terminal

Terminaler i faner (⌃\`) med miljøet til påloggingsskallet, tegnet med Metal. ⌘K tømmer skjermen. Et innlimt eller slept bilde går til en kodeagent som kjører i terminalen, eller limer inn stien til en lagret kopi. ⌘-klikk åpner lenker og filnavn. En utviklingstjener som terminalen starter, kan åpnes i nettleserfanen.

## Nettleser

En nettleserfane med adressefelt (⌘L), tilbake og fram (⌘[ og ⌘]), last inn på nytt (⌘R), søk, nedlastinger som spør først, og nettinspektøren. `@browser` gir siden til en agent, merket som tekst utenfra.

## Git

Git-panelet stager og forkaster filer eller enkeltblokker, lager commits, committer og pusher eller synkroniserer, bytter og oppretter brancher, lagrer stasher, løser sammenslåingskonflikter og tegner en graf over de siste commitene. Blame viser hvem som sist endret linjen under markøren, eller en kolonne for hele filen. En knapp skriver commitmeldingen med KI; differen sendes først når gitleaks ikke har funnet noen hemmelighet i den, og hver commit sjekkes også for hemmeligheter.

## CI

GitHub Actions og GitLab CI, også GitHub Enterprise Server og selvdriftet GitLab:

- kjøringer, jobber og logger for den gjeldende branchen;
- kjøre mislykkede jobber, én jobb eller, på GitHub, en hel kjøring på nytt; starte manuelle jobber eller godkjenne ventende deployments; starte en ny pipeline eller en workflow som kan startes manuelt; avbryte;
- de åpne pull requestene eller merge requestene og issuene, som åpnes i nettleserfanen;
- loggen til en mislykket jobb gitt til en agent, merket som tekst utenfra.

Tokenet kommer fra nøkkelringen, `gh` eller `glab`, miljøet til påloggingsskallet, eller er et du skriver inn i **Innstillinger › CI**.

## Containere

Containere-panelet viser Dockers Compose-prosjekter med tjenestene sine, enkeltstående containere, images og volumer, og følger Dockers hendelser etter hvert som de skjer. En Compose-fil i mappen vises som et prosjekt allerede før noen av containerne finnes. Containere startes, stoppes og startes på nytt; loggene deres og et shell åpnes i terminalfaner, og en publisert port åpnes i nettleserfanen. Sletting av en container, et image eller et volum og opprydding i ubrukte data spør først. Composes up, down og pull kjører bare i mapper du stoler på, siden Compose bygger og kjører det prosjektet beskriver. Panelets meny velger Docker-konteksten for vinduet uten å endre den skallene dine bruker; de samme kommandoene finnes i Containere-menyen. Panelet trenger kommandolinjen `docker`, Docker 24.0.6 eller nyere.

## Markdown

- **Forhåndsvisning.** En Markdown-fil åpnes med forhåndsvisningen ved siden av; **Vis › Vis/skjul forhåndsvisning ved siden av** (⇧⌘V) lukker og åpner den, og **Innstillinger › Generelt** slår av den automatiske forhåndsvisningen. Forhåndsvisningen tegner formler, diagrammer i Mermaid, Graphviz og PlantUML og bildene i mappen; bilder fra nettet lastes bare i en klarert mappe eller når du ber om dem i forhåndsvisningen.
- **Format-menyen:** fet (⌘B), kursiv (⌘I), gjennomstreking (⇧⌘X), kode (⌥⇧⌘C), lenke (⇧⌘K), overskrift 1–3 (⌥⌘1–⌥⌘3), punktlister og nummererte lister, oppgaver, sitater og kodeblokker. Hver kommando slår av og på, på markeringen eller linjene, og er ett angretrinn.
- **Lim inn.** Tekst fra Word eller en nettside kommer inn som Markdown, bildene som filer ved siden av dokumentet; ⌥⇧⌘V limer inn ren tekst.
- **Eksport.** **Fil › Eksporter** skriver PDF, én HTML-fil med bildene inni, eller et Word-dokument. Bilder fra nettet blir bare med fra en klarert mappe.
- **Åpne i Splat.** Der [Splat](https://github.com/sventmueller/splat) er installert, åpner **Fil › Åpne i Splat** og kontekstmenyene i fillisten og fanene dokumentet der, etter at det er lagret.

## Diktering

Dikter inn i meldingsfeltet i en KI-fane eller i editoren: med mikrofonknappen der, **Rediger › Start diktering** (⌃⌥D) eller fn-tasten (hold inne for å ta opp, trykk to ganger for å fortsette opptaket). Talegjenkjenningen er den fra [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak selv trenger ikke være installert.

- **Innstillinger › Diktering** stiller inn modus og modell (på nett med OpenAI eller Groq, lokalt med Whisper eller Parakeet), språket, etterbehandlingen, mikrofonen og ditt eget ordforråd.
- **Lokale modeller** lastes ned fra Hugging Face når du velger en, med fremdrift og mulighet til å avbryte. De havner i mappene Lowspeak bruker, slik at hver app bruker det den andre har lastet ned. En lokal modell holder opptaket på denne Macen.
- **Transkripsjon på nett** krever en nøkkel for OpenAI eller Groq, som Dango lagrer i sin egen oppføring i nøkkelringen.
- **Fra Lowspeak.** Er Lowspeak stilt inn annerledes enn standarden, tilbyr Diktering-fanen én gang å ta over innstillingene, og nøklene på forespørsel; macOS spør før Dango leser en (velg «Tillat», ikke «Tillat alltid»).
- **Tillatelser.** macOS spør om mikrofonen ved første diktering. fn-tasten virker mens Dango er den aktive appen og Lowspeak ikke kjører; den trenger tillatelsen for tilgjengelighet, som Dango ber om først etter den første dikteringen, aldri ved oppstart.

## Tillit

En mappe kan få programmer til å kjøre så snart noe startes i den: Git-hooks og repository-innstillinger som `core.fsmonitor`, språktjenere fra `node_modules`, agent-hooks, `.mcp.json`. Første gang du åpner en mappe, spør Dango om du stoler på den.

- **Klarert:** Git, språktjenere, debuggere, agenter og CI kjører for denne mappen og alt under den.
- **Begrenset:** Du kan lese og redigere; endringsmerkene i editoren kommer fra å lese `.git` direkte. Ingenting mappen konfigurerer, startes. Panelene for Git, CI og KI sier det og tilbyr å klarere mappen.

Tekst som ikke kommer fra deg, kan inneholde instruksjoner som skal styre en agent (prompt injection). Utdata fra en terminal (`@terminal`), en nettside (`@browser`), en CI-logg gitt til en agent, en fil utenfor prosjektet og lengre tekst limt inn fra en nettside sendes derfor merket: Brikken sier «Eksternt innhold», og agenten får teksten med beskjed om at det er ikke-klarerte data som den ikke skal følge instruksjonene i. Nettsidene til den innebygde agenten kommer med samme beskjed. Innlimt tekst regnes som tekst fra en nettside når nettleseren sier det på utklippstavlen, slik Safari, Chrome og Firefox gjør; tekst fra andre steder, og en kort frase på noen få ord, forblir din egen.

## Sandkasse

Dango selv kjører uten App Sandbox, fordi alle programmer appen starter, ville arvet den: Terminaler, Git, språktjenere og agenter kunne ikke fungere. I stedet har hver KI-tråd en sandkassebryter, som er av som standard. Hva den begrenser, avhenger av agenten, og verktøytipset til bryteren sier det:

| Agent | Med sandkassen på |
|---|---|
| Claude | shellkommandoer kjører i Claudes egen sandkasse; verktøyene for filer og nett gjør det ikke |
| Codex | kommandoer og endringer skriver bare i prosjektet, uten nettverk |
| Innebygd agent | filer, nettforespørsler og kommandoer holder seg innenfor prosjektet og de tillatte mappene, uten nettverk (`sandbox-exec`) |
| Antigravity, OpenCode | ingen sandkasse som Dango kan slå på |

## Utseende og tilgjengelighet

- **Mørk modus** følger systemet; **Øk kontrast** og **Reduser bevegelse** respekteres.
- **VoiceOver** navngir sidepanelet, fanene, kortene i KI-tidslinjen, radene i Git og CI, debuggeren og teksten i terminalen. Stoppunkter skilles aldri bare ved farge.
- **Tastatur.** Hver kommando har en snarvei, og alle kan endres i `keybindings.json`; ⌥⌘\` går til neste panel.
- **Høyre mot venstre.** På arabisk og hebraisk speilvendes vinduet, mens kodevisninger forblir venstre mot høyre.

Hva Dango legger til oppå AppKit, område for område, står i [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Språk

Grensesnittet er oversatt til 35 språk: arabisk, katalansk, kinesisk (forenklet), kinesisk (tradisjonelt), kroatisk, tsjekkisk, dansk, nederlandsk, engelsk, finsk, fransk, tysk, gresk, hebraisk, hindi, ungarsk, indonesisk, italiensk, japansk, koreansk, malayisk, norsk, polsk, portugisisk (Brasil), portugisisk (Portugal), rumensk, russisk, slovakisk, slovensk, spansk, svensk, thai, tyrkisk, ukrainsk og vietnamesisk.

Dango følger språkrekkefølgen i **Systeminnstillinger › Generelt › Språk og område**; der kan du også velge et språk bare for Dango under **Apper**. Finnes det ingen oversettelse for språket ditt, bruker appen engelsk. Begrepene følger én ordliste, [docs/glossary.md](../../../docs/glossary.md), og Apples egne oversettelser, så Dango leses som resten av Macen din.

## Tastatursnarveier

| Handling | Snarvei |
|---|---|
| Åpne mappe, nytt vindu, lukk vindu | ⌘O, ⇧⌘N, ⇧⌘W |
| Ny fil, lagre, lagre som | ⌘N, ⌘S, ⇧⌘S |
| Ny fane, lukk fane, neste og forrige fane | ⌘T, ⌘W, ⇧⌘] og ⇧⌘[ |
| Åpne raskt, søk i prosjektet | ⌘P, ⇧⌘F |
| Filer, Søk, Agenter, Git, Tester, Feilsøk, CI, Containere | ⌘1 til ⌘8 |
| Vis eller skjul sidepanelet, neste panel | ⌃⌘S, ⌥⌘\` |
| Tilbake og fram gjennom redigerte steder | ⌃- og ⌃⇧- |
| Slå kommentar av/på, gå til linje | ⌘/, ⌃G |
| Neste forekomst, innsettingspunkt over og under | ⌘D, ⌃⇧↑ og ⌃⇧↓ |
| Brett, brett ut, brett alt, brett ut alt | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Linjebryting, inspektør | ⌥Z, ⌥⌘I |
| Fullføring, hover, gå til definisjon | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referanser, kallhierarki, endre navn | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboler i filen, i prosjektet | ⇧⌘O, ⌥⌘O |
| Hurtigretting, formater | ⌥⌘A, ⌥⇧F |
| Neste og forrige problem | ⌘' og ⇧⌘' |
| Markdown-forhåndsvisning, lim inn som ren tekst | ⇧⌘V, ⌥⇧⌘V |
| Fet, kursiv, gjennomstreking, kode, lenke | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Overskrift 1 til 3 | ⌥⌘1 til ⌥⌘3 |
| Ny terminal, tøm terminalen | ⌃\`, ⌘K |
| Fokus på agentens meldingsfelt, kommenter en plan | ⌘Esc, ⌥⌘K |
| Adressefelt, last inn på nytt, tilbake, fram (nettleser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Start eller stopp diktering | ⌃⌥D |
| Stoppunkt, alle stoppunkter av eller på | ⌘\\, ⌘Y |
| Kjør testen ved innsettingspunktet, kjør de siste testene på nytt | ⌃⌥⌘U, ⌃⌥⌘G |
| Fortsett, gå over, inn, ut, stopp | ⌃⌘Y, F6, F7, F8, ⌘. |
| Innstillinger | ⌘, |

`keybindings.json` i appens datamappe (se [Hvor dataene ligger](#hvor-dataene-ligger)) endrer hvilken som helst av dem.

## Personvern og sikkerhet

- **Ingen telemetri,** ingen analyse, ingen krasjrapporter sendt noe sted. Feil går til systemloggen (Konsoll, under appens navn).
- **Nettverket bare for det du starter:** agentene du bruker, hver med sin egen tjeneste; ACP-registeret og nedlastingene av agenter, språktjenere, feilsøkingsadaptere og dikteringsmodeller når du ber om dem; CI-vertene du logger på; forutsigelser fra Copilot hvis du har valgt det; leverandørene til den innebygde agenten; transkripsjon på nett hvis du har valgt det; sidene du åpner i nettleserfanen; og søket etter oppdateringer når du har tillatt det.
- **Kode forlater Macen** bare der du har valgt en tjeneste for det: en agent, forutsigelsene fra Copilot, en commitmelding skrevet med KI. Differen for en commitmelding sendes først når gitleaks ikke har funnet noen hemmelighet i den.
- **Nøkler og tokener** blir i macOS-nøkkelringen, aldri i en innstillingsfil. Nøkler fra andre apper, som Lowspeak, leses bare når du sier ja, og bare én gang.
- **Fastlåste nedlastinger.** Agenter, språktjenere og feilsøkingsadaptere som Dango installerer, er låst til testede versjoner og kontrollert mot sjekksummene sine.
- **Kodesignering** med Hardened Runtime; oppdateringer installeres bare med appens egen signatur.

## Hvor dataene ligger

`<id>` er pakke-ID-en, `local.agentide` for utgivelsen (`local.agentide.debug` for feilsøkingsbygg). Den inneholder ikke visningsnavnet, så et nytt navn beholder alle data.

| Sted | Hva |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (listen i KI-panelet), `agent-threads/` (samtalene til den innebygde agenten), `layouts/` (faner per prosjekt), `recovery/` (ulagrede editorer etter et krasj), `checkpoints/` (for mapper uten Git), `worktrees/`, `agents/` og `tools/` (installerte agenter, språktjenere og feilsøkingsadaptere), `task-output/` |
| `~/Library/Caches/<id>/` | innlimte bilder, skript for skallintegrasjon, ACP-registeret; kan slettes |
| `~/Library/Logs/<id>/dictation/` | loggfilene til dikteringen |
| `~/Library/Application Support/Lowspeak` og `FluidAudio` | lokale dikteringsmodeller, delt med Lowspeak |
| `~/Library/WebKit/<id>/` | informasjonskapsler og nettstedsdata fra nettleserfanen |
| `<project>/.dango/settings.json` | prosjektets egne innstillinger |
| prosjektets Git-repository | sjekkpunkter, som commits under `refs/ide-checkpoints/`; en vanlig push eller fetch tar dem aldri med |
| nøkkelring, tjeneste `<id>` | API-nøkler for den innebygde agenten og dikteringen, CI-tokener |
| `~/.local/bin/dango` | terminalkommandoen, hvis du har installert den |

Claude lagrer sine egne økter under `~/.claude/projects`; Dango leser dem for å gjenoppta en økt, men endrer dem ikke.

## Kjente begrensninger

- **Ingen App Sandbox** for selve appen, fordi terminaler, Git, språktjenere og agenter ville arvet den; en mappe må være klarert før noe den konfigurerer, kjører. Antigravity og OpenCode har ingen sandkasse som Dango kan slå på.
- **Claudes forbrukstall og fordelingen av konteksten** bygger på deler av protokollen som er merket som eksperimentelle, og skjules når de mangler.
- **Å spole samtalen tilbake** virker bare for Claude; for de andre agentene spoles bare koden tilbake.
- **Pålogging til CI via nettleseren** er slått av; bruk `gh`, `glab`, et token eller miljøet.
- **Gemini CLI tilbys ikke:** Google har avsluttet påloggingen med en personlig konto og viser til Antigravity. Tidligere Gemini-tråder kan fortsatt leses.

## Bygg fra kildekoden

Du trenger:

- macOS 14 eller nyere med Xcode Command Line Tools (`xcode-select --install`); Xcode selv trengs ikke.
- Rust slik `rust-toolchain.toml` angir. Homebrews `rust` bygger for prosessoren i denne Macen; det universelle bygget av en utgivelse krever rustup med målene `aarch64-apple-darwin` og `x86_64-apple-darwin`.
- Utsjekkinger av [splat](https://github.com/sventmueller/splat) og [lowspeak](https://github.com/sventmueller/lowspeak) ved siden av denne (`../splat`, `../lowspeak`), begge på branchen `main`; workspacet henter crates fra dem via sti.
- cmake, for hvert bygg med diktering (`make run`, `make build`, `make release`), som bygger whisper.cpp. `make check` bygger uten.

| Kommando | Hva den gjør |
|---|---|
| `make run` | bygger feilsøkingsappen (`target/debug/Dango Debug.app`) og åpner den |
| `make build` | bygger utgivelsesappen (`target/release/Dango.app`), signert med et lokalt sertifikat som det første bygget oppretter i påloggingsnøkkelringen; lar `VERSION` være uendret |
| `make lint` | formatering og clippy; advarsler får den til å feile |
| `make check` | `lint`, deretter alle tester; trenger ingen nøkkelring, intet vindu og intet nettverk |
| `make test` | `check`, deretter selvtesten `probe` og tilgjengelighetskontrollen, venstre mot høyre og høyre mot venstre, i den ekte appen |
| `make perf` | tidsbudsjettene i et utgivelsesbygg; meningsfullt bare på en Mac som ellers er i ro |
| `make release` | en prøvekjøring av utgivelsen til `target/release-dry-run/`, signert med det lokale sertifikatet; sender ingenting noe sted |
| `make release-notarized` | `check`, deretter selve utgivelsen til `dist/`: Developer ID-signatur, notarisering, oppdateringsstrømmen signert med appens egen nøkkel; lar `VERSION` være uendret |
| `make deploy` | på en ren `main` med grønt CI-segl: notariserer neste versjon, oppdaterer og committer `VERSION`, laster opp utgivelsesfilene og pusher koden, CI-seglet og versjonstaggen; GitHub publiserer utgivelsen automatisk |

Feilsøking og utgivelse er separate apper med egne innstillinger og data. Bygging, testing, selvtestene, utgivelsen og reglene koden følger, er beskrevet i [docs/development.md](../../../docs/development.md), beslutningene bak i [docs/plan.md](../../../docs/plan.md). Utgivelsesnotatene er [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), oversatt ved siden av hver README i `assets/readme/<language>/`.

## Lisens

Dango er lisensiert under enten

- Apache-lisensen, versjon 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-lisensen ([LICENSE-MIT](../../../LICENSE-MIT))

etter eget valg.

Med mindre du uttrykkelig sier noe annet, skal ethvert bidrag som du bevisst sender inn for å bli tatt med i Dango, slik det er definert i Apache-2.0-lisensen, ha dobbel lisens som ovenfor, uten ytterligere vilkår.
