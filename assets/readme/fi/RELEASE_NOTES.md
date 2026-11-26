## Pääte
- Pääte on nyt Tapan, Dangon päätteelle tehdyn sisarsovelluksen, pääte kaikkine sen tuomine lisäyksineen, ja Dango säilyttää sen, mitä sen pääte osasi ennen: kuvien sijoittamisen agenteille, kehityspalvelimet selainvälilehdessä, ⌘-osoituksen editoriin.
- **Näytä › Näytä/kätke syötekenttä** (⌘I, kun pääte on aktiivinen) muuttaa komentorivin tekstikentäksi, jonka yläpuolella näkyvät kansio ja Git-haara. Komennot väritetään osiensa mukaan, ja komento, jota komentotulkkisi ei voi suorittaa, saa punaisen katkoviivan alleen.
- Ehdotukset näkyvät harmaana kirjoittaessasi, kentässä ja ilman sitä: historiastasi, yli 700 komentorivityökalun kuvauksista ja projektista, kuten git-haarat, `package.json`-skriptit sekä make-, just- ja cargo-kohteet. → hyväksyy ehdotuksen; Tab täydentää ja avaa useiden vaihtoehtojen kohdalla valikon, jota kirjoittaminen suodattaa. Ne eivät koskaan käytä verkkoa.
- Komentolohkot: ⌘↑ ja ⌘↓ hyppäävät kehotteiden välillä, **Muokkaa**-valikossa voi valita, kopioida ja tallentaa komennon tulosteen, ja kirjanmerkit merkitsevät rivejä. Kun **Merkitse komennot lohkoiksi** on päällä, palkki näyttää jokaisen lohkon, epäonnistuneet komennot merkitään ja lohkon päällä olevat painikkeet näyttävät sen tilan ja keston.
- **Lähetä lohko agentille** (Muokkaa-valikko ja päätteen kontekstivalikko) avaa AI-välilehden, jonka liitteenä ovat nykyisen lohkon komentorivi ja tuloste Ulkopuolisena sisältönä.
- **Etsi** (⌘F, kun pääte on aktiivinen) hakee koko historiasta säännöllisillä lausekkeilla, kirjainkoon ja kokonaisten sanojen mukaan; vierityspalkki merkitsee tulokset.
- Ohjelman ilmoitus (OSC 9 ja OSC 777) ja sen äänimerkki asettavat punaisen pisteen päätteen välilehteen, kun et näe sitä, ja ilmoitus menee macOS:lle, kun jokin toinen app on edessä. Ohjelman ilmoittama edistyminen (OSC 9;4) näkyy renkaana sen välilehdessä ja palkkina Dockin kuvakkeessa.
- Kaksoisosoitus valitsee kokonaisen URL-osoitteen, polun tai tiivisteen, ⌥-vetäminen valitsee suorakulmion, **Kopioi muotoiluineen** säilyttää värit ja tyylit, ja **Valitse näppäimistöllä** siirtää valintaa Vimin näppäimillä.
- Moniriviset sijoitukset, jotka suoritettaisiin heti, näytetään ensin, ja **Dango › Suojattu näppäimistösyöte** estää muita appeja lukemasta, mitä kirjoitat päätteeseen.
- Muunlaiset linkit kuin verkko-osoitteet, `mailto:` ja tiedostot eivät enää avaudu päätteestä.

## Asetukset
- **Asetukset › Pääte** kytkee päälle ja pois uusien päätteiden syötekentän, sen Vim-näppäimet, ehdotukset, komentojen värityksen, tuntemattomien komentojen alleviivauksen, komentolohkot, valinnan kopioinnin heti, monirivisten sijoitusten näyttämisen ensin sekä ⌥:n metanäppäimenä, joka siirtyi tänne Yleiset-välilehdeltä.
