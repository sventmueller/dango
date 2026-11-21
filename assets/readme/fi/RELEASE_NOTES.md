## Tietoturva
- Agentin vastauksissa, suunnitelmissa ja raporteissa, Markdown-esikatselussa ja PDF-tiedostoissa olevat linkit eivät
  enää avaa mitä tahansa appia yhdellä klikkauksella. Verkko-osoitteet avautuvat selainvälilehteen ja projektin
  tiedostot editorivälilehteen; kaikki muu, kuten muualla oleva tiedosto, verkkojako tai toisen apin linkki, kysyy ensin
  ja näyttää, mihin se vie.
- Kirjautumis- ja Remote Control -osoitteet, jotka agentti tai MCP-palvelin välittää, avautuvat vain, jos ne ovat
  verkko-osoitteita.
- Kansioihin, jotka sisältävät paljon muutakin kuin yhden projektin, kuten kotikansioosi, Työpöytään, Dokumentteihin,
  Latauksiin tai taltion juureen, ei voi enää luottaa kokonaisuutena; luota niiden projekteihin yksi kerrallaan.
  Luottamuskysymys kertoo nyt, että luottamus koskee myös kansion sisältämiä kansioita.
- CI-tokenit ja kirjautumiset eivät enää koskaan kulje salaamattoman http:n kautta muulle palvelimelle kuin tälle
  Macille; jos repositorion etärepositorio alkaa `http://`, sen tiedot kysytään https:n kautta.
- Ilman gitleaksia ennustukset odottavat ennen kuin koodia lähtee tämän Macin ulkopuoliselle palveluntarjoajalle, kunnes
  valitset lähettää tarkistamatta. Projektin omat gitleaks-asetukset eivät enää kytke tarkistusta pois.
- Omaa agenttia ei voi enää saada kirjoittamaan projektin ulkopuolelle linkillä, joka ilmestyy sen jälkeen, kun sen
  kirjoitus on hyväksytty, ja tiedostot, joilla on muita kovia linkkejä, saavat oman tiedostonsa sen sijaan, että niiden
  kautta kirjoitettaisiin.
- Kielipalvelimen täydennykset eivät enää voi jumittaa editoria, ja sen tekemät linkitetyn tiedoston uudelleennimeämiset
  ja poistot kohdistuvat linkkiin eivätkä sen kohteeseen.
- Kehotteeseen liittämäsi kuvat purkaa eristetty dekoodauspalvelu, kuten kaikkialla muuallakin.
- Päätteessä toimiva ohjelma voi korvata leikepöydän vain, kun sen pääte on edessä, ja pääte kertoo siitä.
- rust-analyzer asentuu julkaisusta, joka on kiinnitetty tarkistussummineen, eikä kulloinkin uusimmasta.

## Haku ja navigointi
- Avaa nopeasti kertoo, kun mikään tiedosto ei täsmää, ja näyttää, hakeeko se tiedostoja vai projektin symboleita.
- Projektin symbolit kattavat TypeScriptin ja JavaScriptin, vaikka yksikään niiden tiedosto ei olisi auki. Tarkat
  tulokset projektista ja kielestä, jolla työskentelet, tulevat ensin; kirjastojen ja asennettujen pakettien symbolit on
  merkitty.
- Näytä › Haku siirtää kohdistuksen hakukenttään.

## Editori
- Editori säilyttää kohdistuksen, kun Markdown-tiedosto avautuu esikatselu vieressään, joten Muoto-valikko toimii heti.
  Editorin komennot toimivat myös, kun kohdistus on sivupalkissa.
- Rivin lopussa tai sen alla olevassa laatikossa näkyvä ehdotus kertoo, että sarkain hyväksyy sen.
- Samannimiset välilehdet erottuvat toisistaan välilehtien yleiskatsauksessa, esimerkiksi tiedosto ja sen esikatselu
  selaimessa.
- Näytä-valikko ei enää tarjoa macOS:n omaa ”Kätke välilehtipalkki” -kohtaa, jota Dangon välilehdet eivät käytä.

## Kielet ja virheenjäljitys
- JSON-palvelin tarkistaa nyt tiedostot, ja palvelimet, jotka rekisteröivät diagnostiikkansa vasta käynnistymisen
  jälkeen, kuten basedpyright ja sourcekit-lsp, näyttävät ne.
- Kun pip kieltäytyy asentamasta Pythoniin, kuten Homebrew’n Pythonin kohdalla, virheenjäljitin tarjoutuu asentamaan
  debugpy-paketin projektin virtuaaliympäristöön.
