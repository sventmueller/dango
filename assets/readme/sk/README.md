<p><img src="../../readme-icon.png" alt="Ikona aplikácie Dango" width="168" height="168"></p>

# Dango

**Natívny editor pre macOS na prácu s kódovacími agentmi.**

Dango spája kód, agentov, ktorí ho menia, a všetko okolo nich do jedného okna pre každý priečinok: editor s jazykovými servermi a predpoveďami pri písaní, terminály, prehliadač, Git, CI, ladiaci nástroj a AI vlákna s Claude, Codex, Antigravity, OpenCode alebo s agentom zabudovaným v aplikácii. Každý agent pracuje v tej istej časovej osi, pýta sa, skôr než zmení súbory alebo spustí príkazy, a pred každým promptom zanechá kontrolný bod, takže to, čo urobil, môžete skontrolovať a pretočiť späť správu po správe. Nič, čo priečinok konfiguruje, sa nespustí, kým mu nedôverujete, a text zvonku sa k agentovi dostane označený ako nedôveryhodný. Diktujte do editora a do poľa správy; rozpoznávanie reči môže zostať na vašom Macu. Dango je napísané v Ruste na skutočných ovládacích prvkoch AppKitu, bez Electronu, hovorí 35 jazykmi a nemá žiadnu telemetriu.

