## AI-välilehdet
- AI-välilehti, joka on ollut kolme minuuttia poissa näkyvistä, lopettaa Claude-prosessinsa, kun istunto on joutilaana.
  Keskustelu ja kirjoittamasi teksti jäävät välilehteen, ja seuraava kehote jatkaa istuntoa. Välilehti pitää prosessinsa
  niin kauan kuin Claude työskentelee, kehote odottaa, kysymys on avoinna, aliagentti, taustatehtävä tai komento on yhä
  käynnissä tai Remote Control on päällä.
- Kun agentti lopetetaan, päättyvät myös komennot, jotka se käynnisti omassa prosessiryhmässään tai istunnossaan;
  aiemmin ne saattoivat jäädä käyntiin agentin jälkeen.
- Aikajana lisää uudet merkinnät loppuunsa sen sijaan, että indeksoisi kaikki aiemmat rivit uudelleen.

## Nopeus
- Palautetut ikkunat avautuvat nopeammin: tiedostovälilehti, jossa ei ole tallentamattomia muutoksia, lataa tiedostonsa
  vasta, kun se tulee esiin.
- Projektihaku, jonka paneeli on piilossa, keskeytyy ja jatkuu, kun paneeli tulee näkyviin.
- Editorin Git-muutosmerkinnät tekevät kullekin tiedostolle enintään yhden vertailun kerrallaan; sen aikana tehdyt
  muokkaukset yhdistetään yhdeksi jatkovertailuksi, ja tiedoston commitoitu teksti luetaan uudelleen vain, jos commit on
  muuttunut.
- Kutistettua koodia sisältävän tiedoston minikartta pitää yhden merkinnän kutakin avattua jaksoa kohden eikä yhtä riviä
  kohden.

## Haku
- Suodattimien nuoli on samassa linjassa suoraan sen yläpuolella olevan Korvaa-nuolen kanssa.
