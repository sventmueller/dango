## AI-lapok
- Egy háttérparancs „Kimenet megjelenítése” gombja olyan lapot nyit meg, amely a parancs futása alatt követi a
  kimenetet, ahelyett, hogy annak az adott pillanatbeli másolatát mutatná. A lap a kimenet végén nyílik meg, és az
  érkező új sorok mellett is ott marad, amíg fel nem görget; ha visszagörget a végére, ismét követi a kimenetet. Ha a
  lap már nyitva van, a „Kimenet megjelenítése” előtérbe hozza, a kimenet végén.
  - Ha egy parancs maga írja fájlba a kimenetét, például a `make > build.log 2>&1`, a lap ezt a fájlt mutatja, miközben
    az növekszik.
  - Miután a parancs befejeződött, a lap a teljes kimenetét mutatja; a „Megnyitás a szerkesztőben” megnyitja a mögötte
    lévő fájlt.
  - Egy lap csak akkor olvassa a kimenetet, amikor előtérben van.
