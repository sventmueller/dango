<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Toto je prvé vydanie aplikácie Dango, natívneho editora pre macOS na prácu s kódovacími agentmi. Editor, terminály,
prehliadač, Git, CI, ladiaci nástroj a AI vlákna zdieľajú jedno okno pre každý priečinok. Aplikácia je napísaná v Ruste
nad AppKitom; WebKit sa objavuje iba v tabe prehliadača. Rozhranie je k dispozícii v 35 jazykoch.

## Požiadavky
- macOS 14 alebo novší, na Apple Silicon alebo Intel.
- Pre AI vlákna agent, ktorého si vyberiete: Claude potrebuje príkaz `claude` s prihlásením; Codex potrebuje Node.js a
  aplikácia na požiadanie nainštaluje jeho balík v pevne určenej verzii; Antigravity a OpenCode používajú váš vlastný
  `agy` alebo `opencode`, prípadne stiahnutie pevne určenej verzie; vlastný agent potrebuje kľúč API alebo Ollama či LM
  Studio na tomto Macu.
- Jazykové servery a ladiace adaptéry sa nainštalujú na požiadanie. gopls a `dlv` potrebujú Go; servery pre TypeScript,
  Python, JSON, YAML, CSS, HTML a shell a JavaScriptový debugger potrebujú Node.js; clangd, sourcekit-lsp a `lldb-dap`
  sú súčasťou Xcode Command Line Tools.
- Predpovede pri písaní potrebujú predplatné GitHub Copilot (stačí bezplatné) alebo model typu fill-in-the-middle v
  Ollama alebo LM Studio.

## Okno
- Každý súbor, terminál, stránka prehliadača, AI vlákno, diff a náhľad je tab macOS v okne priečinka. „+“ na konci lišty
  tabov a ⌘T ponúkajú nový súbor, terminál, prehliadač a každého nainštalovaného agenta, aj vo vlastnom git worktree.
- Postranný panel prepína medzi časťami Súbory, Vyhľadávanie, Agenti, Git, CI a Ladenie (⌘1–⌘6); kliknutím pravým
  tlačidlom skryjete tie, ktoré nepoužívate. Na paneli s nástrojmi je späť a dopredu cez miesta, ktoré ste upravovali
  (⌃- a ⌃⇧-), vetva, Rýchlo otvoriť a prehľad Aktivita.
- „Otvoriť vedľa“ zobrazí druhý dokument vedľa prvého. Po reštarte alebo páde sa každé okno vráti so svojimi tabmi a
  editory s neuloženým textom sa vrátia aj s ním.
- Obrázky, zvuk, video, písma a dokumenty Office sa otvárajú v tabe cez Quick Look, PDF cez PDFKit.

## Editor
- Zvýrazňovanie syntaxe, čísla riadkov, značky zmien oproti poslednému commitu, hľadanie a nahrádzanie, Rýchlo otvoriť
  (⌘P) a vyhľadávanie v projekte (⇧⌘F).
- Automatické odsadenie, tabulátory alebo medzery podľa `.editorconfig`, párové zátvorky, prepínanie komentárov,
  zbaľovanie, prechod na riadok, viditeľné medzery a zalamovanie riadkov.
- Viac kurzorov: ⌥-kliknutie, ⌥-ťahanie pre stĺpec, ⌘D pre ďalší výskyt.
- Súbory si zachovávajú kódovanie, značku poradia bajtov a konce riadkov. Súbor zmenený na disku sa znova načíta, keď
  editor nemá neuložený text; inak lišta ponúkne rozdiely a zlúčenie.
- **Predpovede pri písaní:** počas písania sa sivou farbou zobrazí návrh; kláves Tab ho prijme. Návrhy pochádzajú z
  GitHub Copilot alebo z lokálneho modelu v Ollama alebo LM Studio, podľa toho, čo si vyberiete v nastaveniach; lokálny
  model ponechá váš kód na tomto Macu.

