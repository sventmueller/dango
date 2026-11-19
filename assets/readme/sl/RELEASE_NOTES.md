## Zavihki AI
- »Prikaži izhod« pri ukazu v ozadju odpre zavihek, ki sledi izhodu, dokler ukaz teče, namesto kopije izhoda iz tistega
  trenutka. Zavihek se odpre na koncu izhoda in tam ostane, ko prihajajo nove vrstice, dokler se ne pomaknete navzgor;
  ko se pomaknete nazaj na konec, znova sledi izhodu. »Prikaži izhod« pri zavihku, ki je že odprt, ga prenese v ospredje
  in pokaže konec izhoda.
  - Ukaz, ki svoj izhod sam piše v datoteko, na primer `make > build.log 2>&1`, prikazuje to datoteko, medtem ko raste.
  - Ko se ukaz konča, zavihek prikaže ves njegov izhod; »Odpri v urejevalniku« odpre datoteko, iz katere zavihek bere.
  - Zavihek bere izhod le, dokler je v ospredju.
