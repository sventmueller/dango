<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Ovo je prvo izdanje aplikacije Dango, nativnog macOS urednika za rad s agentima za kodiranje. Urednik, terminali,
preglednik, Git, CI, program za otklanjanje pogrešaka i AI niti dijele jedan prozor po mapi. Aplikacija je napisana u
Rustu na AppKitu; WebKit se pojavljuje samo u kartici preglednika. Sučelje je dostupno na 35 jezika.

## Zahtjevi
- macOS 14 ili noviji, na Apple siliconu ili Intelu.
- Za AI niti agent koji odaberete: Claude treba naredbu `claude` s prijavom; Codex treba Node.js, a aplikacija na vaš
  zahtjev instalira njegov paket u fiksnoj verziji; Antigravity i OpenCode koriste vaš vlastiti `agy` ili `opencode` ili
  preuzimanje u fiksnoj verziji; vlastiti agent treba API ključ ili Ollama odnosno LM Studio na ovom Macu.
- Jezični poslužitelji i adapteri za otklanjanje pogrešaka instaliraju se na vaš zahtjev. gopls i `dlv` trebaju Go;
  poslužitelji za TypeScript, Python, JSON, YAML, CSS, HTML i shell te JavaScript program za otklanjanje pogrešaka
  trebaju Node.js; clangd, sourcekit-lsp i `lldb-dap` dolaze s alatima Xcode Command Line Tools.
- Predviđanja pri uređivanju trebaju pretplatu na GitHub Copilot (dovoljna je i besplatna) ili model tipa
  fill-in-the-middle u aplikaciji Ollama ili LM Studio.

## Prozor
- Svaka datoteka, terminal, stranica preglednika, AI nit, prikaz razlika i pregled zasebna je macOS kartica u prozoru
  mape. Znak “+” na kraju trake kartica i ⌘T nude novu datoteku, terminal, preglednik i svaki instalirani agent, i to u
  vlastitom Git worktreeju.
- Rubni stupac prebacuje između prikaza Datoteke, Pretraga, Agenti, Git, Otkloni pogreške i CI (⌘1–⌘6); desnim klikom
  skrivate one koje ne koristite. Alatna traka sadrži Natrag i Naprijed kroz mjesta koja ste uređivali (⌃- i ⌃⇧-),
  granu, Brzo otvaranje i pregled Aktivnost.
- “Otvori sa strane” prikazuje drugi dokument pokraj prvog. Nakon ponovnog pokretanja ili rušenja svaki se prozor vraća
  sa svojim karticama, a urednici s nespremljenim tekstom vraćaju se s tim tekstom.
- Slike, audio, video, fontovi i Office dokumenti otvaraju se u kartici putem značajke Quick Look, a PDF datoteke putem
  PDFKita.

## Urednik
- Isticanje sintakse, brojevi redaka, oznake promjena u odnosu na zadnji commit, traženje i zamjena, Brzo otvaranje (⌘P)
  i pretraga projekta (⇧⌘F).
- Automatsko uvlačenje, tabulatori ili razmaci prema datoteci `.editorconfig`, parovi zagrada, uključivanje i
  isključivanje komentara, sažimanje blokova, odlazak na redak, vidljive praznine i prelamanje redaka.
- Više kursora: ⌥-klik, ⌥-povlačenje za stupac, ⌘D za sljedeće pojavljivanje.
- Datoteke zadržavaju kodiranje, oznaku redoslijeda bajtova i završetke redaka. Datoteka promijenjena na disku ponovno
  se učitava ako urednik nema nespremljenog teksta; inače traka nudi razlike i spajanje.
- **Predviđanja pri uređivanju:** dok tipkate, prijedlog se pojavljuje sivo; tipka Tab ga prihvaća. Prijedlozi dolaze od
  GitHub Copilota ili od lokalnog modela u aplikaciji Ollama ili LM Studio, ovisno o tome što odaberete u postavkama;
  lokalni model zadržava vaš kôd na ovom Macu.

