<p><img src="../../readme-icon.png" alt="Dango-symbol" width="147" height="147"></p>

# Dango

**Den inbyggda macOS-redigeraren för arbete med kodagenter.**

Dango samlar koden, agenterna som ändrar den och allt runt omkring i ett fönster per mapp: en redigerare med språkservrar och förutsägelser medan du skriver, terminaler, en webbläsare, Git, CI, en felsökare och AI-trådar med Claude, Codex, Antigravity, OpenCode eller en agent som är inbyggd i appen. Varje agent arbetar i samma tidslinje, frågar innan den ändrar filer eller kör kommandon och lämnar en kontrollpunkt före varje prompt, så att du kan granska och spola tillbaka det den gjort, meddelande för meddelande. Inget som en mapp konfigurerar körs innan du litar på den, och text utifrån når en agent märkt som opålitlig. Diktera i redigeraren och meddelandefältet, och taligenkänningen kan stanna på din Mac. Dango är skrivet i Rust på äkta AppKit-reglage, utan Electron, talar 35 språk och har ingen telemetri.

[![Hämta Dango.dmg](https://img.shields.io/badge/H%C3%A4mta_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Hämta Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: signerad och notariserad av Apple, för macOS 14 eller senare på Apple Silicon och Intel. Länken blir tillgänglig med den första utgåvan.

**Hittat en bugg?** [Öppna ett issue](https://github.com/sventmueller/dango/issues/new) och berätta med vilken agent och i vilken sorts projekt det hände.

[![Rust, utgåva 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 eller senare](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universell: Apple Silicon och Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 språk i gränssnittet](https://img.shields.io/badge/languages-35-4C9A2A)
![Ingen telemetri](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signerad med Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licens: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licens)

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
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
**Svenska** ·
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

## Innehåll

- [Funktioner i korthet](#funktioner-i-korthet)
- [Krav](#krav)
- [Installation](#installation)
- [Kom igång](#kom-igång)
- [AI-agenter](#ai-agenter)
  - [Tidslinjen](#tidslinjen)
  - [Behörigheter och lägen](#behörigheter-och-lägen)
  - [Kontrollpunkter och worktrees](#kontrollpunkter-och-worktrees)
- [Redigeraren](#redigeraren)
- [Språkservrar](#språkservrar)
- [Tester](#tester)
- [Felsökare](#felsökare)
- [Terminal](#terminal)
- [Webbläsare](#webbläsare)
- [Git](#git)
- [CI](#ci)
- [Containrar](#containrar)
- [Markdown](#markdown)
- [Diktering](#diktering)
- [Förtroende](#förtroende)
- [Sandlåda](#sandlåda)
- [Utseende och hjälpmedel](#utseende-och-hjälpmedel)
- [Språk](#språk)
- [Kortkommandon](#kortkommandon)
- [Integritet och säkerhet](#integritet-och-säkerhet)
- [Var data lagras](#var-data-lagras)
- [Kända begränsningar](#kända-begränsningar)
- [Bygga från källkoden](#bygga-från-källkoden)
- [Licens](#licens)

## Funktioner i korthet

- **Alla agenter i en tidslinje.** Claude via sitt eget kommando `claude`, Codex, Antigravity och OpenCode via Agent Client Protocol, fler från ACP-registret och en agent inbyggd i appen för Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama och LM Studio. Svar, verktygsanrop, behörighetsförfrågningar, planer, att göra-listor, underagenter och bakgrundsuppgifter ser likadana ut oavsett vilken agent som kör.
- **Granska och spola tillbaka.** Före varje prompt sparar Dango en kontrollpunkt. Det en agent har ändrat kan granskas och spolas tillbaka per meddelande, och en tråd kan arbeta i ett eget Git-worktree och lämna över sitt arbete genom sammanslagning eller cherry-pick.
- **Ett fönster per mapp.** Filer, terminaler, webbsidor, AI-trådar, diffar och förhandsvisningar är macOS-flikar i det fönstret. Sidofältet innehåller Filer, Sök, Agenter, Git, Tester, Felsök, CI och Containrar.
- **En riktig kodredigerare.** Syntaxfärger, flera insättningspunkter, vikning, `.editorconfig`, språkservrar för 17 språk med komplettering, gå till definition, referenser, byt namn, snabbkorrigeringar och hierarkier, och förutsägelser medan du skriver från GitHub Copilot eller en lokal modell.
- **Felsökare, Git och CI inbyggda.** Brytpunkter och stegvis körning för Rust, C, C++, Swift, Go, JavaScript, TypeScript och Python; stagea enskilda ändringsblock, grenar, stashar, konflikter och en commitgraf; körningar, jobb och loggar från GitHub Actions och GitLab CI.
- **Försiktig med dina mappar.** En mapp du inte litar på startar ingenting: inga Git-processer, språkservrar, hooks eller agenter. Destruktiva Git-kommandon tillåts aldrig för gott, och commits kontrolleras efter hemligheter.
- **Diktering.** Tala in i redigeraren och meddelandefältet, online via OpenAI eller Groq eller lokalt med Whisper och Parakeet.
- **Inbyggd ända in i kärnan.** AppKit-fönster, -flikar, -menyer och -verktygsfält, TextKit 2 för redigeraren, Metal för terminalen, WebKit bara i webbläsarfliken. Mörkt läge, Öka kontrast och VoiceOver fungerar som i alla Mac-appar.
- **På ditt språk.** Gränssnittet finns på 35 språk: arabiska, katalanska, kinesiska (förenklad), kinesiska (traditionell), kroatiska, tjeckiska, danska, nederländska, engelska, finska, franska, tyska, grekiska, hebreiska, hindi, ungerska, indonesiska, italienska, japanska, koreanska, malajiska, norska, polska, portugisiska (Brasilien), portugisiska (Portugal), rumänska, ryska, slovakiska, slovenska, spanska, svenska, thailändska, turkiska, ukrainska och vietnamesiska.

## Krav

- macOS 14 Sonoma eller senare, på Apple Silicon eller Intel. Dango utvecklas och testas på macOS 27.
- För AI-trådarna den agent du vill använda; se [AI-agenter](#ai-agenter).
- För språkservrar och felsökningsadaptrar, som Dango installerar när du ber om det: Go för gopls och `dlv`; Node.js för servrarna för TypeScript, Python, JSON, YAML, CSS, HTML och skal och JavaScript-felsökaren; Xcode Command Line Tools för clangd, sourcekit-lsp och `lldb-dap`. rust-analyzer hämtas från sina GitHub-versioner.
- För förutsägelser medan du skriver, ett GitHub Copilot-abonnemang (det kostnadsfria räcker) eller en fill-in-the-middle-modell i Ollama eller LM Studio.

## Installation

**Från skivavbilden.** Varje version är en skivavbild, `Dango-<version>.dmg`, med en universell app för Apple Silicon och Intel, signerad med ett Developer ID och notariserad av Apple. Öppna den och dra Dango till mappen Program. En Homebrew Cask installerar samma avbild. Ingen version har släppts än.

**Uppdateringar.** Dango från skivavbilden håller sig själv uppdaterad med [Sparkle](https://sparkle-project.org). Vid andra starten frågar appen om den själv får leta efter uppdateringar; tills du säger ja gör den inga förfrågningar. **Dango › Leta efter uppdateringar…** letar direkt. Uppdateringar är signerade med appens egen nyckel, som Sparkle kontrollerar innan den installerar något. En version du byggt själv har ingen uppdateringsfunktion.

**Från källkoden.** Se [Bygga från källkoden](#bygga-från-källkoden) och flytta sedan `target/release/Dango.app` till `/Applications`.

**Terminalkommandot.** **Dango › Installera kommandot ”dango”…** skriver ett litet skript till `~/.local/bin/dango`, utan administratörsbehörighet; `~/.local/bin` måste finnas i din `PATH`. Sedan öppnar `dango .` den aktuella mappen och `dango file.rs` en fil. Dango ersätter aldrig en fil där som den inte själv har skrivit.

## Kom igång

1. **Öppna en mapp** med ⌘O eller `dango <folder>`. Första gången frågar Dango om du litar på den (se [Förtroende](#förtroende)).
2. **Sidofältet** växlar mellan Filer (⌘1), Sök (⌘2), Agenter (⌘3), Git (⌘4), Tester (⌘5), Felsök (⌘6), CI (⌘7) och Containrar (⌘8); ett högerklick gömmer de du inte använder, och ⌃⌘S fäller undan sidofältet.
3. **Varje dokument är en flik** i mappens fönster, macOS egna flikar. ”+” i slutet av flikfältet och ⌘T erbjuder en ny fil, en terminal, webbläsaren och varje installerad agent, även i ett eget Git-worktree; håller du in ”+” visas samma sak som en meny.
4. **Öppna snabbt** med ⌘P hittar filer, och med `#` projektets symboler; ⇧⌘F söker och ersätter i projektet.
5. **Prata med en agent** i en AI-flik. ⌘Esc flyttar tangentbordsfokus till dess meddelandefält och tillbaka.
6. **Inställningar** med ⌘, innehåller flikarna Allmänt, CI, Agenter, Egen agent, Språkserver, Diktering och Förutsägelse.

Verktygsfältet innehåller bakåt och framåt genom ställena du har redigerat (⌃- och ⌃⇧-), grenen, Öppna snabbt och aktivitetsöversikten, som visar allt som körs. **Öppna bredvid** i en fils kontextmeny visar ett andra dokument bredvid det första. Efter en omstart eller en krasch kommer varje fönster tillbaka med sina flikar, och redigerare med osparad text kommer tillbaka med texten. Bilder, ljud, video, typsnitt och Office-dokument öppnas i en flik via Snabbtitt, PDF-filer via PDFKit.

## AI-agenter

| Agent | Så körs den | Det behöver den |
|---|---|---|
| Claude | kommandot `claude`, som Dango talar med direkt via dess stream-json-protokoll | `claude` installerat och inloggat; Dango letar i inloggningsskalets `PATH`, i `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` och `/usr/local/bin`. Inloggning sker bara i `claude` självt |
| Codex | `codex-acp` via Agent Client Protocol (ACP) | Node.js och npm; Dango installerar det låsta paketet när du ber om det |
| Antigravity | `agy acp` | ditt eget `agy` i `PATH`, eller den låsta hämtningen, installerad när du ber om det |
| OpenCode | `opencode acp` | ditt eget `opencode` i `PATH`, eller den låsta hämtningen, installerad när du ber om det |
| Andra ACP-agenter | från ACP-registret, som bara hämtas när du ber om det | det registrets post anger; poster utan kontrollsumma installeras först när du har bekräftat |
| Egen agent | inbyggd i appen, med verktyg för filer, sökning, kommandon, diagnostik och webbförfrågningar | en API-nyckel för Anthropic, OpenAI, Google Gemini eller OpenRouter, sparad i nyckelringen; Ollama eller LM Studio på den här Macen utan nyckel; alla servrar som talar OpenAI:s eller Anthropics API |

**Inställningar › Agenter** visar vilka agenter den här Macen har, installerar de andra och deras uppdateringar och öppnar ACP-registret. Agenter som Dango installerar är låsta till testade versioner och kontrollerade mot sina publicerade kontrollsummor (npm integrity eller SHA-256); Dango kör inga `npx`-paket utan version.

### Tidslinjen

Varje agent arbetar i en AI-flik med samma tidslinje: svar, verktygsanrop med sina utdata, behörighetsförfrågningar, frågor, planer att godkänna och kommentera (⌥⌘K), att göra-listor, underagenter och bakgrundsuppgifter. Två knappar i flikens överdel listar underagenterna och bakgrundsprocesserna som körs. Nämn en fil, `@terminal` eller `@browser` för att ge agenten innehållet. En grön prick på fliken visar en session som arbetar, en röd en session som väntar på dig, och en notis berättar när den är klar eller väntar medan du tittar någon annanstans. För Claude visar statusraden kontexten och användningen.

### Behörigheter och lägen

Nya trådar börjar i läget ”Manuell”: agenten frågar innan den ändrar filer eller kör kommandon. För destruktiva Git-kommandon (`push --force`, `reset --hard`, `clean -fd`) erbjuder Dango inget ”Tillåt alltid”. Den egna agenten sparar sina ”Tillåt alltid”-regler per projekt, listar dem i **Inställningar › Egen agent** och frågar vid destruktiva Git-kommandon varje gång, oavsett vad som är sparat; Claude och ACP-agenterna sparar sådana regler i sin egen konfiguration. Med ”Tillåt allt” och utan sandlåda visar tråden en permanent varning.

### Kontrollpunkter och worktrees

Före varje prompt sparar Dango en kontrollpunkt, som en commit under `refs/ide-checkpoints/` i projektets repo, eller i en egen mapp för ett projekt utan Git; en vanlig push eller fetch tar aldrig med dem. Det en agent har ändrat kan granskas och spolas tillbaka per meddelande. En tråd kan arbeta i ett eget Git-worktree: Git-panelen visar vad den har ändrat och tar över arbetet genom sammanslagning eller cherry-pick.

## Redigeraren

- **Syntaxfärger, radnummer och ändringsmarkeringar** mot senaste commit, sök och ersätt, Öppna snabbt (⌘P) och sökning i projektet (⇧⌘F).
- **Indrag** följer filen och `.editorconfig`: tabbar eller blanksteg, automatiskt indrag, parentespar.
- **Flera insättningspunkter:** ⌥-klick, ⌥-dra för en kolumn, ⌃⇧↑ och ⌃⇧↓, ⌘D för nästa förekomst.
- **Vikning** (⌥⌘← och ⌥⌘→, med ⇧ för allt), växla kommentar (⌘/), gå till rad (⌃G), synliga blanksteg och radbrytning (⌥Z).
- **Filer behåller sin teckenkodning**, byte order mark och radslut. En fil som ändras på disken läses in igen när redigeraren inte har någon osparad text; annars erbjuder ett fält skillnaderna och en sammanslagning.
- **Förutsägelser medan du skriver.** Medan du skriver visas ett förslag i grått, och Tabb tar det. Det kommer från GitHub Copilot eller från en lokal modell i Ollama eller LM Studio, enligt valet i **Inställningar › Förutsägelse**; en lokal modell håller din kod på den här Macen.
- **Minikarta och fäst rullning** (menyn Redigerare): en smal bild av hela filen bredvid texten, och rubrikraderna för de block som den översta raden ligger i, fästa ovanför den.
- **Diagnostik vid radslut** (menyn Innehåll): varje rads allvarligaste meddelande efter en sekunds vila, med en symbol för dess allvarlighetsgrad.
- **Live-förhandsvisning av statiska sidor:** Redigerare › Öppna i webbläsarflik låter Dango själv servera sidorna i en mapp du litar på och läser in fliken igen när en fil sparas.
- **Prettier** formaterar JavaScript, TypeScript, CSS, HTML, Markdown med mera där projektet konfigurerar det, med projektets eget `node_modules/.bin/prettier`; inställningen `formatter` väljer formaterare per språk.
- **Vim-läge** (Redigerare › Vim-läge): lägena Normal, Infoga, Visuell och Ersätt, antal, operatorer med rörelser och textobjekt, `.`, register, märken och makron. `u` och ⌃R använder redigerarens Ångra, och allt som infogas i en omgång är ett steg. `"+` och `"*` är urklippet; inställningen `vimClipboard` gör även det namnlösa registret till urklippet. `/`, `?`, `n`, `N`, `*`, `:s` och `:%s` söker med Rusts reguljära uttryck, inte med Vims mönster: inget `\v`, och `\b` i stället för `\<` och `\>`. Ex-kommandon är `:w`, `:q`, `:wq`, `:x`, `:noh` och `:<line>`. I läget Normal tar Vim ⌃G från gå till rad.
- **Synkronisering av inställningar** via en mapp på iCloud Drive (**Inställningar › Allmänt**): dina inställningar och kortkommandon följer med dig till dina andra Mac-datorer; sökvägar, lokala adresser och medgivanden stannar på var och en.

## Språkservrar

Go, Rust, TypeScript och JavaScript, Python, C, C++ och Objective-C, Swift, skalskript, JSON, YAML, CSS, HTML, TOML, Markdown och Lua, och egna servrar från **Inställningar › Språkserver**. Dango installerar en server när du ber om det, i sin egen verktygsmapp, och använder ett TypeScript-projekts eget `tsc` när det kan fungera som språkserver.

Det här ger de: diagnostik (⌘' och ⇧⌘' går mellan dem), hovring (⌃⌘I), komplettering (⌥Esc), signaturhjälp, gå till definition (⌃⌘J), typdefinition och implementering, referenser (⌃⇧⌘F), anrops- och typhierarkier (⌃⇧⌘H), byt namn (⌃⌘E), formatering (⌥⇧F), semantiska färger, vikning och dispositionen (⇧⌘O); typer och parameternamn inne i raden; snabbkorrigeringar och refaktoreringar bakom en glödlampa (⌥⌘A); projektets symboler med ⌥⌘O.

Utöver en fils egen server kontrollerar ESLint JavaScript och TypeScript i projekt som använder det, med dess snabbkorrigeringar, och Emmets förkortningar visas i kompletteringen för HTML, CSS och JSX. Dango installerar båda när du ber om det; **Inställningar › Allmänt** åtgärdar ESLint-problem när du sparar, om du vill.

## Tester

Panelen Tester visar en mapps tester per paket och fil med deras status: `cargo test` eller cargo-nextest, Go, pytest, Vitest och Jest. En symbol vid varje test i marginalen kör det, ⌥-klick felsöker det, och en misslyckad assert markerar sin rad. Menyn Felsök kör testet vid insättningspunkten (⌃⌥⌘U), de senaste testerna igen (⌃⌥⌘G) eller alla, och felsöker testet vid insättningspunkten. Tester körs bara i mappar du litar på, eftersom de kör projektets kod; `cargo test` listar sina tester först när du ber om det, eftersom listningen bygger varje testprogram.

## Felsökare

Brytpunkter, även med villkor och som loggpunkter, stegvis körning, trådar med sina anropsstackar, variabler och bevakade uttryck i inspektören (⌥⌘I), värden bredvid koden medan programmet är pausat, och en felsökningskonsol som utvärderar i den pausade ramen.

| Språk | Felsökningsadapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` från Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScript-felsökaren, på Node.js |
| Python | debugpy |

Ett projekts `.vscode/launch.json` läses. Menyn Felsök använder kortkommandona från Xcode: ⌘\\ sätter en brytpunkt, ⌘Y slår på eller av alla, ⌃⌘Y fortsätter, F6, F7 och F8 stegar över, in och ut, ⌘. stoppar.

## Terminal

Terminaler i flikar (⌃\`) med inloggningsskalets miljö, ritade med Metal. ⌘K rensar skärmen. En inklistrad eller indragen bild går till en kodagent som körs i terminalen, eller klistrar in sökvägen till en sparad kopia. ⌘-klick öppnar länkar och filnamn. En utvecklingsserver som terminalen startar kan öppnas i webbläsarfliken.

## Webbläsare

En webbläsarflik med adressfält (⌘L), bakåt och framåt (⌘[ och ⌘]), läs in igen (⌘R), sökning, hämtningar som frågar först, och webbinspektören. `@browser` ger sidan till en agent, märkt som text utifrån.

## Git

Git-panelen stagear och slänger filer eller enskilda ändringsblock, gör commits, committar och pushar eller synkroniserar, byter och skapar grenar, sparar stashar, löser sammanslagningskonflikter och ritar en graf över de senaste commitsen. Blame visar vem som senast ändrade raden under markören, eller en kolumn för hela filen. En knapp skriver commitmeddelandet med AI; diffen skickas först när gitleaks inte hittat någon hemlighet i den, och varje commit kontrolleras också efter hemligheter.

## CI

GitHub Actions och GitLab CI, även GitHub Enterprise Server och egendriftat GitLab:

- körningar, jobb och loggar för den aktuella grenen;
- köra om misslyckade jobb, ett enskilt jobb eller, på GitHub, en hel körning; starta manuella jobb eller godkänna väntande driftsättningar; starta en ny pipeline eller ett arbetsflöde som får startas manuellt; avbryta;
- de öppna pull requests eller merge requests och issues, som öppnas i webbläsarfliken;
- loggen från ett misslyckat jobb, given till en agent, märkt som text utifrån.

Token kommer från nyckelringen, `gh` eller `glab`, inloggningsskalets miljö, eller är en du anger i **Inställningar › CI**.

## Containrar

Panelen Containrar visar Dockers Compose-projekt med deras tjänster, fristående containrar, images och volymer, och följer Dockers händelser när de sker. En Compose-fil i mappen visas som ett projekt redan innan någon av dess containrar finns. Containrar startar, stoppar och startas om; deras loggar och ett skal öppnas i terminalflikar, och en publicerad port öppnas i webbläsarfliken. Att radera en container, en image eller en volym och att rensa oanvända data frågar först. Composes up, down och pull körs bara i mappar du litar på, eftersom Compose bygger och kör det som projektet beskriver. Panelens meny väljer Docker-kontexten för fönstret utan att ändra den som dina skal använder; samma kommandon finns i menyn Containrar. Panelen behöver kommandoraden `docker`, Docker 24.0.6 eller senare.

## Markdown

- **Förhandsvisning.** En Markdown-fil öppnas med sin förhandsvisning bredvid; **Innehåll › Visa/göm förhandsvisning bredvid** (⇧⌘V) stänger och öppnar den, och **Inställningar › Allmänt** stänger av den automatiska förhandsvisningen. Förhandsvisningen ritar formler, diagram i Mermaid, Graphviz och PlantUML och mappens bilder; bilder från webben läses bara in i en mapp du litar på eller när du ber om dem i förhandsvisningen.
- **Menyn Format:** fet (⌘B), kursiv (⌘I), genomstruken (⇧⌘X), kod (⌥⇧⌘C), länk (⇧⌘K), rubrik 1–3 (⌥⌘1–⌥⌘3), punktlistor och numrerade listor, uppgifter, citat och kodblock. Varje kommando växlar, på markeringen eller raderna, och är ett ångrasteg.
- **Klistra in.** Text från Word eller en webbsida kommer in som Markdown, dess bilder som filer bredvid dokumentet; ⌥⇧⌘V klistrar in ren text.
- **Export.** **Arkiv › Exportera** skriver PDF, en enda HTML-fil med bilderna inuti, eller ett Word-dokument. Bilder från webben följer bara med från en mapp du litar på.
- **Öppna i Splat.** Där [Splat](https://github.com/sventmueller/splat) är installerat öppnar **Arkiv › Öppna i Splat** och kontextmenyerna i fillistan och flikarna dokumentet där, efter att det har sparats.

## Diktering

Diktera i en AI-fliks meddelandefält eller i redigeraren: med mikrofonknappen där, **Redigera › Starta diktering** (⌃⌥D) eller fn-tangenten (håll ned för att spela in, tryck två gånger för att fortsätta spela in). Taligenkänningen är den från [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak självt behöver inte vara installerat.

- **Inställningar › Diktering** ställer in läge och modell (online med OpenAI eller Groq, lokalt med Whisper eller Parakeet), språket, efterbearbetningen, mikrofonen och ditt eget ordförråd.
- **Lokala modeller** hämtas från Hugging Face när du väljer en, med förlopp och möjlighet att avbryta. De hamnar i mapparna som Lowspeak använder, så att varje app använder det den andra har hämtat. En lokal modell håller inspelningen på den här Macen.
- **Transkribering online** kräver en nyckel för OpenAI eller Groq, som Dango sparar i ett eget objekt i nyckelringen.
- **Från Lowspeak.** Om Lowspeak är inställt annorlunda än standard erbjuder fliken Diktering en gång att ta över dess inställningar, och dess nycklar på begäran; macOS frågar innan Dango läser en (välj ”Tillåt”, inte ”Tillåt alltid”).
- **Behörigheter.** macOS frågar efter mikrofonen vid första dikteringen. fn-tangenten fungerar medan Dango är den aktiva appen och Lowspeak inte körs; den kräver behörigheten för hjälpmedel, som Dango ber om först efter den första dikteringen, aldrig vid start.

## Förtroende

En mapp kan få program att köras så fort något startas i den: Git-hooks och repoinställningar som `core.fsmonitor`, språkservrar från `node_modules`, agent-hooks, `.mcp.json`. Första gången du öppnar en mapp frågar Dango om du litar på den.

- **Betrodd:** Git, språkservrar, felsökare, agenter och CI körs för den här mappen och allt under den.
- **Begränsad:** du kan läsa och redigera; ändringsmarkeringarna i redigeraren kommer från att läsa `.git` direkt. Inget som mappen konfigurerar startas. Panelerna för Git, CI och AI säger det och erbjuder att lita på mappen.

Text som inte kommer från dig kan innehålla instruktioner som ska styra en agent (prompt injection). Utdata från en terminal (`@terminal`), en webbsida (`@browser`), en CI-logg som ges till en agent, en fil utanför projektet och längre text som klistrats in från en webbsida skickas därför märkta: brickan säger ”Externt innehåll”, och agenten får texten med beskedet att det är opålitliga data vars instruktioner den inte får följa. Den egna agentens webbsidor kommer med samma besked. Inklistrad text räknas som text från en webbsida när webbläsaren anger det på urklippet, som Safari, Chrome och Firefox gör; text från annat håll, och en kort fras på några få ord, förblir din egen.

## Sandlåda

Dango självt körs utan App Sandbox, eftersom varje program det startar skulle ärva den: terminaler, Git, språkservrar och agenter skulle inte kunna fungera. I stället har varje AI-tråd en sandlådebrytare, som är av som standard. Vad den begränsar beror på agenten, och brytarens hjälptext säger det:

| Agent | Med sandlådan på |
|---|---|
| Claude | skalkommandon körs i Claudes egen sandlåda; dess verktyg för filer och webb gör det inte |
| Codex | kommandon och ändringar skriver bara i projektet, utan nätverk |
| Egen agent | filer, webbförfrågningar och kommandon håller sig inom projektet och de tillåtna mapparna, utan nätverk (`sandbox-exec`) |
| Antigravity, OpenCode | ingen sandlåda som Dango kan slå på |

## Utseende och hjälpmedel

- **Mörkt läge** följer systemet; **Öka kontrast** och **Minska rörelse** respekteras.
- **VoiceOver** namnger sidofältet, flikarna, korten i AI-tidslinjen, raderna i Git och CI, felsökaren och terminalens text. Brytpunkter skiljs aldrig åt bara med färg.
- **Tangentbord.** Varje kommando har ett kortkommando, och alla kan ändras i `keybindings.json`; ⌥⌘\` går till nästa panel.
- **Höger till vänster.** På arabiska och hebreiska speglas fönstret, medan kodvyer förblir vänster till höger.

Vad Dango lägger till ovanpå AppKit, område för område, står i [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Språk

Gränssnittet är översatt till 35 språk: arabiska, katalanska, kinesiska (förenklad), kinesiska (traditionell), kroatiska, tjeckiska, danska, nederländska, engelska, finska, franska, tyska, grekiska, hebreiska, hindi, ungerska, indonesiska, italienska, japanska, koreanska, malajiska, norska, polska, portugisiska (Brasilien), portugisiska (Portugal), rumänska, ryska, slovakiska, slovenska, spanska, svenska, thailändska, turkiska, ukrainska och vietnamesiska.

Dango följer språkordningen i **Systeminställningar › Allmänt › Språk och region**; där kan du också välja ett språk bara för Dango under **Appar**. Finns ingen översättning för ditt språk används engelska. Termerna följer en ordlista, [docs/glossary.md](../../../docs/glossary.md), och Apples egna översättningar, så att Dango läses som resten av din Mac.

## Kortkommandon

| Åtgärd | Kortkommando |
|---|---|
| Öppna mapp, nytt fönster, stäng fönster | ⌘O, ⇧⌘N, ⇧⌘W |
| Ny fil, spara, spara som | ⌘N, ⌘S, ⇧⌘S |
| Ny flik, stäng flik, nästa och föregående flik | ⌘T, ⌘W, ⇧⌘] och ⇧⌘[ |
| Öppna snabbt, sök i projektet | ⌘P, ⇧⌘F |
| Filer, Sök, Agenter, Git, Tester, Felsök, CI, Containrar | ⌘1 till ⌘8 |
| Visa eller göm sidofältet, nästa panel | ⌃⌘S, ⌥⌘\` |
| Bakåt och framåt genom redigerade ställen | ⌃- och ⌃⇧- |
| Växla kommentar, gå till rad | ⌘/, ⌃G |
| Nästa förekomst, insättningspunkt ovanför och nedanför | ⌘D, ⌃⇧↑ och ⌃⇧↓ |
| Vik, fäll ut, vik allt, fäll ut allt | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Radbrytning, inspektör | ⌥Z, ⌥⌘I |
| Komplettering, hovring, gå till definition | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referenser, anropshierarki, byt namn | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboler i filen, i projektet | ⇧⌘O, ⌥⌘O |
| Snabbkorrigering, formatera | ⌥⌘A, ⌥⇧F |
| Nästa och föregående problem | ⌘' och ⇧⌘' |
| Markdown-förhandsvisning, klistra in som ren text | ⇧⌘V, ⌥⇧⌘V |
| Fet, kursiv, genomstruken, kod, länk | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Rubrik 1 till 3 | ⌥⌘1 till ⌥⌘3 |
| Ny terminal, rensa terminalen | ⌃\`, ⌘K |
| Fokus på agentens meddelandefält, kommentera en plan | ⌘Esc, ⌥⌘K |
| Adressfält, läs in igen, bakåt, framåt (webbläsare) | ⌘L, ⌘R, ⌘[, ⌘] |
| Starta eller stoppa diktering | ⌃⌥D |
| Brytpunkt, alla brytpunkter på eller av | ⌘\\, ⌘Y |
| Kör testet vid insättningspunkten, kör de senaste testerna igen | ⌃⌥⌘U, ⌃⌥⌘G |
| Fortsätt, stega över, in, ut, stoppa | ⌃⌘Y, F6, F7, F8, ⌘. |
| Inställningar | ⌘, |

`keybindings.json` i appens datamapp (se [Var data lagras](#var-data-lagras)) ändrar vilket som helst av dem.

## Integritet och säkerhet

- **Ingen telemetri,** ingen analys, inga kraschrapporter som skickas någonstans. Fel går till systemloggen (Konsol, under appens namn).
- **Nätverket bara för det du startar:** agenterna du använder, var och en med sin egen tjänst; ACP-registret och hämtningar av agenter, språkservrar, felsökningsadaptrar och dikteringsmodeller när du ber om dem; CI-värdarna du loggar in på; förutsägelser från Copilot om du har valt det; den egna agentens leverantörer; transkribering online om du har valt det; sidorna du öppnar i webbläsarfliken; och sökningen efter uppdateringar när du har tillåtit den.
- **Kod lämnar Macen** bara där du har valt en tjänst för det: en agent, Copilots förutsägelser, ett commitmeddelande skrivet med AI. Diffen för ett commitmeddelande skickas först när gitleaks inte hittat någon hemlighet i den.
- **Nycklar och token** stannar i macOS-nyckelringen, aldrig i en inställningsfil. Andra appars nycklar, som Lowspeaks, läses bara när du säger ja, och bara en gång.
- **Låsta hämtningar.** Agenter, språkservrar och felsökningsadaptrar som Dango installerar är låsta till testade versioner och kontrollerade mot sina kontrollsummor.
- **Kodsignering** med Hardened Runtime; uppdateringar installeras bara med appens egen signatur.

## Var data lagras

`<id>` är paket-ID:t, `local.agentide` för versionen (`local.agentide.debug` för felsökningsbyggen). Det innehåller inte visningsnamnet, så ett namnbyte behåller alla data.

| Plats | Vad |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (AI-panelens lista), `agent-threads/` (den egna agentens konversationer), `layouts/` (flikar per projekt), `recovery/` (osparade redigerare efter en krasch), `checkpoints/` (för mappar utan Git), `worktrees/`, `agents/` och `tools/` (installerade agenter, språkservrar och felsökningsadaptrar), `task-output/` |
| `~/Library/Caches/<id>/` | inklistrade bilder, skript för skalintegration, ACP-registret; kan raderas |
| `~/Library/Logs/<id>/dictation/` | dikteringens loggfiler |
| `~/Library/Application Support/Lowspeak` och `FluidAudio` | lokala dikteringsmodeller, delade med Lowspeak |
| `~/Library/WebKit/<id>/` | cookies och webbplatsdata från webbläsarfliken |
| `<project>/.dango/settings.json` | projektets egna inställningar |
| projektets Git-repo | kontrollpunkter, som commits under `refs/ide-checkpoints/`; en vanlig push eller fetch tar aldrig med dem |
| nyckelring, tjänst `<id>` | API-nycklar för den egna agenten och dikteringen, CI-token |
| `~/.local/bin/dango` | terminalkommandot, om du har installerat det |

Claude sparar sina egna sessioner under `~/.claude/projects`; Dango läser dem för att återuppta en session men ändrar dem inte.

## Kända begränsningar

- **Ingen App Sandbox** för själva appen, eftersom terminaler, Git, språkservrar och agenter skulle ärva den; en mapp måste vara betrodd innan något den konfigurerar körs. Antigravity och OpenCode har ingen sandlåda som Dango kan slå på.
- **Claudes användningssiffror och uppdelningen av kontexten** bygger på delar av protokollet som är märkta som experimentella, och göms när de saknas.
- **Att spola tillbaka konversationen** fungerar bara för Claude; för de andra agenterna spolas bara koden tillbaka.
- **Inloggning till CI via webbläsaren** är avstängd; använd `gh`, `glab`, en token eller miljön.
- **Gemini CLI erbjuds inte:** Google har slutat med inloggning med ett personligt konto och hänvisar till Antigravity. Tidigare Gemini-trådar går fortfarande att läsa.

## Bygga från källkoden

Du behöver:

- macOS 14 eller senare med Xcode Command Line Tools (`xcode-select --install`); Xcode självt behövs inte.
- Rust som `rust-toolchain.toml` anger. Homebrews `rust` bygger för den här Macens processor; det universella bygget av en version kräver rustup med målen `aarch64-apple-darwin` och `x86_64-apple-darwin`.
- Utcheckningar av [splat](https://github.com/sventmueller/splat) och [lowspeak](https://github.com/sventmueller/lowspeak) bredvid den här (`../splat`, `../lowspeak`), båda på sin gren `main`; arbetsytan tar crates från dem via sökväg.
- cmake, för varje bygge med diktering (`make run`, `make build`, `make release`), som bygger whisper.cpp. `make check` bygger utan.

| Kommando | Vad det gör |
|---|---|
| `make run` | bygger felsökningsappen (`target/debug/Dango Debug.app`) och öppnar den |
| `make build` | bygger versionsappen (`target/release/Dango.app`), signerad med ett lokalt certifikat som det första bygget skapar i inloggningsnyckelringen; lämnar `VERSION` oförändrad |
| `make lint` | formatering och clippy; varningar får det att misslyckas |
| `make check` | `lint`, sedan alla tester; behöver ingen nyckelring, inget fönster och inget nätverk |
| `make test` | `check`, sedan självtestet `probe` och hjälpmedelsgranskningen, vänster till höger och höger till vänster, i den riktiga appen |
| `make perf` | tidsbudgetarna i ett versionsbygge; meningsfullt bara på en Mac som annars är i vila |
| `make release` | en provkörning av versionen till `target/release-dry-run/`, signerad med det lokala certifikatet; skickar ingenting någonstans |
| `make release-notarized` | `check`, sedan själva versionen till `dist/`: Developer ID-signatur, notarisering, uppdateringsflödet signerat med appens egen nyckel; lämnar `VERSION` oförändrad |
| `make deploy` | på en ren `main`, med eller utan CI-sigill: notariserar nästa version, committar `VERSION`, laddar upp utgivningsfilerna, pushar koden och versionstaggen, ett grönt CI-sigill med om det finns, och publicerar sedan utgåvan själv |

Felsökning och version är separata appar med egna inställningar och data. Bygge, tester, självtesterna, versionen och reglerna som koden följer beskrivs i [docs/development.md](../../../docs/development.md), besluten bakom i [docs/plan.md](../../../docs/plan.md). Versionsinformationen är [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), översatt bredvid varje README i `assets/readme/<language>/`.

## Licens

Dango är licensierat under någon av

- Apache-licensen, version 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-licensen ([LICENSE-MIT](../../../LICENSE-MIT))

enligt ditt val.

Om du inte uttryckligen anger något annat ska varje bidrag som du avsiktligt skickar in för att tas med i Dango, enligt definitionen i Apache-2.0-licensen, dubbellicensieras enligt ovan, utan ytterligare villkor.
