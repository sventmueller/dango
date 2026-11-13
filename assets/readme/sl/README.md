<p><img src="../../readme-icon.png" alt="Ikona aplikacije Dango" width="168" height="168"></p>

# Dango

**Izvorni urejevalnik za macOS za delo z agenti za programiranje.**

Dango združi kodo, agente, ki jo spreminjajo, in vse okoli njih v eno okno na mapo: urejevalnik z jezikovnimi strežniki in napovedovanjem med tipkanjem, terminale, brskalnik, Git, CI, razhroščevalnik in niti AI s Claude, Codex, Antigravity, OpenCode ali agentom, vgrajenim v aplikacijo. Vsak agent dela v isti časovnici, vpraša, preden spremeni datoteke ali zažene ukaze, in pred vsakim pozivom pusti kontrolno točko, tako da lahko to, kar je naredil, pregledate in previjete nazaj sporočilo za sporočilom. Nič, kar nastavi mapa, se ne zažene, preden ji zaupate, besedilo od zunaj pa pride do agenta označeno kot nezaupanja vredno. Narekujte v urejevalnik in v polje za sporočilo; prepoznavanje govora lahko ostane v vašem Macu. Dango je napisan v jeziku Rust na pristnih kontrolnikih AppKit, brez Electrona, govori 35 jezikov in nima telemetrije.

