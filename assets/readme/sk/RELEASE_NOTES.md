## Pamäť
- Dlhé relácie agentov potrebujú oveľa menej pamäte:
  - Pri otvorení staršej relácie sa jej prepis číta po riadkoch; relácia s prepisom s veľkosťou 100 MB teraz v špičke
    zaberie asi 20 MB namiesto 65 MB.
  - Dokončené volanie nástroja si ponechá iba to, čo ukazuje jeho karta: prvých 400 riadkov výstupu, a len čo karta
    zobrazí svoje polia, už ani surový vstup. Text „ďalšie riadky: N“ naďalej počíta každý riadok.
  - AI tab, ktorý 30 sekúnd nie je vidieť, uvoľní svoje karty, a keď sa znova zobrazí, zostaví tie viditeľné, na mieste,
    kde bol.
  - Po zatvorení tabu alebo otvorení staršej relácie vráti Dango uvoľnenú pamäť systému macOS.
- Zoznamy pri obnovovaní už nerastú v pamäti. V macOS 27 každé obnovenie zoznamu s rozbaľovacími riadkami natrvalo
  zanechalo rozbaľovací trojuholník a každé obnovenie zanechalo kúsok z každého riadka:
  - Zoznamy CI, Kontajnery, Software Factory, Testy, Ladenie, Git a Vyhľadávanie aj hierarchia volaní si kreslia vlastné
    šípky.
  - Zoznamy, ktoré nie je vidieť, sa už neobnovujú; doženú to, keď sa zobrazia.
  - Zoznamy Relácie, CI, Software Factory a Kontajnery si ponechávajú riadky, ktoré sa nezmenili, a Git obnovuje iba
    riadky súborov, ktorých stav sa zmenil.
- Panely zatvoreného okna sa uvoľnia spolu s ním.

## AI taby
- „Zobraziť výstup“ pri príkaze na pozadí, ktorý svoj výstup sám zapisuje do súboru, napríklad `make > build.log 2>&1`,
  zobrazí tento súbor, keď CLI nemá nič z výstupu.
