<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
To je prva izdaja aplikacije Dango, izvornega urejevalnika za macOS za delo z agenti za programiranje. Urejevalnik,
terminali, brskalnik, Git, CI, razhroščevalnik in niti AI si delijo eno okno na mapo. Aplikacija je napisana v jeziku
Rust na ogrodju AppKit; WebKit se pojavi le v zavihku brskalnika. Vmesnik je na voljo v 35 jezikih.

## Zahteve
- macOS 14 ali novejši, na Apple Silicon ali Intel.
- Za niti AI agent, ki ga izberete: Claude potrebuje ukaz `claude` z vpisom; Codex potrebuje Node.js, aplikacija pa na
  zahtevo namesti njegov paket v določeni različici; Antigravity in OpenCode uporabljata vaš lastni `agy` ali `opencode`
  ali prenos določene različice; lastni agent potrebuje ključ API ali Ollama oziroma LM Studio v tem Macu.
- Jezikovne strežnike in adapterje za razhroščevanje aplikacija namesti na zahtevo. gopls in `dlv` potrebujeta Go;
  strežniki za TypeScript, Python, JSON, YAML, CSS, HTML in lupino ter razhroščevalnik za JavaScript potrebujejo
  Node.js; clangd, sourcekit-lsp in `lldb-dap` so del orodij Xcode Command Line Tools.
- Napovedovanje med tipkanjem potrebuje naročnino GitHub Copilot (zadošča brezplačna) ali model fill-in-the-middle v
  Ollama ali LM Studio.

## Okno
- Vsaka datoteka, terminal, stran brskalnika, nit AI, diff in predogled je zavihek macOS v oknu mape. »+« na koncu
  vrstice zavihkov in ⌘T ponujata novo datoteko, terminal, brskalnik in vsakega nameščenega agenta, tudi v lastnem
  worktreeju Git.
- Stranska vrstica preklaplja med razdelki Datoteke, Iskanje, Agenti, Git, CI in Odpravljanje napak (⌘1–⌘6); z desnim
  klikom skrijete tiste, ki jih ne uporabljate. V orodni vrstici so nazaj in naprej po mestih, ki ste jih urejali (⌃- in
  ⌃⇧-), veja, Hitro odpri in pregled Aktivnost.
- »Odpri ob strani« prikaže drugi dokument ob prvem. Po ponovnem zagonu ali sesutju se vsako okno vrne s svojimi
  zavihki, urejevalniki z neshranjenim besedilom pa skupaj z njim.
- Slike, zvok, video, pisave in dokumenti Office se odprejo v zavihku prek Quick Look, PDF-ji prek PDFKit.

## Urejevalnik
- Barvanje sintakse, številke vrstic, oznake sprememb glede na zadnji commit, iskanje in zamenjava, Hitro odpri (⌘P) in
  iskanje po projektu (⇧⌘F).
- Samodejno zamikanje, tabulatorji ali presledki po `.editorconfig`, pari oklepajev, preklop komentarjev, zlaganje,
  pojdi na vrstico, vidni presledki in prelom vrstic.
- Več kazalk: ⌥-klik, ⌥-vlečenje za stolpec, ⌘D za naslednjo pojavitev.
- Datoteke ohranijo kodiranje, oznako vrstnega reda bajtov in konce vrstic. Datoteka, spremenjena na disku, se znova
  naloži, če urejevalnik nima neshranjenega besedila; sicer vrstica ponudi razlike in združitev.
- **Napovedovanje med tipkanjem:** med tipkanjem se v sivi barvi prikaže predlog; tipka Tab ga sprejme. Predlogi
  prihajajo iz GitHub Copilot ali iz lokalnega modela v Ollama ali LM Studio, kar izberete v nastavitvah; lokalni model
  obdrži vašo kodo v tem Macu.

