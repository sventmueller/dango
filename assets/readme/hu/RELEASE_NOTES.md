## Memória
- A hosszú ügynök-munkamenetek sokkal kevesebb memóriát igényelnek:
  - Egy korábbi munkamenet megnyitásakor az átirata soronként töltődik be; egy 100 MB-os átiratú munkamenet csúcsigénye
    most körülbelül 20 MB a korábbi 65 MB helyett.
  - Egy befejezett eszközhívás csak azt őrzi meg, amit a kártyája mutat: a kimenete első 400 sorát, a nyers bemenetet
    pedig már nem, amint a kártya megjeleníti a mezőit. A „További N sor” továbbra is minden sort megszámol.
  - Az az AI-lap, amely 30 másodpercig nem látható, elengedi a kártyáit, és amikor újra megjelenik, a látható kártyákat
    építi fel, ugyanott, ahol tartott.
  - Egy lap bezárása vagy egy korábbi munkamenet megnyitása után a Dango visszaadja a felszabadított memóriát a
    macOS-nek.
- A listák frissítéskor már nem nőnek a memóriában. A macOS 27 alatt egy kinyitható sorokat tartalmazó lista minden
  frissítése végleg hátrahagyott egy kibontási háromszöget, és minden frissítés hátrahagyott egy keveset minden sorból:
  - A CI, a Konténerek, a Software Factory, a Tesztek, a Hibakeresés, a Git és a Keresés listája, valamint a hívási
    hierarchia maguk rajzolják a nyilaikat.
  - A nem látható listák már nem frissülnek; amikor megjelennek, bepótolják.
  - A Munkamenetek, a CI, a Software Factory és a Konténerek lista megtartja a változatlan sorokat, a Git pedig csak
    azoknak a fájloknak a sorait frissíti, amelyek állapota megváltozott.
- Egy bezárt ablak panelei vele együtt felszabadulnak.

## AI-lapok
- Az olyan háttérparancsnál, amely maga írja a kimenetét egy fájlba, például `make > build.log 2>&1`, a „Kimenet
  megjelenítése” ezt a fájlt mutatja, ha a CLI-nek semmi sincs meg a kimenetből.
