<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Tämä on Dango-sovelluksen ensimmäinen julkaisu. Dango on natiivi macOS-editori koodausagenttien kanssa työskentelyyn.
Editori, päätteet, selain, Git, CI, virheenjäljitin ja AI-ketjut jakavat yhden ikkunan kansiota kohden. Sovellus on
kirjoitettu Rust-kielellä AppKit-kehyksen päälle; WebKit on käytössä vain selainvälilehdessä. Käyttöliittymä on
saatavilla 35 kielellä.

## Vaatimukset
- macOS 14 tai uudempi, Apple silicon- tai Intel-suorittimella.
- AI-ketjuja varten valitsemasi agentti: Claude tarvitsee `claude`-komennon ja sisäänkirjautumisen; Codex tarvitsee
  Node.js-ympäristön, ja sovellus asentaa sen paketin kiinnitettynä versiona pyynnöstäsi; Antigravity ja OpenCode
  käyttävät omaa `agy`- tai `opencode`-komentoasi tai latausta kiinnitettynä versiona; oma agentti tarvitsee API-avaimen
  tai Ollama- tai LM Studio -sovelluksen tässä Macissa.
- Kielipalvelimet ja virheenjäljityssovittimet asennetaan pyynnöstäsi. gopls ja `dlv` tarvitsevat Go-kielen;
  TypeScript-, Python-, JSON-, YAML-, CSS-, HTML- ja shell-palvelimet sekä JavaScript-virheenjäljitin tarvitsevat
  Node.js-ympäristön; clangd, sourcekit-lsp ja `lldb-dap` tulevat Xcode Command Line Tools -paketin mukana.
- Muokkausennusteet tarvitsevat GitHub Copilot -tilauksen (ilmainen riittää) tai fill-in-the-middle-mallin Ollama- tai
  LM Studio -sovelluksessa.

## Ikkuna
- Jokainen tiedosto, pääte, selainsivu, AI-ketju, eronäkymä ja esikatselu on macOS-välilehti kansion ikkunassa.
  Välilehtipalkin lopussa oleva ”+” ja ⌘T tarjoavat uuden tiedoston, päätteen, selaimen ja jokaisen asennetun agentin,
  myös omassa Git-worktreessaan.
- Sivupalkki vaihtaa näkymien Tiedostot, Haku, Agentit, Git, CI ja Virheenjäljitys välillä (⌘1–⌘6); oikean painikkeen
  osoituksella piilotat ne, joita et käytä. Työkalupalkissa ovat siirtyminen taakse- ja eteenpäin muokkaamiesi kohtien
  läpi (⌃- ja ⌃⇧-), haara, Avaa nopeasti ja Toiminta-yleiskatsaus.
- ”Avaa viereen” näyttää toisen dokumentin ensimmäisen vieressä. Uudelleenkäynnistyksen tai kaatumisen jälkeen jokainen
  ikkuna palaa välilehtineen, ja editorit, joissa on tallentamatonta tekstiä, palaavat tekstin kanssa.
- Kuvat, ääni, video, fontit ja Office-dokumentit avautuvat välilehteen Quick Look -toiminnolla, PDF-tiedostot
  PDFKit-kehyksellä.

## Editori
- Syntaksivärit, rivinumerot, muutosmerkit suhteessa viimeisimpään commitiin, etsi ja korvaa, Avaa nopeasti (⌘P) ja
  projektihaku (⇧⌘F).
- Automaattinen sisennys, sarkaimet tai välilyönnit `.editorconfig`-tiedoston mukaan, sulkeparit, kommentoinnin
  vaihtaminen, laskostus, siirtyminen riville, näkyvät tyhjämerkit ja rivitys.
- Useita kohdistimia: ⌥-osoitus, ⌥-vetäminen sarakkeelle, ⌘D seuraavalle esiintymälle.
- Tiedostot säilyttävät merkistökoodauksensa, BOM-merkkinsä ja rivinvaihtonsa. Levyllä muuttunut tiedosto ladataan
  uudelleen, jos editorissa ei ole tallentamatonta tekstiä; muuten palkki tarjoaa erot ja yhdistämisen.
- **Muokkausennusteet:** kun kirjoitat, ehdotus näkyy harmaana; Tab hyväksyy sen. Ehdotukset tulevat GitHub Copilot
  -palvelusta tai paikallisesta mallista Ollama- tai LM Studio -sovelluksessa sen mukaan, minkä valitset asetuksissa;
  paikallinen malli pitää koodisi tässä Macissa.