**Še ni izdan.** Prva izdaja bo slika diska, podpisana z Developer ID in notarizirana pri Applu, za macOS 14 ali novejši na Apple Silicon in Intel. Do takrat [zgradite Dango iz izvorne kode](#gradnja-iz-izvorne-kode).

**Ste našli napako?** [Odprite issue](https://github.com/sventmueller/dango/issues/new) in navedite, pri katerem agentu in pri kakšnem projektu se je pojavila.

[![Rust, izdaja 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 ali novejši](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Univerzalno: Apple Silicon in Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit prek objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 jezikov vmesnika](https://img.shields.io/badge/languages-35-4C9A2A)
![Brez telemetrije](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Podpisano s Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licenca: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licenca)

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
**Slovenščina** ·
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

## Vsebina

- [Funkcije na kratko](#funkcije-na-kratko)
- [Zahteve](#zahteve)
- [Namestitev](#namestitev)
- [Prvi koraki](#prvi-koraki)
- [Agenti AI](#agenti-ai)
  - [Časovnica](#časovnica)
  - [Dovoljenja in načini](#dovoljenja-in-načini)
  - [Kontrolne točke in worktreeji](#kontrolne-točke-in-worktreeji)
- [Urejevalnik](#urejevalnik)
- [Jezikovni strežniki](#jezikovni-strežniki)
- [Razhroščevalnik](#razhroščevalnik)
- [Terminal](#terminal)
- [Brskalnik](#brskalnik)
- [Git](#git)
- [CI](#ci)
- [Markdown](#markdown)
- [Narekovanje](#narekovanje)
- [Zaupanje](#zaupanje)
- [Peskovnik](#peskovnik)
- [Videz in dostopnost](#videz-in-dostopnost)
- [Jeziki](#jeziki)
- [Bližnjice na tipkovnici](#bližnjice-na-tipkovnici)
- [Zasebnost in varnost](#zasebnost-in-varnost)
- [Kje so shranjeni podatki](#kje-so-shranjeni-podatki)
- [Znane omejitve](#znane-omejitve)
- [Gradnja iz izvorne kode](#gradnja-iz-izvorne-kode)
- [Licenca](#licenca)

## Funkcije na kratko

- **Vsi agenti v eni časovnici.** Claude prek lastnega ukaza `claude`, Codex, Antigravity in OpenCode prek Agent Client Protocol, drugi iz registra ACP in agent, vgrajen v aplikacijo, za Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama in LM Studio. Odgovori, klici orodij, zahteve za dovoljenje, načrti, opravki, podagenti in opravila v ozadju so videti enako, ne glede na to, kateri agent teče.
- **Pregled in previjanje nazaj.** Pred vsakim pozivom Dango shrani kontrolno točko. Kar je agent spremenil, lahko pregledate in previjete nazaj po posameznih sporočilih, nit pa lahko dela v lastnem worktreeju Git in svoje delo preda z združitvijo ali cherry-pickom.
- **Eno okno na mapo.** Datoteke, terminali, strani brskalnika, niti AI, diffi in predogledi so zavihki macOS tega okna. V stranski vrstici so Datoteke, Iskanje, Agenti, Git, Odpravljanje napak in CI.
- **Pravi urejevalnik kode.** Barvanje sintakse, več kazalk, zlaganje, `.editorconfig`, jezikovni strežniki za 17 jezikov z dokončanjem, pojdi na definicijo, sklici, preimenovanjem, hitrimi popravki in hierarhijami ter napovedovanje med tipkanjem iz GitHub Copilot ali lokalnega modela.
- **Vgrajeni razhroščevalnik, Git in CI.** Prekinitvene točke in izvajanje po korakih za Rust, C, C++, Swift, Go, JavaScript, TypeScript in Python; priprava posameznih odsekov za commit, veje, stashi, konflikti in graf commitov; zagoni, jobi in dnevniki iz GitHub Actions in GitLab CI.
- **Previden z vašimi mapami.** Mapa, ki ji niste zaupali, ne zažene ničesar: nobenih procesov Git, jezikovnih strežnikov, hookov ali agentov. Uničujoči ukazi Git nikoli niso dovoljeni za vedno, commiti pa se preverijo glede skrivnosti.
- **Narekovanje.** Govorite v urejevalnik in v polje za sporočilo, prek spleta z OpenAI ali Groq ali lokalno z Whisper in Parakeet.
- **Izvorno do zadnje podrobnosti.** Okna, zavihki, meniji in orodne vrstice AppKit, TextKit 2 za urejevalnik, Metal za terminal, WebKit le v zavihku brskalnika. Temni način, Povečaj kontrast in VoiceOver delujejo kot v vsaki aplikaciji za Mac.
- **V vašem jeziku.** Vmesnik je na voljo v 35 jezikih: arabščina, katalonščina, kitajščina (poenostavljena), kitajščina (tradicionalna), hrvaščina, češčina, danščina, nizozemščina, angleščina, finščina, francoščina, nemščina, grščina, hebrejščina, hindijščina, madžarščina, indonezijščina, italijanščina, japonščina, korejščina, malajščina, norveščina, poljščina, portugalščina (Brazilija), portugalščina (Portugalska), romunščina, ruščina, slovaščina, slovenščina, španščina, švedščina, tajščina, turščina, ukrajinščina in vietnamščina.

## Zahteve

- macOS 14 Sonoma ali novejši, na Apple Silicon ali Intel. Dango je razvit in preizkušen v macOS 27.
- Za niti AI agent, ki ga želite uporabljati; glejte [Agenti AI](#agenti-ai).
- Za jezikovne strežnike in adapterje za razhroščevanje, ki jih Dango namesti na zahtevo: Go za gopls in `dlv`; Node.js za strežnike za TypeScript, Python, JSON, YAML, CSS, HTML in lupino ter za razhroščevalnik za JavaScript; orodja Xcode Command Line Tools za clangd, sourcekit-lsp in `lldb-dap`. rust-analyzer se prenese iz njegovih izdaj na GitHubu.
- Za napovedovanje med tipkanjem naročnina GitHub Copilot (zadošča brezplačna) ali model fill-in-the-middle v Ollama ali LM Studio.

## Namestitev

**S slike diska.** Vsaka izdaja je slika diska `Dango-<version>.dmg` z eno univerzalno aplikacijo za Apple Silicon in Intel, podpisano z Developer ID in notarizirano pri Applu. Odprite jo in povlecite Dango v mapo Aplikacije. Homebrew Cask namesti isto sliko. Nobena izdaja še ni objavljena.

**Posodobitve.** Dango, nameščen s slike diska, se sam posodablja prek [Sparkle](https://sparkle-project.org). Ob drugem zagonu vpraša, ali sme sam preverjati posodobitve; dokler se ne strinjate, ne pošlje nobene zahteve. **Dango › Preveri, ali so na voljo posodobitve …** preveri takoj. Posodobitve so podpisane z lastnim ključem aplikacije, ki ga Sparkle preveri, preden karkoli namesti. Lastna gradnja nima posodabljanja.

**Iz izvorne kode.** Glejte [Gradnja iz izvorne kode](#gradnja-iz-izvorne-kode), nato premaknite `target/release/Dango.app` v `/Applications`.

**Ukaz za terminal.** **Dango › Namesti ukaz »dango« …** zapiše majhen skript v `~/.local/bin/dango`, brez skrbniških pravic; `~/.local/bin` mora biti v vaši spremenljivki `PATH`. Nato `dango .` odpre trenutno mapo in `dango file.rs` datoteko. Dango tam nikoli ne zamenja datoteke, ki je ni zapisal sam.

## Prvi koraki

1. **Odprite mapo** z ⌘O ali `dango <folder>`. Prvič vas Dango vpraša, ali ji zaupate (glejte [Zaupanje](#zaupanje)).
2. **Stranska vrstica** preklaplja med razdelki Datoteke (⌘1), Iskanje (⌘2), Agenti (⌘3), Git (⌘4), Odpravljanje napak (⌘5) in CI (⌘6); z desnim klikom skrijete tiste, ki jih ne uporabljate, ⌃⌘S pa stransko vrstico zloži.
3. **Vsak dokument je zavihek** okna mape, lastni zavihki macOS. »+« na koncu vrstice zavihkov in ⌘T ponujata novo datoteko, terminal, brskalnik in vsakega nameščenega agenta, tudi v lastnem worktreeju Git; če »+« zadržite, se isto prikaže kot meni.
4. **Hitro odpri** z ⌘P najde datoteke, z `#` pa simbole projekta; ⇧⌘F išče in zamenjuje po projektu.
5. **Pogovarjajte se z agentom** v zavihku AI. ⌘Esc premakne fokus tipkovnice v njegovo polje za sporočilo in nazaj.
6. **Nastavitve** z ⌘, vsebujejo zavihke Splošno, CI, Agenti, Lastni agent, Jezikovni strežnik, Narekovanje in Napovedovanje.

V orodni vrstici so nazaj in naprej po mestih, ki ste jih urejali (⌃- in ⌃⇧-), veja, Hitro odpri in pregled Aktivnost, ki navaja vse, kar teče. **Odpri ob strani** v kontekstnem meniju datoteke prikaže drugi dokument ob prvem. Po ponovnem zagonu ali sesutju se vsako okno vrne s svojimi zavihki, urejevalniki z neshranjenim besedilom pa skupaj z njim. Slike, zvok, video, pisave in dokumenti Office se odprejo v zavihku prek Quick Look, PDF-ji prek PDFKit.

## Agenti AI

| Agent | Kako teče | Kaj potrebuje |
|---|---|---|
| Claude | ukaz `claude`, s katerim Dango govori neposredno prek njegovega protokola stream-json | nameščen `claude` z vpisom; Dango ga išče v `PATH` prijavne lupine, v `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` in `/usr/local/bin`. Vpis poteka le v samem `claude` |
| Codex | `codex-acp` prek Agent Client Protocol (ACP) | Node.js in npm; Dango na zahtevo namesti paket v določeni različici |
| Antigravity | `agy acp` | vaš lastni `agy` v `PATH` ali prenos določene različice, nameščen na zahtevo |
| OpenCode | `opencode acp` | vaš lastni `opencode` v `PATH` ali prenos določene različice, nameščen na zahtevo |
| Drugi agenti ACP | iz registra ACP, ki se prenese le na zahtevo | karkoli navaja vnos v registru; vnosi brez kontrolne vsote se namestijo šele po vaši potrditvi |
| Lastni agent | vgrajen v aplikacijo, z orodji za datoteke, iskanje, ukaze, diagnostiko in spletne zahteve | ključ API za Anthropic, OpenAI, Google Gemini ali OpenRouter, shranjen v verigi ključev; Ollama ali LM Studio v tem Macu brez ključa; kateri koli strežnik, ki govori API OpenAI ali Anthropic |

**Nastavitve › Agenti** pokažejo, katere agente ima ta Mac, namestijo druge in njihove posodobitve ter odprejo register ACP. Agenti, ki jih namesti Dango, so vezani na preizkušene različice in preverjeni z objavljenimi kontrolnimi vsotami (npm integrity ali SHA-256); Dango ne zažene nobenega paketa `npx` brez različice.

### Časovnica

Vsak agent dela v zavihku AI z isto časovnico: odgovori, klici orodij z izhodom, zahteve za dovoljenje, vprašanja, načrti za odobritev in komentiranje (⌥⌘K), opravki, podagenti in opravila v ozadju. Gumba v glavi zavihka navedeta podagente in procese v ozadju, ki tečejo. Omenite datoteko, `@terminal` ali `@browser`, da agentu predate njihovo vsebino. Zelena pika na zavihku pokaže sejo pri delu, rdeča sejo, ki čaka na vas, obvestilo pa sporoči, ko se seja konča ali čaka, medtem ko gledate drugam. Pri Claude vrstica stanja prikazuje kontekst in porabo.

### Dovoljenja in načini

Nove niti se začnejo v načinu »Ročno«: agent vpraša, preden spremeni datoteke ali zažene ukaze. Za uničujoče ukaze Git (`push --force`, `reset --hard`, `clean -fd`) Dango ne ponuja možnosti »Vedno dovoli«. Lastni agent hrani svoja pravila »Vedno dovoli« za vsak projekt posebej, jih navede v **Nastavitve › Lastni agent** in za uničujoče ukaze Git vpraša vsakič, ne glede na to, kaj je shranjeno; Claude in agenti ACP hranijo taka pravila v lastni konfiguraciji. Z »Dovoli vse« in brez peskovnika nit prikazuje stalno opozorilo.

### Kontrolne točke in worktreeji

Pred vsakim pozivom Dango shrani kontrolno točko, kot commit pod `refs/ide-checkpoints/` v repozitoriju projekta ali v lastni mapi za projekt brez Gita; običajen push ali fetch jih nikoli ne prenese. Kar je agent spremenil, lahko pregledate in previjete nazaj po posameznih sporočilih. Nit lahko dela v lastnem worktreeju Git: podokno Git pokaže, kaj je spremenila, in prevzame delo z združitvijo ali cherry-pickom.

## Urejevalnik

- **Barvanje sintakse, številke vrstic in oznake sprememb** glede na zadnji commit, iskanje in zamenjava, Hitro odpri (⌘P) in iskanje po projektu (⇧⌘F).
- **Zamikanje** sledi datoteki in `.editorconfig`: tabulatorji ali presledki, samodejno zamikanje, pari oklepajev.
- **Več kazalk:** ⌥-klik, ⌥-vlečenje za stolpec, ⌃⇧↑ in ⌃⇧↓, ⌘D za naslednjo pojavitev.
- **Zlaganje** (⌥⌘← in ⌥⌘→, s ⇧ za vse), preklop komentarjev (⌘/), pojdi na vrstico (⌃G), vidni presledki in prelom vrstic (⌥Z).
- **Datoteke ohranijo kodiranje**, oznako vrstnega reda bajtov in konce vrstic. Datoteka, spremenjena na disku, se znova naloži, če urejevalnik nima neshranjenega besedila; sicer vrstica ponudi razlike in združitev.
- **Napovedovanje med tipkanjem.** Med tipkanjem se v sivi barvi prikaže predlog, tipka Tab pa ga sprejme. Prihaja iz GitHub Copilot ali iz lokalnega modela v Ollama ali LM Studio, kot izberete v **Nastavitve › Napovedovanje**; lokalni model obdrži vašo kodo v tem Macu.

## Jezikovni strežniki

Go, Rust, TypeScript in JavaScript, Python, C, C++ in Objective-C, Swift, skripti lupine, JSON, YAML, CSS, HTML, TOML, Markdown in Lua ter vaši lastni strežniki iz **Nastavitve › Jezikovni strežnik**. Dango strežnik namesti na zahtevo, v lastno mapo z orodji, in uporabi lastni `tsc` projekta TypeScript, kadar lahko deluje kot jezikovni strežnik.

Kaj prinašajo: diagnostiko (⌘' in ⇧⌘' se premikata med njimi), informacije ob lebdenju (⌃⌘I), dokončanje (⌥Esc), pomoč pri podpisih, pojdi na definicijo (⌃⌘J), na definicijo tipa in implementacijo, sklice (⌃⇧⌘F), hierarhije klicev in tipov (⌃⇧⌘H), preimenovanje (⌃⌘E), oblikovanje (⌥⇧F), semantične barve, zlaganje in oris (⇧⌘O); tipe in imena parametrov znotraj vrstice; hitre popravke in refaktoriranja za žarnico (⌥⌘A); simbole projekta z ⌥⌘O.

## Razhroščevalnik

Prekinitvene točke, tudi s pogoji in kot točke dnevnika, izvajanje po korakih, niti programa z njihovimi skladi klicev, spremenljivke in opazovani izrazi v Inšpektorju (⌥⌘I), vrednosti ob kodi, ko je program ustavljen, in konzola za odpravljanje napak, ki vrednoti v ustavljenem okviru.

| Jezik | Adapter za razhroščevanje |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` iz orodij Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | razhroščevalnik za JavaScript, na Node.js |
| Python | debugpy |

Dango prebere `.vscode/launch.json` projekta. Meni Odpravljanje napak uporablja bližnjice iz Xcode: ⌘\\ nastavi prekinitveno točko, ⌘Y vse vklopi ali izklopi, ⌃⌘Y nadaljuje, F6, F7 in F8 preskočijo, vstopijo in izstopijo, ⌘. ustavi.

## Terminal

Terminali v zavihkih (⌃\`) z okoljem prijavne lupine, izrisani z Metal. ⌘K počisti zaslon. Prilepljena ali povlečena slika gre agentu za programiranje, ki teče v terminalu, sicer se prilepi pot do shranjene kopije. ⌘-klik odpre povezave in imena datotek. Razvojni strežnik, ki ga zažene terminal, lahko odprete v zavihku brskalnika.

## Brskalnik

Zavihek brskalnika z naslovnim poljem (⌘L), nazaj in naprej (⌘[ in ⌘]), ponovnim nalaganjem (⌘R), iskanjem, prenosi, ki najprej vprašajo, in spletnim inšpektorjem. `@browser` preda stran agentu, označeno kot besedilo od zunaj.

## Git

Podokno Git pripravi za commit in zavrže datoteke ali posamezne odseke, naredi commit, naredi commit in push ali sinhronizira, preklaplja in ustvarja veje, hrani stashe, razrešuje konflikte združitve in nariše graf zadnjih commitov. Blame pokaže, kdo je nazadnje spremenil vrstico pod kazalko, ali stolpec za celotno datoteko. Gumb napiše sporočilo commita z AI; diff gre ven šele, ko gitleaks v njem ne najde nobene skrivnosti, in tudi vsak commit se preveri glede skrivnosti.

## CI

GitHub Actions in GitLab CI, tudi GitHub Enterprise Server in samostojno gostovan GitLab:

- zagoni, jobi in dnevniki trenutne veje;
- ponovni zagon neuspelih jobov, posameznega joba ali, na GitHubu, celotnega zagona; zagon ročnih jobov ali odobritev čakajočih uvajanj; zagon novega pipelinea ali poteka dela, ki ga je mogoče zagnati ročno; preklic;
- odprti pull requesti ali merge requesti in zadeve, ki se odprejo v zavihku brskalnika;
- dnevnik neuspelega joba, predan agentu, označen kot besedilo od zunaj.

Žeton pride iz verige ključev, iz `gh` ali `glab`, iz okolja prijavne lupine ali pa je tisti, ki ga vnesete v **Nastavitve › CI**.

## Markdown

- **Predogled.** Datoteka Markdown se odpre s predogledom ob strani; **Prikaz › Prikaži/skrij predogled ob strani** (⇧⌘V) ga zapre in odpre, **Nastavitve › Splošno** pa izklopi samodejni predogled. Predogled izriše formule, diagrame Mermaid, Graphviz in PlantUML ter slike iz mape; slike s spleta se naložijo le v zaupanja vredni mapi ali ko jih zahtevate v predogledu.
- **Meni Oblikovanje:** krepko (⌘B), ležeče (⌘I), prečrtano (⇧⌘X), koda (⌥⇧⌘C), povezava (⇧⌘K), naslovi 1–3 (⌥⌘1–⌥⌘3), označeni in oštevilčeni seznami, opravila, citati in bloki kode. Vsak ukaz deluje kot preklop, na izboru ali na vrsticah, in je en korak razveljavitve.
- **Lepljenje.** Besedilo iz Worda ali s spletne strani pride kot Markdown, njegove slike kot datoteke ob dokumentu; ⌥⇧⌘V prilepi navadno besedilo.
- **Izvozi.** **Datoteka › Izvozi** zapiše PDF, eno samo datoteko HTML s slikami v njej ali dokument Word. Slike s spleta gredo zraven le iz zaupanja vredne mape.
- **Odpri v aplikaciji Splat.** Kjer je nameščen [Splat](https://github.com/sventmueller/splat), **Datoteka › Odpri v aplikaciji Splat** in kontekstni meniji drevesa datotek in zavihkov odprejo dokument v njem, potem ko ga shranijo.

## Narekovanje

Narekujte v polje za sporočilo zavihka AI ali v urejevalnik: z gumbom mikrofona tam, z **Uredi › Začni narekovanje** (⌃⌥D) ali s tipko fn (zadržite za snemanje, dvakrat tapnite za nadaljevanje snemanja). Prepoznavanje govora je tisto iz [Lowspeak](https://github.com/sventmueller/lowspeak); samega Lowspeaka ni treba namestiti.

- **Nastavitve › Narekovanje** določijo način in model (prek spleta z OpenAI ali Groq, lokalno z Whisper ali Parakeet), jezik, naknadno obdelavo, mikrofon in vaše lastno besedišče.
- **Lokalni modeli** se prenesejo s Hugging Face, ko izberete enega, s prikazom napredka in možnostjo preklica. Shranijo se v mape, ki jih uporablja Lowspeak, tako da vsaka od obeh aplikacij uporabi, kar je prenesla druga. Lokalni model obdrži posnetek v tem Macu.
- **Spletni prepis** potrebuje ključ za OpenAI ali Groq, ki ga Dango hrani v lastnem vnosu v verigi ključev.
- **Iz Lowspeaka.** Če je Lowspeak nastavljen drugače od privzetih vrednosti, zavihek Narekovanje enkrat ponudi prevzem njegovih nastavitev in na zahtevo njegovih ključev; macOS vpraša, preden Dango katerega prebere (izberite »Dovoli«, ne »Vedno dovoli«).
- **Dovoljenja.** macOS ob prvem narekovanju vpraša za mikrofon. Tipka fn deluje, ko je Dango aktivna aplikacija in Lowspeak ne teče; potrebuje dovoljenje za dostopnost, za katerega Dango vpraša po prvem narekovanju, nikoli ob zagonu.

## Zaupanje

Mapa lahko povzroči, da se programi zaženejo, takoj ko se v njej kaj zažene: hooki Git in nastavitve repozitorija, kot je `core.fsmonitor`, jezikovni strežniki iz `node_modules`, hooki agentov, `.mcp.json`. Ko mapo odprete prvič, vas Dango vpraša, ali ji zaupate.

- **Zaupanja vredna:** Git, jezikovni strežniki, razhroščevalniki, agenti in CI tečejo za to mapo in vse pod njo.
- **Omejena:** lahko berete in urejate; oznake sprememb v urejevalniku izhajajo iz neposrednega branja `.git`. Nič, kar mapa nastavi, se ne zažene. Podokna Git, CI in AI to povedo in ponudijo, da mapi zaupate.

Besedilo, ki ne prihaja od vas, lahko vsebuje navodila, namenjena usmerjanju agenta (prompt injection). Izhod terminala (`@terminal`), stran brskalnika (`@browser`), dnevnik CI, predan agentu, datoteka zunaj projekta in daljše besedilo, prilepljeno s spletne strani, zato gredo naprej označeni: oznaka pravi »Zunanja vsebina«, agent pa dobi besedilo z obvestilom, da gre za nezaupanja vredne podatke, katerih navodil ne sme upoštevati. Spletne strani lastnega agenta pridejo z enakim obvestilom. Prilepljeno besedilo velja za besedilo s spletne strani, kadar brskalnik to navede v odložišču, kot to počnejo Safari, Chrome in Firefox; besedilo od drugod in kratka fraza iz nekaj besed ostaneta vaša.

## Peskovnik

Dango sam teče brez App Sandbox, ker bi ga podedoval vsak program, ki ga zažene: terminali, Git, jezikovni strežniki in agenti ne bi mogli delovati. Namesto tega ima vsaka nit AI stikalo peskovnika, privzeto izklopljeno. Kaj omeji, je odvisno od agenta, in opis orodja stikala to pove:

| Agent | Z vklopljenim peskovnikom |
|---|---|
| Claude | ukazi lupine tečejo v Claudovem lastnem peskovniku; njegova orodja za datoteke in splet ne |
| Codex | ukazi in spremembe pišejo le v projekt, brez omrežja |
| Lastni agent | datoteke, spletne zahteve in ukazi ostanejo znotraj projekta in dovoljenih map, brez omrežja (`sandbox-exec`) |
| Antigravity, OpenCode | ni peskovnika, ki bi ga Dango lahko vklopil |

## Videz in dostopnost

- **Temni način** sledi sistemu; **Povečaj kontrast** in **Zmanjšaj gibanje** sta upoštevana.
- **VoiceOver** poimenuje stransko vrstico, zavihke, kartice časovnice AI, vrstice Git in CI, razhroščevalnik in besedilo terminala. Prekinitvene točke se nikoli ne razlikujejo samo po barvi.
- **Tipkovnica.** Vsak ukaz ima bližnjico in vse je mogoče spremeniti v `keybindings.json`; ⌥⌘\` premakne na naslednje podokno.
- **Od desne proti levi.** V arabščini in hebrejščini je okno prezrcaljeno, pogledi kode pa ostanejo od leve proti desni.

Kaj Dango doda AppKitu, po posameznih področjih, je v [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Jeziki

Vmesnik je preveden v 35 jezikov: arabščino, katalonščino, kitajščino (poenostavljeno), kitajščino (tradicionalno), hrvaščino, češčino, danščino, nizozemščino, angleščino, finščino, francoščino, nemščino, grščino, hebrejščino, hindijščino, madžarščino, indonezijščino, italijanščino, japonščino, korejščino, malajščino, norveščino, poljščino, portugalščino (Brazilija), portugalščino (Portugalska), romunščino, ruščino, slovaščino, slovenščino, španščino, švedščino, tajščino, turščino, ukrajinščino in vietnamščino.

Dango sledi vrstnemu redu jezikov v **Nastavitve sistema › Splošno › Jezik in regija**; tam lahko pod **Aplikacije** izberete tudi jezik samo za Dango. Če za vaš jezik ni prevoda, uporabi angleščino. Izrazi sledijo enemu glosarju, [docs/glossary.md](../../../docs/glossary.md), in Applovim lastnim prevodom, zato se Dango bere kot preostanek vašega Maca.

## Bližnjice na tipkovnici

| Dejanje | Bližnjica |
|---|---|
| Odpri mapo, novo okno, zapri okno | ⌘O, ⇧⌘N, ⇧⌘W |
| Nova datoteka, shrani, shrani kot | ⌘N, ⌘S, ⇧⌘S |
| Nov zavihek, zapri zavihek, naslednji in prejšnji zavihek | ⌘T, ⌘W, ⇧⌘] in ⇧⌘[ |
| Hitro odpri, išči po projektu | ⌘P, ⇧⌘F |
| Datoteke, Iskanje, Agenti, Git, Odpravljanje napak, CI | ⌘1 do ⌘6 |
| Prikaži ali skrij stransko vrstico, naslednje podokno | ⌃⌘S, ⌥⌘\` |
| Nazaj in naprej po urejanih mestih | ⌃- in ⌃⇧- |
| Preklopi komentar, pojdi na vrstico | ⌘/, ⌃G |
| Naslednja pojavitev, kazalka zgoraj in spodaj | ⌘D, ⌃⇧↑ in ⌃⇧↓ |
| Zloži, razgrni, zloži vse, razgrni vse | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Prelom vrstic, inšpektor | ⌥Z, ⌥⌘I |
| Dokončanje, informacije ob lebdenju, pojdi na definicijo | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Sklici, hierarhija klicev, preimenuj | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simboli v datoteki, v projektu | ⇧⌘O, ⌥⌘O |
| Hitri popravek, oblikuj | ⌥⌘A, ⌥⇧F |
| Naslednja in prejšnja težava | ⌘' in ⇧⌘' |
| Predogled Markdown, prilepi kot navadno besedilo | ⇧⌘V, ⌥⇧⌘V |
| Krepko, ležeče, prečrtano, koda, povezava | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Naslov 1 do 3 | ⌥⌘1 do ⌥⌘3 |
| Nov terminal, počisti terminal | ⌃\`, ⌘K |
| Fokus v polje za sporočilo agenta, komentiraj načrt | ⌘Esc, ⌥⌘K |
| Naslovno polje, znova naloži, nazaj, naprej (brskalnik) | ⌘L, ⌘R, ⌘[, ⌘] |
| Začni ali ustavi narekovanje | ⌃⌥D |
| Prekinitvena točka, vklopi ali izklopi vse prekinitvene točke | ⌘\\, ⌘Y |
| Nadaljuj, preskoči, vstopi, izstopi, ustavi | ⌃⌘Y, F6, F7, F8, ⌘. |
| Nastavitve | ⌘, |

`keybindings.json` v mapi s podatki aplikacije (glejte [Kje so shranjeni podatki](#kje-so-shranjeni-podatki)) spremeni katero koli od njih.

## Zasebnost in varnost

- **Brez telemetrije,** brez analitike, brez poročil o sesutjih, poslanih kamor koli. Napake gredo v sistemski dnevnik (aplikacija Konzola, pod imenom aplikacije).
- **Omrežje le za tisto, kar zaženete:** agenti, ki jih uporabljate, vsak s svojo storitvijo; register ACP in prenosi agentov, jezikovnih strežnikov, adapterjev za razhroščevanje in modelov za narekovanje, ko jih zahtevate; gostitelji CI, v katere se vpišete; napovedovanje med tipkanjem iz Copilota, če ste ga izbrali; ponudniki lastnega agenta; spletni prepis, če ste ga izbrali; strani, ki jih odprete v zavihku brskalnika; in preverjanje posodobitev, ko ste ga dovolili.
- **Koda zapusti Mac** le tam, kjer ste za to izbrali storitev: agent, napovedi Copilota, sporočilo commita, ki ga je napisala AI. Diff za sporočilo commita gre ven šele, ko gitleaks v njem ne najde nobene skrivnosti.
- **Ključi in žetoni** ostanejo v verigi ključev macOS, nikoli v datoteki z nastavitvami. Ključi drugih aplikacij, kot so Lowspeakovi, se preberejo le, če se strinjate, in le enkrat.
- **Prenosi določenih različic.** Agenti, jezikovni strežniki in adapterji za razhroščevanje, ki jih namesti Dango, so vezani na preizkušene različice in preverjeni z njihovimi kontrolnimi vsotami.
- **Podpisovanje kode** s Hardened Runtime; posodobitve se namestijo le z lastnim podpisom aplikacije.

## Kje so shranjeni podatki

`<id>` je identifikator paketa, `local.agentide` za izdajo (`local.agentide.debug` za razvojne gradnje). Ne vsebuje prikaznega imena, zato preimenovanje ohrani vse podatke.

| Mesto | Kaj |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (seznam podokna AI), `agent-threads/` (pogovori lastnega agenta), `layouts/` (zavihki po projektih), `recovery/` (neshranjeni urejevalniki po sesutju), `checkpoints/` (za mape brez Gita), `worktrees/`, `agents/` in `tools/` (nameščeni agenti, jezikovni strežniki in adapterji za razhroščevanje), `task-output/` |
| `~/Library/Caches/<id>/` | prilepljene slike, skripti za integracijo z lupino, register ACP; lahko se izbriše |
| `~/Library/Logs/<id>/dictation/` | dnevniške datoteke narekovanja |
| `~/Library/Application Support/Lowspeak` in `FluidAudio` | lokalni modeli za narekovanje, v skupni rabi z Lowspeakom |
| `~/Library/WebKit/<id>/` | piškotki in podatki spletnih mest zavihka brskalnika |
| `<project>/.dango/settings.json` | lastne nastavitve projekta |
| repozitorij Git projekta | kontrolne točke, kot commiti pod `refs/ide-checkpoints/`; običajen push ali fetch jih nikoli ne prenese |
| veriga ključev, storitev `<id>` | ključi API lastnega agenta in narekovanja, žetoni CI |
| `~/.local/bin/dango` | ukaz za terminal, če ste ga namestili |

Claude hrani svoje seje pod `~/.claude/projects`; Dango jih prebere, da nadaljuje sejo, vendar jih ne spreminja.

## Znane omejitve

- **Brez App Sandbox** za samo aplikacijo, ker bi ga podedovali terminali, Git, jezikovni strežniki in agenti; mapi morate zaupati, preden se zažene karkoli, kar nastavi. Antigravity in OpenCode nimata peskovnika, ki bi ga Dango lahko vklopil.
- **Claudovi podatki o porabi in razčlenitev konteksta** temeljijo na delih njegovega protokola, označenih kot poskusni, in so skriti, kadar manjkajo.
- **Previjanje pogovora nazaj** deluje le za Claude; pri drugih agentih se previje nazaj le koda.
- **Vpis v CI prek brskalnika** je izklopljen; uporabite `gh`, `glab`, žeton ali okolje.
- **Gemini CLI ni na voljo:** Google je ukinil vpis z osebnim računom in napotuje na Antigravity. Prejšnje niti Gemini ostanejo berljive.

## Gradnja iz izvorne kode

Potrebujete:

- macOS 14 ali novejši z orodji Xcode Command Line Tools (`xcode-select --install`); sam Xcode ni potreben.
- Rust, kot ga navaja `rust-toolchain.toml`. Homebrewov `rust` gradi za procesor tega Maca; univerzalna gradnja izdaje potrebuje rustup s ciljema `aarch64-apple-darwin` in `x86_64-apple-darwin`.
- Kopiji [splat](https://github.com/sventmueller/splat) in [lowspeak](https://github.com/sventmueller/lowspeak) poleg te (`../splat`, `../lowspeak`), obe na veji `main`; delovni prostor iz njiju jemlje crate po poti.
- cmake, za vsako gradnjo z narekovanjem (`make run`, `make build`, `make release`), ki zgradi whisper.cpp. `make check` gradi brez njega.

| Ukaz | Kaj naredi |
|---|---|
| `make run` | zgradi razvojno aplikacijo (`target/debug/Dango Debug.app`) in jo odpre |
| `make build` | zgradi aplikacijo za izdajo (`target/release/Dango.app`), podpisano z lokalnim potrdilom, ki ga prva gradnja ustvari v prijavni verigi ključev; poveča številko gradnje v `VERSION` |
| `make lint` | oblikovanje in clippy; opozorila povzročijo neuspeh |
| `make check` | `lint`, nato vsi preizkusi; ne potrebuje verige ključev, okna ali omrežja |
| `make test` | `check`, nato samopreizkus `probe` in revizija dostopnosti, od leve proti desni in od desne proti levi, v pravi aplikaciji |
| `make perf` | časovni proračuni, v gradnji za izdajo; smiselno le na sicer nedejavnem Macu |
| `make release` | poskusni zagon izdaje v `target/release-dry-run/`, podpisan z lokalnim potrdilom; nikamor ničesar ne pošlje |
| `make release-notarized` | `check`, nato sama izdaja v `dist/`: podpis Developer ID, notarizacija, vir posodobitev, podpisan z lastnim ključem aplikacije; poveča številko gradnje |

Razvojna različica in različica za izdajo sta ločeni aplikaciji z lastnimi nastavitvami in podatki. Gradnja, preizkušanje, samopreizkusi, izdaja in pravila, ki jim sledi koda, so opisani v [docs/development.md](../../../docs/development.md), odločitve za tem pa v [docs/plan.md](../../../docs/plan.md). Opombe ob izdaji so [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), prevedene poleg vsakega README v `assets/readme/<language>/`.

## Licenca

Dango je licenciran po eni od naslednjih licenc

- Licenca Apache, različica 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licenca MIT ([LICENSE-MIT](../../../LICENSE-MIT))

po vaši izbiri.

Če izrecno ne navedete drugače, je vsak prispevek, ki ga za vključitev v Dango namenoma predložite, kot je opredeljeno v licenci Apache-2.0, dvojno licenciran kot zgoraj, brez dodatnih pogojev.