## Jezični poslužitelji
Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, shell skripte, JSON, YAML, CSS, HTML, TOML,
Markdown i Lua te vlastiti poslužitelji iz postavki. Dijagnostika, informacije pri prelasku pokazivačem, dovršavanje,
pomoć za potpise, odlazak na definiciju, definiciju tipa i implementaciju, reference, hijerarhije poziva i tipova,
preimenovanje, formatiranje, semantičke boje, sažimanje blokova i struktura; tipovi i nazivi parametara unutar retka;
brzi popravci i refaktoriranja iza žarulje (⌥⌘A); simboli projekta pomoću ⌥⌘O ili `#` u prozoru Brzo otvaranje. Vlastiti
`tsc` projekta TypeScript koristi se kad može služiti kao jezični poslužitelj.

## Program za otklanjanje pogrešaka
Prekidne točke, i s uvjetima, izvršavanje korak po korak, niti sa stogovima poziva, varijable i praćeni izrazi u
Inspektoru (⌥⌘I), vrijednosti pokraj koda dok je program zaustavljen te konzola za otklanjanje pogrešaka koja izračunava
izraze u zaustavljenom okviru. Rust, C, C++ i Swift putem `lldb-dap`, Go putem `dlv`, JavaScript i TypeScript putem
JavaScript programa za otklanjanje pogrešaka, Python putem debugpyja. Čita se datoteka `.vscode/launch.json` projekta.
Izbornik Otkloni pogreške koristi prečace iz Xcodea.

## Terminal
Terminali u karticama s okruženjem prijavne ljuske, iscrtani pomoću Metala. Zalijepljena ili ispuštena slika predaje se
agentu za kodiranje koji radi u terminalu ili se lijepi putanja spremljene kopije. ⌘-klik otvara poveznice i nazive
datoteka. Razvojni poslužitelj koji terminal pokrene može se otvoriti u kartici preglednika.

## Markdown
Markdown datoteka otvara se s pregledom pokraj sebe (⇧⌘V), s formulama, dijagramima Mermaid, Graphviz i PlantUML te
slikama iz mape. Izbornik Formatiraj za podebljano, kurziv, poveznice, naslove, popise, citate i blokove koda; tekst
zalijepljen iz Worda ili s web stranice stiže kao Markdown. **Datoteka › Eksportiraj** zapisuje PDF, jednu HTML datoteku
ili Word dokument. Ako je Splat instaliran, **Datoteka › Otvori u aplikaciji Splat** otvara dokument u njemu.

## Preglednik
Kartica preglednika s poljem adrese, Natrag i Naprijed, pretragom, preuzimanjima uz prethodno pitanje te Web
Inspectorom.

## Git
Okno Git s pripremom i odbacivanjem datoteka ili pojedinačnih blokova, commitovima, commitom i pushom ili
sinkronizacijom, granama, stashevima, konfliktima spajanja i grafom nedavnih commitova. Blame prikazuje tko je zadnji
promijenio redak na kojem je kursor ili stupac za cijelu datoteku. Gumb piše poruku commita pomoću AI-ja; razlike se
šalju tek kad gitleaks u njima ne pronađe tajne podatke, a svaki se commit također provjerava na tajne podatke. AI nit
može raditi u vlastitom Git worktreeju; okno prikazuje što je promijenila i preuzima rad spajanjem ili cherry-pickom.

## CI
GitHub Actions i GitLab CI, također GitHub Enterprise Server i samostalno upravljani GitLab: pokretanja, poslovi i
zapisnici trenutačne grane; ponovno pokretanje neuspjelih poslova, jednog posla ili (na GitHubu) cijelog pokretanja,
pokretanje ručnih poslova ili odobravanje implementacija na čekanju, pokretanje novog pipelinea ili workflowa koji se
smije pokrenuti ručno te otkazivanje. Otvoreni merge requestovi odnosno pull requestovi i issuesi prikazuju se u istom
oknu i otvaraju u kartici preglednika. Zapisnik neuspjelog posla može se predati agentu, označen kao vanjski tekst.
Token dolazi iz privjeska ključeva, od `gh` ili `glab`, iz okruženja prijavne ljuske ili ga sami unesete.

