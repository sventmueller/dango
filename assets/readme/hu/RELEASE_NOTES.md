<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Ez a Dango első kiadása: natív macOS-szerkesztő a kódoló ügynökökkel való munkához. A szerkesztő, a terminálok, egy
böngésző, a Git, a CI, egy hibakereső és az AI-szálak mappánként egy közös ablakban kapnak helyet. Az app Rust nyelven,
AppKitre épül; WebKit csak a böngészőlapon jelenik meg. A felület 35 nyelven érhető el.

## Követelmények
- macOS 14 vagy újabb, Apple chippel vagy Intel processzorral.
- Az AI-szálakhoz a választott ügynök: a Claude-hoz a bejelentkezett `claude` parancs kell; a Codexhez Node.js kell, és
  kérésre az app telepíti a rögzített verziójú csomagját; az Antigravity és az OpenCode a saját `agy` vagy `opencode`
  parancsát használja, vagy egy rögzített verziójú letöltést; a beépített ügynökhöz API-kulcs kell, vagy Ollama, illetve
  LM Studio ezen a Macen.
- A nyelvi szervereket és a hibakereső adaptereket kérésre telepíti az app. A gopls és a `dlv` Go-t igényel; a
  TypeScript-, Python-, JSON-, YAML-, CSS-, HTML- és shellszerverekhez, valamint a JavaScript-hibakeresőhöz Node.js
  kell; a clangd, a sourcekit-lsp és az `lldb-dap` az Xcode Command Line Tools része.
- A szerkesztési előrejelzésekhez GitHub Copilot-előfizetés (az ingyenes is megfelel), vagy egy fill-in-the-middle
  modell kell az Ollamában vagy az LM Studióban.

## Ablak
- Minden fájl, terminál, böngészőoldal, AI-szál, különbségnézet és előnézet a mappa ablakának egy-egy macOS-lapja. A
  lapsor végén lévő „+” és a ⌘T új fájlt, terminált, a böngészőt és minden telepített ügynököt felkínál, saját
  Git-worktree-ben is.
- Az oldalsáv a Fájlok, Keresés, Ügynökök, Git, Hibakeresés és CI nézetek között vált (⌘1–⌘6); jobb kattintással
  elrejthetők a nem használt nézetek. Az eszközsoron található a vissza és előre lépés a szerkesztett helyek között (⌃-
  és ⌃⇧-), az ág, a Gyors megnyitás és a Tevékenység áttekintése.
- A „Megnyitás mellette” egy második dokumentumot jelenít meg az első mellett. Újraindítás vagy összeomlás után minden
  ablak visszatér a lapjaival, a nem mentett szöveget tartalmazó szerkesztők pedig ezzel a szöveggel.
- A képek, hang- és videofájlok, betűtípusok és Office-dokumentumok a Quick Look segítségével nyílnak meg lapon, a
  PDF-ek a PDFKit segítségével.

## Szerkesztő
- Szintaxisszínezés, sorszámok, változásjelek az utolsó commithoz képest, keresés és csere, Gyors megnyitás (⌘P) és
  keresés a projektben (⇧⌘F).
- Automatikus behúzás, tabulátorok vagy szóközök a `.editorconfig` alapján, zárójelpárok, megjegyzés be- és
  kikapcsolása, kódrészek összecsukása, ugrás sorra, látható szóközök és sortördelés.
- Több kurzor: ⌥-kattintás, ⌥-húzás oszlophoz, ⌘D a következő előforduláshoz.
- A fájlok megőrzik a kódolásukat, a bájtsorrend-jelzőjüket és a sorvégeiket. A lemezen módosult fájl újratöltődik, ha a
  szerkesztőben nincs mentetlen szöveg; ellenkező esetben egy sáv felkínálja a különbségeket és az összefésülést.
- **Szerkesztési előrejelzések:** gépelés közben szürkén megjelenik egy javaslat; a Tab elfogadja. A javaslatok a GitHub
  Copilottól vagy egy helyi modelltől érkeznek az Ollamában vagy az LM Studióban, aszerint, hogy mit választ a
  beállításokban; helyi modellel a kód ezen a Macen marad.

