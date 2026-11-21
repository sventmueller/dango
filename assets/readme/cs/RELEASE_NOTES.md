## Zabezpečení
- Odkazy v odpovědích, plánech a zprávách agenta, v náhledu Markdownu a v PDF už jedním kliknutím neotevřou libovolnou
  aplikaci. Webové adresy se otevřou v panelu prohlížeče, soubory v projektu v panelu editoru; vše ostatní, například
  soubor jinde, síťové sdílení nebo odkaz jiné aplikace, se nejdřív zeptá a ukáže, kam vede.
- Adresy pro přihlášení a Remote Control, které předá agent nebo server MCP, se otevřou jen tehdy, když jsou to webové
  adresy.
- Složkám, které obsahují mnohem víc než jeden projekt, například domovské složce, Ploše, Dokumentům, Staženým souborům
  nebo kořeni svazku, už nelze důvěřovat jako celku; důvěřujte jejich projektům jednotlivě. Dotaz na důvěru teď uvádí,
  že se vztahuje i na složky uvnitř.
- Tokeny CI a přihlášení už nikdy nejdou přes nešifrované http na jiný server než tento Mac; repozitáře, jehož vzdálený
  repozitář začíná na `http://`, se aplikace ptá přes https.
- Bez gitleaks čekají předpovědi, než kód odejde k poskytovateli mimo tento Mac, dokud nezvolíte odeslání bez kontroly.
  Vlastní nastavení gitleaks v projektu už kontrolu nevypne.
- Vlastního agenta už nelze odkazem, který se objeví až po schválení jeho zápisu, přimět k zápisu mimo projekt, a
  soubory s dalšími pevnými odkazy dostanou vlastní soubor, místo aby se zapisovalo přes ně.
- Doplňování jazykového serveru už nemůže zablokovat editor a jeho přejmenování a mazání odkazovaného souboru působí na
  odkaz, ne na jeho cíl.
- Obrázky, které připojíte k promptu, dekóduje izolovaná dekódovací služba, stejně jako všude jinde.
- Program v terminálu může nahradit schránku jen tehdy, když je jeho terminál vpředu, a terminál to oznámí.
- rust-analyzer se instaluje z vydání pevně určeného i s kontrolním součtem, ne z toho, které je právě nejnovější.

## Hledání a navigace
- Rychle otevřít oznámí, když žádný soubor neodpovídá, a ukáže, zda hledá soubory, nebo symboly projektu.
- Symboly projektu zahrnují TypeScript a JavaScript, i když není otevřený žádný jejich soubor. Přesné výsledky z
  projektu a v jazyce, ve kterém pracujete, jsou první; symboly z knihoven a nainstalovaných balíčků jsou označené.
- Zobrazení › Hledání přesune fokus do vyhledávacího pole.

## Editor
- Editor si ponechá fokus, když se soubor Markdown otevře s náhledem vedle, takže nabídka Formát funguje hned. Příkazy
  editoru fungují i tehdy, když je fokus v bočním panelu.
- Návrh na konci řádku nebo v rámečku pod ním říká, že ho klávesa Tab přijme.
- Stejnojmenné panely v přehledu panelů jsou rozlišitelné, například soubor a jeho náhled v prohlížeči.
- Nabídka Zobrazení už nenabízí položku „Skrýt řádek panelů“ systému macOS, kterou panely aplikace Dango nepoužívají.

## Jazyky a ladění
- Server JSON teď kontroluje soubory a servery, které své diagnostiky registrují až po spuštění, například basedpyright
  a sourcekit-lsp, je zobrazují.
- Tam, kde pip odmítá instalovat do Pythonu, jako u Pythonu z Homebrew, nabídne ladicí nástroj instalaci debugpy do
  virtuálního prostředí projektu.
