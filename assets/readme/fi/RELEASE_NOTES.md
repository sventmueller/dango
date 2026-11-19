## AI-välilehdet
- Taustakomennon ”Näytä tuloste” avaa välilehden, joka seuraa tulostetta niin kauan kuin komento on käynnissä, sen
  sijaan että näyttäisi kopion tulosteesta sillä hetkellä. Välilehti avautuu tulosteen loppuun ja pysyy siellä uusien
  rivien tullessa, kunnes vierität ylöspäin; kun vierität takaisin loppuun, se seuraa taas. Jos välilehti on jo auki,
  ”Näytä tuloste” tuo sen eteen tulosteen loppuun.
  - Kun komento kirjoittaa tulosteensa itse tiedostoon, kuten `make > build.log 2>&1`, välilehti näyttää tuon tiedoston
    sitä mukaa kuin se kasvaa.
  - Kun komento on päättynyt, välilehti näyttää sen koko tulosteen; ”Avaa editorissa” avaa tiedoston, josta välilehti
    lukee.
  - Välilehti lukee tulostetta vain ollessaan edessä.
