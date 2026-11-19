## Pomnilnik
- Dolge seje agentov potrebujejo veliko manj pomnilnika:
  - Ob odpiranju pretekle seje se njen prepis bere vrstico za vrstico; seja s prepisom velikosti 100 MB zdaj na vrhuncu
    porabi približno 20 MB namesto 65 MB.
  - Končan klic orodja obdrži le tisto, kar prikazuje njegova kartica: prvih 400 vrstic izhoda, surovega vnosa pa ne
    več, ko kartica prikaže svoja polja. »Dodatne vrstice: N« še vedno šteje vsako vrstico.
  - Zavihek AI, ki 30 sekund ni viden, sprosti svoje kartice, in ko se znova prikaže, zgradi tiste, ki so vidne, na
    mestu, kjer je bil.
  - Po zaprtju zavihka ali odprtju pretekle seje Dango vrne sproščeni pomnilnik sistemu macOS.
- Seznami ob osveževanju ne rastejo več v pomnilniku. V macOS 27 je vsaka osvežitev seznama z razširljivimi vrsticami za
  seboj trajno pustila trikotnik za razširitev, vsaka osvežitev pa je za seboj pustila tudi delček vsake vrstice:
  - Seznami CI, Vsebniki, Software Factory, Testi, Odpravljanje napak, Git in Iskanje ter hierarhija klicev sami rišejo
    svoje puščice.
  - Seznami, ki niso vidni, se ne osvežujejo več; zamujeno nadoknadijo, ko se prikažejo.
  - Seznami Seje, CI, Software Factory in Vsebniki obdržijo vrstice, ki se niso spremenile, Git pa osveži le vrstice
    datotek, katerih stanje se je spremenilo.
- Podokna zaprtega okna se sprostijo skupaj z njim.

## Zavihki AI
- »Prikaži izhod« pri ukazu v ozadju, ki svoj izhod sam zapisuje v datoteko, na primer `make > build.log 2>&1`, prikaže
  to datoteko, ko CLI nima ničesar od izhoda.