## Jazykové servery
Go, Rust, TypeScript a JavaScript, Python, C, C++ a Objective-C, Swift, shellové skripty, JSON, YAML, CSS, HTML, TOML,
Markdown a Lua, plus vlastné servery z nastavení. Diagnostika, informácie pri podržaní ukazovateľa, dopĺňanie, pomoc so
signatúrou, prechod na definíciu, definíciu typu a implementáciu, referencie, hierarchie volaní a typov, premenovanie,
formátovanie, sémantické farby, zbaľovanie a osnova; typy a názvy parametrov priamo v riadku; rýchle opravy a
refaktorovania za žiarovkou (⌥⌘A); symboly projektu cez ⌥⌘O alebo `#` v Rýchlo otvoriť. Vlastný `tsc` projektu
TypeScript sa použije, keď môže slúžiť ako jazykový server.

## Ladenie
Body prerušenia, aj s podmienkami, krokovanie, vlákna programu s ich zásobníkmi volaní, premenné a sledované výrazy v
Inšpektore (⌥⌘I), hodnoty vedľa kódu, kým je program pozastavený, a ladiaca konzola, ktorá vyhodnocuje v pozastavenom
rámci. Rust, C, C++ a Swift cez `lldb-dap`, Go cez `dlv`, JavaScript a TypeScript cez JavaScriptový debugger, Python cez
debugpy. Načíta sa `.vscode/launch.json` projektu. Ponuka Ladenie používa skratky z Xcode.

## Terminál
Terminály v taboch s prostredím prihlasovacieho shellu, vykresľované cez Metal. Vložený alebo pretiahnutý obrázok sa
odovzdá kódovaciemu agentovi spustenému v termináli, inak sa vloží cesta k uloženej kópii. ⌘-kliknutie otvára odkazy a
názvy súborov. Vývojový server, ktorý terminál spustí, možno otvoriť v tabe prehliadača.

## Markdown
Súbor Markdown sa otvorí s náhľadom vedľa (⇧⌘V), so vzorcami, diagramami Mermaid, Graphviz a PlantUML a obrázkami z
priečinka. Ponuka Formát pre tučné písmo, kurzívu, odkazy, nadpisy, zoznamy, citácie a bloky kódu; text vložený z Wordu
alebo webovej stránky príde ako Markdown. **Súbor › Exportovať** zapíše PDF, jeden súbor HTML alebo dokument Word. Ak je
nainštalovaný Splat, **Súbor › Otvoriť v aplikácii Splat** v ňom dokument otvorí.

## Prehliadač
Tab prehliadača s poľom adresy, tlačidlami späť a dopredu, vyhľadávaním, sťahovaním s predchádzajúcou otázkou a webovým
inšpektorom.

## Git
Panel Git s pripravovaním na commit a zahadzovaním celých súborov alebo jednotlivých úsekov, commitmi, commitom s pushom
alebo synchronizáciou, vetvami, stashmi, konfliktmi pri zlúčení a grafom posledných commitov. Blame ukazuje, kto
naposledy zmenil riadok pod kurzorom, alebo stĺpec pre celý súbor. Tlačidlo napíše správu commitu pomocou AI; diff sa
odošle až vtedy, keď v ňom gitleaks nenašiel žiadne tajné údaje, a každý commit sa na tajné údaje kontroluje tiež. AI
vlákno môže pracovať vo vlastnom git worktree; panel ukáže, čo v ňom zmenilo, a prevezme prácu zlúčením alebo
cherry-pickom.

## CI
GitHub Actions a GitLab CI, aj GitHub Enterprise Server a GitLab na vlastnom serveri: spustenia, joby a logy aktuálnej
vetvy; opätovné spustenie neúspešných jobov, jedného jobu alebo (na GitHube) celého spustenia, spúšťanie manuálnych
jobov alebo schvaľovanie čakajúcich nasadení, spustenie novej pipeline alebo workflowu, ktorý možno spustiť ručne, a
zrušenie. Otvorené merge requesty alebo pull requesty a issues sa zobrazia v tom istom paneli a otvárajú sa v tabe
prehliadača. Log neúspešného jobu možno odovzdať agentovi, označený ako text zvonku. Token pochádza z kľúčenky, z `gh`
alebo `glab`, z prostredia prihlasovacieho shellu, alebo ho zadáte.

