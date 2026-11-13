<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Toto je první vydání aplikace Dango, nativního editoru pro macOS určeného k práci s agenty pro programování. Editor,
terminály, prohlížeč, Git, CI, ladicí nástroj a vlákna AI sdílejí jedno okno pro každou složku. Aplikace je napsaná v
jazyce Rust nad AppKit; WebKit se objevuje jen v panelu prohlížeče. Rozhraní je k dispozici ve 35 jazycích.

## Požadavky
- macOS 14 nebo novější, na Apple silicon nebo Intel.
- Pro vlákna AI agent, kterého si vyberete: Claude potřebuje příkaz `claude` s přihlášením; Codex potřebuje Node.js a
  aplikace na požádání nainstaluje jeho balíček v pevně dané verzi; Antigravity a OpenCode používají váš vlastní `agy`
  nebo `opencode`, případně stažení v pevně dané verzi; vlastní agent potřebuje klíč API, nebo aplikaci Ollama či LM
  Studio na tomto Macu.
- Jazykové servery a ladicí adaptéry se instalují na požádání. gopls a `dlv` potřebují Go; servery pro TypeScript,
  Python, JSON, YAML, CSS, HTML a shell a ladicí nástroj pro JavaScript potřebují Node.js; clangd, sourcekit-lsp a
  `lldb-dap` jsou součástí Xcode Command Line Tools.
- Předpovědi úprav potřebují předplatné GitHub Copilot (stačí bezplatné), nebo model typu fill-in-the-middle v aplikaci
  Ollama nebo LM Studio.

## Okno
- Každý soubor, terminál, stránka prohlížeče, vlákno AI, zobrazení rozdílů a náhled je panelem macOS v okně složky.
  Tlačítko „+“ na konci pruhu panelů a ⌘T nabízejí nový soubor, terminál, prohlížeč a každého nainstalovaného agenta,
  také ve vlastním worktree Gitu.
- Boční panel přepíná mezi podokny Soubory, Hledání, Agenti, Git, CI a Ladit (⌘1–⌘6); ta, která nepoužíváte, skryjete
  kliknutím pravým tlačítkem. Na nástrojovém panelu jsou zpět a vpřed po místech, která jste upravovali (⌃- a ⌃⇧-),
  větev, Rychle otevřít a přehled Aktivita.
- „Otevřít vedle“ zobrazí druhý dokument vedle prvního. Po restartu nebo pádu se každé okno vrátí se svými panely a
  editory s neuloženým textem se vrátí i s tímto textem.
- Obrázky, zvuk, video, písma a dokumenty Office se otevírají v panelu přes Quick Look, PDF přes PDFKit.

## Editor
- Barvy syntaxe, čísla řádků, značky změn oproti poslednímu commitu, hledání a nahrazování, Rychle otevřít (⌘P) a
  hledání v projektu (⇧⌘F).
- Automatické odsazování, tabulátory nebo mezery podle `.editorconfig`, párové závorky, přepínání komentářů, sbalování,
  přechod na řádek, viditelné mezery a zalamování řádků.
- Více kurzorů: ⌥-kliknutí, ⌥-tažení pro sloupec, ⌘D pro další výskyt.
- Soubory si zachovávají kódování, značku pořadí bajtů a konce řádků. Soubor změněný na disku se znovu načte, pokud v
  editoru není neuložený text; jinak lišta nabídne rozdíly a sloučení.
- **Předpovědi úprav:** během psaní se šedě zobrazí návrh; klávesa Tab ho přijme. Návrhy přicházejí z GitHub Copilot
  nebo z lokálního modelu v aplikaci Ollama nebo LM Studio, podle toho, co zvolíte v nastavení; lokální model ponechá
  váš kód na tomto Macu.

