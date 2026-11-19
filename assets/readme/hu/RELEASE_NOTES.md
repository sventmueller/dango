## AI-lapok
- Az az AI-lap, amely három percig nem látható, leállítja a Claude-folyamatát, amíg a munkamenet tétlen. A beszélgetés
  és a beírt szöveg a lapon marad, a következő prompt pedig folytatja a munkamenetet. A lap megtartja a folyamatát,
  amíg Claude dolgozik, egy prompt várakozik, egy kérdés nyitva van, egy alügynök, háttérfeladat vagy parancs még fut,
  vagy be van kapcsolva a Remote Control.
- Egy ügynök leállítása azokat a parancsokat is leállítja, amelyeket saját folyamatcsoportban vagy munkamenetben
  indított, és amelyek korábban túlélhették az ügynököt.
- Az idővonal a végéhez fűzi az új bejegyzéseket ahelyett, hogy az összes korábbi sort újraindexelné.

## Sebesség
- A visszaállított ablakok gyorsabban nyílnak meg: a mentetlen módosítások nélküli fájllap csak akkor tölti be a
  fájlját, amikor előtérbe kerül.
- Az a projektkeresés, amelynek panelje rejtve van, szünetel, és folytatódik, amikor a panel megjelenik.
- A szerkesztő Git-módosításjelölői fájlonként egyszerre legfeljebb egy összehasonlítást futtatnak; a közben végzett
  szerkesztések egyetlen következő összehasonlításba olvadnak össze, a fájl commitolt szövege pedig csak akkor töltődik
  be újra, ha a commit megváltozott.
- Az összecsukott kódot tartalmazó fájl minitérképe soronként egy bejegyzés helyett kibontott szakaszonként egy
  bejegyzést tart meg.

## Keresés
- A szűrők nyila egy vonalba esik a fölötte lévő csere nyilával.