[![Stiahnuť Dango.dmg](https://img.shields.io/badge/Stiahnu%C5%A5_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Stiahnuť Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: podpísaný a notarizovaný spoločnosťou Apple, pre macOS 14 alebo novší na Apple Silicon a Intel. Odkaz bude dostupný od prvého vydania.

**Našli ste chybu?** [Otvorte issue](https://github.com/sventmueller/dango/issues/new) a uveďte, s ktorým agentom a pri akom type projektu sa vyskytla.

[![Rust, edícia 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 alebo novší](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Univerzálna: Apple Silicon a Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit cez objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminál: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 jazykov rozhrania](https://img.shields.io/badge/languages-35-4C9A2A)
![Bez telemetrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Podpísané s Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licencia: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licencia)

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
**Slovenčina** ·
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

- [Funkcie v skratke](#funkcie-v-skratke)
- [Požiadavky](#požiadavky)
- [Inštalácia](#inštalácia)
- [Začíname](#začíname)
- [AI agenti](#ai-agenti)
  - [Časová os](#časová-os)
  - [Povolenia a režimy](#povolenia-a-režimy)
  - [Kontrolné body a worktree](#kontrolné-body-a-worktree)
- [Editor](#editor)
- [Jazykové servery](#jazykové-servery)
- [Testy](#testy)
- [Ladiaci nástroj](#ladiaci-nástroj)
- [Terminál](#terminál)
- [Prehliadač](#prehliadač)
- [Git](#git)
- [CI](#ci)
- [Kontajnery](#kontajnery)
- [Markdown](#markdown)
- [Diktovanie](#diktovanie)
- [Dôvera](#dôvera)
- [Sandbox](#sandbox)
- [Vzhľad a prístupnosť](#vzhľad-a-prístupnosť)
- [Jazyky](#jazyky)
- [Klávesové skratky](#klávesové-skratky)
- [Súkromie a bezpečnosť](#súkromie-a-bezpečnosť)
- [Kde sú uložené údaje](#kde-sú-uložené-údaje)
- [Známe obmedzenia](#známe-obmedzenia)
- [Zostavenie zo zdrojového kódu](#zostavenie-zo-zdrojového-kódu)
- [Licencia](#licencia)

## Funkcie v skratke

- **Všetci agenti v jednej časovej osi.** Claude cez vlastný príkaz `claude`, Codex, Antigravity a OpenCode cez Agent Client Protocol, ďalší z registra ACP a agent zabudovaný v aplikácii pre Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama a LM Studio. Odpovede, volania nástrojov, žiadosti o povolenie, plány, zoznamy úloh, subagenti a úlohy na pozadí vyzerajú rovnako, nech beží ktorýkoľvek agent.
- **Kontrola a pretočenie späť.** Pred každým promptom si Dango uloží kontrolný bod. To, čo agent zmenil, možno skontrolovať a pretočiť späť po jednotlivých správach, a vlákno môže pracovať vo vlastnom git worktree a odovzdať svoju prácu zlúčením alebo cherry-pickom.
- **Jedno okno pre každý priečinok.** Súbory, terminály, stránky prehliadača, AI vlákna, diffy a náhľady sú taby macOS v tomto okne. Postranný panel obsahuje Súbory, Vyhľadávanie, Agenti, Git, Testy, Ladenie, CI a Kontajnery.
- **Skutočný editor kódu.** Zvýrazňovanie syntaxe, viac kurzorov, zbaľovanie, `.editorconfig`, jazykové servery pre 17 jazykov s dopĺňaním, prechodom na definíciu, referenciami, premenovaním, rýchlymi opravami a hierarchiami a predpovede pri písaní z GitHub Copilot alebo z lokálneho modelu.
- **Ladiaci nástroj, Git a CI priamo v aplikácii.** Body prerušenia a krokovanie pre Rust, C, C++, Swift, Go, JavaScript, TypeScript a Python; príprava jednotlivých úsekov na commit, vetvy, stashe, konflikty a graf commitov; spustenia, joby a logy z GitHub Actions a GitLab CI.
- **Opatrné s vašimi priečinkami.** Priečinok, ktorému ste nedôverovali, nespustí nič: žiadne procesy gitu, jazykové servery, hooky ani agentov. Deštruktívne príkazy gitu sa nikdy nepovolia natrvalo a commity sa kontrolujú na tajné údaje.
- **Diktovanie.** Hovorte do editora a do poľa správy, online cez OpenAI alebo Groq, alebo lokálne s Whisper a Parakeet.
- **Natívne do poslednej kosti.** Okná, taby, ponuky a panely s nástrojmi z AppKitu, TextKit 2 pre editor, Metal pre terminál, WebKit iba v tabe prehliadača. Tmavý režim, Zvýšiť kontrast a VoiceOver fungujú ako v každej aplikácii pre Mac.
- **Vo vašom jazyku.** Rozhranie je k dispozícii v 35 jazykoch: arabčina, katalánčina, čínština (zjednodušená), čínština (tradičná), chorvátčina, čeština, dánčina, holandčina, angličtina, fínčina, francúzština, nemčina, gréčtina, hebrejčina, hindčina, maďarčina, indonézština, taliančina, japončina, kórejčina, malajčina, nórčina, poľština, portugalčina (Brazília), portugalčina (Portugalsko), rumunčina, ruština, slovenčina, slovinčina, španielčina, švédčina, thajčina, turečtina, ukrajinčina a vietnamčina.

## Požiadavky

- macOS 14 Sonoma alebo novší, na Apple Silicon alebo Intel. Dango sa vyvíja a testuje na macOS 27.
- Pre AI vlákna agent, ktorého chcete používať; pozrite [AI agenti](#ai-agenti).
- Pre jazykové servery a ladiace adaptéry, ktoré Dango nainštaluje na požiadanie: Go pre gopls a `dlv`; Node.js pre servery pre TypeScript, Python, JSON, YAML, CSS, HTML a shell a pre JavaScriptový debugger; Xcode Command Line Tools pre clangd, sourcekit-lsp a `lldb-dap`. rust-analyzer sa sťahuje z jeho vydaní na GitHube.
- Pre predpovede pri písaní predplatné GitHub Copilot (stačí bezplatné) alebo model typu fill-in-the-middle v Ollama alebo LM Studio.

## Inštalácia

**Z obrazu disku.** Každé vydanie je obraz disku `Dango-<version>.dmg` s jednou univerzálnou aplikáciou pre Apple Silicon a Intel, podpísanou identifikátorom Developer ID a notarizovanou spoločnosťou Apple. Otvorte ho a pretiahnite Dango do priečinka Aplikácie. Homebrew Cask nainštaluje ten istý obraz. Zatiaľ nie je zverejnené žiadne vydanie.

**Aktualizácie.** Dango z obrazu disku sa samo udržiava aktuálne cez [Sparkle](https://sparkle-project.org). Pri druhom spustení sa spýta, či môže samo vyhľadávať aktualizácie; kým nesúhlasíte, neodošle žiadnu požiadavku. **Dango › Vyhľadať aktualizácie…** ich vyhľadá hneď. Aktualizácie sú podpísané vlastným kľúčom aplikácie, ktorý Sparkle overí skôr, než čokoľvek nainštaluje. Vlastné zostavenie nemá aktualizátor.

**Zo zdrojového kódu.** Pozrite [Zostavenie zo zdrojového kódu](#zostavenie-zo-zdrojového-kódu) a potom presuňte `target/release/Dango.app` do `/Applications`.

**Príkaz pre terminál.** **Dango › Inštalovať príkaz „dango“…** zapíše malý skript do `~/.local/bin/dango`, bez práv administrátora; `~/.local/bin` musí byť vo vašej premennej `PATH`. Potom `dango .` otvorí aktuálny priečinok a `dango file.rs` súbor. Dango tam nikdy nenahradí súbor, ktorý nezapísalo samo.

## Začíname

1. **Otvorte priečinok** cez ⌘O alebo `dango <folder>`. Pri prvom otvorení sa Dango spýta, či mu dôverujete (pozrite [Dôvera](#dôvera)).
2. **Postranný panel** prepína medzi časťami Súbory (⌘1), Vyhľadávanie (⌘2), Agenti (⌘3), Git (⌘4), Testy (⌘5), Ladenie (⌘6), CI (⌘7) a Kontajnery (⌘8); kliknutím pravým tlačidlom skryjete tie, ktoré nepoužívate, a ⌃⌘S postranný panel zbalí.
3. **Každý dokument je tab** okna priečinka, vlastné taby macOS. „+“ na konci lišty tabov a ⌘T ponúkajú nový súbor, terminál, prehliadač a každého nainštalovaného agenta, aj vo vlastnom git worktree; podržanie „+“ zobrazí to isté ako ponuku.
4. **Rýchlo otvoriť** cez ⌘P nájde súbory a s `#` symboly projektu; ⇧⌘F vyhľadáva a nahrádza v projekte.
5. **Hovorte s agentom** v AI tabe. ⌘Esc presunie fokus klávesnice do jeho poľa správy a späť.
6. **Nastavenia** cez ⌘, obsahujú taby Všeobecné, CI, Agenti, Vlastný agent, Jazykový server, Diktovanie a Predpoveď.

Na paneli s nástrojmi je späť a dopredu cez miesta, ktoré ste upravovali (⌃- a ⌃⇧-), vetva, Rýchlo otvoriť a prehľad Aktivita, ktorý zobrazuje všetko, čo beží. **Otvoriť vedľa** v kontextovej ponuke súboru zobrazí druhý dokument vedľa prvého. Po reštarte alebo páde sa každé okno vráti so svojimi tabmi a editory s neuloženým textom sa vrátia aj s ním. Obrázky, zvuk, video, písma a dokumenty Office sa otvárajú v tabe cez Quick Look, PDF cez PDFKit.

## AI agenti

| Agent | Ako beží | Čo potrebuje |
|---|---|---|
| Claude | príkaz `claude`, s ktorým Dango komunikuje priamo cez jeho protokol stream-json | nainštalovaný a prihlásený `claude`; Dango ho hľadá v `PATH` prihlasovacieho shellu, v `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` a `/usr/local/bin`. Prihlásenie prebieha iba v samotnom `claude` |
| Codex | `codex-acp` cez Agent Client Protocol (ACP) | Node.js a npm; Dango nainštaluje balík v pevne určenej verzii, keď o to požiadate |
| Antigravity | `agy acp` | váš vlastný `agy` v `PATH` alebo stiahnutie pevne určenej verzie, nainštalované na požiadanie |
| OpenCode | `opencode acp` | váš vlastný `opencode` v `PATH` alebo stiahnutie pevne určenej verzie, nainštalované na požiadanie |
| Ďalší agenti ACP | z registra ACP, ktorý sa načíta iba na požiadanie | čokoľvek, čo uvádza položka registra; položky bez kontrolného súčtu sa nainštalujú až po vašom potvrdení |
| Vlastný agent | zabudovaný v aplikácii, s nástrojmi na súbory, vyhľadávanie, príkazy, diagnostiku a webové požiadavky | kľúč API pre Anthropic, OpenAI, Google Gemini alebo OpenRouter, uložený v kľúčenke; Ollama alebo LM Studio na tomto Macu bez kľúča; akýkoľvek server, ktorý hovorí API od OpenAI alebo Anthropic |

**Nastavenia › Agenti** ukazujú, ktorých agentov tento Mac má, inštalujú ostatných a ich aktualizácie a otvárajú register ACP. Agenti, ktorých Dango inštaluje, sú pevne nastavení na otestované verzie a overení podľa svojich zverejnených kontrolných súčtov (npm integrity alebo SHA-256); Dango nespúšťa žiadne balíky `npx` bez verzie.

### Časová os

Každý agent pracuje v AI tabe s rovnakou časovou osou: odpovede, volania nástrojov s ich výstupom, žiadosti o povolenie, otázky, plány na schválenie a komentovanie (⌥⌘K), zoznamy úloh, subagenti a úlohy na pozadí. Dve tlačidlá v hlavičke tabu zobrazia bežiacich subagentov a procesy na pozadí. Spomeňte súbor, `@terminal` alebo `@browser` a agent dostane ich obsah. Zelená bodka na tabe ukazuje reláciu, ktorá pracuje, červená reláciu, ktorá na vás čaká, a hlásenie vás upozorní, keď relácia skončí alebo čaká, kým sa venujete niečomu inému. Pri Claude stavový riadok zobrazuje kontext a využitie.

### Povolenia a režimy

Nové vlákna začínajú v režime „Manuálne“: agent sa pýta, skôr než zmení súbory alebo spustí príkazy. Pre deštruktívne príkazy gitu (`push --force`, `reset --hard`, `clean -fd`) Dango neponúka „Vždy povoliť“. Vlastný agent si pravidlá „Vždy povoliť“ uchováva pre každý projekt, uvádza ich v **Nastavenia › Vlastný agent** a pri deštruktívnych príkazoch gitu sa pýta zakaždým, nech je uložené čokoľvek; Claude a agenti ACP si takéto pravidlá uchovávajú vo vlastnej konfigurácii. Pri „Povoliť všetko“ bez sandboxu vlákno zobrazuje trvalé upozornenie.

### Kontrolné body a worktree

Pred každým promptom si Dango uloží kontrolný bod, ako commit pod `refs/ide-checkpoints/` v repozitári projektu, alebo vo vlastnom priečinku pri projekte bez gitu; bežný push ani fetch ich nikdy neprenesie. To, čo agent zmenil, možno skontrolovať a pretočiť späť po jednotlivých správach. Vlákno môže pracovať vo vlastnom git worktree: panel Git ukáže, čo v ňom zmenilo, a prevezme prácu zlúčením alebo cherry-pickom.

## Editor

- **Zvýrazňovanie syntaxe, čísla riadkov a značky zmien** oproti poslednému commitu, hľadanie a nahrádzanie, Rýchlo otvoriť (⌘P) a vyhľadávanie v projekte (⇧⌘F).
- **Odsadenie** sa riadi súborom a `.editorconfig`: tabulátory alebo medzery, automatické odsadenie, párové zátvorky.
- **Viac kurzorov:** ⌥-kliknutie, ⌥-ťahanie pre stĺpec, ⌃⇧↑ a ⌃⇧↓, ⌘D pre ďalší výskyt.
- **Zbaľovanie** (⌥⌘← a ⌥⌘→, s ⇧ pre všetko), prepínanie komentárov (⌘/), prechod na riadok (⌃G), viditeľné medzery a zalamovanie riadkov (⌥Z).
- **Súbory si zachovávajú kódovanie**, značku poradia bajtov a konce riadkov. Súbor zmenený na disku sa znova načíta, keď editor nemá neuložený text; inak lišta ponúkne rozdiely a zlúčenie.
- **Predpovede pri písaní.** Počas písania sa sivou farbou zobrazí návrh a kláves Tab ho prijme. Pochádza z GitHub Copilot alebo z lokálneho modelu v Ollama alebo LM Studio, podľa výberu v **Nastavenia › Predpoveď**; lokálny model ponechá váš kód na tomto Macu.
- **Minimapa a pevné posúvanie** (ponuka Editor): úzky obraz celého súboru vedľa textu a riadky hlavičiek blokov, v ktorých leží horný riadok, pridržané nad ním.
- **Diagnostika na konci riadka** (ponuka Zobraziť): najzávažnejšie hlásenie každého riadka po sekunde pokoja, so symbolom jeho závažnosti.
- **Živý náhľad statických stránok:** Editor › Otvoriť v tabe prehliadača poskytuje stránky dôveryhodného priečinka priamo z aplikácie Dango a znova načíta tab, keď sa uloží súbor.
- **Prettier** formátuje JavaScript, TypeScript, CSS, HTML, Markdown a ďalšie jazyky, kde ho projekt nastavuje, vlastným `node_modules/.bin/prettier` projektu; nastavenie `formatter` vyberá formátovač pre každý jazyk.
- **Režim Vim** (Editor › Režim Vim): normálny, vkladací, vizuálny a nahrádzací režim, počty, operátory s pohybmi a textovými objektmi, `.`, registre, značky a makrá. `u` a ⌃R používajú históriu odvolaní editora a jeden prechod vkladania je jeden krok. `"+` a `"*` sú schránka; nastavenie `vimClipboard` urobí schránkou aj nepomenovaný register. `/`, `?`, `n`, `N`, `*`, `:s` a `:%s` hľadajú regulárnymi výrazmi Rustu, nie vzormi Vimu: žiadne `\v` a `\b` namiesto `\<` a `\>`. Príkazy Ex sú `:w`, `:q`, `:wq`, `:x`, `:noh` a `:<line>`. V normálnom režime si Vim berie ⌃G od prechodu na riadok.
- **Synchronizácia nastavení** cez priečinok na iCloud Drive (**Nastavenia › Všeobecné**): vaše nastavenia a skratky vás nasledujú na vaše ďalšie Macy; cesty, lokálne adresy a súhlasy zostávajú na každom zvlášť.

## Jazykové servery

Go, Rust, TypeScript a JavaScript, Python, C, C++ a Objective-C, Swift, shellové skripty, JSON, YAML, CSS, HTML, TOML, Markdown a Lua a k tomu vaše vlastné servery z **Nastavenia › Jazykový server**. Dango nainštaluje server na požiadanie, do vlastného priečinka s nástrojmi, a použije vlastný `tsc` projektu TypeScript, keď môže slúžiť ako jazykový server.

Čo prinášajú: diagnostiku (⌘' a ⇧⌘' medzi nimi prechádzajú), informácie pri podržaní ukazovateľa (⌃⌘I), dopĺňanie (⌥Esc), pomoc so signatúrou, prechod na definíciu (⌃⌘J), na definíciu typu a implementáciu, referencie (⌃⇧⌘F), hierarchie volaní a typov (⌃⇧⌘H), premenovanie (⌃⌘E), formátovanie (⌥⇧F), sémantické farby, zbaľovanie a osnovu (⇧⌘O); typy a názvy parametrov priamo v riadku; rýchle opravy a refaktorovania za žiarovkou (⌥⌘A); symboly projektu cez ⌥⌘O.

Popri vlastnom serveri súboru kontroluje ESLint JavaScript a TypeScript projektov, ktoré ho používajú, aj s jeho rýchlymi opravami, a skratky Emmet sa zobrazujú v dopĺňaní pre HTML, CSS a JSX. Dango oba nainštaluje na požiadanie; **Nastavenia › Všeobecné** opraví problémy ESLint pri ukladaní, ak chcete.

## Testy

Panel Testy zobrazuje testy priečinka podľa balíka a súboru s ich stavom: `cargo test` alebo cargo-nextest, Go, pytest, Vitest a Jest. Symbol pri každom teste na okraji ho spustí, ⌥-kliknutie ho ladí a neúspešný assert označí svoj riadok. Ponuka Ladenie spustí test pri kurzore (⌃⌥⌘U), znova posledné testy (⌃⌥⌘G) alebo všetky a ladí test pri kurzore. Testy bežia len v dôveryhodných priečinkoch, pretože spúšťajú kód projektu; `cargo test` vypíše svoje testy, až keď o to požiadate, pretože výpis zostaví každý testovací program.

## Ladiaci nástroj

Body prerušenia, aj s podmienkami a ako body záznamu, krokovanie, vlákna programu s ich zásobníkmi volaní, premenné a sledované výrazy v Inšpektore (⌥⌘I), hodnoty vedľa kódu, kým je program pozastavený, a ladiaca konzola, ktorá vyhodnocuje v pozastavenom rámci.

| Jazyk | Ladiaci adaptér |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` z Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScriptový debugger, na Node.js |
| Python | debugpy |

Načíta sa `.vscode/launch.json` projektu. Ponuka Ladenie používa skratky z Xcode: ⌘\\ nastaví bod prerušenia, ⌘Y všetky zapne alebo vypne, ⌃⌘Y pokračuje, F6, F7 a F8 prejdú cez, vstúpia a vystúpia, ⌘. zastaví.

## Terminál

Terminály v taboch (⌃\`) s prostredím prihlasovacieho shellu, vykresľované cez Metal. ⌘K vymaže obrazovku. Vložený alebo pretiahnutý obrázok sa odovzdá kódovaciemu agentovi spustenému v termináli, inak sa vloží cesta k uloženej kópii. ⌘-kliknutie otvára odkazy a názvy súborov. Vývojový server, ktorý terminál spustí, možno otvoriť v tabe prehliadača.

## Prehliadač

Tab prehliadača s poľom adresy (⌘L), späť a dopredu (⌘[ a ⌘]), opätovným načítaním (⌘R), vyhľadávaním, sťahovaním, ktoré sa najprv opýta, a webovým inšpektorom. `@browser` odovzdá stránku agentovi, označenú ako text zvonku.

## Git

Panel Git pripravuje na commit a zahadzuje súbory alebo jednotlivé úseky, commituje, commituje a pushuje alebo synchronizuje, prepína a vytvára vetvy, uchováva stashe, rieši konflikty pri zlúčení a kreslí graf posledných commitov. Blame ukazuje, kto naposledy zmenil riadok pod kurzorom, alebo stĺpec pre celý súbor. Tlačidlo napíše správu commitu pomocou AI; diff sa odošle až vtedy, keď v ňom gitleaks nenašiel žiadne tajné údaje, a každý commit sa na tajné údaje kontroluje tiež.

## CI

GitHub Actions a GitLab CI, aj GitHub Enterprise Server a GitLab na vlastnom serveri:

- spustenia, joby a logy aktuálnej vetvy;
- opätovné spustenie neúspešných jobov, jedného jobu alebo (na GitHube) celého spustenia; spúšťanie manuálnych jobov alebo schvaľovanie čakajúcich nasadení; spustenie novej pipeline alebo workflowu, ktorý možno spustiť ručne; zrušenie;
- otvorené pull requesty alebo merge requesty a issues, ktoré sa otvárajú v tabe prehliadača;
- log neúspešného jobu odovzdaný agentovi, označený ako text zvonku.

Token pochádza z kľúčenky, z `gh` alebo `glab`, z prostredia prihlasovacieho shellu, alebo je to ten, ktorý zadáte v **Nastavenia › CI**.

## Kontajnery

Panel Kontajnery zobrazuje projekty Compose v Dockeri s ich službami, samostatné kontajnery, obrazy a zväzky a sleduje udalosti Dockera, ako nastávajú. Súbor Compose v priečinku sa zobrazí ako projekt ešte predtým, ako existuje ktorýkoľvek z jeho kontajnerov. Kontajnery sa spúšťajú, zastavujú a reštartujú; ich logy a shell sa otvárajú v taboch terminálu a zverejnený port sa otvorí v tabe prehliadača. Vymazanie kontajnera, obrazu alebo zväzku a vyčistenie nepoužívaných dát sa najprv opýtajú. Príkazy up, down a pull z Compose bežia len v dôveryhodných priečinkoch, pretože Compose zostavuje a spúšťa to, čo projekt opisuje. Ponuka panela vyberá kontext Dockera pre okno bez zmeny toho, ktorý používajú vaše shelly; rovnaké príkazy sú v ponuke Kontajnery. Panel potrebuje príkazový riadok `docker`, Docker 24.0.6 alebo novší.

## Markdown

- **Náhľad.** Súbor Markdown sa otvorí s náhľadom vedľa; **Zobraziť › Zobraziť/skryť náhľad vedľa** (⇧⌘V) ho zatvorí a otvorí a **Nastavenia › Všeobecné** vypne automatický náhľad. Náhľad vykresľuje vzorce, diagramy Mermaid, Graphviz a PlantUML a obrázky z priečinka; obrázky z webu sa načítajú iba v dôveryhodnom priečinku alebo keď o ne v náhľade požiadate.
- **Ponuka Formát:** tučné (⌘B), kurzíva (⌘I), prečiarknuté (⇧⌘X), kód (⌥⇧⌘C), odkaz (⇧⌘K), nadpisy 1–3 (⌥⌘1–⌥⌘3), zoznamy s odrážkami a číslované zoznamy, úlohy, citácie a bloky kódu. Každý príkaz funguje ako prepínač, na výbere alebo na riadkoch, a je jedným krokom pri odvolaní.
- **Vkladanie.** Text z Wordu alebo z webovej stránky príde ako Markdown, jeho obrázky ako súbory vedľa dokumentu; ⌥⇧⌘V vloží čistý text.
- **Exportovať.** **Súbor › Exportovať** zapíše PDF, jeden súbor HTML s obrázkami vo vnútri alebo dokument Word. Obrázky z webu sa pridajú iba z dôveryhodného priečinka.
- **Otvoriť v aplikácii Splat.** Kde je nainštalovaný [Splat](https://github.com/sventmueller/splat), **Súbor › Otvoriť v aplikácii Splat** a kontextové ponuky stromu súborov a tabov v ňom dokument otvoria, keď ho predtým uložia.

## Diktovanie

Diktujte do poľa správy AI tabu alebo do editora: tlačidlom mikrofónu, ktoré tam je, cez **Upraviť › Spustiť diktovanie** (⌃⌥D) alebo klávesom fn (podržaním nahrávate, dvojitým ťuknutím nahrávate ďalej). Rozpoznávanie reči je to z [Lowspeak](https://github.com/sventmueller/lowspeak); samotný Lowspeak nemusí byť nainštalovaný.

- **Nastavenia › Diktovanie** určujú režim a model (online s OpenAI alebo Groq, lokálne s Whisper alebo Parakeet), jazyk, následné spracovanie, mikrofón a vašu vlastnú slovnú zásobu.
- **Lokálne modely** sa stiahnu z Hugging Face, keď si niektorý vyberiete, s ukazovateľom priebehu a možnosťou zrušiť. Ukladajú sa do priečinkov, ktoré používa Lowspeak, takže každá z oboch aplikácií použije to, čo stiahla tá druhá. Lokálny model ponechá nahrávku na tomto Macu.
- **Online prepis** potrebuje kľúč pre OpenAI alebo Groq, ktorý Dango uchováva vo vlastnej položke kľúčenky.
- **Z Lowspeak.** Ak je Lowspeak nastavený inak, než sú predvolené hodnoty, tab Diktovanie raz ponúkne prevzatie jeho nastavení a na požiadanie aj jeho kľúčov; macOS sa opýta, skôr než Dango niektorý z nich prečíta (vyberte „Povoliť“, nie „Vždy povoliť“).
- **Povolenia.** Pri prvom diktovaní sa macOS opýta na mikrofón. Kláves fn funguje, kým je Dango aktívnou aplikáciou a Lowspeak nebeží; potrebuje povolenie Prístupnosť, o ktoré Dango požiada po prvom diktovaní, nikdy pri spustení.

## Dôvera

Priečinok môže spôsobiť, že sa programy spustia, len čo sa v ňom niečo spustí: git hooky a nastavenia repozitára ako `core.fsmonitor`, jazykové servery z `node_modules`, hooky agentov, `.mcp.json`. Keď priečinok otvoríte prvýkrát, Dango sa spýta, či mu dôverujete.

- **Dôveryhodný:** git, jazykové servery, ladiace nástroje, agenti a CI bežia pre tento priečinok a všetko pod ním.
- **Obmedzený:** môžete čítať a upravovať; značky zmien v editore pochádzajú z priameho čítania `.git`. Nič, čo priečinok konfiguruje, sa nespustí. Panely Git, CI a AI to uvádzajú a ponúkajú, aby ste priečinku začali dôverovať.

Text, ktorý nepochádza od vás, môže obsahovať pokyny, ktoré majú riadiť agenta (prompt injection). Výstup terminálu (`@terminal`), stránka prehliadača (`@browser`), log CI odovzdaný agentovi, súbor mimo projektu a dlhší text vložený z webovej stránky preto idú ďalej označené: štítok uvádza „Externý obsah“ a agent dostane text s upozornením, že ide o nedôveryhodné údaje, ktorých pokyny nesmie nasledovať. Webové stránky vlastného agenta prichádzajú s rovnakým upozornením. Vložený text sa považuje za text z webovej stránky, keď to prehliadač uvedie v schránke, ako to robia Safari, Chrome a Firefox; text odinakiaľ a krátka fráza z niekoľkých slov zostávajú vaše.

## Sandbox

Samotné Dango beží bez App Sandbox, pretože by ho zdedil každý program, ktorý spúšťa: terminály, git, jazykové servery a agenti by nemohli fungovať. Namiesto toho má každé AI vlákno prepínač sandboxu, predvolene vypnutý. Čo obmedzuje, závisí od agenta, a popis prepínača to uvádza:

| Agent | So zapnutým sandboxom |
|---|---|
| Claude | príkazy shellu bežia vo vlastnom sandboxe Claude; jeho nástroje na súbory a web nie |
| Codex | príkazy a zmeny zapisujú iba do projektu, bez siete |
| Vlastný agent | súbory, webové požiadavky a príkazy zostávajú v rámci projektu a povolených priečinkov, bez siete (`sandbox-exec`) |
| Antigravity, OpenCode | žiadny sandbox, ktorý by Dango mohlo zapnúť |

## Vzhľad a prístupnosť

- **Tmavý režim** sa riadi systémom; **Zvýšiť kontrast** a **Obmedziť pohyb** sa rešpektujú.
- **VoiceOver** pomenúva postranný panel, taby, karty časovej osi AI, riadky Git a CI, ladiaci nástroj a text terminálu. Body prerušenia sa nikdy nerozlišujú iba farbou.
- **Klávesnica.** Každý príkaz má skratku a všetky sa dajú zmeniť v `keybindings.json`; ⌥⌘\` prejde na ďalší panel.
- **Sprava doľava.** V arabčine a hebrejčine je okno zrkadlené, zobrazenia kódu však zostávajú zľava doprava.

Čo Dango pridáva nad rámec AppKitu, oblasť po oblasti, je v [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Jazyky

Rozhranie je preložené do 35 jazykov: arabčina, katalánčina, čínština (zjednodušená), čínština (tradičná), chorvátčina, čeština, dánčina, holandčina, angličtina, fínčina, francúzština, nemčina, gréčtina, hebrejčina, hindčina, maďarčina, indonézština, taliančina, japončina, kórejčina, malajčina, nórčina, poľština, portugalčina (Brazília), portugalčina (Portugalsko), rumunčina, ruština, slovenčina, slovinčina, španielčina, švédčina, thajčina, turečtina, ukrajinčina a vietnamčina.

Dango sa riadi poradím jazykov v **Systémové nastavenia › Všeobecné › Jazyk a región**; tam v časti **Aplikácie** môžete vybrať jazyk aj iba pre Dango. Ak pre váš jazyk nie je preklad, použije angličtinu. Pojmy sa riadia jedným glosárom, [docs/glossary.md](../../../docs/glossary.md), a vlastnými prekladmi od Applu, takže Dango pôsobí ako zvyšok vášho Macu.

## Klávesové skratky

| Akcia | Skratka |
|---|---|
| Otvoriť priečinok, nové okno, zatvoriť okno | ⌘O, ⇧⌘N, ⇧⌘W |
| Nový súbor, uložiť, uložiť ako | ⌘N, ⌘S, ⇧⌘S |
| Nový tab, zatvoriť tab, ďalší a predchádzajúci tab | ⌘T, ⌘W, ⇧⌘] a ⇧⌘[ |
| Rýchlo otvoriť, vyhľadávať v projekte | ⌘P, ⇧⌘F |
| Súbory, Vyhľadávanie, Agenti, Git, Testy, Ladenie, CI, Kontajnery | ⌘1 až ⌘8 |
| Zobraziť alebo skryť postranný panel, ďalší panel | ⌃⌘S, ⌥⌘\` |
| Späť a dopredu cez upravované miesta | ⌃- a ⌃⇧- |
| Prepnúť komentár, prejsť na riadok | ⌘/, ⌃G |
| Ďalší výskyt, kurzor nad a pod | ⌘D, ⌃⇧↑ a ⌃⇧↓ |
| Zbaliť, rozbaliť, zbaliť všetko, rozbaliť všetko | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Zalamovanie riadkov, inšpektor | ⌥Z, ⌥⌘I |
| Dopĺňanie, informácie pri podržaní ukazovateľa, prechod na definíciu | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referencie, hierarchia volaní, premenovať | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboly v súbore, v projekte | ⇧⌘O, ⌥⌘O |
| Rýchla oprava, formátovať | ⌥⌘A, ⌥⇧F |
| Ďalší a predchádzajúci problém | ⌘' a ⇧⌘' |
| Náhľad Markdownu, vložiť ako čistý text | ⇧⌘V, ⌥⇧⌘V |
| Tučné, kurzíva, prečiarknuté, kód, odkaz | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Nadpis 1 až 3 | ⌥⌘1 až ⌥⌘3 |
| Nový terminál, vymazať terminál | ⌃\`, ⌘K |
| Fokus do poľa správy agenta, komentovať plán | ⌘Esc, ⌥⌘K |
| Pole adresy, znova načítať, späť, dopredu (prehliadač) | ⌘L, ⌘R, ⌘[, ⌘] |
| Spustiť alebo zastaviť diktovanie | ⌃⌥D |
| Bod prerušenia, zapnúť alebo vypnúť všetky body prerušenia | ⌘\\, ⌘Y |
| Spustiť test pri kurzore, znova spustiť posledné testy | ⌃⌥⌘U, ⌃⌥⌘G |
| Pokračovať, prejsť cez, vstúpiť, vystúpiť, zastaviť | ⌃⌘Y, F6, F7, F8, ⌘. |
| Nastavenia | ⌘, |

`keybindings.json` v priečinku s údajmi aplikácie (pozrite [Kde sú uložené údaje](#kde-sú-uložené-údaje)) zmení ktorúkoľvek z nich.

## Súkromie a bezpečnosť

- **Žiadna telemetria,** žiadna analytika, žiadne hlásenia o pádoch odosielané kamkoľvek. Chyby idú do systémového logu (aplikácia Konzola, pod názvom aplikácie).
- **Sieť iba pre to, čo spustíte:** agenti, ktorých používate, každý so svojou vlastnou službou; register ACP a sťahovanie agentov, jazykových serverov, ladiacich adaptérov a modelov na diktovanie, keď oň požiadate; hostitelia CI, do ktorých sa prihlásite; predpovede pri písaní z Copilot, ak ste si ho vybrali; poskytovatelia vlastného agenta; online prepis, ak ste si ho vybrali; stránky, ktoré otvoríte v tabe prehliadača; a kontrola aktualizácií, keď ste ju povolili.
- **Kód opúšťa Mac** iba tam, kde ste si na to vybrali službu: agent, predpovede Copilot, správa commitu napísaná pomocou AI. Diff pre správu commitu sa odošle až vtedy, keď v ňom gitleaks nenašiel žiadne tajné údaje.
- **Kľúče a tokeny** zostávajú v kľúčenke macOS, nikdy nie v súbore s nastaveniami. Kľúče iných aplikácií, napríklad Lowspeak, sa prečítajú iba s vaším súhlasom, a iba raz.
- **Sťahovanie pevne určených verzií.** Agenti, jazykové servery a ladiace adaptéry, ktoré Dango inštaluje, sú pevne nastavené na otestované verzie a overené podľa svojich kontrolných súčtov.
- **Podpisovanie kódu** s Hardened Runtime; aktualizácie sa inštalujú iba s vlastným podpisom aplikácie.

## Kde sú uložené údaje

`<id>` je identifikátor balíka, `local.agentide` pre vydanie (`local.agentide.debug` pre ladiace zostavenia). Neobsahuje zobrazovaný názov, takže premenovanie zachová všetky údaje.

| Miesto | Čo |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (zoznam panela AI), `agent-threads/` (konverzácie vlastného agenta), `layouts/` (taby pre každý projekt), `recovery/` (neuložené editory po páde), `checkpoints/` (pre priečinky bez gitu), `worktrees/`, `agents/` a `tools/` (nainštalovaní agenti, jazykové servery a ladiace adaptéry), `task-output/` |
| `~/Library/Caches/<id>/` | vložené obrázky, skripty integrácie shellu, register ACP; možno ich vymazať |
| `~/Library/Logs/<id>/dictation/` | logy diktovania |
| `~/Library/Application Support/Lowspeak` a `FluidAudio` | lokálne modely na diktovanie, zdieľané s Lowspeak |
| `~/Library/WebKit/<id>/` | cookies a údaje webových stránok tabu prehliadača |
| `<project>/.dango/settings.json` | vlastné nastavenia projektu |
| git repozitár projektu | kontrolné body, ako commity pod `refs/ide-checkpoints/`; bežný push ani fetch ich nikdy neprenesie |
| kľúčenka, služba `<id>` | kľúče API vlastného agenta a diktovania, tokeny CI |
| `~/.local/bin/dango` | príkaz pre terminál, ak ste ho nainštalovali |

Claude si uchováva vlastné relácie v `~/.claude/projects`; Dango ich číta, aby mohlo reláciu obnoviť, ale nemení ich.

## Známe obmedzenia

- **Žiadny App Sandbox** pre samotnú aplikáciu, pretože by ho zdedili terminály, git, jazykové servery a agenti; priečinku musíte dôverovať skôr, než sa spustí čokoľvek, čo konfiguruje. Antigravity a OpenCode nemajú sandbox, ktorý by Dango mohlo zapnúť.
- **Údaje o využití a rozpis kontextu pri Claude** sa opierajú o časti jeho protokolu označené ako experimentálne a skryjú sa, keď chýbajú.
- **Pretočenie konverzácie späť** funguje iba pri Claude; pri ostatných agentoch sa pretočí späť iba kód.
- **Prihlásenie do CI cez prehliadač** je vypnuté; použite `gh`, `glab`, token alebo prostredie.
- **Gemini CLI sa neponúka:** Google ukončil prihlasovanie osobným účtom a odkazuje na Antigravity. Staršie vlákna Gemini zostávajú čitateľné.

## Zostavenie zo zdrojového kódu

Potrebujete:

- macOS 14 alebo novší s Xcode Command Line Tools (`xcode-select --install`); samotný Xcode nie je potrebný.
- Rust podľa `rust-toolchain.toml`. `rust` z Homebrew zostavuje pre procesor tohto Macu; univerzálne zostavenie vydania potrebuje rustup s cieľmi `aarch64-apple-darwin` a `x86_64-apple-darwin`.
- Kópie repozitárov [splat](https://github.com/sventmueller/splat) a [lowspeak](https://github.com/sventmueller/lowspeak) vedľa tohto (`../splat`, `../lowspeak`), obe na vetve `main`; pracovný priestor z nich berie crates podľa cesty.
- cmake pre každé zostavenie s diktovaním (`make run`, `make build`, `make release`), ktoré zostavuje whisper.cpp. `make check` sa zostavuje bez neho.

| Príkaz | Čo robí |
|---|---|
| `make run` | zostaví ladiacu aplikáciu (`target/debug/Dango Debug.app`) a otvorí ju |
| `make build` | zostaví aplikáciu na vydanie (`target/release/Dango.app`), podpísanú lokálnym certifikátom, ktorý prvé zostavenie vytvorí v prihlasovacej kľúčenke; ponechá `VERSION` bez zmeny |
| `make lint` | formátovanie a clippy; varovania spôsobia zlyhanie |
| `make check` | `lint`, potom všetky testy; nepotrebuje kľúčenku, okno ani sieť |
| `make test` | `check`, potom samotest `probe` a audit prístupnosti, zľava doprava a sprava doľava, v skutočnej aplikácii |
| `make perf` | časové rozpočty, v zostavení na vydanie; zmysluplné iba na inak nečinnom Macu |
| `make release` | skúšobné vydanie do `target/release-dry-run/`, podpísané lokálnym certifikátom; nikam nič neodošle |
| `make release-notarized` | `check`, potom samotné vydanie do `dist/`: podpis Developer ID, notarizácia, kanál aktualizácií podpísaný vlastným kľúčom aplikácie; ponechá `VERSION` bez zmeny |
| `make deploy` | na čistej vetve `main` so zelenou pečaťou CI: notarizuje ďalšiu verziu, aktualizuje a commituje `VERSION`, nahrá súbory vydania a odošle kód, pečať CI a značku verzie; GitHub vydanie automaticky zverejní |

Ladiaca aplikácia a aplikácia na vydanie sú samostatné aplikácie s vlastnými nastaveniami a údajmi. Zostavovanie, testovanie, samotesty, vydanie a pravidlá, ktorými sa kód riadi, sú opísané v [docs/development.md](../../../docs/development.md), rozhodnutia za nimi v [docs/plan.md](../../../docs/plan.md). Poznámky k vydaniu sú v [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), preložené vedľa každého README v `assets/readme/<language>/`.

## Licencia

Dango je licencované podľa jednej z týchto licencií

- Licencia Apache, verzia 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licencia MIT ([LICENSE-MIT](../../../LICENSE-MIT))

podľa vášho výberu.

Ak výslovne neuvediete inak, každý príspevok, ktorý ste zámerne odovzdali na zahrnutie do aplikácie Dango, ako je definované v licencii Apache-2.0, bude duálne licencovaný ako vyššie, bez ďalších podmienok.