## AI
Svaki agent radi u AI kartici s istom vremenskom crtom: odgovori, pozivi alata s njihovim izlazom, zahtjevi za dozvolu,
pitanja, planovi za odobravanje i komentiranje, obaveze, podagenti i pozadinski zadaci; dva gumba u zaglavlju kartice
navode podagente i pozadinske procese koji su u tijeku. Prije svakog prompta aplikacija sprema kontrolnu točku; promjene
se mogu pregledati i premotati po poruci. Nove niti počinju u načinu “Ručno”, a destruktivne Git naredbe nikad se ne
dozvoljavaju trajno. Pregled Aktivnost navodi sve što radi. Zelena točka na kartici označava sesiju koja radi, crvena
sesiju koja čeka vas, a obavijest javlja kad sesija završi ili čeka dok ste drugdje.
- **Claude** putem nepromijenjene naredbe `claude`, sa sesijama, podagentima, pozadinskim zadacima, kontekstom i
  potrošnjom.
- **ACP agenti:** Codex, Antigravity i OpenCode te drugi iz ACP registra na vaš zahtjev; instalirani agenti vezani su uz
  provjerene verzije i provjeravaju se prema kontrolnim zbrojevima.
- **Vlastiti agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio i poslužitelji koji govore API
  tvrtke OpenAI ili Anthropic, s alatima za datoteke, pretragu, naredbe, dijagnostiku i web zahtjeve; ključevi ostaju u
  privjesku ključeva.
- Sandbox prekidač po niti ograničava što smiju naredbe agenta, koliko god to agent podržava.
- Vanjski tekst, poput izlaza terminala, web stranice ili CI zapisnika, stiže agentu označen kao nepouzdan.

## Diktat
Diktat u polje za AI poruke i u urednik putem Lowspeakova prepoznavanja govora: gumb mikrofona, ⌃⌥D ili tipka fn dok je
Dango aktivna aplikacija. Koristi Lowspeakove postavke i modele; mrežna transkripcija treba ključ za OpenAI ili Groq.

## Ažuriranja
Aplikacija se ažurira putem Sparklea. Pri drugom pokretanju pita smije li sama provjeravati ažuriranja; dok ne
pristanete, ne šalje nikakav zahtjev. **Dango › Potraži ažuriranja…** odmah provjerava.

## Pristupačnost
VoiceOver nazivi za rubni stupac, kartice, kartice AI vremenske crte, retke Gita i CI-ja, program za otklanjanje
pogrešaka i tekst terminala; potpuno upravljanje tipkovnicom s prečacima koje možete promijeniti u datoteci
`keybindings.json`; Povećaj kontrast i Smanji kretanje; prikazi koda ostaju slijeva nadesno i u sučeljima koja se pišu
zdesna nalijevo. Pojedinosti su u datoteci ACCESSIBILITY.md.

## Privatnost
Bez telemetrije, bez analitike i bez izvješća o rušenju. Aplikacija se povezuje samo za ono što sami pokrenete: agente
koje koristite, preuzimanja koja zatražite, CI hostove na koje se prijavite, predviđanja od Copilota ako ste ga
odabrali, mrežnu transkripciju ako ste je odabrali, stranice koje otvorite i provjeru ažuriranja nakon što ste je
dozvolili. Ključevi i tokeni ostaju u macOS privjesku ključeva.

## Poznata ograničenja
- Prijava u CI putem preglednika isključena je; koristite `gh`, `glab`, token ili okruženje.
- Aplikacija radi bez značajke App Sandbox jer bi je naslijedili terminali, Git, jezični poslužitelji i agenti; mapi
  morate vjerovati prije nego što se pokrene išta što ona konfigurira. Antigravity i OpenCode nemaju sandbox koji
  aplikacija može uključiti.
- Claudeovi podaci o potrošnji i raščlamba konteksta oslanjaju se na dijelove njegova protokola označene kao
  eksperimentalni i skriveni su kad ti dijelovi nedostaju. Premotavanje razgovora radi samo za Claude; za ostale agente
  premotava se samo kôd.
- Gemini CLI se ne nudi: Google je ukinuo prijavu osobnim računom i upućuje na Antigravity. Ranije Gemini niti ostaju
  čitljive.
