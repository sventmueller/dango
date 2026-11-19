## Nopeus ja muisti
- Kaikki, mitä agentti käynnistää millä tahansa kielellä, kuten käännökset, testit, komentotulkit ja MCP-palvelimet,
  toimii macOS:n taustatilassa: alimmalla prioriteetilla ja rajoitetulla levyn ja verkon käytöllä. Itse agentit Clauden
  aliagentteineen toimivat keskitason prioriteetilla. Täydellä kuormalla toimivat agentit eivät enää hidasta Dangoa;
  tämä koskee myös Software Factoryn agentteja.
- Palautetut ikkunat käynnistävät vain sen, mikä näkyy: muiden takana oleva AI-välilehti käynnistää CLI:nsä, kun se
  tulee eteen tai saa kehotteen, ja selain-, sivustoesikatselu- ja päätevälilehdet lataavat sivunsa tai käynnistävät
  komentotulkkinsa, kun ne näkyvät ensimmäisen kerran.
- Pitkä projektihaku ei enää piirrä kaikkia tuloksiaan uudelleen ollessaan käynnissä. Software Factory, CI-lokit ja
  Testit-paneeli tekevät vähemmän, kun ne eivät ole näkyvissä; testit ja käynnistysmääritykset luetaan, kun niiden
  paneeli näkyy ensimmäisen kerran, eivätkä piilotetut CI- ja Kontit-paneelit lue mitään ennen kuin ne tulevat näkyviin.
- Pitkät istunnot tarvitsevat vähemmän muistia: valmis diff säilyttää vain rivit, jotka sen kortti näyttää, vastattu
  lupapyyntö vapauttaa syötteensä, päätteen glyfivälimuisti ei enää kasva rajatta, ja Tiedostot- ja Git-paneelit
  vapauttavat rivit, joita ei enää ole.
- Itse ohjelma on noin 40 % pienempi.

## Ikkunat ja valikot
- Ruudun sulkeminen ei enää sulje sen välilehtiä, vaan ne siirtyvät edelliseen ruutuun tai, jos sellaista ei ole,
  seuraavaan. Vain esikatselun ruutu ja viimeinen ruutu sulkeutuvat välilehtineen.
- Näytä-valikko luettelee paneelit samassa järjestyksessä kuin sivupalkin symbolit.
- ”Hallitse agentteja…” poistui +-valikosta; se löytyy yhä Agentit-paneelin ”+ Uusi istunto” -valikosta.

## Korjaukset
- ”Sisällytä ja jätä pois tiedostoja” Haku-paneelissa ei enää näytä ylimääräistä kirjainta nuolensa vieressä.
- Kun määrittelyn kommenttien ponnahdusikkuna suljetaan, ohjelma ei enää kaadu.