## AI
Každý agent pracuje v AI tabe s rovnakou časovou osou: odpovede, volania nástrojov s ich výstupom, žiadosti o povolenie,
otázky, plány na schválenie a komentovanie, zoznamy úloh, subagenti a úlohy na pozadí; dve tlačidlá v hlavičke tabu
zobrazia bežiacich subagentov a procesy na pozadí. Pred každým promptom si aplikácia uloží kontrolný bod; zmeny možno
skontrolovať a pretočiť späť po jednotlivých správach. Nové vlákna začínajú v režime „Manuálne“ a deštruktívne príkazy
gitu sa nikdy nepovolia natrvalo. Prehľad Aktivita zobrazuje všetko, čo beží. Zelená bodka na tabe ukazuje reláciu,
ktorá pracuje, červená reláciu, ktorá na vás čaká, a hlásenie vás upozorní, keď relácia skončí alebo čaká, kým sa
venujete niečomu inému.
- **Claude** cez nezmenený príkaz `claude`, s reláciami, subagentmi, úlohami na pozadí, kontextom a využitím.
- **ACP agenti:** Codex, Antigravity a OpenCode a na požiadanie ďalší z registra ACP; nainštalovaní agenti sú pevne
  nastavení na otestované verzie a overení podľa svojich kontrolných súčtov.
- **Vlastný agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio a servery s API od OpenAI alebo
  Anthropic, s nástrojmi na súbory, vyhľadávanie, príkazy, diagnostiku a webové požiadavky; kľúče zostávajú v kľúčenke.
- Prepínač sandboxu pre každé vlákno obmedzí, čo smú príkazy agenta robiť, pokiaľ to agent podporuje.
- Text zvonku, napríklad výstup terminálu, webová stránka alebo log CI, ide agentovi označený ako nedôveryhodný.

## Diktovanie
Diktovanie do poľa správy AI a do editora cez rozpoznávanie reči z Lowspeak: tlačidlo mikrofónu, ⌃⌥D alebo kláves fn,
kým je Dango aktívnou aplikáciou. Používa nastavenia a modely z Lowspeak; online prepis potrebuje kľúč pre OpenAI alebo
Groq.

## Aktualizácie
Aplikácia sa aktualizuje sama cez Sparkle. Pri druhom spustení sa spýta, či môže sama vyhľadávať aktualizácie; kým
nesúhlasíte, neodošle žiadnu požiadavku. **Dango › Vyhľadať aktualizácie…** ich vyhľadá hneď.

## Prístupnosť
Názvy pre VoiceOver pre postranný panel, taby, karty časovej osi AI, riadky Git a CI, ladiaci nástroj a text terminálu;
úplné ovládanie klávesnicou so skratkami, ktoré môžete zmeniť v `keybindings.json`; Zvýšiť kontrast a Obmedziť pohyb;
zobrazenia kódu zostávajú zľava doprava aj v rozhraniach písaných sprava doľava. Podrobnosti sú v ACCESSIBILITY.md.

## Súkromie
Žiadna telemetria, žiadna analytika a žiadne hlásenia o pádoch. Aplikácia sa pripája iba kvôli tomu, čo spustíte:
agenti, ktorých používate, sťahovania, o ktoré požiadate, hostitelia CI, do ktorých sa prihlásite, predpovede z Copilot,
ak ste si ho vybrali, online prepis, ak ste si ho vybrali, stránky, ktoré otvoríte, a kontrola aktualizácií, keď ste ju
povolili. Kľúče a tokeny zostávajú v kľúčenke macOS.

## Známe obmedzenia
- Prihlásenie do CI cez prehliadač je vypnuté; použite `gh`, `glab`, token alebo prostredie.
- Aplikácia beží bez App Sandbox, pretože by ho zdedili terminály, git, jazykové servery aj agenti; priečinku musíte
  dôverovať skôr, než sa spustí čokoľvek, čo konfiguruje. Antigravity a OpenCode nemajú sandbox, ktorý by aplikácia
  mohla zapnúť.
- Údaje o využití a rozpis kontextu pri Claude sa opierajú o časti jeho protokolu označené ako experimentálne a skryjú
  sa, keď chýbajú. Pretočenie konverzácie späť funguje iba pri Claude; pri ostatných agentoch sa pretočí späť iba kód.
- Gemini CLI sa neponúka: Google ukončil prihlasovanie osobným účtom a odkazuje na Antigravity. Staršie vlákna Gemini
  zostávajú čitateľné.