## Nyelvi szerverek
Go, Rust, TypeScript és JavaScript, Python, C, C++ és Objective-C, Swift, shellszkriptek, JSON, YAML, CSS, HTML, TOML,
Markdown és Lua, valamint saját szerverek a beállításokból. Diagnosztika, információ rámutatáskor, kiegészítés,
aláírássúgó, ugrás definícióra, típusdefinícióra és implementációra, hivatkozások, hívási és típushierarchia, átnevezés,
formázás, szemantikus színek, összecsukás és vázlat; típusok és paraméternevek a soron belül; gyorsjavítások és
refaktorálások egy villanykörte mögött (⌥⌘A); a projekt szimbólumai a ⌥⌘O billentyűkkel vagy `#` jellel a Gyors
megnyitásban. Egy TypeScript-projekt saját `tsc` fordítóját használja az app, ha az nyelvi szerverként működni tud.

## Hibakereső
Töréspontok, feltételekkel is, léptetés, szálak a hívási vermükkel, változók és figyelt kifejezések az Inspektorban
(⌥⌘I), értékek a kód mellett, amíg a program szüneteltetve van, és egy hibakeresési konzol, amely a szüneteltetett
keretben értékel ki. Rust, C, C++ és Swift az `lldb-dap`, Go a `dlv`, JavaScript és TypeScript a JavaScript-hibakereső,
Python a debugpy segítségével. Az app beolvassa a projekt `.vscode/launch.json` fájlját. A Hibakeresés menü az Xcode
billentyűparancsait használja.

## Terminál
Terminálok lapokon, a bejelentkezési shell környezetével, Metallal rajzolva. Kép beillesztésekor vagy behúzásakor a kép
a terminálban futó kódoló ügynökhöz kerül, vagy egy mentett másolat útvonala illesztődik be. A ⌘-kattintás megnyitja a
linkeket és a fájlneveket. A terminálból indított fejlesztői szerver megnyitható a böngészőlapon.

## Markdown
A Markdown-fájl mellette megjelenő előnézettel nyílik meg (⇧⌘V), képletekkel, Mermaid-, Graphviz- és
PlantUML-diagramokkal és a mappa képeivel. Formátum menü félkövér, dőlt, linkek, címsorok, listák, idézetek és
kódblokkok számára; a Wordből vagy weboldalról beillesztett szöveg Markdownként érkezik. A **Fájl › Exportálás** PDF-et,
egyetlen HTML-fájlt vagy Word-dokumentumot ír. Ha a Splat telepítve van, a **Fájl › Megnyitás a Splatben** ott nyitja
meg a dokumentumot.

## Böngésző
Böngészőlap címmezővel, vissza és előre lépéssel, kereséssel, előzetes kérdéssel induló letöltésekkel és a Web
Inspectorral.

## Git
Git-panel fájlok vagy egyes blokkok előkészítésével és elvetésével, commitokkal, commitolással és pushsal vagy
szinkronizálással, ágakkal, stashekkel, összefésülési ütközésekkel és a legutóbbi commitok gráfjával. A Blame
megmutatja, ki módosította utoljára a kurzor alatti sort, vagy egy oszlopot az egész fájlhoz. Egy gomb AI-jal írja meg a
commitüzenetet; a különbségek csak akkor mennek ki, ha a gitleaks nem talált bennük titkos adatot, és minden commit is
ellenőrzésen esik át titkos adatok után kutatva. Egy AI-szál dolgozhat saját Git-worktree-ben; a panel megmutatja, mit
módosított, és összefésüléssel vagy cherry-pickkel átveszi a munkát.

## CI
GitHub Actions és GitLab CI, GitHub Enterprise Server és saját üzemeltetésű GitLab is: az aktuális ág futásai, jobjai és
naplói; sikertelen jobok, egyetlen job vagy (GitHubon) egy teljes futás újraindítása, kézi jobok indítása vagy várakozó
telepítések jóváhagyása, új pipeline vagy kézzel indítható workflow indítása, valamint megszakítás. A nyitott merge
requestek, illetve pull requestek és az issue-k ugyanabban a panelben jelennek meg, és a böngészőlapon nyílnak meg. Egy
sikertelen job naplója átadható egy ügynöknek, külső szövegként megjelölve. A token a kulcskarikából, a `gh` vagy a
`glab` parancsból, a bejelentkezési shell környezetéből származik, vagy kézzel adható meg.

