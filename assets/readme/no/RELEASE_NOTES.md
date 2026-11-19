## Minne
- Lange agentøkter trenger mye mindre minne:
  - Når en tidligere økt åpnes, leses transkripsjonen én linje om gangen; en økt med en transkripsjon på 100 MB topper
    nå på omtrent 20 MB i stedet for 65 MB.
  - Et fullført verktøykall beholder bare det kortet viser: de første 400 linjene av utdataene, og ikke lenger rå
    inndata når kortet viser feltene sine. «N linjer til» teller fortsatt hver linje.
  - En KI-fane som er ute av syne i 30 sekunder, slipper kortene sine, og når den vises igjen, bygger den dem som er
    synlige, på stedet der den var.
  - Etter at en fane lukkes eller en tidligere økt åpnes, gir Dango minnet den frigjorde, tilbake til macOS.
- Lister vokser ikke lenger i minnet når de oppdateres. På macOS 27 etterlot hver oppdatering av en liste med rader som
  kan utvides, en utvidelsestrekant for godt, og hver oppdatering etterlot litt av hver rad:
  - Listene CI, Containere, Software Factory, Tester, Feilsøk, Git og Søk samt kallhierarkiet tegner sine egne piler.
  - Lister som ikke er synlige, oppdateres ikke lenger; de tar det igjen når de vises.
  - Listene Økter, CI, Software Factory og Containere beholder radene som ikke er endret, og Git oppdaterer bare radene
    til filer som har endret tilstand.
- Panelene i et lukket vindu frigjøres sammen med det.

## KI-faner
- «Vis utdata» for en bakgrunnskommando som selv skriver utdataene sine til en fil, for eksempel
  `make > build.log 2>&1`, viser den filen når CLI ikke har noe av utdataene.
