<p><img src="../../readme-icon.png" alt="A Dango ikonja" width="168" height="168"></p>

# Dango

**A natív macOS-szerkesztő a kódoló ügynökökkel való munkához.**

A Dango a kódot, az azt módosító ügynököket és mindent, ami körülöttük van, mappánként egy ablakba gyűjti: szerkesztő nyelvi szerverekkel és szerkesztési előrejelzésekkel, terminálok, böngésző, Git, CI, hibakereső és AI-szálak a Claude, a Codex, az Antigravity, az OpenCode vagy egy az appba épített ügynök társaságában. Minden ügynök ugyanazon az idővonalon dolgozik, megkérdezi, mielőtt fájlokat módosít vagy parancsokat futtat, és minden prompt előtt visszaállítási pontot hagy, így üzenetenként átnézheti és visszatekerheti, amit tett. Semmi, amit egy mappa beállít, nem fut, amíg meg nem bízik benne, a kívülről érkező szöveg pedig nem megbízhatóként megjelölve jut el az ügynökhöz. Diktálhat a szerkesztőbe és az üzenetmezőbe, a beszédfelismerés pedig a Macen maradhat. A Dango Rust nyelven, valódi AppKit-vezérlőkre épül, Electron nélkül, 35 nyelven beszél, és nincs benne telemetria.