## Jezikovni strežniki
Go, Rust, TypeScript in JavaScript, Python, C, C++ in Objective-C, Swift, skripti lupine, JSON, YAML, CSS, HTML, TOML,
Markdown in Lua ter lastni strežniki iz nastavitev. Diagnostika, informacije ob lebdenju, dokončanje, pomoč pri
podpisih, pojdi na definicijo, definicijo tipa in implementacijo, sklici, hierarhije klicev in tipov, preimenovanje,
oblikovanje, semantične barve, zlaganje in oris; tipi in imena parametrov znotraj vrstice; hitri popravki in
refaktoriranja za žarnico (⌥⌘A); simboli projekta z ⌥⌘O ali `#` v Hitro odpri. Lastni `tsc` projekta TypeScript se
uporabi, kadar lahko deluje kot jezikovni strežnik.

## Odpravljanje napak
Prekinitvene točke, tudi s pogoji, izvajanje po korakih, niti programa z njihovimi skladi klicev, spremenljivke in
opazovani izrazi v Inšpektorju (⌥⌘I), vrednosti ob kodi, ko je program ustavljen, in konzola za odpravljanje napak, ki
vrednoti v ustavljenem okviru. Rust, C, C++ in Swift prek `lldb-dap`, Go prek `dlv`, JavaScript in TypeScript prek
razhroščevalnika za JavaScript, Python prek debugpy. Aplikacija prebere `.vscode/launch.json` projekta. Meni
Odpravljanje napak uporablja bližnjice iz Xcode.

## Terminal
Terminali v zavihkih z okoljem prijavne lupine, izrisani z Metal. Prilepljena ali povlečena slika gre agentu za
programiranje, ki teče v terminalu, sicer se prilepi pot do shranjene kopije. ⌘-klik odpre povezave in imena datotek.
Razvojni strežnik, ki ga zažene terminal, lahko odprete v zavihku brskalnika.

## Markdown
Datoteka Markdown se odpre s predogledom ob strani (⇧⌘V), s formulami, diagrami Mermaid, Graphviz in PlantUML ter
slikami iz mape. Meni Oblikovanje za krepko, ležeče, povezave, naslove, sezname, citate in bloke kode; besedilo,
prilepljeno iz Worda ali spletne strani, pride kot Markdown. **Datoteka › Izvozi** zapiše PDF, eno datoteko HTML ali
dokument Word. Kjer je nameščen Splat, **Datoteka › Odpri v aplikaciji Splat** odpre dokument v njem.

## Brskalnik
Zavihek brskalnika z naslovnim poljem, nazaj in naprej, iskanjem, prenosi z vprašanjem vnaprej in spletnim inšpektorjem.

## Git
Podokno Git s pripravo za commit in zavrženjem celih datotek ali posameznih odsekov, commiti, commitom s pushem ali
sinhronizacijo, vejami, stashi, konflikti združitve in grafom zadnjih commitov. Blame pokaže, kdo je nazadnje spremenil
vrstico pod kazalko, ali stolpec za celotno datoteko. Gumb napiše sporočilo commita z AI; diff gre ven šele, ko gitleaks
v njem ne najde nobene skrivnosti, in tudi vsak commit se preveri glede skrivnosti. Nit AI lahko dela v lastnem
worktreeju Git; podokno pokaže, kaj je spremenila, in prevzame delo z združitvijo ali cherry-pickom.

## CI
GitHub Actions in GitLab CI, tudi GitHub Enterprise Server in samostojno gostovan GitLab: zagoni, jobi in dnevniki
trenutne veje; ponovni zagon neuspelih jobov, posameznega joba ali (na GitHubu) celotnega zagona, zagon ročnih jobov ali
odobritev čakajočih uvajanj, zagon novega pipelinea ali poteka dela, ki ga je mogoče zagnati ročno, in preklic. Odprti
merge requesti ali pull requesti in zadeve so navedeni v istem podoknu in se odprejo v zavihku brskalnika. Dnevnik
neuspelega joba lahko pošljete agentu, označen kot besedilo od zunaj. Žeton pride iz verige ključev, iz `gh` ali `glab`,
iz okolja prijavne lupine ali pa ga vnesete.

