<p><img src="../../readme-icon.png" alt="Ikona aplikacije Dango" width="147" height="147"></p>

# Dango

**Nativni macOS urednik za rad s agentima za kodiranje.**

Dango okuplja kôd, agente koji ga mijenjaju i sve oko njih u jednom prozoru po mapi: urednik s jezičnim poslužiteljima i predviđanjima pri uređivanju, terminali, preglednik, Git, CI, program za otklanjanje pogrešaka i AI niti s Claudeom, Codexom, Antigravityjem, OpenCodeom ili agentom ugrađenim u aplikaciju. Svaki agent radi na istoj vremenskoj crti, pita prije nego što promijeni datoteke ili pokrene naredbe i prije svakog prompta ostavlja kontrolnu točku, tako da ono što je napravio možete pregledati i premotati poruku po poruku. Ništa što mapa konfigurira ne pokreće se prije nego što joj vjerujete, a tekst izvana stiže do agenta označen kao nepouzdan. Diktirajte u urednik i u polje za poruku; prepoznavanje govora može ostati na vašem Macu. Dango je napisan u Rustu na pravim AppKit kontrolama, bez Electrona, govori 35 jezika i nema telemetriju.

[![Preuzmi Dango.dmg](https://img.shields.io/badge/Preuzmi_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Preuzmi Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: potpisan i ovjeren kod Applea (notarizacija), za macOS 14 ili noviji na Apple siliconu i Intelu. Poveznica će biti dostupna s prvim izdanjem.

**Pronašli ste pogrešku?** [Otvorite issue](https://github.com/sventmueller/dango/issues/new) i navedite s kojim se agentom i u kakvom projektu pojavila.

[![Rust, izdanje 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 ili noviji](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![Univerzalna: Apple silicon i Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![AppKit putem objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![Terminal: Tapa term-vt](https://img.shields.io/badge/terminal-Tapa_term--vt-F46D01)](https://github.com/sventmueller/tapa) ![35 jezika sučelja](https://img.shields.io/badge/languages-35-4C9A2A) ![Bez telemetrije](https://img.shields.io/badge/telemetry-none-4C9A2A) ![Potpisano s Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![Licenca: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licenca)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · **Hrvatski** · [Italiano](../it/README.md) · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · [Polski](../pl/README.md) · [Português (Brasil)](../pt_BR/README.md) · [Português (Portugal)](../pt_PT/README.md) · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · [简体中文](../zh-Hans/README.md) · [繁體中文](../zh-Hant/README.md)

## Sadržaj

- [Značajke ukratko](#značajke-ukratko)
- [Zahtjevi](#zahtjevi)
- [Instalacija](#instalacija)
- [Prvi koraci](#prvi-koraci)
- [AI agenti](#ai-agenti)
  - [Vremenska crta](#vremenska-crta)
  - [Dozvole i načini rada](#dozvole-i-načini-rada)
  - [Kontrolne točke i worktreeji](#kontrolne-točke-i-worktreeji)
- [Urednik](#urednik)
- [Jezični poslužitelji](#jezični-poslužitelji)
- [Testovi](#testovi)
- [Program za otklanjanje pogrešaka](#program-za-otklanjanje-pogrešaka)
- [Terminal](#terminal)
- [Preglednik](#preglednik)
- [Git](#git)
- [CI](#ci)
- [Kontejneri](#kontejneri)
- [Markdown](#markdown)
- [Diktat](#diktat)
- [Povjerenje](#povjerenje)
- [Sandbox](#sandbox)
- [Izgled i pristupačnost](#izgled-i-pristupačnost)
- [Jezici](#jezici)
- [Tipkovnički prečaci](#tipkovnički-prečaci)
- [Privatnost i sigurnost](#privatnost-i-sigurnost)
- [Gdje se nalaze podaci](#gdje-se-nalaze-podaci)
- [Poznata ograničenja](#poznata-ograničenja)
- [Izgradnja iz izvornog koda](#izgradnja-iz-izvornog-koda)
- [Licenca](#licenca)

## Značajke ukratko

- **Svi agenti na jednoj vremenskoj crti.** Claude putem vlastite naredbe `claude`, Codex, Antigravity i OpenCode putem protokola Agent Client Protocol, drugi iz ACP registra te agent ugrađen u aplikaciju za Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama i LM Studio. Odgovori, pozivi alata, zahtjevi za dozvolu, planovi, obaveze, podagenti i pozadinski zadaci izgledaju jednako, koji god agent radio.
- **Pregled i premotavanje.** Prije svakog prompta Dango sprema kontrolnu točku. Ono što je agent promijenio može se pregledati i premotati po poruci, a nit može raditi u vlastitom Git worktreeju i predati svoj rad spajanjem ili cherry-pickom.
- **Jedan prozor po mapi.** Datoteke, terminali, stranice preglednika, AI niti, prikazi razlika i pregledi macOS su kartice tog prozora. Rubni stupac sadrži Datoteke, Pretraga, Agenti, Git, Testovi, Otklanjanje pogrešaka, CI i Kontejneri.
- **Pravi urednik koda.** Isticanje sintakse, više kursora, sažimanje blokova, `.editorconfig`, jezični poslužitelji za 17 jezika s dovršavanjem, odlaskom na definiciju, referencama, preimenovanjem, brzim popravcima i hijerarhijama te predviđanja pri uređivanju od GitHub Copilota ili lokalnog modela.
- **Ugrađeni program za otklanjanje pogrešaka, Git i CI.** Prekidne točke i izvršavanje korak po korak za Rust, C, C++, Swift, Go, JavaScript, TypeScript i Python; priprema pojedinačnih blokova, grane, stashevi, konflikti i graf commitova; pokretanja, poslovi i zapisnici iz GitHub Actions i GitLab CI.
- **Pažljiv s vašim mapama.** Mapa kojoj niste ukazali povjerenje ne pokreće ništa: nikakve Git procese, jezične poslužitelje, hookove ni agente. Destruktivne Git naredbe nikad se ne dozvoljavaju trajno, a commitovi se provjeravaju na tajne podatke.
- **Diktat.** Govorite u urednik i u polje za poruku, mrežno putem OpenAI-ja ili Groqa ili lokalno s Whisperom i Parakeetom.
- **Nativno do srži.** AppKit prozori, kartice, izbornici i alatne trake, TextKit 2 za urednik, Metal za terminal, WebKit samo u kartici preglednika. Tamni način, Povećaj kontrast i VoiceOver rade kao u svakoj aplikaciji za Mac.
- **Na vašem jeziku.** Sučelje je dostupno na 35 jezika: arapski, katalonski, kineski (pojednostavljeni), kineski (tradicionalni), hrvatski, češki, danski, nizozemski, engleski, finski, francuski, njemački, grčki, hebrejski, hindski, mađarski, indonezijski, talijanski, japanski, korejski, malajski, norveški, poljski, portugalski (Brazil), portugalski (Portugal), rumunjski, ruski, slovački, slovenski, španjolski, švedski, tajlandski, turski, ukrajinski i vijetnamski.

## Zahtjevi

- macOS 14 Sonoma ili noviji, na Apple siliconu ili Intelu. Dango se razvija i testira na macOS-u 27.
- Za AI niti agent koji želite koristiti; pogledajte [AI agenti](#ai-agenti).
- Za jezične poslužitelje i adaptere za otklanjanje pogrešaka, koje Dango instalira na vaš zahtjev: Go za gopls i `dlv`; Node.js za poslužitelje za TypeScript, Python, JSON, YAML, CSS, HTML i shell te za JavaScript program za otklanjanje pogrešaka; Xcode Command Line Tools za clangd, sourcekit-lsp i `lldb-dap`. rust-analyzer preuzima se iz njegovih izdanja na GitHubu.
- Za predviđanja pri uređivanju pretplata na GitHub Copilot (dovoljna je i besplatna) ili model tipa fill-in-the-middle u aplikaciji Ollama ili LM Studio.

## Instalacija

**Sa slike diska.** Svako izdanje je slika diska `Dango-<version>.dmg` s jednom univerzalnom aplikacijom za Apple silicon i Intel, potpisanom s Developer ID i ovjerenom kod Applea. Otvorite je i povucite Dango u mapu Aplikacije. Homebrew Cask instalira istu sliku. Još nijedno izdanje nije objavljeno.

**Ažuriranja.** Dango sa slike diska sam se ažurira putem [Sparkle](https://sparkle-project.org). Pri drugom pokretanju pita smije li sam provjeravati ažuriranja; dok ne pristanete, ne šalje nikakav zahtjev. **Dango › Potraži ažuriranja…** odmah provjerava. Ažuriranja su potpisana vlastitim ključem aplikacije, koji Sparkle provjerava prije nego što išta instalira. Vlastita izgradnja nema mehanizam ažuriranja.

**Iz izvornog koda.** Pogledajte [Izgradnja iz izvornog koda](#izgradnja-iz-izvornog-koda), zatim premjestite `target/release/Dango.app` u `/Applications`.

**Naredba za terminal.** **Dango › Instaliraj naredbu “dango”…** zapisuje malu skriptu u `~/.local/bin/dango`, bez administratorskih prava; `~/.local/bin` mora biti u vašem `PATH`-u. Nakon toga `dango .` otvara trenutačnu mapu, a `dango file.rs` datoteku. Dango ondje nikad ne zamjenjuje datoteku koju nije sam zapisao.

## Prvi koraci

1. **Otvorite mapu** pomoću ⌘O ili `dango <folder>`. Prvi put Dango pita vjerujete li joj (pogledajte [Povjerenje](#povjerenje)).
2. **Rubni stupac** prebacuje između prikaza Datoteke (⌘1), Pretraga (⌘2), Agenti (⌘3), Git (⌘4), Testovi (⌘5), Otklanjanje pogrešaka (⌘6), CI (⌘7) i Kontejneri (⌘8); desnim klikom skrivate one koje ne koristite, a ⌃⌘S sklapa rubni stupac.
3. **Svaki dokument je kartica** prozora mape, i to vlastite macOS kartice. Znak “+” na kraju trake kartica i ⌘T nude novu datoteku, terminal, preglednik i svaki instalirani agent, i u vlastitom Git worktreeju; držanjem znaka “+” isto se prikazuje kao izbornik.
4. **Brzo otvaranje** pomoću ⌘P pronalazi datoteke, a s `#` simbole projekta; ⇧⌘F traži i zamjenjuje u projektu.
5. **Razgovarajte s agentom** u AI kartici. ⌘Esc premješta fokus tipkovnice u njegovo polje za poruku i natrag.
6. **Postavke** pomoću ⌘, sadrže kartice Općenito, CI, Agenti, Vlastiti agent, Jezični poslužitelj, Diktat i Predviđanje.

Alatna traka sadrži Natrag i Naprijed kroz mjesta koja ste uređivali (⌃- i ⌃⇧-), granu, Brzo otvaranje i pregled Aktivnost, koji navodi sve što radi. **Otvori sa strane** u kontekstnom izborniku datoteke prikazuje drugi dokument pokraj prvog. Nakon ponovnog pokretanja ili rušenja svaki se prozor vraća sa svojim karticama, a urednici s nespremljenim tekstom vraćaju se s tim tekstom. Slike, audio, video, fontovi i Office dokumenti otvaraju se u kartici putem značajke Quick Look, a PDF datoteke putem PDFKita.

## AI agenti

| Agent | Kako radi | Što treba |
|---|---|---|
| Claude | naredba `claude`, s kojom Dango komunicira izravno putem njezina protokola stream-json | instaliran `claude` s prijavom; Dango ga traži u `PATH`-u prijavne ljuske, u `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` i `/usr/local/bin`. Prijava se obavlja samo u samom `claude` |
| Codex | `codex-acp` putem protokola Agent Client Protocol (ACP) | Node.js i npm; Dango na vaš zahtjev instalira paket u fiksnoj verziji |
| Antigravity | `agy acp` | vaš vlastiti `agy` u `PATH`-u ili preuzimanje u fiksnoj verziji, instalirano na vaš zahtjev |
| OpenCode | `opencode acp` | vaš vlastiti `opencode` u `PATH`-u ili preuzimanje u fiksnoj verziji, instalirano na vaš zahtjev |
| Ostali ACP agenti | iz ACP registra, koji se dohvaća samo na vaš zahtjev | ono što navodi unos u registru; unosi bez kontrolnog zbroja instaliraju se tek nakon vaše potvrde |
| Vlastiti agent | ugrađen u aplikaciju, s alatima za datoteke, pretragu, naredbe, dijagnostiku i web zahtjeve | API ključ za Anthropic, OpenAI, Google Gemini ili OpenRouter, spremljen u privjesku ključeva; Ollama ili LM Studio na ovom Macu bez ključa; bilo koji poslužitelj koji govori API tvrtke OpenAI ili Anthropic |

**Postavke › Agenti** prikazuje koje agente ovaj Mac ima, instalira ostale i njihova ažuriranja te otvara ACP registar. Agenti koje Dango instalira vezani su uz provjerene verzije i provjeravaju se prema objavljenim kontrolnim zbrojevima (npm integrity ili SHA-256); Dango ne pokreće nijedan `npx` paket bez verzije.

### Vremenska crta

Svaki agent radi u AI kartici s istom vremenskom crtom: odgovori, pozivi alata s njihovim izlazom, zahtjevi za dozvolu, pitanja, planovi za odobravanje i komentiranje (⌥⌘K), obaveze, podagenti i pozadinski zadaci. Dva gumba u zaglavlju kartice navode podagente i pozadinske procese koji su u tijeku. Spomenite datoteku, `@terminal` ili `@browser` kako biste agentu predali njihov sadržaj. Zelena točka na kartici označava sesiju koja radi, crvena sesiju koja čeka vas, a obavijest javlja kad sesija završi ili čeka dok ste drugdje. Za Claude statusni redak prikazuje kontekst i potrošnju.

### Dozvole i načini rada

Nove niti počinju u načinu “Ručno”: agent pita prije nego što promijeni datoteke ili pokrene naredbe. Za destruktivne Git naredbe (`push --force`, `reset --hard`, `clean -fd`) Dango ne nudi “Uvijek dozvoli”. Vlastiti agent čuva svoja pravila “Uvijek dozvoli” po projektu, navodi ih u **Postavke › Vlastiti agent** i za destruktivne Git naredbe pita svaki put, bez obzira na to što je spremljeno; Claude i ACP agenti čuvaju takva pravila u vlastitoj konfiguraciji. S “Dozvoli sve” i bez sandboxa nit prikazuje trajno upozorenje.

### Kontrolne točke i worktreeji

Prije svakog prompta Dango sprema kontrolnu točku, kao commit pod `refs/ide-checkpoints/` u repozitoriju projekta ili u vlastitoj mapi za projekt bez Gita; običan push ili fetch nikad ih ne prenosi. Ono što je agent promijenio može se pregledati i premotati po poruci. Nit može raditi u vlastitom Git worktreeju: okno Git prikazuje što je promijenila i preuzima rad spajanjem ili cherry-pickom.

## Urednik

- **Isticanje sintakse, brojevi redaka i oznake promjena** u odnosu na zadnji commit, traženje i zamjena, Brzo otvaranje (⌘P) i pretraga projekta (⇧⌘F).
- **Uvlačenje** slijedi datoteku i `.editorconfig`: tabulatori ili razmaci, automatsko uvlačenje, parovi zagrada.
- **Više kursora:** ⌥-klik, ⌥-povlačenje za stupac, ⌃⇧↑ i ⌃⇧↓, ⌘D za sljedeće pojavljivanje.
- **Sažimanje blokova** (⌥⌘← i ⌥⌘→, s ⇧ za sve), uključivanje i isključivanje komentara (⌘/), odlazak na redak (⌃G), vidljive praznine i prelamanje redaka (⌥Z).
- **Datoteke zadržavaju kodiranje**, oznaku redoslijeda bajtova i završetke redaka. Datoteka promijenjena na disku ponovno se učitava ako urednik nema nespremljenog teksta; inače traka nudi razlike i spajanje.
- **Predviđanja pri uređivanju.** Dok tipkate, prijedlog se pojavljuje sivo, a tipka Tab ga prihvaća. Dolazi od GitHub Copilota ili od lokalnog modela u aplikaciji Ollama ili LM Studio, kako odaberete u **Postavke › Predviđanje**; lokalni model zadržava vaš kôd na ovom Macu.
- **Minikarta i fiksno pomicanje** (izbornik Urednik): uska slika cijele datoteke pokraj teksta te retci zaglavlja blokova u kojima je gornji redak, zadržani iznad njega.
- **Dijagnostika na kraju retka** (izbornik Prikaz): najozbiljnija poruka svakog retka nakon sekunde mirovanja, sa simbolom njezine ozbiljnosti.
- **Pregled statičkih stranica uživo:** Urednik › Otvori u kartici preglednika poslužuje stranice pouzdane mape izravno iz aplikacije Dango i ponovno učitava karticu kad se datoteka spremi.
- **Prettier** formatira JavaScript, TypeScript, CSS, HTML, Markdown i drugo gdje ga projekt konfigurira, vlastitim `node_modules/.bin/prettier` projekta; postavka `formatter` bira alat za formatiranje po jeziku.
- **Način rada Vim** (Urednik › Način rada Vim): načini Normal, Insert, Visual i Replace, brojevi ponavljanja, operatori s pokretima i tekstnim objektima, `.`, registri, oznake i makronaredbe. `u` i ⌃R koriste poništavanje urednika, a jedan prolaz umetanja jedan je korak. `"+` i `"*` su memorijski pretinac; postavka `vimClipboard` i neimenovani registar pretvara u memorijski pretinac. `/`, `?`, `n`, `N`, `*`, `:s` i `:%s` pretražuju regularnim izrazima Rusta, ne Vimovim uzorcima: nema `\v`, a `\b` umjesto `\<` i `\>`. Ex naredbe su `:w`, `:q`, `:wq`, `:x`, `:noh` i `:<line>`. U načinu Normal Vim preuzima ⌃G od naredbe Idi na redak.
- **Sinkronizacija postavki** putem mape na iCloud Driveu (**Postavke › Općenito**): vaše postavke i prečaci prate vas na vaša druga Mac računala; putanje, lokalne adrese i privole ostaju na svakom zasebno.

## Jezični poslužitelji

Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, shell skripte, JSON, YAML, CSS, HTML, TOML, Markdown i Lua te vaši vlastiti poslužitelji iz **Postavke › Jezični poslužitelj**. Dango instalira poslužitelj na vaš zahtjev, u vlastitu mapu s alatima, i koristi vlastiti `tsc` projekta TypeScript kad može služiti kao jezični poslužitelj.

Što donose: dijagnostiku (⌘' i ⇧⌘' prelaze između stavki), informacije pri prelasku pokazivačem (⌃⌘I), dovršavanje (⌥Esc), pomoć za potpise, odlazak na definiciju (⌃⌘J), na definiciju tipa i implementaciju, reference (⌃⇧⌘F), hijerarhije poziva i tipova (⌃⇧⌘H), preimenovanje (⌃⌘E), formatiranje (⌥⇧F), semantičke boje, sažimanje blokova i strukturu (⇧⌘O); tipove i nazive parametara unutar retka; brze popravke i refaktoriranja iza žarulje (⌥⌘A); simbole projekta pomoću ⌥⌘O.

Uz vlastiti poslužitelj datoteke ESLint provjerava JavaScript i TypeScript projekata koji ga koriste, sa svojim brzim popravcima, a Emmet kratice pojavljuju se u dovršavanju za HTML, CSS i JSX. Dango instalira oba na vaš zahtjev; **Postavke › Općenito** po želji ispravlja probleme ESLinta pri spremanju.

## Testovi

Okno Testovi navodi testove mape po paketu i datoteci s njihovim stanjem: `cargo test` ili cargo-nextest, Go, pytest, Vitest i Jest. Simbol uz svaki test na rubu pokreće ga, ⌥-klik pokreće otklanjanje pogrešaka u njemu, a neuspjeli assert označava svoj redak. Izbornik Otklanjanje pogrešaka pokreće test na mjestu kursora (⌃⌥⌘U), ponovno zadnje testove (⌃⌥⌘G) ili sve te pokreće otklanjanje pogrešaka u testu na mjestu kursora. Testovi se pokreću samo u pouzdanim mapama jer izvršavaju kod projekta; `cargo test` navodi svoje testove tek kad to zatražite jer popisivanje gradi svaki testni program.

## Program za otklanjanje pogrešaka

Prekidne točke, i s uvjetima i kao točke zapisivanja, izvršavanje korak po korak, niti sa stogovima poziva, varijable i praćeni izrazi u Inspektoru (⌥⌘I), vrijednosti pokraj koda dok je program zaustavljen te konzola za otklanjanje pogrešaka koja izračunava izraze u zaustavljenom okviru.

| Jezik | Adapter za otklanjanje pogrešaka |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` iz alata Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScript program za otklanjanje pogrešaka, na Node.js |
| Python | debugpy |

Čita se datoteka `.vscode/launch.json` projekta. Izbornik Otklanjanje pogrešaka koristi prečace iz Xcodea: ⌘\\ postavlja prekidnu točku, ⌘Y uključuje ili isključuje sve, ⌃⌘Y nastavlja, F6, F7 i F8 preskaču, ulaze u funkciju i izlaze iz nje, ⌘. zaustavlja.

## Terminal

Terminali u karticama (⌃\`) s okruženjem prijavne ljuske, iscrtani pomoću Metala. ⌘K briše zaslon. Zalijepljena ili ispuštena slika predaje se agentu za kodiranje koji radi u terminalu ili se lijepi putanja spremljene kopije. ⌘-klik otvara poveznice i nazive datoteka. Razvojni poslužitelj koji terminal pokrene može se otvoriti u kartici preglednika.

## Preglednik

Kartica preglednika s poljem adrese (⌘L), Natrag i Naprijed (⌘[ i ⌘]), ponovnim učitavanjem (⌘R), pretragom, preuzimanjima koja najprije pitaju te Web Inspectorom. `@browser` predaje stranicu agentu, označenu kao vanjski tekst.

## Git

Okno Git priprema i odbacuje datoteke ili pojedinačne blokove, izrađuje commitove, izrađuje commit i radi push ili sinkronizaciju, prebacuje i stvara grane, čuva stasheve, razrješava konflikte spajanja i crta graf nedavnih commitova. Blame prikazuje tko je zadnji promijenio redak na kojem je kursor ili stupac za cijelu datoteku. Gumb piše poruku commita pomoću AI-ja; razlike se šalju tek kad gitleaks u njima ne pronađe tajne podatke, a svaki se commit također provjerava na tajne podatke.

## CI

GitHub Actions i GitLab CI, također GitHub Enterprise Server i samostalno upravljani GitLab:

- pokretanja, poslovi i zapisnici trenutačne grane;
- ponovno pokretanje neuspjelih poslova, jednog posla ili, na GitHubu, cijelog pokretanja; pokretanje ručnih poslova ili odobravanje implementacija na čekanju; pokretanje novog pipelinea ili workflowa koji se smije pokrenuti ručno; otkazivanje;
- otvoreni pull requestovi odnosno merge requestovi i issuesi, koji se otvaraju u kartici preglednika;
- zapisnik neuspjelog posla predan agentu, označen kao vanjski tekst.

Token dolazi iz privjeska ključeva, od `gh` ili `glab`, iz okruženja prijavne ljuske ili je to onaj koji unesete u **Postavke › CI**.

## Kontejneri

Okno Kontejneri prikazuje Dockerove Compose projekte s njihovim servisima, pojedinačne kontejnere, slike i volumene te prati Dockerove događaje kako se događaju. Compose datoteka mape prikazuje se kao projekt i prije nego što postoji ijedan njezin kontejner. Kontejneri se pokreću, zaustavljaju i ponovno pokreću; njihovi zapisnici i ljuska otvaraju se u karticama terminala, a objavljeni port otvara se u kartici preglednika. Brisanje kontejnera, slike ili volumena te čišćenje nekorištenih podataka najprije traže potvrdu. Naredbe up, down i pull iz Composea pokreću se samo u pouzdanim mapama jer Compose gradi i pokreće ono što projekt opisuje. Izbornik okna bira Docker kontekst za prozor bez mijenjanja onog koji koriste vaše ljuske; iste su naredbe u izborniku Kontejneri. Potrebni su naredbeni redak `docker` i Docker 24.0.6 ili noviji.

## Markdown

- **Pregled.** Markdown datoteka otvara se s pregledom pokraj sebe; **Prikaz › Prikaži/sakrij pregled sa strane** (⇧⌘V) zatvara ga i otvara, a **Postavke › Općenito** isključuje automatski pregled. Pregled iscrtava formule, dijagrame Mermaid, Graphviz i PlantUML te slike iz mape; slike s weba učitavaju se samo u pouzdanoj mapi ili nakon što ih zatražite u pregledu.
- **Izbornik Formatiraj:** podebljano (⌘B), kurziv (⌘I), precrtano (⇧⌘X), kôd (⌥⇧⌘C), poveznica (⇧⌘K), naslovi 1–3 (⌥⌘1–⌥⌘3), popisi s grafičkim oznakama i numerirani popisi, zadaci, citati i blokovi koda. Svaka naredba uključuje i isključuje, na odabiru ili na recima, i jedan je korak poništavanja.
- **Lijepljenje.** Tekst iz Worda ili s web stranice stiže kao Markdown, a njegove slike kao datoteke pokraj dokumenta; ⌥⇧⌘V lijepi običan tekst.
- **Eksportiraj.** **Datoteka › Eksportiraj** zapisuje PDF, jednu HTML datoteku sa slikama unutar nje ili Word dokument. Slike s weba uključuju se samo iz pouzdane mape.
- **Otvori u aplikaciji Splat.** Ako je [Splat](https://github.com/sventmueller/splat) instaliran, **Datoteka › Otvori u aplikaciji Splat** i kontekstni izbornici stabla datoteka i kartica otvaraju dokument u njemu, nakon što ga spreme.

## Diktat

Diktirajte u polje za poruku AI kartice ili u urednik: gumbom mikrofona koji se ondje nalazi, s **Uredi › Započni diktat** (⌃⌥D) ili tipkom fn (držite za snimanje, dvaput dodirnite za nastavak snimanja). Prepoznavanje govora je ono iz aplikacije [Lowspeak](https://github.com/sventmueller/lowspeak); sam Lowspeak ne mora biti instaliran.

- **Postavke › Diktat** određuje način rada i model (mrežno s OpenAI-jem ili Groqom, lokalno s Whisperom ili Parakeetom), jezik, naknadnu obradu, mikrofon i vaš vlastiti rječnik.
- **Lokalni modeli** preuzimaju se s Hugging Facea kad odaberete neki od njih, uz prikaz napretka i mogućnost otkazivanja. Spremaju se u mape koje koristi Lowspeak, pa svaka od dviju aplikacija koristi ono što je preuzela druga. Lokalni model zadržava snimku na ovom Macu.
- **Mrežna transkripcija** treba ključ za OpenAI ili Groq, koji Dango čuva u vlastitom unosu u privjesku ključeva.
- **Iz Lowspeaka.** Ako je Lowspeak postavljen drukčije od zadanih vrijednosti, kartica Diktat jednom nudi preuzimanje njegovih postavki, a na zahtjev i njegovih ključeva; macOS pita prije nego što Dango pročita neki od njih (odaberite “Dozvoli”, a ne “Uvijek dozvoli”).
- **Dozvole.** macOS pri prvom diktatu traži pristup mikrofonu. Tipka fn radi dok je Dango aktivna aplikacija i Lowspeak nije pokrenut; treba dozvolu za pristupačnost, koju Dango traži nakon prvog diktata, nikad pri pokretanju.

## Povjerenje

Mapa može pokrenuti programe čim se u njoj nešto pokrene: Git hookove i postavke repozitorija kao što je `core.fsmonitor`, jezične poslužitelje iz `node_modules`, hookove agenata, `.mcp.json`. Kad prvi put otvorite mapu, Dango pita vjerujete li joj.

- **Pouzdana:** Git, jezični poslužitelji, programi za otklanjanje pogrešaka, agenti i CI rade za ovu mapu i sve ispod nje.
- **Ograničena:** možete čitati i uređivati; oznake promjena u uredniku dolaze iz izravnog čitanja `.git`. Ništa što mapa konfigurira ne pokreće se. Okna Git, CI i AI to navode i nude da mapi ukažete povjerenje.

Tekst koji ne dolazi od vas može sadržavati upute namijenjene upravljanju agentom (prompt injection). Izlaz terminala (`@terminal`), stranica preglednika (`@browser`), CI zapisnik predan agentu, datoteka izvan projekta i duži tekst zalijepljen s web stranice zato se prosljeđuju označeni: oznaka kaže “Vanjski sadržaj”, a agent dobiva tekst s napomenom da su to nepouzdani podaci čije upute ne smije slijediti. Web stranice vlastitog agenta stižu s istom napomenom. Zalijepljeni tekst smatra se tekstom s web stranice kad to preglednik navede u memorijskom pretincu, kao što to čine Safari, Chrome i Firefox; tekst odnekud drugdje i kratka fraza od nekoliko riječi ostaju vaši.

## Sandbox

Sam Dango radi bez značajke App Sandbox jer bi je naslijedio svaki program koji pokreće: terminali, Git, jezični poslužitelji i agenti ne bi mogli raditi. Umjesto toga svaka AI nit ima sandbox prekidač, prema zadanim postavkama isključen. Što ograničava ovisi o agentu, a opis alata prekidača to navodi:

| Agent | S uključenim sandboxom |
|---|---|
| Claude | shell naredbe izvode se u Claudeovu vlastitom sandboxu; njegovi alati za datoteke i web ne |
| Codex | naredbe i promjene pišu samo u projekt, bez mreže |
| Vlastiti agent | datoteke, web zahtjevi i naredbe ostaju unutar projekta i dozvoljenih mapa, bez mreže (`sandbox-exec`) |
| Antigravity, OpenCode | nema sandboxa koji Dango može uključiti |

## Izgled i pristupačnost

- **Tamni način** slijedi sustav; **Povećaj kontrast** i **Smanji kretanje** se poštuju.
- **VoiceOver** imenuje rubni stupac, kartice, kartice AI vremenske crte, retke Gita i CI-ja, program za otklanjanje pogrešaka i tekst terminala. Prekidne točke nikad se ne razlikuju samo po boji.
- **Tipkovnica.** Svaka naredba ima prečac i svi se mogu promijeniti u datoteci `keybindings.json`; ⌥⌘\` prelazi na sljedeće okno.
- **Zdesna nalijevo.** Na arapskom i hebrejskom prozor je zrcaljen, a prikazi koda ostaju slijeva nadesno.

Što Dango dodaje povrh AppKita, područje po područje, opisano je u [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Jezici

Sučelje je prevedeno na 35 jezika: arapski, katalonski, kineski (pojednostavljeni), kineski (tradicionalni), hrvatski, češki, danski, nizozemski, engleski, finski, francuski, njemački, grčki, hebrejski, hindski, mađarski, indonezijski, talijanski, japanski, korejski, malajski, norveški, poljski, portugalski (Brazil), portugalski (Portugal), rumunjski, ruski, slovački, slovenski, španjolski, švedski, tajlandski, turski, ukrajinski i vijetnamski.

Dango slijedi redoslijed jezika u **Postavke sustava › Općenito › Jezik i regija**; ondje pod **Aplikacije** možete odabrati i jezik samo za Dango. Ako nema prijevoda za vaš jezik, koristi engleski. Izrazi slijede jedan pojmovnik, [docs/glossary.md](../../../docs/glossary.md), i Appleove vlastite prijevode, pa se Dango čita kao ostatak vašeg Maca.

## Tipkovnički prečaci

| Radnja | Prečac |
|---|---|
| Otvori mapu, novi prozor, zatvori prozor | ⌘O, ⇧⌘N, ⇧⌘W |
| Nova datoteka, spremi, spremi kao | ⌘N, ⌘S, ⇧⌘S |
| Nova kartica, zatvori karticu, sljedeća i prethodna kartica | ⌘T, ⌘W, ⇧⌘] i ⇧⌘[ |
| Brzo otvaranje, pretraga u projektu | ⌘P, ⇧⌘F |
| Datoteke, Pretraga, Agenti, Git, Testovi, Otklanjanje pogrešaka, CI, Kontejneri | ⌘1 do ⌘8 |
| Prikaži ili sakrij rubni stupac, sljedeće okno | ⌃⌘S, ⌥⌘\` |
| Natrag i naprijed kroz uređivana mjesta | ⌃- i ⌃⇧- |
| Uključi/isključi komentar, idi na redak | ⌘/, ⌃G |
| Sljedeće pojavljivanje, kursor iznad i ispod | ⌘D, ⌃⇧↑ i ⌃⇧↓ |
| Sažmi, proširi, sažmi sve, proširi sve | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Prelamanje redaka, inspektor | ⌥Z, ⌥⌘I |
| Dovršavanje, informacije pri prelasku pokazivačem, idi na definiciju | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Reference, hijerarhija poziva, preimenuj | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simboli u datoteci, u projektu | ⇧⌘O, ⌥⌘O |
| Brzi popravak, formatiraj | ⌥⌘A, ⌥⇧F |
| Sljedeći i prethodni problem | ⌘' i ⇧⌘' |
| Markdown pregled, zalijepi kao običan tekst | ⇧⌘V, ⌥⇧⌘V |
| Podebljano, kurziv, precrtano, kôd, poveznica | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Naslov 1 do 3 | ⌥⌘1 do ⌥⌘3 |
| Novi terminal, očisti terminal | ⌃\`, ⌘K |
| Fokus na polje za poruku agenta, komentiraj plan | ⌘Esc, ⌥⌘K |
| Polje adrese, ponovno učitaj, natrag, naprijed (preglednik) | ⌘L, ⌘R, ⌘[, ⌘] |
| Započni ili zaustavi diktat | ⌃⌥D |
| Prekidna točka, uključi ili isključi sve prekidne točke | ⌘\\, ⌘Y |
| Pokreni test na mjestu kursora, ponovno pokreni zadnje testove | ⌃⌥⌘U, ⌃⌥⌘G |
| Nastavi, preskoči, uđi u funkciju, iziđi iz funkcije, zaustavi | ⌃⌘Y, F6, F7, F8, ⌘. |
| Postavke | ⌘, |

`keybindings.json` u mapi s podacima aplikacije (pogledajte [Gdje se nalaze podaci](#gdje-se-nalaze-podaci)) mijenja bilo koji od njih.

## Privatnost i sigurnost

- **Bez telemetrije,** bez analitike, bez izvješća o rušenju poslanih bilo kamo. Pogreške idu u zapisnik sustava (aplikacija Konzola, pod nazivom aplikacije).
- **Mreža samo za ono što pokrenete:** agenti koje koristite, svaki sa svojom uslugom; ACP registar i preuzimanja agenata, jezičnih poslužitelja, adaptera za otklanjanje pogrešaka i modela za diktat kad ih zatražite; CI hostovi na koje se prijavite; predviđanja pri uređivanju od Copilota ako ste ga odabrali; pružatelji vlastitog agenta; mrežna transkripcija ako ste je odabrali; stranice koje otvorite u kartici preglednika; i provjera ažuriranja nakon što ste je dozvolili.
- **Kôd napušta Mac** samo ondje gdje ste za to odabrali uslugu: agent, Copilotova predviđanja, poruka commita koju je napisao AI. Razlike za poruku commita šalju se tek kad gitleaks u njima ne pronađe tajne podatke.
- **Ključevi i tokeni** ostaju u macOS privjesku ključeva, nikad u datoteci postavki. Ključevi drugih aplikacija, poput Lowspeakovih, čitaju se samo ako pristanete, i to samo jednom.
- **Preuzimanja u fiksnim verzijama.** Agenti, jezični poslužitelji i adapteri za otklanjanje pogrešaka koje Dango instalira vezani su uz provjerene verzije i provjeravaju se prema kontrolnim zbrojevima.
- **Potpisivanje koda** s Hardened Runtime; ažuriranja se instaliraju samo s vlastitim potpisom aplikacije.

## Gdje se nalaze podaci

`<id>` je identifikator paketa, `local.agentide` za izdanje (`local.agentide.debug` za razvojne izgradnje). Ne sadrži prikazni naziv, pa preimenovanje zadržava sve podatke.

| Mjesto | Što |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (popis AI okna), `agent-threads/` (razgovori vlastitog agenta), `layouts/` (kartice po projektu), `recovery/` (nespremljeni urednici nakon rušenja), `checkpoints/` (za mape bez Gita), `worktrees/`, `agents/` i `tools/` (instalirani agenti, jezični poslužitelji i adapteri za otklanjanje pogrešaka), `task-output/` |
| `~/Library/Caches/<id>/` | zalijepljene slike, skripte za integraciju s ljuskom, ACP registar; može se izbrisati |
| `~/Library/Logs/<id>/dictation/` | datoteke zapisnika diktata |
| `~/Library/Application Support/Lowspeak` i `FluidAudio` | lokalni modeli za diktat, zajednički s Lowspeakom |
| `~/Library/WebKit/<id>/` | kolačići i podaci web stranica kartice preglednika |
| `<project>/.dango/settings.json` | vlastite postavke projekta |
| Git repozitorij projekta | kontrolne točke, kao commitovi pod `refs/ide-checkpoints/`; običan push ili fetch nikad ih ne prenosi |
| privjesak ključeva, usluga `<id>` | API ključevi vlastitog agenta i diktata, CI tokeni |
| `~/.local/bin/dango` | naredba za terminal, ako ste je instalirali |

Claude čuva svoje sesije pod `~/.claude/projects`; Dango ih čita kako bi nastavio sesiju, ali ih ne mijenja.

## Poznata ograničenja

- **Nema značajke App Sandbox** za samu aplikaciju jer bi je naslijedili terminali, Git, jezični poslužitelji i agenti; mapi morate vjerovati prije nego što se pokrene išta što ona konfigurira. Antigravity i OpenCode nemaju sandbox koji Dango može uključiti.
- **Claudeovi podaci o potrošnji i raščlamba konteksta** oslanjaju se na dijelove njegova protokola označene kao eksperimentalni i skriveni su kad ti dijelovi nedostaju.
- **Premotavanje razgovora** radi samo za Claude; za ostale agente premotava se samo kôd.
- **Prijava u CI putem preglednika** isključena je; koristite `gh`, `glab`, token ili okruženje.
- **Gemini CLI se ne nudi:** Google je ukinuo prijavu osobnim računom i upućuje na Antigravity. Ranije Gemini niti ostaju čitljive.

## Izgradnja iz izvornog koda

Potrebno vam je:

- macOS 14 ili noviji s alatima Xcode Command Line Tools (`xcode-select --install`); sam Xcode nije potreban.
- Rust kako ga navodi `rust-toolchain.toml`. Homebrewov `rust` gradi za procesor ovog Maca; univerzalna izgradnja izdanja treba rustup s ciljevima `aarch64-apple-darwin` i `x86_64-apple-darwin`.
- Kopije repozitorija [splat](https://github.com/sventmueller/splat) i [lowspeak](https://github.com/sventmueller/lowspeak) pokraj ovoga (`../splat`, `../lowspeak`), obje na grani `main`; radni prostor iz njih preuzima crateove po putanji.
- cmake, za svaku izgradnju s diktatom (`make run`, `make build`, `make release`), koja gradi whisper.cpp. `make check` gradi bez njega.

| Naredba | Što radi |
|---|---|
| `make run` | gradi razvojnu aplikaciju (`target/debug/Dango Debug.app`) i otvara je |
| `make build` | gradi aplikaciju za izdanje (`target/release/Dango.app`), potpisanu lokalnim certifikatom koji prva izgradnja stvara u privjesku ključeva za prijavu; ostavlja `Cargo.toml` nepromijenjenim |
| `make lint` | formatiranje i clippy; upozorenja uzrokuju neuspjeh |
| `make check` | `lint`, zatim svi testovi; ne treba privjesak ključeva, prozor ni mrežu |
| `make test` | `check`, zatim samotest `probe` i revizija pristupačnosti, slijeva nadesno i zdesna nalijevo, u pravoj aplikaciji |
| `make perf` | vremenski proračuni, u izgradnji za izdanje; smisleno samo na inače neaktivnom Macu |
| `make release` | probno izdanje u `target/release-dry-run/`, potpisano lokalnim certifikatom; nikamo ništa ne šalje |
| `make release-notarized` | `check`, zatim samo izdanje u `dist/`: potpis Developer ID, notarizacija, feed ažuriranja potpisan vlastitim ključem aplikacije; ostavlja `Cargo.toml` nepromijenjenim |
| `make deploy` | na čistoj grani `main`, s pečatom CI-ja ili bez njega: notarizira sljedeću verziju, commita `Cargo.toml`, prenosi datoteke izdanja, pusha kôd i oznaku verzije, uz zeleni pečat CI-ja ako postoji, a zatim sam objavljuje izdanje |

Razvojna verzija i verzija za izdanje zasebne su aplikacije s vlastitim postavkama i podacima. Izgradnja, testiranje, samotestovi, izdanje i pravila kojih se kôd drži opisani su u [docs/development.md](../../../docs/development.md), a odluke iza toga u [docs/plan.md](../../../docs/plan.md). Bilješke o izdanju nalaze se u [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), prevedene pokraj svakog README-a u `assets/readme/<language>/`.

## Licenca

Dango je licenciran pod jednom od sljedećih licenci

- Licenca Apache, verzija 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licenca MIT ([LICENSE-MIT](../../../LICENSE-MIT))

po vašem izboru.

Osim ako izričito ne navedete drukčije, svaki doprinos koji ste namjerno poslali za uključivanje u Dango, kako je definiran u licenci Apache-2.0, dvostruko je licenciran kako je gore navedeno, bez dodatnih uvjeta.
