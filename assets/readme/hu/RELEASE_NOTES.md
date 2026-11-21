## Biztonság
- Az ügynök válaszaiban, terveiben és jelentéseiben, a Markdown-előnézetben és a PDF-ekben lévő hivatkozások már nem
  nyitnak meg egyetlen kattintással bármilyen appot. A webcímek böngészőlapon nyílnak meg, a projekten belüli fájlok
  szerkesztőlapon; minden más, például egy máshol lévő fájl, egy hálózati megosztás vagy egy másik app hivatkozása,
  előbb rákérdez, és megmutatja, hová vezet.
- A bejelentkezési és Remote Control-címek, amelyeket egy ügynök vagy egy MCP-szerver átad, csak akkor nyílnak meg, ha
  webcímek.
- Az egy projektnél sokkal többet tartalmazó mappákban, például a saját mappában, az Íróasztalon, a Dokumentumokban, a
  Letöltésekben vagy egy kötet gyökerében, már nem lehet egészében megbízni; a bennük lévő projektekben egyenként bízzon
  meg. A bizalmi kérdés most már jelzi, hogy a bizalom a mappában lévő mappákra is kiterjed.
- A CI-tokenek és a bejelentkezések soha többé nem mennek titkosítatlan http-n más szerverre, mint ez a Mac; az a
  tároló, amelynek távoli tárolója `http://`-vel kezdődik, https-en keresztül kap kérést.
- gitleaks nélkül az előrejelzések, mielőtt a kód egy ezen a Macen kívüli szolgáltatóhoz kerülne, addig várnak, amíg az
  ellenőrzés nélküli küldést nem választja. Egy projekt saját gitleaks-beállításai már nem kapcsolják ki az ellenőrzést.
- A beépített ügynököt már nem lehet rávenni, hogy a projekten kívülre írjon egy olyan hivatkozáson keresztül, amely az
  írás engedélyezése után jelenik meg, a további kemény hivatkozásokkal rendelkező fájlok pedig saját fájlt kapnak
  ahelyett, hogy rajtuk keresztül íródnának.
- Egy nyelvi szerver kiegészítései már nem tudják megakasztani a szerkesztőt, egy hivatkozott fájl általa végzett
  átnevezése és törlése pedig magára a hivatkozásra hat, nem a céljára.
- A prompthoz csatolt képeket az elkülönített dekódolószolgáltatás dekódolja, mint mindenhol máshol.
- A terminálban futó program csak addig cserélheti le a vágólapot, amíg a terminálja előtérben van, és a terminál ezt
  jelzi.
- A rust-analyzer egy ellenőrzőösszeggel együtt rögzített kiadásból települ, nem a mindenkori legújabból.

## Keresés és navigáció
- A Gyors megnyitás jelzi, ha egyetlen fájl sem egyezik, és mutatja, hogy fájlokat vagy a projekt szimbólumait keresi.
- A projekt szimbólumai a TypeScriptet és a JavaScriptet is tartalmazzák, akkor is, ha egyik fájljuk sincs megnyitva. A
  projektből származó és az Ön által használt nyelvű pontos találatok kerülnek előre; a könyvtárakból és telepített
  csomagokból származó szimbólumok meg vannak jelölve.
- A Nézet › Keresés a keresőmezőbe helyezi a fókuszt.

## Szerkesztő
- A szerkesztő megtartja a fókuszt, amikor egy Markdown-fájl az előnézetével együtt nyílik meg mellette, így a Formátum
  menü azonnal működik. A szerkesztő parancsai akkor is működnek, ha a fókusz az oldalsávon van.
- A sor végén vagy az alatta lévő mezőben megjelenő javaslat jelzi, hogy a Tab billentyű elfogadja.
- Az azonos nevű lapok megkülönböztethetők a lapáttekintésben, például egy fájl és annak előnézete a böngészőben.
- A Nézet menü már nem kínálja fel a macOS saját „Lapsor elrejtése” parancsát, amelyet a Dango lapjai nem használnak.

## Nyelvek és hibakeresés
- A JSON-szerver mostantól ellenőrzi a fájlokat, és azok a szerverek, amelyek csak indulás után regisztrálják a
  diagnosztikájukat, például a basedpyright és a sourcekit-lsp, megjelenítik azt.
- Ahol a pip megtagadja a telepítést a Pythonba, mint a Homebrew-féle Python esetében, a hibakereső felajánlja, hogy a
  debugpy-t a projekt virtuális környezetébe telepíti.