## Kielipalvelimet
Go, Rust, TypeScript ja JavaScript, Python, C, C++ ja Objective-C, Swift, shell-skriptit, JSON, YAML, CSS, HTML, TOML,
Markdown ja Lua sekä omat palvelimet asetuksista. Diagnostiikka, hover-tiedot, täydennys, allekirjoitusohje, siirtyminen
määrittelyyn, tyypin määrittelyyn ja toteutukseen, viittaukset, kutsu- ja tyyppihierarkiat, uudelleennimeäminen,
muotoilu, semanttiset värit, laskostus ja jäsennys; tyypit ja parametrien nimet rivin sisällä; pikakorjaukset ja
refaktoroinnit hehkulampun takana (⌥⌘A); projektin symbolit pikanäppäimellä ⌥⌘O tai `#`-merkillä Avaa nopeasti
-näkymässä. TypeScript-projektin omaa `tsc`-komentoa käytetään, kun se kelpaa kielipalvelimeksi.

## Virheenjäljitin
Keskeytyskohdat, myös ehdollisina, askellus, säikeet kutsupinoineen, muuttujat ja tarkkailtavat lausekkeet
Tarkastus-näkymässä (⌥⌘I), arvot koodin vieressä ohjelman ollessa pysäytettynä sekä virheenjäljityskonsoli, joka evaluoi
lausekkeet pysäytetyssä kehyksessä. Rust, C, C++ ja Swift: `lldb-dap`; Go: `dlv`; JavaScript ja TypeScript:
JavaScript-virheenjäljitin; Python: debugpy. Projektin `.vscode/launch.json` luetaan. Virheenjäljitys-valikko käyttää
Xcode-sovelluksen pikanäppäimiä.

## Pääte
Päätteet välilehdissä kirjautumiskomentotulkin ympäristöllä, piirrettyinä Metal-rajapinnalla. Kuvan liittäminen tai
pudottaminen antaa sen päätteessä toimivalle koodausagentille tai liittää tallennetun kopion polun. ⌘-osoitus avaa
linkit ja tiedostonimet. Päätteen käynnistämän kehityspalvelimen voi avata selainvälilehdessä.

## Markdown
Markdown-tiedosto avautuu esikatselu vieressään (⇧⌘V), mukana kaavat, Mermaid-, Graphviz- ja PlantUML-kaaviot sekä
kansion kuvat. Muoto-valikko lihavointiin, kursiiviin, linkkeihin, otsikoihin, luetteloihin, lainauksiin ja
koodilohkoihin; Word-dokumentista tai verkkosivulta liitetty teksti saapuu Markdown-muodossa. **Tiedosto › Vie**
kirjoittaa PDF-tiedoston, yksittäisen HTML-tiedoston tai Word-dokumentin. Jos Splat on asennettu, **Tiedosto › Avaa
Splatissa** avaa dokumentin siinä.

## Selain
Selainvälilehti, jossa on osoitekenttä, takaisin ja eteenpäin, haku, ensin kysyvät lataukset ja Web Inspector.

## Git
Git-paneeli, jossa voi valmistella ja hylätä tiedostoja tai yksittäisiä lohkoja, commitoida, commitoida ja pushata tai
synkronoida, ja jossa ovat haarat, stashit, yhdistämisristiriidat ja kaavio viimeisimmistä commiteista. Blame näyttää,
kuka muutti kohdistimen rivin viimeksi, tai sarakkeen koko tiedostolle. Painike kirjoittaa commit-viestin AI:n avulla;
erot lähtevät vasta, kun gitleaks ei ole löytänyt niistä salaisuuksia, ja jokainen commit tarkistetaan myös
salaisuuksien varalta. AI-ketju voi työskennellä omassa Git-worktreessaan; paneeli näyttää, mitä ketju muutti, ja ottaa
työn käyttöön yhdistämällä tai cherry-pickillä.

## CI
GitHub Actions ja GitLab CI, myös GitHub Enterprise Server ja itse ylläpidetty GitLab: nykyisen haaran ajot, työt ja
lokit; epäonnistuneiden töiden, yksittäisen työn tai (GitHub-palvelussa) koko ajon ajaminen uudelleen, manuaalisten
töiden käynnistäminen tai odottavien käyttöönottojen hyväksyminen, uuden pipelinen tai käsin käynnistettävän workflow’n
käynnistäminen sekä peruuttaminen. Avoimet merge requestit tai pull requestit ja issuet näkyvät samassa paneelissa ja
avautuvat selainvälilehdessä. Epäonnistuneen työn lokin voi antaa agentille ulkopuoliseksi tekstiksi merkittynä. Token
tulee Avainnipusta, `gh`- tai `glab`-komennolta tai kirjautumiskomentotulkin ympäristöstä, tai syötät sen itse.