## Jazykové servery
Go, Rust, TypeScript a JavaScript, Python, C, C++ a Objective-C, Swift, skripty shellu, JSON, YAML, CSS, HTML, TOML,
Markdown a Lua a také vlastní servery z nastavení. Diagnostika, informace při najetí myší, doplňování, nápověda k
signatuře, přechod na definici, definici typu a implementaci, odkazy, hierarchie volání a typů, přejmenování,
formátování, sémantické barvy, sbalování a osnova; typy a názvy parametrů přímo v řádku; rychlé opravy a refaktorování
za žárovkou (⌥⌘A); symboly projektu přes ⌥⌘O nebo `#` v Rychle otevřít. Vlastní `tsc` projektu TypeScript se použije,
pokud může sloužit jako jazykový server.

## Ladicí nástroj
Zarážky, i s podmínkami, krokování, vlákna s jejich zásobníky volání, proměnné a sledované výrazy v Inspektoru (⌥⌘I),
hodnoty vedle kódu, dokud je program pozastaven, a konzole ladění, která vyhodnocuje v pozastaveném rámci. Rust, C, C++
a Swift přes `lldb-dap`, Go přes `dlv`, JavaScript a TypeScript přes ladicí nástroj pro JavaScript, Python přes debugpy.
Načítá se soubor `.vscode/launch.json` projektu. Nabídka Ladit používá klávesové zkratky Xcode.

## Terminál
Terminály v panelech s prostředím přihlašovacího shellu, vykreslované přes Metal. Vložení nebo přetažení obrázku ho
předá agentovi pro programování, který v terminálu běží, nebo vloží cestu k uložené kopii. ⌘-kliknutí otevírá odkazy a
názvy souborů. Vývojový server, který terminál spustí, lze otevřít v panelu prohlížeče.

## Markdown
Soubor Markdown se otevře s náhledem vedle sebe (⇧⌘V), se vzorci, diagramy Mermaid, Graphviz a PlantUML a obrázky ze
složky. Nabídka Formát pro tučné písmo, kurzívu, odkazy, nadpisy, seznamy, citace a bloky kódu; text vložený z Wordu
nebo z webové stránky dorazí jako Markdown. **Soubor › Exportovat** zapíše PDF, jeden soubor HTML nebo dokument Wordu.
Pokud je nainstalovaný Splat, **Soubor › Otevřít v aplikaci Splat** v něm dokument otevře.

## Prohlížeč
Panel prohlížeče s adresním polem, zpět a vpřed, hledáním, stahováním s předchozím dotazem a Web Inspectorem.

## Git
Podokno Git s přípravou k commitu a zahazováním souborů nebo jednotlivých bloků, commity, commitem s pushem nebo
synchronizací, větvemi, stashi, konflikty sloučení a grafem posledních commitů. Blame ukazuje, kdo naposledy změnil
řádek pod kurzorem, nebo sloupec pro celý soubor. Tlačítko napíše zprávu commitu pomocí AI; rozdíly odejdou až poté, co
v nich gitleaks nenašel žádné tajné údaje, a na tajné údaje se kontroluje i každý commit. Vlákno AI může pracovat ve
vlastním worktree Gitu; podokno ukáže, co změnilo, a převezme práci sloučením nebo cherry-pickem.

## CI
GitHub Actions a GitLab CI, také GitHub Enterprise Server a GitLab ve vlastní správě: běhy, joby a logy aktuální větve;
opětovné spuštění neúspěšných jobů, jednoho jobu nebo (na GitHubu) celého běhu, spouštění ručních jobů nebo schvalování
čekajících nasazení, spuštění nové pipeline nebo workflow, který lze spustit ručně, a rušení. Otevřené merge requesty
nebo pull requesty a issues jsou ve stejném podokně a otevírají se v panelu prohlížeče. Log neúspěšného jobu lze předat
agentovi, označený jako cizí text. Token pochází z Klíčenky, z `gh` nebo `glab`, z prostředí přihlašovacího shellu, nebo
ho zadáte.

