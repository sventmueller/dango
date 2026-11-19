## Paměť
- Dlouhé relace agentů potřebují mnohem méně paměti:
  - Při otevření dřívější relace se její přepis čte po řádcích; relace s přepisem o velikosti 100 MB teď ve špičce
    zabere asi 20 MB místo 65 MB.
  - Dokončené volání nástroje si ponechá jen to, co ukazuje jeho karta: prvních 400 řádků výstupu, a jakmile karta
    zobrazí svá pole, už ani surový vstup. „Další řádky: N“ dál počítá každý řádek.
  - Panel AI, který 30 sekund není vidět, uvolní své karty, a když se znovu zobrazí, sestaví ty viditelné, na místě, kde
    byl.
  - Po zavření panelu nebo otevření dřívější relace vrátí Dango uvolněnou paměť systému macOS.
- Seznamy už při obnovování nerostou v paměti. V macOS 27 nechávala každá obnova seznamu s rozbalovacími řádky natrvalo
  za sebou rozbalovací trojúhelník a každá obnova po sobě nechala kousek každého řádku:
  - Seznamy CI, Kontejnery, Software Factory, Testy, Ladit, Git a Hledání i hierarchie volání kreslí své šipky samy.
  - Seznamy, které nejsou vidět, se už neobnovují; doženou to, jakmile se zobrazí.
  - Seznamy Relace, CI, Software Factory a Kontejnery si ponechávají řádky, které se nezměnily, a Git obnovuje jen řádky
    souborů, jejichž stav se změnil.
- Podokna zavřeného okna se uvolní spolu s ním.

## Panely AI
- „Zobrazit výstup“ u příkazu na pozadí, který svůj výstup sám zapisuje do souboru, například `make > build.log 2>&1`,
  zobrazí tento soubor, když CLI nemá nic z výstupu.