## AI
Minden ügynök AI-lapon dolgozik, ugyanazzal az idővonallal: válaszok, eszközhívások a kimenetükkel, engedélykérések,
kérdések, jóváhagyható és véleményezhető tervek, teendők, alügynökök és háttérfeladatok; a lap fejlécében két gomb
listázza a futó alügynököket és háttérfolyamatokat. Minden prompt előtt az app visszaállítási pontot ment; a változások
üzenetenként átnézhetők és visszatekerhetők. Az új szálak „Kézi” módban indulnak, a romboló Git-parancsok pedig soha nem
kapnak tartós engedélyt. A Tevékenység áttekintése mindent felsorol, ami fut. A lapon egy zöld pont a dolgozó
munkamenetet, egy piros az Önre váró munkamenetet jelzi, és értesítés szól, amikor egy munkamenet befejeződik vagy
várakozik, miközben Ön máshol dolgozik.
- **Claude** a változatlan `claude` paranccsal, munkamenetekkel, alügynökökkel, háttérfeladatokkal, kontextussal és
  használati adatokkal.
- **ACP-ügynökök:** Codex, Antigravity és OpenCode, kérésre továbbiak az ACP-jegyzékből; a telepített ügynökök tesztelt
  verziókhoz vannak rögzítve, és az ellenőrzőösszegük alapján vannak ellenőrizve.
- **Beépített ügynök:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio és az OpenAI vagy az Anthropic
  API-ját beszélő szerverek, eszközökkel fájlokhoz, kereséshez, parancsokhoz, diagnosztikához és webes kérésekhez; a
  kulcsok a kulcskarikában maradnak.
- A szálanként állítható homokozókapcsoló korlátozza, mit tehetnek az ügynök parancsai, amennyiben az ügynök ezt
  támogatja.
- A külső szöveg, például terminálkimenet, weboldal vagy CI-napló, nem megbízhatóként megjelölve jut el az ügynökhöz.

## Diktálás
Diktálás az AI-üzenetmezőbe és a szerkesztőbe a Lowspeak beszédfelismerésével: a mikrofon gombbal, a ⌃⌥D billentyűkkel
vagy az fn billentyűvel, amíg a Dango az aktív app. A Lowspeak beállításait és modelljeit használja; az online
átiratkészítéshez OpenAI- vagy Groq-kulcs szükséges.

## Frissítések
Az app a Sparkle segítségével frissíti magát. A második indításkor megkérdezi, hogy önállóan kereshet-e frissítéseket;
amíg nem egyezik bele, nem küld kérést. A **Dango › Frissítések keresése…** azonnal ellenőriz.

## Kisegítő lehetőségek
VoiceOver-nevek az oldalsávhoz, a lapokhoz, az AI-idővonal kártyáihoz, a Git- és CI-sorokhoz, a hibakeresőhöz és a
terminál szövegéhez; teljes billentyűzetes vezérlés a `keybindings.json` fájlban módosítható billentyűparancsokkal;
Kontraszt növelése és Mozgás csökkentése; a kódnézetek jobbról balra író felületeken is balról jobbra futnak. A
részletek az ACCESSIBILITY.md fájlban találhatók.

## Adatvédelem
Nincs telemetria, nincs analitika és nincsenek összeomlási jelentések. Az app csak azért kapcsolódik, amit Ön indít el:
a használt ügynökök, a kért letöltések, azok a CI-hosztok, amelyekre bejelentkezik, a Copilot előrejelzései, ha azt
választotta, az online átiratkészítés, ha azt választotta, a megnyitott oldalak, valamint a frissítések keresése, miután
engedélyezte. A kulcsok és tokenek a macOS kulcskarikájában maradnak.

## Ismert korlátok
- A CI-be a böngészőn keresztül történő bejelentkezés ki van kapcsolva; használja a `gh` vagy a `glab` parancsot, egy
  tokent vagy a környezetet.
- Az app App Sandbox nélkül fut, mivel a terminálok, a Git, a nyelvi szerverek és az ügynökök örökölnék; egy mappában
  beállított dolgok csak akkor futnak, ha megbízik a mappában. Az Antigravity és az OpenCode nem rendelkezik olyan
  homokozóval, amelyet az app bekapcsolhatna.
- A Claude használati adatai és kontextuslebontása a protokolljának kísérletiként jelölt részeire támaszkodnak, és
  rejtve maradnak, ha ezek hiányoznak. A beszélgetés visszatekerése csak a Claude esetében működik; a többi ügynöknél
  csak a kód tekerhető vissza.
- A Gemini CLI nem érhető el: a Google megszüntette a személyes fiókkal való bejelentkezést, és az Antigravityt ajánlja.
  A korábbi Gemini-szálak továbbra is olvashatók.
