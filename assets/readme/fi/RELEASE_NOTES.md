## Muisti
- Pitkät agenttien istunnot tarvitsevat paljon vähemmän muistia:
  - Aiemman istunnon avaaminen lukee sen keskustelulokin rivi kerrallaan; istunto, jonka keskusteluloki on 100 MB, vie
    nyt enimmillään noin 20 MB eikä 65 MB.
  - Valmis työkalukutsu säilyttää vain sen, minkä sen kortti näyttää: tulosteensa ensimmäiset 400 riviä, eikä enää
    raakasyötettä, kun kortti näyttää kenttänsä. ”N riviä lisää” laskee edelleen jokaisen rivin.
  - AI-välilehti, joka on poissa näkyvistä 30 sekuntia, vapauttaa korttinsa, ja kun se tulee taas näkyviin, se rakentaa
    näkyvissä olevat kortit samaan kohtaan, jossa se oli.
  - Kun välilehti suljetaan tai aiempi istunto avataan, Dango palauttaa vapauttamansa muistin macOS:lle.
- Luettelot eivät enää kasva muistissa päivittyessään. macOS 27:ssä jokainen avautuvia rivejä sisältävän luettelon
  päivitys jätti pysyvästi jälkeensä avauskolmion, ja jokainen päivitys jätti jälkeensä vähän jokaisesta rivistä:
  - Luettelot CI, Kontit, Software Factory, Testit, Virheenjäljitys, Git ja Haku sekä kutsuhierarkia piirtävät omat
    nuolensa.
  - Näkymättömissä olevat luettelot eivät enää päivity; ne kurovat eron kiinni, kun ne tulevat näkyviin.
  - Luettelot Istunnot, CI, Software Factory ja Kontit säilyttävät rivit, jotka eivät muuttuneet, ja Git päivittää vain
    niiden tiedostojen rivit, joiden tila muuttui.
- Suljetun ikkunan paneelit vapautetaan sen mukana.

## AI-välilehdet
- Kun taustakomento kirjoittaa tulosteensa itse tiedostoon, kuten `make > build.log 2>&1`, ”Näytä tuloste” näyttää tuon
  tiedoston, jos CLI:llä ei ole tulosteesta mitään.