## AI
Každý agent pracuje v panelu AI se stejnou časovou osou: odpovědi, volání nástrojů s jejich výstupem, žádosti o
povolení, dotazy, plány ke schválení a komentování, úkoly, subagenti a úlohy na pozadí; dvě tlačítka v záhlaví panelu
vypíšou běžící subagenty a procesy na pozadí. Před každým promptem aplikace uloží kontrolní bod; změny lze zkontrolovat
a přetočit zpět po jednotlivých zprávách. Nová vlákna začínají v režimu „Ručně“ a destruktivní příkazy Gitu nejsou nikdy
povoleny natrvalo. Přehled Aktivita ukazuje vše, co běží. Zelená tečka na panelu značí pracující relaci, červená relaci,
která čeká na vás, a oznámení vám dá vědět, když relace skončí nebo čeká, zatímco se díváte jinam.
- **Claude** přes nezměněný příkaz `claude`, s relacemi, subagenty, úlohami na pozadí, kontextem a využitím.
- **Agenti ACP:** Codex, Antigravity a OpenCode a na požádání další z registru ACP; nainstalovaní agenti jsou pevně
  nastavení na otestované verze a ověření podle kontrolních součtů.
- **Vlastní agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio a servery s API kompatibilním s
  OpenAI nebo Anthropic, s nástroji pro soubory, hledání, příkazy, diagnostiku a webové požadavky; klíče zůstávají v
  Klíčence.
- Přepínač sandboxu pro každé vlákno omezuje, co smějí příkazy agenta dělat, pokud to agent podporuje.
- Cizí text, například výstup terminálu, webová stránka nebo log CI, jde k agentovi označený jako nedůvěryhodný.

## Diktování
Diktování do pole zprávy AI a do editoru přes rozpoznávání řeči aplikace Lowspeak: tlačítko mikrofonu, ⌃⌥D nebo klávesa
fn, dokud je Dango aktivní aplikací. Používá nastavení a modely aplikace Lowspeak; online přepis potřebuje klíč OpenAI
nebo Groq.

## Aktualizace
Aplikace se aktualizuje sama přes Sparkle. Při druhém spuštění se zeptá, zda smí aktualizace vyhledávat sama; dokud
nesouhlasíte, neodešle žádný požadavek. **Dango › Vyhledat aktualizace…** je vyhledá okamžitě.

## Zpřístupnění
Názvy pro VoiceOver u bočního panelu, panelů, karet časové osy AI, řádků Gitu a CI, ladicího nástroje a textu terminálu;
úplné ovládání klávesnicí se zkratkami, které můžete změnit v `keybindings.json`; Zvýšit kontrast a Omezit pohyb;
zobrazení kódu zůstávají zleva doprava i v rozhraních psaných zprava doleva. Podrobnosti jsou v ACCESSIBILITY.md.

## Soukromí
Žádná telemetrie, žádná analytika a žádná hlášení o pádech. Aplikace se připojuje jen kvůli tomu, co sami spustíte: k
agentům, které používáte, ke stahování, o které požádáte, k hostitelům CI, ke kterým se přihlásíte, k předpovědím úprav
od GitHub Copilot, pokud jste je zvolili, k online přepisu, pokud jste ho zvolili, ke stránkám, které otevřete, a ke
kontrole aktualizací, jakmile jste ji povolili. Klíče a tokeny zůstávají v Klíčence macOS.

## Známá omezení
- Přihlášení k CI přes prohlížeč je vypnuté; použijte `gh`, `glab`, token nebo prostředí.
- Aplikace běží bez App Sandbox, protože by ho zdědily terminály, Git, jazykové servery i agenti; než se spustí cokoli,
  co složka nastavuje, musíte složce důvěřovat. Antigravity a OpenCode nemají sandbox, který by aplikace mohla zapnout.
- Údaje o využití a rozpis kontextu u agenta Claude závisí na částech jeho protokolu označených jako experimentální a
  jsou skryté, když chybí. Přetočení konverzace zpět funguje jen u agenta Claude; u ostatních agentů se přetočí zpět jen
  kód.
- Gemini CLI se nenabízí: Google ukončil přihlašování osobním účtem a odkazuje na Antigravity. Dřívější vlákna Gemini
  zůstávají čitelná.