## AI
Jokainen agentti toimii AI-välilehdessä, jossa on sama aikajana: vastaukset, työkalukutsut tulosteineen, lupapyynnöt,
kysymykset, hyväksyttävät ja kommentoitavat suunnitelmat, tehtävälista, aliagentit ja taustatehtävät; kaksi painiketta
välilehden otsakkeessa luettelee käynnissä olevat aliagentit ja taustaprosessit. Ennen jokaista kehotetta sovellus
tallentaa tarkistuspisteen; muutokset voi tarkistaa ja kelata taaksepäin viesti kerrallaan. Uudet ketjut alkavat tilassa
”Manuaalinen”, eikä tuhoavia Git-komentoja sallita koskaan pysyvästi. Toiminta-yleiskatsaus näyttää kaiken, mikä on
käynnissä. Vihreä piste välilehdessä tarkoittaa työskentelevää istuntoa, punainen sinua odottavaa istuntoa, ja ilmoitus
kertoo, kun istunto valmistuu tai jää odottamaan sillä aikaa, kun katsot muualle.
- **Claude** muuttamattomalla `claude`-komennolla: istunnot, aliagentit, taustatehtävät, konteksti ja käyttö.
- **ACP-agentit:** Codex, Antigravity ja OpenCode sekä pyynnöstä lisää ACP-rekisteristä; asennetut agentit on
  kiinnitetty testattuihin versioihin ja tarkistettu tarkistussummiaan vasten.
- **Oma agentti:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio ja palvelimet, joissa on OpenAI- tai
  Anthropic-yhteensopiva API, ja työkalut tiedostoille, haulle, komennoille, diagnostiikalle ja verkkopyynnöille;
  avaimet pysyvät Avainnipussa.
- Ketjukohtainen hiekkalaatikkokytkin rajoittaa, mitä agentin komennot saavat tehdä, siltä osin kuin agentti sitä tukee.
- Ulkopuolinen teksti, kuten päätteen tuloste, verkkosivu tai CI-loki, menee agentille epäluotettavaksi merkittynä.

## Sanelu
Sanelu AI-viestikenttään ja editoriin Lowspeak-sovelluksen puheentunnistuksella: mikrofonipainike, ⌃⌥D tai fn-näppäin,
kun Dango on aktiivinen sovellus. Sanelu käyttää Lowspeak-sovelluksen asetuksia ja malleja; verkkolitterointi tarvitsee
OpenAI- tai Groq-avaimen.

## Päivitykset
Sovellus päivittää itsensä Sparkle-kehyksellä. Toisella käynnistyskerralla se kysyy, saako se etsiä päivityksiä itse;
ennen kuin hyväksyt, se ei tee yhtään pyyntöä. **Dango › Etsi päivityksiä…** etsii heti.

## Käyttöapu
VoiceOver-nimet sivupalkille, välilehdille, AI-aikajanan korteille, Git- ja CI-riveille, virheenjäljittimelle ja
päätteen tekstille; täysi näppäimistöohjaus pikanäppäimillä, joita voit muuttaa tiedostossa `keybindings.json`; Lisää
kontrastia ja Vähennä liikettä; koodinäkymät pysyvät vasemmalta oikealle myös oikealta vasemmalle kirjoitettavissa
käyttöliittymissä. Lisätietoja on tiedostossa ACCESSIBILITY.md.

## Yksityisyys
Ei telemetriaa, ei analytiikkaa eikä kaatumisraportteja. Sovellus muodostaa yhteyden vain siihen, minkä itse käynnistät:
käyttämiisi agentteihin, pyytämiisi latauksiin, CI-palvelimiin, joille kirjaudut, GitHub Copilot -muokkausennusteisiin,
jos valitsit ne, verkkolitterointiin, jos valitsit sen, avaamiisi sivuihin sekä päivitysten etsintään, kun olet sallinut
sen. Avaimet ja tokenit pysyvät macOS:n Avainnipussa.

## Tunnetut rajoitukset
- CI-kirjautuminen selaimen kautta on pois päältä; käytä `gh`- tai `glab`-komentoa, tokenia tai ympäristöä.
- Sovellus toimii ilman App Sandbox -suojausta, koska päätteet, Git, kielipalvelimet ja agentit perisivät sen; kansioon
  on luotettava, ennen kuin mitään sen määrittämää ajetaan. Antigravity- ja OpenCode-agenteilla ei ole hiekkalaatikkoa,
  jonka sovellus voisi kytkeä päälle.
- Claude-agentin käyttöluvut ja kontekstin erittely perustuvat sen protokollan kokeellisiksi merkittyihin osiin, ja ne
  piilotetaan, kun niitä ei ole. Keskustelun kelaus taaksepäin toimii vain Claude-agentilla; muilla agenteilla kelataan
  taaksepäin vain koodi.
- Gemini CLI -työkalua ei tarjota: Google lopetti sen kirjautumisen henkilökohtaisella tilillä ja ohjaa käyttämään
  Antigravity-agenttia. Aiemmat Gemini-ketjut pysyvät luettavina.
