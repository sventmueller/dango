## Sebesség és memória
- Minden, amit egy ügynök elindít, bármilyen nyelven, például buildek, tesztek, shellek és MCP-szerverek, a macOS
  háttérállapotában fut: a legalacsonyabb prioritással, visszafogott lemez- és hálózati hozzáféréssel. Maguk az
  ügynökök, a Claude alügynökeivel együtt, közepes prioritással futnak. A teljes terheléssel dolgozó ügynökök már nem
  lassítják le a Dangót; ez a Software Factory ügynökeire is érvényes.
- A visszaállított ablakok csak azt indítják el, ami látszik: a többi lap mögött lévő AI-lap akkor indítja el a CLI-jét,
  amikor előtérbe kerül vagy promptot kap, a böngésző-, weboldal-előnézeti és terminállapok pedig akkor töltik be az
  oldalukat vagy indítják el a shelljüket, amikor először megjelennek.
- Egy hosszú projektkeresés futás közben már nem rajzolja újra az összes találatát. A Software Factory, a CI-naplók és a
  Tesztek panel kevesebbet dolgozik, amíg nem látható; a tesztek és a hibakeresési konfigurációk akkor töltődnek be,
  amikor a paneljük először megjelenik, a rejtett CI és Konténerek panel pedig semmit sem olvas be, amíg meg nem
  jelenik.
- A hosszú munkamenetek kevesebb memóriát igényelnek: egy befejezett különbségkártya csak azokat a sorokat tartja meg,
  amelyeket mutat, egy megválaszolt engedélykérés felszabadítja a bemenetét, a terminál glifgyorsítótára már nem nő
  korlátlanul, a Fájlok és a Git panel pedig felszabadítja a már nem létező sorokat.
- Maga a program mintegy 40%-kal kisebb.

## Ablakok és menük
- Egy ablaktábla bezárásakor a lapjai nem záródnak be, hanem átkerülnek az előtte lévő ablaktáblába, ha pedig az nincs,
  az utána lévőbe. Csak egy előnézet ablaktáblája és az utolsó ablaktábla záródik be a lapjaival együtt.
- A Nézet menü a paneleket az oldalsáv ikonjainak sorrendjében sorolja fel.
- Az „Ügynökök kezelése…” kikerült a + menüből; az Ügynökök panelen a „+ Új munkamenet” alatt továbbra is megtalálható.

## Javítások
- A Keresés panelen a „Fájlok belefoglalása és kizárása” nyila mellett már nem jelenik meg egy oda nem illő betű.
- Egy specifikáció megjegyzéseket tartalmazó felugró ablakának bezárása már nem okoz összeomlást.