**Még nem jelent meg.** Az első kiadás egy Developer ID-val aláírt és az Apple által hitelesített lemezkép lesz, macOS 14 vagy újabb rendszerhez, Apple chipes és Intel-alapú Macekre. Addig [fordítsa le a Dangót forráskódból](#fordítás-forráskódból).

**Hibát talált?** [Nyisson egy issue-t](https://github.com/sventmueller/dango/issues/new), és írja meg, melyik ügynökkel és milyen projektben fordult elő.

[![Rust, 2024-es kiadás](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 vagy újabb](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Univerzális: Apple chip és Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit objc2-n keresztül](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminál: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 felületnyelv](https://img.shields.io/badge/languages-35-4C9A2A)
![Nincs telemetria](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Hardened Runtime-mal aláírva](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licenc: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licenc)

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
**Magyar** ·
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

## Tartalom

- [Funkciók röviden](#funkciók-röviden)
- [Követelmények](#követelmények)
- [Telepítés](#telepítés)
- [Első lépések](#első-lépések)
- [AI-ügynökök](#ai-ügynökök)
  - [Az idővonal](#az-idővonal)
  - [Engedélyek és módok](#engedélyek-és-módok)
  - [Visszaállítási pontok és worktree-k](#visszaállítási-pontok-és-worktree-k)
- [Szerkesztő](#szerkesztő)
- [Nyelvi szerverek](#nyelvi-szerverek)
- [Tesztek](#tesztek)
- [Hibakereső](#hibakereső)
- [Terminál](#terminál)
- [Böngésző](#böngésző)
- [Git](#git)
- [CI](#ci)
- [Konténerek](#konténerek)
- [Markdown](#markdown)
- [Diktálás](#diktálás)
- [Bizalom](#bizalom)
- [Homokozó](#homokozó)
- [Megjelenés és kisegítő lehetőségek](#megjelenés-és-kisegítő-lehetőségek)
- [Nyelvek](#nyelvek)
- [Billentyűparancsok](#billentyűparancsok)
- [Adatvédelem és biztonság](#adatvédelem-és-biztonság)
- [Hol tárolódnak az adatok](#hol-tárolódnak-az-adatok)
- [Ismert korlátok](#ismert-korlátok)
- [Fordítás forráskódból](#fordítás-forráskódból)
- [Licenc](#licenc)

## Funkciók röviden

- **Minden ügynök egy idővonalon.** A Claude a saját `claude` parancsán keresztül, a Codex, az Antigravity és az OpenCode az Agent Client Protocolon keresztül, további ügynökök az ACP-jegyzékből, valamint egy az appba épített ügynök az Anthropichoz, az OpenAI-hoz, a Google Geminihez, az OpenRouterhez, az Ollamához és az LM Studióhoz. A válaszok, eszközhívások, engedélykérések, tervek, teendők, alügynökök és háttérfeladatok ugyanúgy néznek ki, bármelyik ügynök fut is.
- **Átnézés és visszatekerés.** A Dango minden prompt előtt visszaállítási pontot ment. Amit egy ügynök módosított, az üzenetenként átnézhető és visszatekerhető, egy szál pedig dolgozhat saját Git-worktree-ben, és összefésüléssel vagy cherry-pickkel adhatja át a munkáját.
- **Mappánként egy ablak.** A fájlok, terminálok, böngészőoldalak, AI-szálak, különbségnézetek és előnézetek ennek az ablaknak a macOS-lapjai. Az oldalsávon a Fájlok, Keresés, Ügynökök, Git, Tesztek, Hibakeresés, CI és Konténerek található.
- **Valódi kódszerkesztő.** Szintaxisszínezés, több kurzor, összecsukás, `.editorconfig`, nyelvi szerverek 17 nyelvhez kiegészítéssel, ugrással a definícióra, hivatkozásokkal, átnevezéssel, gyorsjavításokkal és hierarchiákkal, valamint szerkesztési előrejelzések a GitHub Copilottól vagy egy helyi modelltől.
- **Beépített hibakereső, Git és CI.** Töréspontok és léptetés Rust, C, C++, Swift, Go, JavaScript, TypeScript és Python nyelvhez; egyes blokkok előkészítése, ágak, stashek, ütközések és commitgráf; futások, jobok és naplók a GitHub Actionsből és a GitLab CI-ból.
- **Óvatos a mappáival.** Egy mappa, amelyben nem bízott meg, semmit sem indít: sem Git-folyamatokat, sem nyelvi szervereket, sem hookokat, sem ügynököket. A romboló Git-parancsok soha nem kapnak tartós engedélyt, a commitokat pedig titkos adatok után kutatva ellenőrzi az app.
- **Diktálás.** Beszéljen a szerkesztőbe és az üzenetmezőbe, online az OpenAI vagy a Groq segítségével, vagy helyben a Whisperrel és a Parakeettel.
- **Natív a velejéig.** AppKit-ablakok, -lapok, -menük és -eszközsorok, TextKit 2 a szerkesztőhöz, Metal a terminálhoz, WebKit csak a böngészőlapon. A sötét mód, a Kontraszt növelése és a VoiceOver úgy működik, mint minden Mac-appban.
- **Az Ön nyelvén.** A felület 35 nyelven érhető el: arab, katalán, kínai (egyszerűsített), kínai (hagyományos), horvát, cseh, dán, holland, angol, finn, francia, német, görög, héber, hindi, magyar, indonéz, olasz, japán, koreai, maláj, norvég, lengyel, portugál (Brazília), portugál (Portugália), román, orosz, szlovák, szlovén, spanyol, svéd, thai, török, ukrán és vietnami.

## Követelmények

- macOS 14 Sonoma vagy újabb, Apple chippel vagy Intel processzorral. A Dango fejlesztése és tesztelése macOS 27 alatt történik.
- Az AI-szálakhoz a használni kívánt ügynök; lásd: [AI-ügynökök](#ai-ügynökök).
- A nyelvi szerverekhez és hibakereső adapterekhez, amelyeket a Dango kérésre telepít: Go a gopls és a `dlv` számára; Node.js a TypeScript-, Python-, JSON-, YAML-, CSS-, HTML- és shellszerverekhez, valamint a JavaScript-hibakeresőhöz; az Xcode Command Line Tools a clangd, a sourcekit-lsp és az `lldb-dap` számára. A rust-analyzer a GitHubon közzétett kiadásaiból töltődik le.
- A szerkesztési előrejelzésekhez GitHub Copilot-előfizetés (az ingyenes is megfelel), vagy egy fill-in-the-middle modell az Ollamában vagy az LM Studióban.

## Telepítés

**A lemezképről.** Minden kiadás egy lemezkép, `Dango-<version>.dmg`, egyetlen univerzális appal Apple chipes és Intel-alapú Macekhez, Developer ID-val aláírva és az Apple által hitelesítve. Nyissa meg, és húzza a Dangót az Alkalmazások mappára. Egy Homebrew Cask ugyanezt a lemezképet telepíti. Még nem jelent meg kiadás.

**Frissítések.** A lemezképről telepített Dango a [Sparkle](https://sparkle-project.org) segítségével tartja magát naprakészen. A második indításkor megkérdezi, hogy önállóan kereshet-e frissítéseket; amíg bele nem egyezik, nem küld kérést. A **Dango › Frissítések keresése…** azonnal ellenőriz. A frissítéseket az app saját kulcsa írja alá, amelyet a Sparkle ellenőriz, mielőtt bármit telepítene. A saját fordítású példánynak nincs frissítője.

**Forráskódból.** Lásd: [Fordítás forráskódból](#fordítás-forráskódból), majd helyezze át a `target/release/Dango.app` appot az `/Applications` mappába.

**A terminálparancs.** A **Dango › A(z) „dango” parancs telepítése…** egy kis szkriptet ír a `~/.local/bin/dango` helyre, rendszergazdai jogok nélkül; a `~/.local/bin` mappának szerepelnie kell a `PATH` változóban. Ezután a `dango .` megnyitja az aktuális mappát, a `dango file.rs` pedig egy fájlt. A Dango ott soha nem cserél le olyan fájlt, amelyet nem ő maga írt.

## Első lépések

1. **Nyisson meg egy mappát** a ⌘O billentyűkkel vagy a `dango <folder>` paranccsal. Első alkalommal a Dango megkérdezi, megbízik-e benne (lásd: [Bizalom](#bizalom)).
2. **Az oldalsáv** a Fájlok (⌘1), Keresés (⌘2), Ügynökök (⌘3), Git (⌘4), Tesztek (⌘5), Hibakeresés (⌘6), CI (⌘7) és Konténerek (⌘8) nézetek között vált; jobb kattintással elrejtheti a nem használt nézeteket, a ⌃⌘S pedig összecsukja az oldalsávot.
3. **Minden dokumentum egy lap** a mappa ablakában, a macOS saját lapjai közül. A lapsor végén lévő „+” és a ⌘T új fájlt, terminált, a böngészőt és minden telepített ügynököt felkínál, saját Git-worktree-ben is; a „+” lenyomva tartása ugyanezt menüként jeleníti meg.
4. **A Gyors megnyitás** a ⌘P billentyűkkel fájlokat talál, `#` jellel pedig a projekt szimbólumait; a ⇧⌘F keres és cserél a projektben.
5. **Beszéljen egy ügynökkel** egy AI-lapon. A ⌘Esc a billentyűzetfókuszt az üzenetmezőjébe viszi, majd vissza.
6. **A Beállítások** a ⌘, billentyűkkel nyílik meg, lapjai: Általános, CI, Ügynökök, Beépített ügynök, Nyelvi szerver, Diktálás és Előrejelzés.

Az eszközsoron található a vissza és előre lépés a szerkesztett helyek között (⌃- és ⌃⇧-), az ág, a Gyors megnyitás és a Tevékenység áttekintése, amely mindent felsorol, ami fut. Egy fájl helyi menüjében a **Megnyitás mellette** egy második dokumentumot jelenít meg az első mellett. Újraindítás vagy összeomlás után minden ablak visszatér a lapjaival, a nem mentett szöveget tartalmazó szerkesztők pedig ezzel a szöveggel. A képek, hang- és videofájlok, betűtípusok és Office-dokumentumok a Quick Look segítségével nyílnak meg lapon, a PDF-ek a PDFKit segítségével.

## AI-ügynökök

| Ügynök | Hogyan fut | Mire van szüksége |
|---|---|---|
| Claude | a `claude` parancs, amellyel a Dango közvetlenül, a stream-json protokollján kommunikál | telepített és bejelentkezett `claude`; a Dango a bejelentkezési shell `PATH` változójában, valamint a `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` és `/usr/local/bin` helyen keresi. Bejelentkezni csak magában a `claude` parancsban lehet |
| Codex | `codex-acp` az Agent Client Protocolon (ACP) keresztül | Node.js és npm; a Dango kérésre telepíti a rögzített verziójú csomagot |
| Antigravity | `agy acp` | a saját `agy` parancsa a `PATH` változóban, vagy a rögzített verziójú letöltés, kérésre telepítve |
| OpenCode | `opencode acp` | a saját `opencode` parancsa a `PATH` változóban, vagy a rögzített verziójú letöltés, kérésre telepítve |
| További ACP-ügynökök | az ACP-jegyzékből, amelyet csak kérésre tölt le | amit a jegyzék bejegyzése megad; az ellenőrzőösszeg nélküli bejegyzések csak az Ön megerősítése után települnek |
| Beépített ügynök | az appba építve, eszközökkel fájlokhoz, kereséshez, parancsokhoz, diagnosztikához és webes kérésekhez | API-kulcs az Anthropichoz, az OpenAI-hoz, a Google Geminihez vagy az OpenRouterhez, a kulcskarikában tárolva; Ollama vagy LM Studio ezen a Macen, kulcs nélkül; bármely szerver, amely az OpenAI vagy az Anthropic API-ját beszéli |

A **Beállítások › Ügynökök** megmutatja, mely ügynökök vannak meg ezen a Macen, telepíti a többit és a frissítéseiket, és megnyitja az ACP-jegyzéket. A Dango által telepített ügynökök tesztelt verziókhoz vannak rögzítve, és a közzétett ellenőrzőösszegük (npm integrity vagy SHA-256) alapján vannak ellenőrizve; a Dango nem futtat verzió nélküli `npx`-csomagokat.

### Az idővonal

Minden ügynök AI-lapon dolgozik, ugyanazzal az idővonallal: válaszok, eszközhívások a kimenetükkel, engedélykérések, kérdések, jóváhagyható és véleményezhető tervek (⌥⌘K), teendők, alügynökök és háttérfeladatok. A lap fejlécében két gomb listázza a futó alügynököket és háttérfolyamatokat. Említsen meg egy fájlt, a `@terminal` vagy a `@browser` elemet, és az ügynök megkapja a tartalmát. A lapon egy zöld pont a dolgozó munkamenetet, egy piros az Önre váró munkamenetet jelzi, és értesítés szól, amikor egy munkamenet befejeződik vagy várakozik, miközben Ön máshol jár. A Claude esetében az állapotsor a kontextust és a használatot mutatja.

### Engedélyek és módok

Az új szálak „Kézi” módban indulnak: az ügynök megkérdezi, mielőtt fájlokat módosít vagy parancsokat futtat. A romboló Git-parancsokhoz (`push --force`, `reset --hard`, `clean -fd`) a Dango nem kínál „Engedélyezés mindig” lehetőséget. A beépített ügynök projektenként tárolja az „Engedélyezés mindig” szabályait, felsorolja őket a **Beállítások › Beépített ügynök** lapon, és a romboló Git-parancsoknál minden alkalommal rákérdez, bármi legyen is tárolva; a Claude és az ACP-ügynökök a saját konfigurációjukban tárolják az ilyen szabályokat. „Minden engedélyezése” módban, homokozó nélkül a szál állandó figyelmeztetést jelenít meg.

### Visszaállítási pontok és worktree-k

A Dango minden prompt előtt visszaállítási pontot ment, commitként a `refs/ide-checkpoints/` alatt a projekt tárolójában, vagy Git nélküli projektnél egy saját mappában; egy szokásos push vagy fetch soha nem viszi magával őket. Amit egy ügynök módosított, az üzenetenként átnézhető és visszatekerhető. Egy szál dolgozhat saját Git-worktree-ben: a Git-panel megmutatja, mit módosított, és összefésüléssel vagy cherry-pickkel átveszi a munkát.

## Szerkesztő

- **Szintaxisszínezés, sorszámok és változásjelek** az utolsó commithoz képest, keresés és csere, Gyors megnyitás (⌘P) és keresés a projektben (⇧⌘F).
- **A behúzás** a fájlt és a `.editorconfig` beállításait követi: tabulátorok vagy szóközök, automatikus behúzás, zárójelpárok.
- **Több kurzor:** ⌥-kattintás, ⌥-húzás oszlophoz, ⌃⇧↑ és ⌃⇧↓, ⌘D a következő előforduláshoz.
- **Összecsukás** (⌥⌘← és ⌥⌘→, ⇧ billentyűvel mindenre), megjegyzés be- és kikapcsolása (⌘/), ugrás sorra (⌃G), látható szóközök és sortördelés (⌥Z).
- **A fájlok megőrzik a kódolásukat**, a bájtsorrend-jelzőjüket és a sorvégeiket. A lemezen módosult fájl újratöltődik, ha a szerkesztőben nincs mentetlen szöveg; ellenkező esetben egy sáv felkínálja a különbségeket és az összefésülést.
- **Szerkesztési előrejelzések.** Gépelés közben szürkén megjelenik egy javaslat, a Tab pedig elfogadja. A GitHub Copilottól vagy egy helyi modelltől érkezik az Ollamában vagy az LM Studióban, aszerint, hogy mit választ a **Beállítások › Előrejelzés** lapon; helyi modellel a kód ezen a Macen marad.
- **Minitérkép és rögzített görgetés** (Szerkesztő menü): a teljes fájl keskeny képe a szöveg mellett, és azoknak a blokkoknak a fejsorai, amelyekben a legfelső sor áll, fölötte rögzítve.
- **Diagnosztika a sor végén** (Nézet menü): minden sor legsúlyosabb üzenete egy másodpercnyi nyugalom után, a súlyosságát jelző szimbólummal.
- **Statikus oldalak élő előnézete:** a Szerkesztő › Megnyitás böngészőlapon egy megbízható mappa oldalait magából a Dangóból szolgálja ki, és egy fájl mentésekor újratölti a lapot.
- **A Prettier** formázza a JavaScriptet, a TypeScriptet, a CSS-t, a HTML-t, a Markdownt és egyebeket, ahol a projekt beállítja, a projekt saját `node_modules/.bin/prettier` programjával; a `formatter` beállítás nyelvenként választja ki a formázót.
- **Vim mód** (Szerkesztő › Vim mód): normál, beszúró, vizuális és csere mód, ismétlésszámok, operátorok mozgásokkal és szövegobjektumokkal, `.`, regiszterek, jelölők és makrók. Az `u` és a ⌃R a szerkesztő visszavonását használja, és egy beszúrási menet egyetlen lépés. A `"+` és a `"*` a vágólap; a `vimClipboard` beállítás a névtelen regisztert is vágólappá teszi. A keresés (`/`, `?`, `n`, `N`, `*`, `:s` és `:%s`) a Rust reguláris kifejezéseit használja, nem a Vim mintáit: nincs `\v`, és `\<` meg `\>` helyett `\b` áll. Az Ex-parancsok: `:w`, `:q`, `:wq`, `:x`, `:noh` és `:<line>`. Normál módban a Vim elveszi a ⌃G billentyűt az Ugrás sorra parancstól.
- **Beállítások szinkronizálása** egy iCloud Drive-mappán keresztül (**Beállítások › Általános**): a beállításai és billentyűparancsai a többi Macére is elkísérik; az elérési utak, a helyi címek és a hozzájárulások mindegyik gépen külön maradnak.

## Nyelvi szerverek

Go, Rust, TypeScript és JavaScript, Python, C, C++ és Objective-C, Swift, shellszkriptek, JSON, YAML, CSS, HTML, TOML, Markdown és Lua, valamint saját szerverek a **Beállítások › Nyelvi szerver** lapról. A Dango kérésre telepít egy szervert, a saját eszközmappájába, és egy TypeScript-projekt saját `tsc` fordítóját használja, ha az nyelvi szerverként működni tud.

Amit nyújtanak: diagnosztika (a ⌘' és a ⇧⌘' lép közöttük), információ rámutatáskor (⌃⌘I), kiegészítés (⌥Esc), aláírássúgó, ugrás definícióra (⌃⌘J), típusdefinícióra és implementációra, hivatkozások (⌃⇧⌘F), hívási és típushierarchia (⌃⇧⌘H), átnevezés (⌃⌘E), formázás (⌥⇧F), szemantikus színek, összecsukás és vázlat (⇧⌘O); típusok és paraméternevek a soron belül; gyorsjavítások és refaktorálások egy villanykörte mögött (⌥⌘A); a projekt szimbólumai a ⌥⌘O billentyűkkel.

A fájl saját szervere mellett az ESLint ellenőrzi az azt használó projektek JavaScript- és TypeScript-kódját, a saját gyorsjavításaival együtt, az Emmet rövidítései pedig megjelennek a kiegészítésben HTML-hez, CSS-hez és JSX-hez. A Dango kérésre mindkettőt telepíti; a **Beállítások › Általános** igény szerint mentéskor kijavítja az ESLint-problémákat.

## Tesztek

A Tesztek panel csomag és fájl szerint listázza egy mappa tesztjeit az állapotukkal: `cargo test` vagy cargo-nextest, Go, pytest, Vitest és Jest. Az egyes tesztek mellett a margón lévő szimbólum futtatja a tesztet, a ⌥-kattintás hibakeresést indít rajta, a sikertelen assert pedig megjelöli a sorát. A Hibakeresés menü futtatja a kurzornál lévő tesztet (⌃⌥⌘U), újra a legutóbbi teszteket (⌃⌥⌘G) vagy az összeset, és hibakeresést indít a kurzornál lévő teszten. A tesztek csak megbízható mappákban futnak, mivel a projekt kódját futtatják; a `cargo test` csak kérésre listázza a tesztjeit, mert a listázás minden tesztprogramot lefordít.

## Hibakereső

Töréspontok, feltételekkel és naplópontként is, léptetés, szálak a hívási vermükkel, változók és figyelt kifejezések az Inspektorban (⌥⌘I), értékek a kód mellett, amíg a program szüneteltetve van, és egy hibakeresési konzol, amely a szüneteltetett keretben értékel ki.

| Nyelv | Hibakereső adapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` az Xcode Command Line Toolsból |
| Go | `dlv` |
| JavaScript, TypeScript | a JavaScript-hibakereső, Node.js-en |
| Python | debugpy |

Az app beolvassa a projekt `.vscode/launch.json` fájlját. A Hibakeresés menü az Xcode billentyűparancsait használja: a ⌘\\ töréspontot állít be, a ⌘Y mindet be- vagy kikapcsolja, a ⌃⌘Y folytatja, az F6, F7 és F8 átlép, belép és kilép, a ⌘. leállít.

## Terminál

Terminálok lapokon (⌃\`), a bejelentkezési shell környezetével, Metallal rajzolva. A ⌘K törli a képernyőt. Kép beillesztésekor vagy behúzásakor a kép a terminálban futó kódoló ügynökhöz kerül, vagy egy mentett másolat útvonala illesztődik be. A ⌘-kattintás megnyitja a linkeket és a fájlneveket. A terminálból indított fejlesztői szerver megnyitható a böngészőlapon.

## Böngésző

Böngészőlap címmezővel (⌘L), vissza és előre lépéssel (⌘[ és ⌘]), újratöltéssel (⌘R), kereséssel, előbb rákérdező letöltésekkel és a Web Inspectorral. A `@browser` átadja az oldalt egy ügynöknek, külső szövegként megjelölve.

## Git

A Git-panel fájlokat vagy egyes blokkokat készít elő és vet el, commitol, commitol és pushol vagy szinkronizál, ágakat vált és hoz létre, stasheket tárol, összefésülési ütközéseket old fel, és gráfot rajzol a legutóbbi commitokról. A Blame megmutatja, ki módosította utoljára a kurzor alatti sort, vagy egy oszlopot az egész fájlhoz. Egy gomb AI-jal írja meg a commitüzenetet; a különbségek csak akkor mennek ki, ha a gitleaks nem talált bennük titkos adatot, és minden commit is ellenőrzésen esik át titkos adatok után kutatva.

## CI

GitHub Actions és GitLab CI, GitHub Enterprise Server és saját üzemeltetésű GitLab is:

- az aktuális ág futásai, jobjai és naplói;
- sikertelen jobok, egyetlen job vagy (GitHubon) egy teljes futás újraindítása; kézi jobok indítása vagy várakozó telepítések jóváhagyása; új pipeline vagy kézzel indítható workflow indítása; megszakítás;
- a nyitott pull requestek, illetve merge requestek és az issue-k, amelyek a böngészőlapon nyílnak meg;
- egy sikertelen job naplója átadva egy ügynöknek, külső szövegként megjelölve.

A token a kulcskarikából, a `gh` vagy a `glab` parancsból, a bejelentkezési shell környezetéből származik, vagy az, amelyet a **Beállítások › CI** lapon megad.

## Konténerek

A Konténerek panel megjeleníti a Docker Compose-projektjeit a szolgáltatásaikkal, az önálló konténereket, az image-eket és a köteteket, és követi a Docker eseményeit, ahogy történnek. A mappa Compose-fájlja projektként jelenik meg már azelőtt, hogy bármelyik konténere létezne. A konténerek indíthatók, leállíthatók és újraindíthatók; a naplóik és egy shell terminállapokon nyílnak meg, a közzétett port pedig a böngészőlapon. Egy konténer, image vagy kötet törlése, illetve a nem használt adatok kitakarítása előbb rákérdez. A Compose up, down és pull parancsa csak megbízható mappákban fut, mivel a Compose azt építi fel és futtatja, amit a projekt leír. A panel menüje kiválasztja az ablak Docker-kontextusát anélkül, hogy megváltoztatná a shelljei által használtat; ugyanezek a parancsok a Konténerek menüben is megtalálhatók. A panelhez a `docker` parancssor és Docker 24.0.6 vagy újabb szükséges.

## Markdown

- **Előnézet.** A Markdown-fájl mellette megjelenő előnézettel nyílik meg; a **Nézet › Előnézet megjelenítése/elrejtése mellette** (⇧⌘V) bezárja és megnyitja, a **Beállítások › Általános** pedig kikapcsolja az automatikus előnézetet. Az előnézet megjeleníti a képleteket, a Mermaid-, Graphviz- és PlantUML-diagramokat és a mappa képeit; a webes képek csak megbízható mappában töltődnek be, vagy ha az előnézetben kéri őket.
- **Formátum menü:** félkövér (⌘B), dőlt (⌘I), áthúzott (⇧⌘X), kód (⌥⇧⌘C), link (⇧⌘K), 1–3. szintű címsor (⌥⌘1–⌥⌘3), felsorolásos és számozott listák, feladatok, idézetek és kódblokkok. Minden parancs ki-be kapcsol, a kijelölésen vagy a sorokon, és egyetlen visszavonási lépés.
- **Beillesztés.** A Wordből vagy weboldalról érkező szöveg Markdownként érkezik, a képei pedig fájlokként a dokumentum mellé; a ⌥⇧⌘V egyszerű szövegként illeszt be.
- **Exportálás.** A **Fájl › Exportálás** PDF-et, egyetlen, a képeit magában foglaló HTML-fájlt vagy Word-dokumentumot ír. A webes képek csak megbízható mappából kerülnek bele.
- **Megnyitás a Splatben.** Ahol a [Splat](https://github.com/sventmueller/splat) telepítve van, a **Fájl › Megnyitás a Splatben**, valamint a fájlfa és a lapok helyi menüje ott nyitja meg a dokumentumot, miután mentette.

## Diktálás

Diktáljon egy AI-lap üzenetmezőjébe vagy a szerkesztőbe: az ottani mikrofon gombbal, a **Szerkesztés › Diktálás indítása** (⌃⌥D) paranccsal vagy az fn billentyűvel (nyomva tartva rögzít, dupla koppintásra tovább rögzít). A beszédfelismerés a [Lowspeak](https://github.com/sventmueller/lowspeak) beszédfelismerése; magát a Lowspeaket nem kell telepíteni.

- **A Beállítások › Diktálás** lapon állítható be a mód és a modell (online az OpenAI vagy a Groq segítségével, helyben a Whisperrel vagy a Parakeettel), a nyelv, az utófeldolgozás, a mikrofon és a saját szókincs.
- **A helyi modellek** a Hugging Face-ről töltődnek le, amikor kiválaszt egyet, folyamatjelzővel és megszakítási lehetőséggel. Azokba a mappákba kerülnek, amelyeket a Lowspeak használ, így mindkét app használja, amit a másik letöltött. Helyi modellel a felvétel ezen a Macen marad.
- **Az online átiratkészítéshez** OpenAI- vagy Groq-kulcs szükséges, amelyet a Dango a saját kulcskarika-bejegyzésében tárol.
- **A Lowspeakből.** Ha a Lowspeak az alapértelmezettől eltérően van beállítva, a Diktálás lap egyszer felajánlja a beállításai átvételét, kérésre a kulcsaiét is; a macOS rákérdez, mielőtt a Dango beolvas egyet (az „Engedélyezés” lehetőséget válassza, ne az „Engedélyezés mindig” lehetőséget).
- **Engedélyek.** Az első diktáláskor a macOS engedélyt kér a mikrofonhoz. Az fn billentyű akkor működik, ha a Dango az aktív app, és a Lowspeak nem fut; ehhez a Kisegítő lehetőségek engedély szükséges, amelyet a Dango az első diktálás után kér, soha nem indításkor.

## Bizalom

Egy mappa programokat futtathat, amint valami elindul benne: Git-hookokat és tárolóbeállításokat, például `core.fsmonitor`, nyelvi szervereket a `node_modules` mappából, ügynökhookokat, `.mcp.json` fájlt. Amikor először nyit meg egy mappát, a Dango megkérdezi, megbízik-e benne.

- **Megbízható:** a Git, a nyelvi szerverek, a hibakeresők, az ügynökök és a CI futnak ehhez a mappához és mindenhez, ami alatta van.
- **Korlátozott:** olvashat és szerkeszthet; a szerkesztő változásjelei a `.git` közvetlen olvasásából származnak. Semmi nem indul el, amit a mappa beállít. A Git-, CI- és AI-panel jelzi ezt, és felajánlja, hogy megbízzon a mappában.

A nem Öntől származó szöveg tartalmazhat olyan utasításokat, amelyek egy ügynök irányítására szolgálnak (prompt injection). Egy terminál kimenete (`@terminal`), egy böngészőoldal (`@browser`), egy ügynöknek átadott CI-napló, egy projekten kívüli fájl és egy weboldalról beillesztett hosszabb szöveg ezért megjelölve kerül tovább: a címke „Külső tartalom” feliratú, az ügynök pedig azzal a megjegyzéssel kapja meg a szöveget, hogy az nem megbízható adat, amelynek utasításait nem követheti. A beépített ügynök weboldalai ugyanezzel a megjegyzéssel érkeznek. A beillesztett szöveg akkor számít weboldalról származónak, ha a böngésző ezt jelzi a vágólapon, ahogy a Safari, a Chrome és a Firefox teszi; a máshonnan származó szöveg és egy néhány szavas rövid kifejezés az Öné marad.

## Homokozó

Maga a Dango App Sandbox nélkül fut, mert minden általa indított program örökölné: a terminálok, a Git, a nyelvi szerverek és az ügynökök nem tudnának működni. Ehelyett minden AI-szálnak van egy homokozókapcsolója, alapértelmezés szerint kikapcsolva. Hogy mit korlátoz, az ügynöktől függ, és a kapcsoló súgócímkéje ezt meg is mondja:

| Ügynök | Bekapcsolt homokozóval |
|---|---|
| Claude | a shellparancsok a Claude saját homokozójában futnak; a fájl- és webes eszközei nem |
| Codex | a parancsok és a módosítások csak a projektbe írnak, hálózat nélkül |
| Beépített ügynök | a fájlok, a webes kérések és a parancsok a projekten és az engedélyezett mappákon belül maradnak, hálózat nélkül (`sandbox-exec`) |
| Antigravity, OpenCode | nincs olyan homokozó, amelyet a Dango bekapcsolhatna |

## Megjelenés és kisegítő lehetőségek

- **A sötét mód** a rendszert követi; a **Kontraszt növelése** és a **Mozgás csökkentése** beállítást figyelembe veszi.
- **A VoiceOver** megnevezi az oldalsávot, a lapokat, az AI-idővonal kártyáit, a Git- és CI-sorokat, a hibakeresőt és a terminál szövegét. A töréspontok soha nem csak színük alapján különböznek.
- **Billentyűzet.** Minden parancsnak van billentyűparancsa, és mindegyik módosítható a `keybindings.json` fájlban; a ⌥⌘\` a következő panelre lép.
- **Jobbról balra.** Arab és héber nyelven az ablak tükrözve jelenik meg, a kódnézetek viszont balról jobbra maradnak.

Hogy a Dango területenként mit tesz hozzá az AppKithez, az [ACCESSIBILITY.md](../../../ACCESSIBILITY.md) fájlban olvasható.

## Nyelvek

A felület 35 nyelvre van lefordítva: arab, katalán, kínai (egyszerűsített), kínai (hagyományos), horvát, cseh, dán, holland, angol, finn, francia, német, görög, héber, hindi, magyar, indonéz, olasz, japán, koreai, maláj, norvég, lengyel, portugál (Brazília), portugál (Portugália), román, orosz, szlovák, szlovén, spanyol, svéd, thai, török, ukrán és vietnami.

A Dango a **Rendszerbeállítások › Általános › Nyelv és régió** nyelvi sorrendjét követi; ugyanott az **Alkalmazások** alatt a Dangónak külön nyelvet is választhat. Ha az Ön nyelvére nincs fordítás, angolt használ. A kifejezések egyetlen szójegyzéket, a [docs/glossary.md](../../../docs/glossary.md) fájlt, valamint az Apple saját fordításait követik, így a Dango úgy szól, mint a Mac többi része.

## Billentyűparancsok

| Művelet | Billentyűparancs |
|---|---|
| Mappa megnyitása, új ablak, ablak bezárása | ⌘O, ⇧⌘N, ⇧⌘W |
| Új fájl, mentés, mentés másként | ⌘N, ⌘S, ⇧⌘S |
| Új lap, lap bezárása, következő és előző lap | ⌘T, ⌘W, ⇧⌘] és ⇧⌘[ |
| Gyors megnyitás, keresés a projektben | ⌘P, ⇧⌘F |
| Fájlok, Keresés, Ügynökök, Git, Tesztek, Hibakeresés, CI, Konténerek | ⌘1–⌘8 |
| Oldalsáv megjelenítése vagy elrejtése, következő panel | ⌃⌘S, ⌥⌘\` |
| Vissza és előre a szerkesztett helyek között | ⌃- és ⌃⇧- |
| Megjegyzés be/ki, ugrás sorra | ⌘/, ⌃G |
| Következő előfordulás, kurzor felette és alatta | ⌘D, ⌃⇧↑ és ⌃⇧↓ |
| Összecsukás, kibontás, összes összecsukása, összes kibontása | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Sortördelés, inspektor | ⌥Z, ⌥⌘I |
| Kiegészítés, információ rámutatáskor, ugrás definícióra | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Hivatkozások, hívási hierarchia, átnevezés | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Szimbólumok a fájlban, a projektben | ⇧⌘O, ⌥⌘O |
| Gyorsjavítás, formázás | ⌥⌘A, ⌥⇧F |
| Következő és előző probléma | ⌘' és ⇧⌘' |
| Markdown-előnézet, beillesztés egyszerű szövegként | ⇧⌘V, ⌥⇧⌘V |
| Félkövér, dőlt, áthúzott, kód, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| 1–3. szintű címsor | ⌥⌘1–⌥⌘3 |
| Új terminál, terminál törlése | ⌃\`, ⌘K |
| Fókusz az ügynök üzenetmezőjére, megjegyzés egy tervhez | ⌘Esc, ⌥⌘K |
| Címmező, újratöltés, vissza, előre (böngésző) | ⌘L, ⌘R, ⌘[, ⌘] |
| Diktálás indítása vagy leállítása | ⌃⌥D |
| Töréspont, összes töréspont be- vagy kikapcsolása | ⌘\\, ⌘Y |
| A kurzornál lévő teszt futtatása, a legutóbbi tesztek újrafuttatása | ⌃⌥⌘U, ⌃⌥⌘G |
| Folytatás, átlépés, belépés, kilépés, leállítás | ⌃⌘Y, F6, F7, F8, ⌘. |
| Beállítások | ⌘, |

Az app adatmappájában lévő `keybindings.json` (lásd: [Hol tárolódnak az adatok](#hol-tárolódnak-az-adatok)) bármelyiket módosíthatja.

## Adatvédelem és biztonság

- **Nincs telemetria,** nincs analitika, és sehová nem megy összeomlási jelentés. A hibák a rendszernaplóba kerülnek (Konzol app, az app nevénél).
- **Hálózat csak ahhoz, amit Ön indít el:** a használt ügynökök, mindegyik a saját szolgáltatásával; az ACP-jegyzék, valamint az ügynökök, nyelvi szerverek, hibakereső adapterek és diktálási modellek letöltése, amikor kéri őket; azok a CI-hosztok, amelyekre bejelentkezik; a Copilot szerkesztési előrejelzései, ha azt választotta; a beépített ügynök szolgáltatói; az online átiratkészítés, ha azt választotta; a böngészőlapon megnyitott oldalak; és a frissítések keresése, miután engedélyezte.
- **Kód csak ott hagyja el a Macet,** ahol ehhez szolgáltatást választott: egy ügynök, a Copilot előrejelzései, egy AI által írt commitüzenet. A commitüzenethez szükséges különbségek csak akkor mennek ki, ha a gitleaks nem talált bennük titkos adatot.
- **A kulcsok és tokenek** a macOS kulcskarikájában maradnak, soha nem egy beállításfájlban. Más appok kulcsait, például a Lowspeakéit, csak az Ön beleegyezésével olvassa be, és csak egyszer.
- **Rögzített letöltések.** A Dango által telepített ügynökök, nyelvi szerverek és hibakereső adapterek tesztelt verziókhoz vannak rögzítve, és az ellenőrzőösszegük alapján vannak ellenőrizve.
- **Kódaláírás** Hardened Runtime-mal; frissítések csak az app saját aláírásával települnek.

## Hol tárolódnak az adatok

Az `<id>` a csomagazonosító: `local.agentide` a kiadáshoz (`local.agentide.debug` a hibakereső buildekhez). Nem tartalmazza a megjelenített nevet, így átnevezéskor minden adat megmarad.

| Hely | Mi |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (az AI-panel listája), `agent-threads/` (a beépített ügynök beszélgetései), `layouts/` (lapok projektenként), `recovery/` (mentetlen szerkesztők összeomlás után), `checkpoints/` (Git nélküli mappákhoz), `worktrees/`, `agents/` és `tools/` (telepített ügynökök, nyelvi szerverek és hibakereső adapterek), `task-output/` |
| `~/Library/Caches/<id>/` | beillesztett képek, shellintegrációs szkriptek, az ACP-jegyzék; törölhető |
| `~/Library/Logs/<id>/dictation/` | a diktálás naplófájljai |
| `~/Library/Application Support/Lowspeak` és `FluidAudio` | helyi diktálási modellek, a Lowspeakkel közösen |
| `~/Library/WebKit/<id>/` | a böngészőlap cookie-jai és webhelyadatai |
| `<project>/.dango/settings.json` | a projekt saját beállításai |
| a projekt Git-tárolója | visszaállítási pontok, commitokként a `refs/ide-checkpoints/` alatt; egy szokásos push vagy fetch soha nem viszi magával őket |
| kulcskarika, `<id>` szolgáltatás | a beépített ügynök és a diktálás API-kulcsai, CI-tokenek |
| `~/.local/bin/dango` | a terminálparancs, ha telepítette |

A Claude a saját munkameneteit a `~/.claude/projects` alatt tárolja; a Dango beolvassa őket egy munkamenet folytatásához, de nem módosítja őket.

## Ismert korlátok

- **Nincs App Sandbox** magához az apphoz, mivel a terminálok, a Git, a nyelvi szerverek és az ügynökök örökölnék; egy mappában meg kell bízni, mielőtt bármi lefutna, amit beállít. Az Antigravity és az OpenCode nem rendelkezik olyan homokozóval, amelyet a Dango bekapcsolhatna.
- **A Claude használati adatai és kontextuslebontása** a protokolljának kísérletiként jelölt részeire támaszkodnak, és rejtve maradnak, ha ezek hiányoznak.
- **A beszélgetés visszatekerése** csak a Claude esetében működik; a többi ügynöknél csak a kód tekerhető vissza.
- **A CI-be a böngészőn keresztül történő bejelentkezés** ki van kapcsolva; használja a `gh` vagy a `glab` parancsot, egy tokent vagy a környezetet.
- **A Gemini CLI nem érhető el:** a Google megszüntette a személyes fiókkal való bejelentkezést, és az Antigravityt ajánlja. A korábbi Gemini-szálak továbbra is olvashatók.

## Fordítás forráskódból

Amire szüksége van:

- macOS 14 vagy újabb az Xcode Command Line Toolsszal (`xcode-select --install`); magára az Xcode-ra nincs szükség.
- Rust, ahogyan a `rust-toolchain.toml` megadja. A Homebrew `rust` csomagja ennek a Macnek a processzorára fordít; egy kiadás univerzális buildjéhez rustup kell az `aarch64-apple-darwin` és az `x86_64-apple-darwin` céllal.
- A [splat](https://github.com/sventmueller/splat) és a [lowspeak](https://github.com/sventmueller/lowspeak) kicsekkolt példánya e mellett (`../splat`, `../lowspeak`), mindkettő a `main` ágon; a munkaterület útvonal alapján vesz át belőlük crate-eket.
- cmake minden diktálással készülő buildhez (`make run`, `make build`, `make release`), amely lefordítja a whisper.cpp-t. A `make check` nélküle fordít.

| Parancs | Mit csinál |
|---|---|
| `make run` | lefordítja a hibakereső appot (`target/debug/Dango Debug.app`), és megnyitja |
| `make build` | lefordítja a kiadási appot (`target/release/Dango.app`), egy helyi tanúsítvánnyal aláírva, amelyet az első build hoz létre a bejelentkezési kulcskarikában; megnöveli a buildszámot a `VERSION` fájlban |
| `make lint` | formázás és clippy; a figyelmeztetések hibát okoznak |
| `make check` | `lint`, majd az összes teszt; nem kell hozzá kulcskarika, ablak vagy hálózat |
| `make test` | `check`, majd a `probe` öntesztje és a kisegítő lehetőségek auditja, balról jobbra és jobbról balra, a valódi appban |
| `make perf` | az időkeretek, kiadási buildben; csak egy egyébként tétlen Macen van értelme |
| `make release` | a kiadás próbafuttatása a `target/release-dry-run/` mappába, a helyi tanúsítvánnyal aláírva; semmit nem küld sehová |
| `make release-notarized` | `check`, majd maga a kiadás a `dist/` mappába: Developer ID-aláírás, hitelesítés, az app saját kulcsával aláírt frissítési hírcsatorna; megnöveli a buildszámot |

A hibakereső és a kiadási változat külön appok, saját beállításokkal és adatokkal. A fordítást, a tesztelést, az önteszteket, a kiadást és a kódra vonatkozó szabályokat a [docs/development.md](../../../docs/development.md) írja le, a mögöttük álló döntéseket a [docs/plan.md](../../../docs/plan.md). A kiadási megjegyzések a [RELEASE_NOTES.md](../../../RELEASE_NOTES.md) fájlban találhatók, lefordítva minden README mellett az `assets/readme/<language>/` mappában.

## Licenc

A Dango licence az alábbiak bármelyike

- Apache-licenc, 2.0-s verzió ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-licenc ([LICENSE-MIT](../../../LICENSE-MIT))

az Ön választása szerint.

Hacsak Ön kifejezetten másként nem nyilatkozik, a Dangóba való felvételre szándékosan benyújtott minden hozzájárulás, az Apache-2.0 licenc meghatározása szerint, a fentiek szerint kettős licenc alatt áll, további feltételek nélkül.
