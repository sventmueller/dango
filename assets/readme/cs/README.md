<p><img src="../../readme-icon.png" alt="Ikona aplikace Dango" width="147" height="147"></p>

# Dango

**Nativní editor pro macOS pro práci s programovacími agenty.**

Dango spojuje kód, agenty, kteří ho mění, a vše kolem nich do jednoho okna pro každou složku: editor s jazykovými servery a předpověďmi při psaní, terminály, prohlížeč, Git, CI, ladicí nástroj a AI vlákna s Claude, Codex, Antigravity, OpenCode nebo agentem vestavěným do aplikace. Každý agent pracuje ve stejné časové ose, ptá se, než změní soubory nebo spustí příkazy, a před každým promptem zanechá kontrolní bod, takže můžete zkontrolovat a přetočit zpět, co udělal, zprávu po zprávě. Nic, co složka nastavuje, se nespustí, dokud jí nedůvěřujete, a text zvenčí dorazí k agentovi označený jako nedůvěryhodný. Diktujte do editoru a do pole zprávy; rozpoznávání řeči může zůstat na vašem Macu. Dango je napsané v Rustu na skutečných ovládacích prvcích AppKitu, bez Electronu, mluví 35 jazyky a nemá telemetrii.

[![Stáhnout Dango.dmg](https://img.shields.io/badge/St%C3%A1hnout_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Stáhnout Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: podepsaný a notářsky ověřený společností Apple, pro macOS 14 nebo novější na Apple Silicon a Intel. Odkaz bude dostupný od prvního vydání.

**Našli jste chybu?** [Založte issue](https://github.com/sventmueller/dango/issues/new) a uveďte, s jakým agentem a v jakém typu projektu nastala.

[![Rust, edice 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 nebo novější](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Univerzální: Apple Silicon a Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit přes objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminál: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 jazyků rozhraní](https://img.shields.io/badge/languages-35-4C9A2A)
![Bez telemetrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Podepsáno s Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licence: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licence)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
**Čeština** ·
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

## Obsah

- [Funkce v kostce](#funkce-v-kostce)
- [Požadavky](#požadavky)
- [Instalace](#instalace)
- [První kroky](#první-kroky)
- [AI agenti](#ai-agenti)
  - [Časová osa](#časová-osa)
  - [Oprávnění a režimy](#oprávnění-a-režimy)
  - [Kontrolní body a worktree](#kontrolní-body-a-worktree)
- [Editor](#editor)
- [Jazykové servery](#jazykové-servery)
- [Testy](#testy)
- [Ladicí nástroj](#ladicí-nástroj)
- [Terminál](#terminál)
- [Prohlížeč](#prohlížeč)
- [Git](#git)
- [CI](#ci)
- [Kontejnery](#kontejnery)
- [Markdown](#markdown)
- [Diktování](#diktování)
- [Důvěra](#důvěra)
- [Sandbox](#sandbox)
- [Vzhled a zpřístupnění](#vzhled-a-zpřístupnění)
- [Jazyky](#jazyky)
- [Klávesové zkratky](#klávesové-zkratky)
- [Soukromí a zabezpečení](#soukromí-a-zabezpečení)
- [Kde jsou data](#kde-jsou-data)
- [Známá omezení](#známá-omezení)
- [Sestavení ze zdrojového kódu](#sestavení-ze-zdrojového-kódu)
- [Licence](#licence)

## Funkce v kostce

- **Všichni agenti v jedné časové ose.** Claude přes vlastní příkaz `claude`, Codex, Antigravity a OpenCode přes Agent Client Protocol, další z registru ACP a agent vestavěný do aplikace pro Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama a LM Studio. Odpovědi, volání nástrojů, žádosti o oprávnění, plány, úkoly, subagenti a úlohy na pozadí vypadají stejně, ať běží kterýkoli agent.
- **Kontrola a přetáčení zpět.** Před každým promptem si Dango uloží kontrolní bod. Co agent změnil, lze zkontrolovat a přetočit zpět po jednotlivých zprávách, a vlákno může pracovat ve vlastním worktree Gitu a předat práci sloučením nebo cherry-pickem.
- **Jedno okno na složku.** Soubory, terminály, webové stránky, AI vlákna, rozdíly a náhledy jsou panely macOS tohoto okna. Boční panel obsahuje Soubory, Hledání, Agenti, Git, Testy, Ladit, CI a Kontejnery.
- **Skutečný editor kódu.** Barvy syntaxe, více kurzorů, sbalování, `.editorconfig`, jazykové servery pro 17 jazyků s doplňováním, přechodem na definici, odkazy, přejmenováním, rychlými opravami a hierarchiemi, a předpovědi při psaní z GitHub Copilotu nebo lokálního modelu.
- **Vestavěný ladicí nástroj, Git a CI.** Zarážky a krokování pro Rust, C, C++, Swift, Go, JavaScript, TypeScript a Python; příprava jednotlivých bloků k commitu, větve, stashe, konflikty a graf commitů; běhy, joby a logy z GitHub Actions a GitLab CI.
- **Opatrnost s vašimi složkami.** Složka, které nedůvěřujete, nic nespustí: žádné procesy Gitu, jazykové servery, hooky ani agenty. Destruktivní příkazy Gitu se nikdy nepovolí natrvalo a commity se kontrolují na tajné údaje.
- **Diktování.** Mluvte do editoru a do pole zprávy, online přes OpenAI nebo Groq, nebo lokálně s Whisper a Parakeet.
- **Nativní až do morku kostí.** Okna, panely, nabídky a nástrojové panely AppKitu, TextKit 2 pro editor, Metal pro terminál, WebKit jen v panelu prohlížeče. Tmavý režim, Zvýšit kontrast a VoiceOver fungují jako v každé aplikaci pro Mac.
- **Ve vašem jazyce.** Rozhraní je dostupné v 35 jazycích: arabština, katalánština, čínština (zjednodušená), čínština (tradiční), chorvatština, čeština, dánština, nizozemština, angličtina, finština, francouzština, němčina, řečtina, hebrejština, hindština, maďarština, indonéština, italština, japonština, korejština, malajština, norština, polština, portugalština (Brazílie), portugalština (Portugalsko), rumunština, ruština, slovenština, slovinština, španělština, švédština, thajština, turečtina, ukrajinština a vietnamština.

## Požadavky

- macOS 14 Sonoma nebo novější, na Apple Silicon nebo Intel. Dango se vyvíjí a testuje na macOS 27.
- Pro AI vlákna agent, kterého chcete používat; viz [AI agenti](#ai-agenti).
- Pro jazykové servery a ladicí adaptéry, které Dango nainstaluje na požádání: Go pro gopls a `dlv`; Node.js pro servery TypeScript, Python, JSON, YAML, CSS, HTML a shell a pro ladicí nástroj pro JavaScript; Xcode Command Line Tools pro clangd, sourcekit-lsp a `lldb-dap`. rust-analyzer se stahuje z jeho vydání na GitHubu.
- Pro předpovědi při psaní předplatné GitHub Copilot (stačí bezplatné) nebo model fill-in-the-middle v Ollama či LM Studio.

## Instalace

**Z obrazu disku.** Každá verze je obraz disku, `Dango-<version>.dmg`, s jednou univerzální aplikací pro Apple Silicon a Intel, podepsanou identifikátorem Developer ID a notářsky ověřenou společností Apple. Otevřete ho a přetáhněte Dango do složky Aplikace. Homebrew Cask instaluje stejný obraz. Zatím nebyla vydána žádná verze.

**Aktualizace.** Dango z obrazu disku se samo udržuje aktuální pomocí [Sparkle](https://sparkle-project.org). Při druhém spuštění se zeptá, zda smí samo vyhledávat aktualizace; dokud nesouhlasíte, neodešle žádný požadavek. **Dango › Vyhledat aktualizace…** vyhledává ihned. Aktualizace jsou podepsány vlastním klíčem aplikace, který Sparkle ověří, než cokoli nainstaluje. Vlastnoručně sestavená verze aktualizace nemá.

**Ze zdrojového kódu.** Viz [Sestavení ze zdrojového kódu](#sestavení-ze-zdrojového-kódu) a pak přesuňte `target/release/Dango.app` do `/Applications`.

**Příkaz pro terminál.** **Dango › Instalovat příkaz „dango“…** zapíše malý skript do `~/.local/bin/dango`, bez oprávnění správce; `~/.local/bin` musí být ve vašem `PATH`. Pak `dango .` otevře aktuální složku a `dango file.rs` soubor. Dango tam nikdy nenahradí soubor, který samo nezapsalo.

## První kroky

1. **Otevřete složku** pomocí ⌘O nebo `dango <folder>`. Poprvé se Dango zeptá, zda jí důvěřujete (viz [Důvěra](#důvěra)).
2. **Boční panel** přepíná mezi Soubory (⌘1), Hledáním (⌘2), Agenty (⌘3), Gitem (⌘4), Testy (⌘5), Ladit (⌘6), CI (⌘7) a Kontejnery (⌘8); kliknutím pravým tlačítkem skryjete ty, které nepoužíváte, a ⌃⌘S boční panel sbalí.
3. **Každý dokument je panel** okna složky, vlastní panely macOS. „+“ na konci řádku panelů a ⌘T nabízejí nový soubor, terminál, prohlížeč a každého nainstalovaného agenta, i ve vlastním worktree Gitu; když „+“ podržíte, zobrazí se totéž jako nabídka.
4. **Rychle otevřít** pomocí ⌘P najde soubory a s `#` symboly projektu; ⇧⌘F hledá a nahrazuje v projektu.
5. **Mluvte s agentem** v AI panelu. ⌘Esc přesune fokus klávesnice do jeho pole zprávy a zpět.
6. **Nastavení** pomocí ⌘, obsahuje panely Obecné, CI, Agenti, Vlastní agent, Jazykový server, Diktování a Předpověď.

Nástrojový panel obsahuje zpět a vpřed přes místa, která jste upravovali (⌃- a ⌃⇧-), větev, Rychle otevřít a přehled Aktivita, který uvádí vše, co běží. **Otevřít vedle** v místní nabídce souboru zobrazí druhý dokument vedle prvního. Po restartu nebo pádu se každé okno vrátí se svými panely a editory s neuloženým textem se vrátí s tímto textem. Obrázky, zvuk, video, písma a dokumenty Office se otevírají v panelu přes Rychlý náhled, PDF přes PDFKit.

## AI agenti

| Agent | Jak běží | Co potřebuje |
|---|---|---|
| Claude | příkaz `claude`, se kterým Dango komunikuje přímo přes jeho protokol stream-json | nainstalovaný a přihlášený `claude`; Dango hledá v `PATH` přihlašovacího shellu, v `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` a `/usr/local/bin`. Přihlášení probíhá jen v samotném `claude` |
| Codex | `codex-acp` přes Agent Client Protocol (ACP) | Node.js a npm; Dango na požádání nainstaluje balíček s pevnou verzí |
| Antigravity | `agy acp` | vlastní `agy` v `PATH`, nebo stažení s pevnou verzí, nainstalované na požádání |
| OpenCode | `opencode acp` | vlastní `opencode` v `PATH`, nebo stažení s pevnou verzí, nainstalované na požádání |
| Další agenti ACP | z registru ACP, který se stahuje jen na požádání | co uvádí položka registru; položky bez kontrolního součtu se instalují až po vašem potvrzení |
| Vlastní agent | vestavěný do aplikace, s nástroji pro soubory, hledání, příkazy, diagnostiku a webové požadavky | klíč API pro Anthropic, OpenAI, Google Gemini nebo OpenRouter, uložený v klíčence; Ollama nebo LM Studio na tomto Macu bez klíče; libovolný server, který umí API OpenAI nebo Anthropic |

**Nastavení › Agenti** ukazuje, které agenty tento Mac má, instaluje ostatní a jejich aktualizace a otevírá registr ACP. Agenti instalovaní Dangem mají pevné, otestované verze a ověřují se podle zveřejněných kontrolních součtů (npm integrity nebo SHA-256); Dango nespouští balíčky `npx` bez verze.

### Časová osa

Každý agent pracuje v AI panelu se stejnou časovou osou: odpovědi, volání nástrojů s jejich výstupem, žádosti o oprávnění, otázky, plány ke schválení a okomentování (⌥⌘K), úkoly, subagenti a úlohy na pozadí. Dvě tlačítka v záhlaví panelu uvádějí běžící subagenty a procesy na pozadí. Zmiňte soubor, `@terminal` nebo `@browser`, abyste agentovi předali jejich obsah. Zelená tečka na panelu značí pracující relaci, červená relaci, která na vás čeká, a oznámení dá vědět, když relace skončí nebo čeká, zatímco se díváte jinam. U Claude ukazuje stavový řádek kontext a využití.

### Oprávnění a režimy

Nová vlákna začínají v režimu „Ručně“: agent se ptá, než změní soubory nebo spustí příkazy. Pro destruktivní příkazy Gitu (`push --force`, `reset --hard`, `clean -fd`) Dango nenabízí „Vždy povolit“. Vlastní agent si ukládá pravidla „Vždy povolit“ pro každý projekt, uvádí je v **Nastavení › Vlastní agent** a u destruktivních příkazů Gitu se ptá pokaždé, ať je uloženo cokoli; Claude a agenti ACP si taková pravidla ukládají ve vlastní konfiguraci. Při „Povolit vše“ a bez sandboxu zobrazuje vlákno trvalé varování.

### Kontrolní body a worktree

Před každým promptem si Dango uloží kontrolní bod jako commit pod `refs/ide-checkpoints/` v repozitáři projektu, nebo ve vlastní složce u projektu bez Gitu; běžný push nebo fetch je nikdy nepřenese. Co agent změnil, lze zkontrolovat a přetočit zpět po jednotlivých zprávách. Vlákno může pracovat ve vlastním worktree Gitu: podokno Git ukazuje, co změnilo, a převezme práci sloučením nebo cherry-pickem.

## Editor

- **Barvy syntaxe, čísla řádků a značky změn** oproti poslednímu commitu, hledání a nahrazování, Rychle otevřít (⌘P) a hledání v projektu (⇧⌘F).
- **Odsazení** se řídí souborem a `.editorconfig`: tabulátory nebo mezery, automatické odsazení, párové závorky.
- **Více kurzorů:** ⌥-kliknutí, ⌥-tažení pro sloupec, ⌃⇧↑ a ⌃⇧↓, ⌘D pro další výskyt.
- **Sbalování** (⌥⌘← a ⌥⌘→, s ⇧ pro vše), přepnutí komentáře (⌘/), přechod na řádek (⌃G), viditelné bílé znaky a zalamování řádků (⌥Z).
- **Soubory si zachovávají kódování**, značku pořadí bajtů a konce řádků. Soubor změněný na disku se znovu načte, pokud editor nemá neuložený text; jinak lišta nabídne rozdíly a sloučení.
- **Předpovědi při psaní.** Během psaní se šedě zobrazí návrh a Tab ho přijme. Pochází z GitHub Copilotu nebo z lokálního modelu v Ollama či LM Studio, podle volby v **Nastavení › Předpověď**; lokální model nechá váš kód na tomto Macu.
- **Minimapa a pevné posouvání** (nabídka Editor): úzký obraz celého souboru vedle textu a řádky záhlaví bloků, ve kterých leží horní řádek, přidržené nad ním.
- **Diagnostika na konci řádku** (nabídka Zobrazení): nejzávažnější hlášení každého řádku po sekundě klidu, se symbolem jeho závažnosti.
- **Živý náhled statických stránek:** Editor › Otevřít v panelu prohlížeče poskytuje stránky důvěryhodné složky přímo z Danga a po uložení souboru panel znovu načte.
- **Prettier** formátuje JavaScript, TypeScript, CSS, HTML, Markdown a další jazyky, kde ho projekt nastavuje, a to vlastním `node_modules/.bin/prettier` projektu; nastavení `formatter` vybírá formátovač pro každý jazyk.
- **Režim Vim** (Editor › Režim Vim): režimy Normal, Insert, Visual a Replace, počty, operátory s pohyby a textovými objekty, `.`, registry, značky a makra. `u` a ⌃R používají kroky zpět editoru a jeden průchod vkládáním je jeden krok. `"+` a `"*` jsou schránka; nastavení `vimClipboard` udělá schránkou i nepojmenovaný registr. `/`, `?`, `n`, `N`, `*`, `:s` a `:%s` hledají regulárními výrazy Rustu, ne vzory Vimu: žádné `\v` a `\b` místo `\<` a `\>`. Příkazy Ex jsou `:w`, `:q`, `:wq`, `:x`, `:noh` a `:<line>`. V režimu Normal si Vim bere ⌃G od přechodu na řádek.
- **Synchronizace nastavení** přes složku na iCloud Drive (**Nastavení › Obecné**): vaše nastavení a zkratky vás následují na vaše ostatní Macy; cesty, místní adresy a souhlasy zůstávají na každém zvlášť.

## Jazykové servery

Go, Rust, TypeScript a JavaScript, Python, C, C++ a Objective-C, Swift, skripty shellu, JSON, YAML, CSS, HTML, TOML, Markdown a Lua, plus vlastní servery z **Nastavení › Jazykový server**. Dango nainstaluje server na požádání do vlastní složky nástrojů a použije `tsc` projektu TypeScript, pokud může sloužit jako jazykový server.

Co přinášejí: diagnostiku (⌘' a ⇧⌘' přecházejí mezi hlášeními), informace po najetí (⌃⌘I), doplňování (⌥Esc), nápovědu k signaturám, přechod na definici (⌃⌘J), definici typu a implementaci, odkazy (⌃⇧⌘F), hierarchie volání a typů (⌃⇧⌘H), přejmenování (⌃⌘E), formátování (⌥⇧F), sémantické barvy, sbalování a osnovu (⇧⌘O); typy a názvy parametrů přímo v řádku; rychlé opravy a refaktoring za žárovkou (⌥⌘A); symboly projektu pomocí ⌥⌘O.

Vedle vlastního serveru souboru kontroluje ESLint JavaScript a TypeScript projektů, které ho používají, i se svými rychlými opravami, a zkratky Emmet se objevují v doplňování pro HTML, CSS a JSX. Dango oba nainstaluje na požádání; **Nastavení › Obecné** opraví problémy ESLint při ukládání, pokud chcete.

## Testy

Podokno Testy vypisuje testy složky podle balíčku a souboru s jejich stavem: `cargo test` nebo cargo-nextest, Go, pytest, Vitest a Jest. Symbol u každého testu na okraji vedle čísel řádků test spustí, ⌥-kliknutí ho ladí a neúspěšný assert označí svůj řádek. Nabídka Ladit spustí test u kurzoru (⌃⌥⌘U), znovu poslední testy (⌃⌥⌘G) nebo všechny a ladí test u kurzoru. Testy běží jen v důvěryhodných složkách, protože spouštějí kód projektu; `cargo test` vypíše své testy, až když o to požádáte, protože výpis sestaví každý testovací program.

## Ladicí nástroj

Zarážky, i s podmínkami a jako logpointy, krokování, vlákna s jejich zásobníky volání, proměnné a sledované výrazy v inspektoru (⌥⌘I), hodnoty vedle kódu, dokud je program pozastavený, a ladicí konzole, která vyhodnocuje v pozastaveném rámci.

| Jazyk | Ladicí adaptér |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` z Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | ladicí nástroj pro JavaScript, na Node.js |
| Python | debugpy |

Soubor `.vscode/launch.json` projektu se načte. Nabídka Ladit používá zkratky z Xcode: ⌘\\ nastaví zarážku, ⌘Y zapne nebo vypne všechny, ⌃⌘Y pokračuje, F6, F7 a F8 přeskočí, vstoupí a vystoupí, ⌘. zastaví.

## Terminál

Terminály v panelech (⌃\`) s prostředím přihlašovacího shellu, vykreslované přes Metal. ⌘K vymaže obrazovku. Vložený nebo přetažený obrázek se předá programovacímu agentovi běžícímu v terminálu, nebo se vloží cesta k uložené kopii. ⌘-kliknutí otevírá odkazy a názvy souborů. Vývojový server spuštěný z terminálu lze otevřít v panelu prohlížeče.

## Prohlížeč

Panel prohlížeče s polem adresy (⌘L), zpět a vpřed (⌘[ a ⌘]), znovunačtením (⌘R), hledáním, stahováním s předchozím dotazem a webovým inspektorem. `@browser` předá stránku agentovi, označenou jako text zvenčí.

## Git

Podokno Git připravuje k commitu a zahazuje soubory nebo jednotlivé bloky, vytváří commity, commituje a pushuje nebo synchronizuje, přepíná a vytváří větve, uchovává stashe, řeší konflikty sloučení a kreslí graf posledních commitů. Blame ukazuje, kdo naposledy změnil řádek pod kurzorem, nebo sloupec pro celý soubor. Tlačítko napíše zprávu commitu pomocí AI; rozdíl odejde až poté, co v něm gitleaks nenašel žádný tajný údaj, a každý commit se na tajné údaje kontroluje také.

## CI

GitHub Actions a GitLab CI, také GitHub Enterprise Server a vlastní instalace GitLabu:

- běhy, joby a logy aktuální větve;
- opětovné spuštění neúspěšných jobů, jednoho jobu nebo na GitHubu celého běhu; spouštění ručních jobů nebo schvalování čekajících nasazení; spuštění nové pipeline nebo workflow, který lze spustit ručně; zrušení;
- otevřené pull requesty nebo merge requesty a issues, které se otevírají v panelu prohlížeče;
- log neúspěšného jobu předaný agentovi, označený jako text zvenčí.

Token pochází z klíčenky, z `gh` nebo `glab`, z prostředí přihlašovacího shellu, nebo je to ten, který zadáte v **Nastavení › CI**.

## Kontejnery

Podokno Kontejnery zobrazuje projekty Compose v Dockeru s jejich službami, samostatné kontejnery, obrazy a svazky a sleduje události Dockeru, jak nastávají. Soubor Compose ve složce se zobrazí jako projekt ještě dřív, než existuje kterýkoli z jeho kontejnerů. Kontejnery lze spustit, zastavit a restartovat; jejich logy a shell se otevírají v panelech terminálu a publikovaný port se otevře v panelu prohlížeče. Smazání kontejneru, obrazu nebo svazku a vyčištění nepoužívaných dat se nejdřív zeptá. Příkazy up, down a pull z Compose běží jen v důvěryhodných složkách, protože Compose sestavuje a spouští, co projekt popisuje. Nabídka podokna vybírá kontext Dockeru pro okno, aniž by měnila ten, který používají vaše shelly; stejné příkazy jsou v nabídce Kontejnery. Potřebuje příkazový řádek `docker`, Docker 24.0.6 nebo novější.

## Markdown

- **Náhled.** Soubor Markdown se otevře s náhledem vedle; **Zobrazení › Zobrazit/skrýt náhled vedle** (⇧⌘V) ho zavře a otevře a **Nastavení › Obecné** automatický náhled vypne. Náhled vykresluje vzorce, diagramy Mermaid, Graphviz a PlantUML a obrázky ze složky; obrázky z webu se načtou jen v důvěryhodné složce nebo když si o ně v náhledu řeknete.
- **Nabídka Formát:** tučné (⌘B), kurzíva (⌘I), přeškrtnuté (⇧⌘X), kód (⌥⇧⌘C), odkaz (⇧⌘K), nadpisy 1–3 (⌥⌘1–⌥⌘3), seznamy s odrážkami a číslované, úkoly, citace a bloky kódu. Každý příkaz přepíná, na výběru nebo řádcích, a je jedním krokem zpět.
- **Vkládání.** Text z Wordu nebo webové stránky dorazí jako Markdown, jeho obrázky jako soubory vedle dokumentu; ⌥⇧⌘V vloží prostý text.
- **Export.** **Soubor › Exportovat** zapíše PDF, jediný soubor HTML s obrázky uvnitř nebo dokument Word. Obrázky z webu se přidají jen z důvěryhodné složky.
- **Otevřít v aplikaci Splat.** Pokud je nainstalovaný [Splat](https://github.com/sventmueller/splat), **Soubor › Otevřít v aplikaci Splat** a místní nabídky seznamu souborů a panelů tam dokument otevřou, poté co ho uloží.

## Diktování

Diktujte do pole zprávy AI panelu nebo do editoru: tlačítkem mikrofonu, **Úpravy › Spustit diktování** (⌃⌥D) nebo klávesou fn (podržením nahráváte, dvojím stisknutím nahráváte dál). Rozpoznávání řeči je to z [Lowspeak](https://github.com/sventmueller/lowspeak); samotný Lowspeak nemusí být nainstalovaný.

- **Nastavení › Diktování** určuje režim a model (online s OpenAI nebo Groq, lokálně s Whisper nebo Parakeet), jazyk, následné zpracování, mikrofon a vaši vlastní slovní zásobu.
- **Lokální modely** se stahují z Hugging Face, když si některý vyberete, s průběhem a možností zrušení. Ukládají se do složek, které používá Lowspeak, takže každá aplikace použije, co stáhla druhá. Lokální model nechá nahrávku na tomto Macu.
- **Online přepis** potřebuje klíč pro OpenAI nebo Groq, který Dango uloží do vlastní položky klíčenky.
- **Z Lowspeaku.** Je-li Lowspeak nastavený jinak než výchozí, panel Diktování jednou nabídne převzetí jeho nastavení a na požádání i klíčů; macOS se zeptá, než Dango některý přečte (zvolte „Povolit“, ne „Vždy povolit“).
- **Oprávnění.** macOS se na mikrofon zeptá při prvním diktování. Klávesa fn funguje, dokud je Dango aktivní aplikací a Lowspeak neběží; potřebuje oprávnění zpřístupnění, o které Dango žádá až po prvním diktování, nikdy při spuštění.

## Důvěra

Složka může spouštět programy, jakmile se v ní něco spustí: hooky Gitu a nastavení repozitáře jako `core.fsmonitor`, jazykové servery z `node_modules`, hooky agentů, `.mcp.json`. Když složku otevřete poprvé, Dango se zeptá, zda jí důvěřujete.

- **Důvěryhodná:** Git, jazykové servery, ladicí nástroje, agenti a CI běží pro tuto složku a vše pod ní.
- **Omezená:** můžete číst a upravovat; značky změn v editoru pocházejí z přímého čtení `.git`. Nic, co složka nastavuje, se nespustí. Podokna Git, CI a AI to uvádějí a nabízejí složce důvěřovat.

Text, který nepochází od vás, může nést pokyny, které mají řídit agenta (prompt injection). Výstup terminálu (`@terminal`), webová stránka (`@browser`), log CI předaný agentovi, soubor mimo projekt a delší text vložený z webové stránky proto odcházejí označené: štítek říká „Cizí obsah“ a agent dostane text s upozorněním, že jde o nedůvěryhodná data, jejichž pokyny nesmí plnit. Webové stránky vlastního agenta přicházejí se stejným upozorněním. Vložený text se považuje za text z webové stránky, když to prohlížeč uvede ve schránce, jak to dělají Safari, Chrome a Firefox; text odjinud a krátká fráze o několika slovech zůstávají vaše.

## Sandbox

Samotné Dango běží bez App Sandboxu, protože by ho zdědil každý program, který spustí: terminály, Git, jazykové servery a agenti by nemohli fungovat. Místo toho má každé AI vlákno přepínač sandboxu, ve výchozím stavu vypnutý. Co omezuje, závisí na agentovi, a popisek přepínače to říká:

| Agent | Se zapnutým sandboxem |
|---|---|
| Claude | příkazy shellu běží ve vlastním sandboxu Claude; jeho nástroje pro soubory a web ne |
| Codex | příkazy a změny zapisují jen do projektu, bez sítě |
| Vlastní agent | soubory, webové požadavky a příkazy zůstávají v projektu a povolených složkách, bez sítě (`sandbox-exec`) |
| Antigravity, OpenCode | žádný sandbox, který by Dango mohlo zapnout |

## Vzhled a zpřístupnění

- **Tmavý režim** se řídí systémem; **Zvýšit kontrast** a **Omezit pohyb** jsou respektovány.
- **VoiceOver** pojmenovává boční panel, panely, karty časové osy AI, řádky v Gitu a CI, ladicí nástroj a text terminálu. Zarážky se nikdy nerozlišují jen barvou.
- **Klávesnice.** Každý příkaz má zkratku a všechny lze změnit v `keybindings.json`; ⌥⌘\` přejde na další podokno.
- **Zprava doleva.** V arabštině a hebrejštině se okno zrcadlí, zatímco zobrazení kódu zůstávají zleva doprava.

Co Dango přidává k AppKitu, oblast po oblasti, popisuje [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Jazyky

Rozhraní je přeloženo do 35 jazyků: arabština, katalánština, čínština (zjednodušená), čínština (tradiční), chorvatština, čeština, dánština, nizozemština, angličtina, finština, francouzština, němčina, řečtina, hebrejština, hindština, maďarština, indonéština, italština, japonština, korejština, malajština, norština, polština, portugalština (Brazílie), portugalština (Portugalsko), rumunština, ruština, slovenština, slovinština, španělština, švédština, thajština, turečtina, ukrajinština a vietnamština.

Dango se řídí pořadím jazyků v **Nastavení systému › Obecné › Jazyk a oblast**; tam také můžete vybrat jazyk jen pro Dango v části **Aplikace**. Pokud pro váš jazyk překlad není, použije angličtinu. Pojmy se řídí jedním glosářem, [docs/glossary.md](../../../docs/glossary.md), a překlady Applu, takže se Dango čte jako zbytek vašeho Macu.

## Klávesové zkratky

| Akce | Zkratka |
|---|---|
| Otevřít složku, nové okno, zavřít okno | ⌘O, ⇧⌘N, ⇧⌘W |
| Nový soubor, uložit, uložit jako | ⌘N, ⌘S, ⇧⌘S |
| Nový panel, zavřít panel, další a předchozí panel | ⌘T, ⌘W, ⇧⌘] a ⇧⌘[ |
| Rychle otevřít, hledat v projektu | ⌘P, ⇧⌘F |
| Soubory, Hledání, Agenti, Git, Testy, Ladit, CI, Kontejnery | ⌘1 až ⌘8 |
| Zobrazit nebo skrýt boční panel, další podokno | ⌃⌘S, ⌥⌘\` |
| Zpět a vpřed přes upravená místa | ⌃- a ⌃⇧- |
| Přepnout komentář, přejít na řádek | ⌘/, ⌃G |
| Další výskyt, kurzor nad a pod | ⌘D, ⌃⇧↑ a ⌃⇧↓ |
| Sbalit, rozbalit, sbalit vše, rozbalit vše | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Zalamování řádků, inspektor | ⌥Z, ⌥⌘I |
| Doplňování, informace, přejít na definici | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Odkazy, hierarchie volání, přejmenovat | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboly v souboru, v projektu | ⇧⌘O, ⌥⌘O |
| Rychlá oprava, formátovat | ⌥⌘A, ⌥⇧F |
| Další a předchozí problém | ⌘' a ⇧⌘' |
| Náhled Markdownu, vložit jako prostý text | ⇧⌘V, ⌥⇧⌘V |
| Tučné, kurzíva, přeškrtnuté, kód, odkaz | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Nadpis 1 až 3 | ⌥⌘1 až ⌥⌘3 |
| Nový terminál, vymazat terminál | ⌃\`, ⌘K |
| Fokus do pole zprávy agenta, okomentovat plán | ⌘Esc, ⌥⌘K |
| Pole adresy, znovu načíst, zpět, vpřed (prohlížeč) | ⌘L, ⌘R, ⌘[, ⌘] |
| Spustit nebo zastavit diktování | ⌃⌥D |
| Zarážka, zapnout nebo vypnout všechny | ⌘\\, ⌘Y |
| Spustit test u kurzoru, znovu spustit poslední testy | ⌃⌥⌘U, ⌃⌥⌘G |
| Pokračovat, přeskočit, vstoupit, vystoupit, zastavit | ⌃⌘Y, F6, F7, F8, ⌘. |
| Nastavení | ⌘, |

`keybindings.json` ve složce dat aplikace (viz [Kde jsou data](#kde-jsou-data)) mění kteroukoli z nich.

## Soukromí a zabezpečení

- **Bez telemetrie,** bez analytiky, bez hlášení o pádech odesílaných kamkoli. Chyby jdou do systémového protokolu (Konzole, pod názvem aplikace).
- **Síť jen pro to, co spustíte:** agenti, které používáte, každý se svou službou; registr ACP a stahování agentů, jazykových serverů, ladicích adaptérů a modelů diktování, když o ně požádáte; hostitelé CI, ke kterým se přihlásíte; předpovědi z Copilotu, pokud jste ho zvolili; poskytovatelé vlastního agenta; online přepis, pokud jste ho zvolili; stránky, které otevřete v panelu prohlížeče; a vyhledávání aktualizací, jakmile ho povolíte.
- **Kód opouští Mac** jen tam, kde jste k tomu zvolili službu: agenta, předpovědi z Copilotu, zprávu commitu napsanou pomocí AI. Rozdíl pro zprávu commitu odejde až poté, co v něm gitleaks nenašel žádný tajný údaj.
- **Klíče a tokeny** zůstávají v klíčence macOS, nikdy v souboru nastavení. Klíče jiných aplikací, například Lowspeaku, se čtou jen s vaším souhlasem a jen jednou.
- **Stahování s pevnými verzemi.** Agenti, jazykové servery a ladicí adaptéry, které Dango instaluje, mají pevné, otestované verze a ověřují se podle kontrolních součtů.
- **Podpis kódu** s Hardened Runtime; aktualizace se instalují jen s vlastním podpisem aplikace.

## Kde jsou data

`<id>` je identifikátor balíčku, `local.agentide` pro vydanou verzi (`local.agentide.debug` pro ladicí sestavení). Neobsahuje zobrazovaný název, takže přejmenování zachová všechna data.

| Místo | Co |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (seznam podokna AI), `agent-threads/` (konverzace vlastního agenta), `layouts/` (panely pro každý projekt), `recovery/` (neuložené editory po pádu), `checkpoints/` (pro složky bez Gitu), `worktrees/`, `agents/` a `tools/` (nainstalovaní agenti, jazykové servery a ladicí adaptéry), `task-output/` |
| `~/Library/Caches/<id>/` | vložené obrázky, skripty integrace se shellem, registr ACP; lze smazat |
| `~/Library/Logs/<id>/dictation/` | soubory protokolu diktování |
| `~/Library/Application Support/Lowspeak` a `FluidAudio` | lokální modely diktování, sdílené s Lowspeakem |
| `~/Library/WebKit/<id>/` | cookies a data webů z panelu prohlížeče |
| `<project>/.dango/settings.json` | vlastní nastavení projektu |
| repozitář Gitu projektu | kontrolní body jako commity pod `refs/ide-checkpoints/`; běžný push nebo fetch je nikdy nepřenese |
| klíčenka, služba `<id>` | klíče API vlastního agenta a diktování, tokeny CI |
| `~/.local/bin/dango` | příkaz pro terminál, pokud jste ho nainstalovali |

Claude si ukládá vlastní relace do `~/.claude/projects`; Dango je čte, aby mohlo relaci obnovit, ale nemění je.

## Známá omezení

- **Bez App Sandboxu** pro samotnou aplikaci, protože by ho zdědily terminály, Git, jazykové servery a agenti; složka musí být důvěryhodná, než se spustí cokoli, co nastavuje. Antigravity a OpenCode nemají sandbox, který by Dango mohlo zapnout.
- **Údaje o využití a rozpis kontextu Claude** vycházejí z částí jeho protokolu označených jako experimentální a jsou skryté, když chybějí.
- **Přetočení konverzace zpět** funguje jen u Claude; u ostatních agentů se přetočí zpět jen kód.
- **Přihlášení k CI přes prohlížeč** je vypnuté; použijte `gh`, `glab`, token nebo prostředí.
- **Gemini CLI se nenabízí:** Google ukončil přihlašování osobním účtem a odkazuje na Antigravity. Dřívější vlákna Gemini zůstávají čitelná.

## Sestavení ze zdrojového kódu

Potřebujete:

- macOS 14 nebo novější s Xcode Command Line Tools (`xcode-select --install`); samotný Xcode není potřeba.
- Rust ve verzi uvedené v `rust-toolchain.toml`. `rust` z Homebrew sestavuje pro procesor tohoto Macu; univerzální sestavení verze potřebuje rustup s cíli `aarch64-apple-darwin` a `x86_64-apple-darwin`.
- Kopie [splat](https://github.com/sventmueller/splat) a [lowspeak](https://github.com/sventmueller/lowspeak) vedle této (`../splat`, `../lowspeak`), obě na větvi `main`; workspace z nich bere crates podle cesty.
- cmake pro každé sestavení s diktováním (`make run`, `make build`, `make release`), které sestavuje whisper.cpp. `make check` sestavuje bez něj.

| Příkaz | Co dělá |
|---|---|
| `make run` | sestaví ladicí aplikaci (`target/debug/Dango Debug.app`) a otevře ji |
| `make build` | sestaví vydávanou aplikaci (`target/release/Dango.app`), podepsanou lokálním certifikátem, který první sestavení vytvoří v přihlašovací klíčence; ponechá `VERSION` beze změny |
| `make lint` | formátování a clippy; varování vedou k selhání |
| `make check` | `lint`, pak všechny testy; nepotřebuje klíčenku, okno ani síť |
| `make test` | `check`, pak autotest `probe` a audit zpřístupnění, zleva doprava a zprava doleva, ve skutečné aplikaci |
| `make perf` | časové rozpočty ve vydávaném sestavení; vypovídající jen na jinak nečinném Macu |
| `make release` | zkouška vydání do `target/release-dry-run/`, podepsaná lokálním certifikátem; nic nikam neodesílá |
| `make release-notarized` | `check`, pak samotné vydání do `dist/`: podpis Developer ID, notářské ověření, kanál aktualizací podepsaný vlastním klíčem aplikace; ponechá `VERSION` beze změny |
| `make deploy` | na čisté větvi `main`, s pečetí CI i bez ní: notářsky ověří další verzi, commituje `VERSION`, nahraje soubory vydání, odešle kód a značku verze, a pokud existuje, i zelenou pečeť CI, a pak vydání sám zveřejní |

Ladicí a vydávaná verze jsou oddělené aplikace s vlastním nastavením a daty. Sestavení, testy, autotesty, vydání a pravidla, kterými se kód řídí, popisuje [docs/development.md](../../../docs/development.md), rozhodnutí za nimi [docs/plan.md](../../../docs/plan.md). Poznámky k vydání jsou [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), přeložené vedle každého README v `assets/readme/<language>/`.

## Licence

Dango je licencováno podle jedné z těchto licencí:

- Licence Apache, verze 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licence MIT ([LICENSE-MIT](../../../LICENSE-MIT))

dle vaší volby.

Pokud výslovně neuvedete jinak, každý příspěvek, který jste záměrně předložili k zahrnutí do Danga, jak je definováno v licenci Apache-2.0, bude licencován duálně jako výše, bez dalších podmínek.