## AI
Vsak agent dela v zavihku AI z isto časovnico: odgovori, klici orodij z izhodom, zahteve za dovoljenje, vprašanja,
načrti za odobritev in komentiranje, opravki, podagenti in opravila v ozadju; gumba v glavi zavihka navedeta podagente
in procese v ozadju, ki tečejo. Pred vsakim pozivom aplikacija shrani kontrolno točko; spremembe lahko pregledate in
previjete nazaj po posameznih sporočilih. Nove niti se začnejo v načinu »Ročno«, uničujoči ukazi Git pa nikoli niso
dovoljeni za vedno. Pregled Aktivnost navaja vse, kar teče. Zelena pika na zavihku pokaže sejo pri delu, rdeča sejo, ki
čaka na vas, obvestilo pa sporoči, ko se seja konča ali čaka, medtem ko gledate drugam.
- **Claude** prek nespremenjenega ukaza `claude`, s sejami, podagenti, opravili v ozadju, kontekstom in porabo.
- **Agenti ACP:** Codex, Antigravity in OpenCode ter na zahtevo še drugi iz registra ACP; nameščeni agenti so vezani na
  preizkušene različice in preverjeni z njihovimi kontrolnimi vsotami.
- **Lastni agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio in strežniki, ki govorijo API OpenAI
  ali Anthropic, z orodji za datoteke, iskanje, ukaze, diagnostiko in spletne zahteve; ključi ostanejo v verigi ključev.
- Stikalo peskovnika za vsako nit omeji, kaj smejo ukazi agenta, kolikor agent to podpira.
- Besedilo od zunaj, na primer izhod terminala, spletna stran ali dnevnik CI, gre agentu označeno kot nezaupanja vredno.

## Narekovanje
Narekovanje v polje za sporočila AI in v urejevalnik prek Lowspeakovega prepoznavanja govora: gumb mikrofona, ⌃⌥D ali
tipka fn, ko je Dango aktivna aplikacija. Uporablja Lowspeakove nastavitve in modele; spletni prepis potrebuje ključ za
OpenAI ali Groq.

## Posodobitve
Aplikacija se posodablja sama prek Sparkle. Ob drugem zagonu vpraša, ali sme sama preverjati posodobitve; dokler se ne
strinjate, ne pošlje nobene zahteve. **Dango › Preveri, ali so na voljo posodobitve …** preveri takoj.

## Dostopnost
Imena VoiceOver za stransko vrstico, zavihke, kartice časovnice AI, vrstice Git in CI, razhroščevalnik in besedilo
terminala; popoln nadzor s tipkovnico z bližnjicami, ki jih lahko spremenite v `keybindings.json`; Povečaj kontrast in
Zmanjšaj gibanje; pogledi kode ostanejo od leve proti desni tudi v vmesnikih, ki se berejo od desne proti levi.
Podrobnosti so v ACCESSIBILITY.md.

## Zasebnost
Brez telemetrije, brez analitike in brez poročil o sesutjih. Aplikacija se poveže le za tisto, kar zaženete: agenti, ki
jih uporabljate, prenosi, ki jih zahtevate, gostitelji CI, v katere se vpišete, napovedi iz Copilota, če ste ga izbrali,
spletni prepis, če ste ga izbrali, strani, ki jih odprete, in preverjanje posodobitev, ko ste ga dovolili. Ključi in
žetoni ostanejo v verigi ključev macOS.

## Znane omejitve
- Vpis v CI prek brskalnika je izklopljen; uporabite `gh`, `glab`, žeton ali okolje.
- Aplikacija teče brez App Sandbox, ker bi ga podedovali terminali, Git, jezikovni strežniki in agenti; mapi morate
  zaupati, preden se zažene karkoli, kar nastavi. Antigravity in OpenCode nimata peskovnika, ki bi ga aplikacija lahko
  vklopila.
- Claudovi podatki o porabi in razčlenitev konteksta temeljijo na delih njegovega protokola, označenih kot poskusni, in
  so skriti, kadar manjkajo. Previjanje pogovora nazaj deluje le za Claude; pri drugih agentih se previje nazaj le koda.
- Gemini CLI ni na voljo: Google je ukinil vpis z osebnim računom in napotuje na Antigravity. Prejšnje niti Gemini
  ostanejo berljive.
