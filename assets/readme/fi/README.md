<p><img src="../../readme-icon.png" alt="Dangon sovelluskuvake" width="168" height="168"></p>

# Dango

**Natiivi macOS-editori koodausagenttien kanssa työskentelyyn.**

Dango kokoaa koodin, sitä muuttavat agentit ja kaiken niiden ympärillä olevan yhteen ikkunaan kansiota kohden: editori kielipalvelimineen ja muokkausennusteineen, päätteet, selain, Git, CI, virheenjäljitin ja AI-ketjut, joissa toimii Claude, Codex, Antigravity, OpenCode tai sovellukseen sisäänrakennettu agentti. Jokainen agentti työskentelee samalla aikajanalla, kysyy ennen kuin muuttaa tiedostoja tai ajaa komentoja ja jättää tarkistuspisteen ennen jokaista kehotetta, joten voit tarkistaa ja kelata taaksepäin sen tekemiset viesti kerrallaan. Mitään kansion määrittämää ei ajeta, ennen kuin luotat kansioon, ja ulkopuolinen teksti saapuu agentille epäluotettavaksi merkittynä. Sanele editoriin ja viestikenttään; puheentunnistus voi pysyä Macissasi. Dango on kirjoitettu Rust-kielellä aitojen AppKit-säädinten päälle ilman Electronia, se puhuu 35 kieltä, eikä siinä ole telemetriaa.

**Ei vielä julkaistu.** Ensimmäinen julkaisu on levykuva, joka on allekirjoitettu Developer ID:llä ja jonka Apple on notarisoinut, macOS 14:lle tai uudemmalle Apple Silicon- ja Intel-Maceissa. Siihen asti voit [kääntää Dangon lähdekoodista](#kääntäminen-lähdekoodista).

**Löysitkö bugin?** [Avaa issue](https://github.com/sventmueller/dango/issues/new) ja kerro, minkä agentin kanssa ja millaisessa projektissa se tapahtui.

[![Rust, editio 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 tai uudempi](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universaali: Apple Silicon ja Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit objc2:n kautta](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Pääte: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 käyttöliittymän kieltä](https://img.shields.io/badge/languages-35-4C9A2A)
![Ei telemetriaa](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Allekirjoitettu, Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lisenssi: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lisenssi)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
**Suomi** ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Sisältö

- [Ominaisuudet lyhyesti](#ominaisuudet-lyhyesti)
- [Vaatimukset](#vaatimukset)
- [Asennus](#asennus)
- [Aloittaminen](#aloittaminen)
- [AI-agentit](#ai-agentit)
  - [Aikajana](#aikajana)
  - [Luvat ja tilat](#luvat-ja-tilat)
  - [Tarkistuspisteet ja worktreet](#tarkistuspisteet-ja-worktreet)
- [Editori](#editori)
- [Kielipalvelimet](#kielipalvelimet)
- [Virheenjäljitin](#virheenjäljitin)
- [Pääte](#pääte)
- [Selain](#selain)
- [Git](#git)
- [CI](#ci)
- [Markdown](#markdown)
- [Sanelu](#sanelu)
- [Luottamus](#luottamus)
- [Hiekkalaatikko](#hiekkalaatikko)
- [Ulkoasu ja käyttöapu](#ulkoasu-ja-käyttöapu)
- [Kielet](#kielet)
- [Pikanäppäimet](#pikanäppäimet)
- [Yksityisyys ja turvallisuus](#yksityisyys-ja-turvallisuus)
- [Missä tiedot ovat](#missä-tiedot-ovat)
- [Tunnetut rajoitukset](#tunnetut-rajoitukset)
- [Kääntäminen lähdekoodista](#kääntäminen-lähdekoodista)
- [Lisenssi](#lisenssi)

## Ominaisuudet lyhyesti

- **Kaikki agentit yhdellä aikajanalla.** Claude oman `claude`-komentonsa kautta, Codex, Antigravity ja OpenCode Agent Client Protocolin kautta, lisää ACP-rekisteristä sekä sovellukseen sisäänrakennettu agentti, joka käyttää palveluja Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama ja LM Studio. Vastaukset, työkalukutsut, lupapyynnöt, suunnitelmat, tehtävälistat, aliagentit ja taustatehtävät näyttävät samalta, toimipa mikä agentti tahansa.
- **Tarkista ja kelaa taaksepäin.** Ennen jokaista kehotetta Dango tallentaa tarkistuspisteen. Agentin muutokset voi tarkistaa ja kelata taaksepäin viesti kerrallaan, ja ketju voi työskennellä omassa Git-worktreessaan ja luovuttaa työnsä yhdistämällä tai cherry-pickillä.
- **Yksi ikkuna kansiota kohden.** Tiedostot, päätteet, selainsivut, AI-ketjut, eronäkymät ja esikatselut ovat tuon ikkunan macOS-välilehtiä. Sivupalkissa ovat Tiedostot, Haku, Agentit, Git, Virheenjäljitys ja CI.
- **Oikea koodieditori.** Syntaksivärit, useita kohdistimia, laskostus, `.editorconfig`, kielipalvelimet 17 kielelle, joissa on täydennys, siirtyminen määrittelyyn, viittaukset, uudelleennimeäminen, pikakorjaukset ja hierarkiat, sekä muokkausennusteet GitHub Copilotilta tai paikalliselta mallilta.
- **Virheenjäljitin, Git ja CI sisäänrakennettuina.** Keskeytyskohdat ja askellus kielille Rust, C, C++, Swift, Go, JavaScript, TypeScript ja Python; yksittäisten lohkojen valmistelu, haarat, stashit, ristiriidat ja commit-kaavio; ajot, työt ja lokit GitHub Actionsista ja GitLab CI:stä.
- **Varovainen kansioidesi kanssa.** Kansio, johon et ole luottanut, ei käynnistä mitään: ei Git-prosesseja, kielipalvelimia, hookeja eikä agentteja. Tuhoavia Git-komentoja ei koskaan sallita pysyvästi, ja commitit tarkistetaan salaisuuksien varalta.
- **Sanelu.** Puhu editoriin ja viestikenttään verkon kautta OpenAI:lla tai Groqilla tai paikallisesti Whisperillä ja Parakeetilla.
- **Natiivi läpikotaisin.** AppKitin ikkunat, välilehdet, valikot ja työkalupalkit, TextKit 2 editorille, Metal päätteelle, WebKit vain selainvälilehdessä. Tumma tila, Lisää kontrastia ja VoiceOver toimivat kuten kaikissa Mac-sovelluksissa.
- **Omalla kielelläsi.** Käyttöliittymä on saatavilla 35 kielellä: arabia, englanti, espanja, heprea, hindi, hollanti, indonesia, italia, japani, katalaani, kiina (yksinkertaistettu), kiina (perinteinen), korea, kreikka, kroatia, malaiji, norja, portugali (Brasilia), portugali (Portugali), puola, ranska, romania, ruotsi, saksa, slovakki, sloveeni, suomi, tanska, thai, turkki, tšekki, ukraina, unkari, venäjä ja vietnam.

## Vaatimukset

- macOS 14 Sonoma tai uudempi, Apple Silicon- tai Intel-Macissa. Dangoa kehitetään ja testataan macOS 27:ssä.
- AI-ketjuja varten agentti, jota haluat käyttää; katso [AI-agentit](#ai-agentit).
- Kielipalvelimet ja virheenjäljityssovittimet, jotka Dango asentaa pyynnöstäsi: gopls ja `dlv` tarvitsevat Go-kielen; TypeScript-, Python-, JSON-, YAML-, CSS-, HTML- ja shell-palvelimet sekä JavaScript-virheenjäljitin tarvitsevat Node.js-ympäristön; clangd, sourcekit-lsp ja `lldb-dap` tarvitsevat Xcode Command Line Tools -paketin. rust-analyzer ladataan sen GitHub-julkaisuista.
- Muokkausennusteet tarvitsevat GitHub Copilot -tilauksen (ilmainen riittää) tai fill-in-the-middle-mallin Ollamassa tai LM Studiossa.

## Asennus

**Levykuvasta.** Jokainen julkaisu on levykuva, `Dango-<version>.dmg`, jossa on yksi universaali sovellus Apple Siliconille ja Intelille, allekirjoitettu Developer ID:llä ja Applen notarisoima. Avaa se ja vedä Dango Apit-kansioon. Homebrew Cask asentaa saman levykuvan. Yhtään julkaisua ei ole vielä julkaistu.

**Päivitykset.** Levykuvasta asennettu Dango pitää itsensä ajan tasalla [Sparkle](https://sparkle-project.org)-kehyksellä. Toisella käynnistyskerralla se kysyy, saako se etsiä päivityksiä itse; ennen kuin hyväksyt, se ei tee yhtään pyyntöä. **Dango › Etsi päivityksiä…** etsii heti. Päivitykset on allekirjoitettu sovelluksen omalla avaimella, jonka Sparkle tarkistaa ennen kuin asentaa mitään. Itse käännetyssä versiossa ei ole päivittäjää.

**Lähdekoodista.** Katso [Kääntäminen lähdekoodista](#kääntäminen-lähdekoodista) ja siirrä sitten `target/release/Dango.app` kansioon `/Applications`.

**Päätekomento.** **Dango › Asenna komento ”dango”…** kirjoittaa pienen skriptin polkuun `~/.local/bin/dango` ilman ylläpitäjän oikeuksia; `~/.local/bin` täytyy olla `PATH`-muuttujassasi. Sen jälkeen `dango .` avaa nykyisen kansion ja `dango file.rs` tiedoston. Dango ei koskaan korvaa siellä tiedostoa, jota se ei ole itse kirjoittanut.

## Aloittaminen

1. **Avaa kansio** näppäimillä ⌘O tai komennolla `dango <folder>`. Ensimmäisellä kerralla Dango kysyy, luotatko kansioon (katso [Luottamus](#luottamus)).
2. **Sivupalkki** vaihtaa näkymien Tiedostot (⌘1), Haku (⌘2), Agentit (⌘3), Git (⌘4), Virheenjäljitys (⌘5) ja CI (⌘6) välillä; oikean painikkeen osoituksella piilotat ne, joita et käytä, ja ⌃⌘S laskostaa sivupalkin piiloon.
3. **Jokainen dokumentti on välilehti** kansion ikkunassa, macOS:n omia välilehtiä. Välilehtipalkin lopussa oleva ”+” ja ⌘T tarjoavat uuden tiedoston, päätteen, selaimen ja jokaisen asennetun agentin, myös omassa Git-worktreessaan; kun pidät ”+”-painiketta painettuna, samat vaihtoehdot näkyvät valikkona.
4. **Avaa nopeasti** (⌘P) löytää tiedostot ja `#`-merkillä projektin symbolit; ⇧⌘F etsii ja korvaa projektissa.
5. **Keskustele agentin kanssa** AI-välilehdessä. ⌘Esc siirtää näppäimistön kohdistuksen sen viestikenttään ja takaisin.
6. **Asetukset** (⌘,) sisältävät välilehdet Yleiset, CI, Agentit, Oma agentti, Kielipalvelin, Sanelu ja Ennustus.

Työkalupalkissa ovat siirtyminen taakse- ja eteenpäin muokkaamiesi kohtien läpi (⌃- ja ⌃⇧-), haara, Avaa nopeasti ja Toiminta-yleiskatsaus, joka luettelee kaiken käynnissä olevan. Tiedoston kontekstivalikon **Avaa viereen** näyttää toisen dokumentin ensimmäisen vieressä. Uudelleenkäynnistyksen tai kaatumisen jälkeen jokainen ikkuna palaa välilehtineen, ja editorit, joissa on tallentamatonta tekstiä, palaavat tekstin kanssa. Kuvat, ääni, video, fontit ja Office-dokumentit avautuvat välilehteen Quick Look -toiminnolla, PDF-tiedostot PDFKit-kehyksellä.

## AI-agentit

| Agentti | Miten se toimii | Mitä se tarvitsee |
|---|---|---|
| Claude | `claude`-komento, jota ohjataan suoraan sen stream-json-protokollalla | `claude` asennettuna ja sisäänkirjautuneena; Dango etsii kirjautumiskomentotulkin `PATH`-muuttujasta sekä kansioista `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` ja `/usr/local/bin`. Sisäänkirjautuminen tapahtuu vain `claude`-komennossa itsessään |
| Codex | `codex-acp` Agent Client Protocolin (ACP) kautta | Node.js ja npm; Dango asentaa paketin kiinnitettynä versiona pyynnöstäsi |
| Antigravity | `agy acp` | oma `agy` `PATH`-muuttujassa tai lataus kiinnitettynä versiona, asennetaan pyynnöstäsi |
| OpenCode | `opencode acp` | oma `opencode` `PATH`-muuttujassa tai lataus kiinnitettynä versiona, asennetaan pyynnöstäsi |
| Muut ACP-agentit | ACP-rekisteristä, joka haetaan vain pyynnöstäsi | mitä rekisterimerkintä nimeää; merkinnät ilman tarkistussummaa asennetaan vasta vahvistuksesi jälkeen |
| Oma agentti | sisäänrakennettu sovellukseen, työkalut tiedostoille, haulle, komennoille, diagnostiikalle ja verkkopyynnöille | API-avain palveluun Anthropic, OpenAI, Google Gemini tai OpenRouter, säilytetään Avainnipussa; Ollama tai LM Studio tässä Macissa ilman avainta; mikä tahansa palvelin, joka puhuu OpenAI:n tai Anthropicin API:a |

**Asetukset › Agentit** näyttää, mitkä agentit tässä Macissa on, asentaa muut ja niiden päivitykset ja avaa ACP-rekisterin. Dangon asentamat agentit on kiinnitetty testattuihin versioihin ja tarkistettu niiden julkaistuja tarkistussummia vasten (npm integrity tai SHA-256); Dango ei aja yhtään versioimatonta `npx`-pakettia.

### Aikajana

Jokainen agentti työskentelee AI-välilehdessä, jossa on sama aikajana: vastaukset, työkalukutsut tulosteineen, lupapyynnöt, kysymykset, hyväksyttävät ja kommentoitavat suunnitelmat (⌥⌘K), tehtävälistat, aliagentit ja taustatehtävät. Kaksi painiketta välilehden otsakkeessa luettelee käynnissä olevat aliagentit ja taustaprosessit. Mainitse tiedosto, `@terminal` tai `@browser`, niin agentti saa sen sisällön. Vihreä piste välilehdessä tarkoittaa työskentelevää istuntoa, punainen sinua odottavaa istuntoa, ja ilmoitus kertoo, kun istunto valmistuu tai jää odottamaan sillä aikaa, kun katsot muualle. Claudella tilarivi näyttää kontekstin ja käytön.

### Luvat ja tilat

Uudet ketjut alkavat tilassa ”Manuaalinen”: agentti kysyy ennen kuin muuttaa tiedostoja tai ajaa komentoja. Tuhoaville Git-komennoille (`push --force`, `reset --hard`, `clean -fd`) Dango ei tarjoa vaihtoehtoa ”Salli aina”. Oma agentti säilyttää ”Salli aina” -sääntönsä projektikohtaisesti, luettelee ne kohdassa **Asetukset › Oma agentti** ja kysyy tuhoavista Git-komennoista joka kerta riippumatta siitä, mitä on tallennettu; Claude ja ACP-agentit säilyttävät tällaiset säännöt omissa määrityksissään. Kun käytössä on ”Salli kaikki” eikä hiekkalaatikkoa, ketju näyttää pysyvän varoituksen.

### Tarkistuspisteet ja worktreet

Ennen jokaista kehotetta Dango tallentaa tarkistuspisteen commitina polkuun `refs/ide-checkpoints/` projektin repositoriossa tai omaan kansioonsa, jos projektissa ei ole Gitiä; tavallinen push tai fetch ei koskaan siirrä niitä. Agentin muutokset voi tarkistaa ja kelata taaksepäin viesti kerrallaan. Ketju voi työskennellä omassa Git-worktreessaan: Git-paneeli näyttää, mitä se muutti, ja ottaa työn käyttöön yhdistämällä tai cherry-pickillä.

## Editori

- **Syntaksivärit, rivinumerot ja muutosmerkit** suhteessa viimeisimpään commitiin, etsi ja korvaa, Avaa nopeasti (⌘P) ja projektihaku (⇧⌘F).
- **Sisennys** seuraa tiedostoa ja `.editorconfig`-tiedostoa: sarkaimet tai välilyönnit, automaattinen sisennys, sulkeparit.
- **Useita kohdistimia:** ⌥-osoitus, ⌥-vetäminen sarakkeelle, ⌃⇧↑ ja ⌃⇧↓, ⌘D seuraavalle esiintymälle.
- **Laskostus** (⌥⌘← ja ⌥⌘→, ⇧-näppäimen kanssa kaikki), kommentoinnin vaihtaminen (⌘/), siirtyminen riville (⌃G), näkyvät tyhjämerkit ja rivitys (⌥Z).
- **Tiedostot säilyttävät merkistökoodauksensa**, BOM-merkkinsä ja rivinvaihtonsa. Levyllä muuttunut tiedosto ladataan uudelleen, jos editorissa ei ole tallentamatonta tekstiä; muuten palkki tarjoaa erot ja yhdistämisen.
- **Muokkausennusteet.** Kun kirjoitat, ehdotus näkyy harmaana, ja Tab hyväksyy sen. Ehdotus tulee GitHub Copilotilta tai paikalliselta mallilta Ollamassa tai LM Studiossa sen mukaan, mitä valitset kohdassa **Asetukset › Ennustus**; paikallinen malli pitää koodisi tässä Macissa.

## Kielipalvelimet

Go, Rust, TypeScript ja JavaScript, Python, C, C++ ja Objective-C, Swift, shell-skriptit, JSON, YAML, CSS, HTML, TOML, Markdown ja Lua sekä omat palvelimesi kohdasta **Asetukset › Kielipalvelin**. Dango asentaa palvelimen pyynnöstäsi omaan työkalukansioonsa ja käyttää TypeScript-projektin omaa `tsc`-komentoa, kun se kelpaa kielipalvelimeksi.

Mitä ne tuovat: diagnostiikka (⌘' ja ⇧⌘' siirtävät niiden välillä), hover-tiedot (⌃⌘I), täydennys (⌥Esc), allekirjoitusohje, siirtyminen määrittelyyn (⌃⌘J), tyypin määrittelyyn ja toteutukseen, viittaukset (⌃⇧⌘F), kutsu- ja tyyppihierarkiat (⌃⇧⌘H), uudelleennimeäminen (⌃⌘E), muotoilu (⌥⇧F), semanttiset värit, laskostus ja jäsennys (⇧⌘O); tyypit ja parametrien nimet rivin sisällä; pikakorjaukset ja refaktoroinnit hehkulampun takana (⌥⌘A); projektin symbolit näppäimillä ⌥⌘O.

## Virheenjäljitin

Keskeytyskohdat, myös ehdollisina ja lokipisteinä, askellus, säikeet kutsupinoineen, muuttujat ja tarkkailtavat lausekkeet Tarkastus-näkymässä (⌥⌘I), arvot koodin vieressä ohjelman ollessa pysäytettynä sekä virheenjäljityskonsoli, joka evaluoi lausekkeet pysäytetyssä kehyksessä.

| Kieli | Virheenjäljityssovitin |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` Xcode Command Line Tools -paketista |
| Go | `dlv` |
| JavaScript, TypeScript | JavaScript-virheenjäljitin, Node.js-ympäristössä |
| Python | debugpy |

Projektin `.vscode/launch.json` luetaan. Virheenjäljitys-valikko käyttää Xcoden pikanäppäimiä: ⌘\\ asettaa keskeytyskohdan, ⌘Y kytkee ne kaikki päälle tai pois, ⌃⌘Y jatkaa, F6, F7 ja F8 askeltavat yli, sisään ja ulos, ⌘. pysäyttää.

## Pääte

Päätteet välilehdissä (⌃\`) kirjautumiskomentotulkin ympäristöllä, piirrettyinä Metalilla. ⌘K tyhjentää näytön. Kuvan liittäminen tai pudottaminen antaa sen päätteessä toimivalle koodausagentille tai liittää tallennetun kopion polun. ⌘-osoitus avaa linkit ja tiedostonimet. Päätteen käynnistämän kehityspalvelimen voi avata selainvälilehdessä.

## Selain

Selainvälilehti, jossa on osoitekenttä (⌘L), takaisin ja eteenpäin (⌘[ ja ⌘]), uudelleenlataus (⌘R), haku, ensin kysyvät lataukset ja Web Inspector. `@browser` antaa sivun agentille ulkopuoliseksi tekstiksi merkittynä.

## Git

Git-paneeli valmistelee ja hylkää tiedostoja tai yksittäisiä lohkoja, commitoi, commitoi ja pushaa tai synkronoi, vaihtaa ja luo haaroja, säilyttää stasheja, ratkaisee yhdistämisristiriidat ja piirtää kaavion viimeisimmistä commiteista. Blame näyttää, kuka muutti kohdistimen rivin viimeksi, tai sarakkeen koko tiedostolle. Painike kirjoittaa commit-viestin AI:n avulla; erot lähtevät vasta, kun gitleaks ei ole löytänyt niistä salaisuuksia, ja jokainen commit tarkistetaan myös salaisuuksien varalta.

## CI

GitHub Actions ja GitLab CI, myös GitHub Enterprise Server ja itse ylläpidetty GitLab:

- nykyisen haaran ajot, työt ja lokit;
- epäonnistuneiden töiden, yksittäisen työn tai GitHubissa koko ajon ajaminen uudelleen; manuaalisten töiden käynnistäminen tai odottavien käyttöönottojen hyväksyminen; uuden pipelinen tai käsin käynnistettävän workflow’n käynnistäminen; peruuttaminen;
- avoimet pull requestit tai merge requestit sekä issuet, jotka avautuvat selainvälilehdessä;
- epäonnistuneen työn loki agentille annettuna, ulkopuoliseksi tekstiksi merkittynä.

Token tulee Avainnipusta, `gh`- tai `glab`-komennolta tai kirjautumiskomentotulkin ympäristöstä, tai se on se, jonka syötät kohdassa **Asetukset › CI**.

## Markdown

- **Esikatselu.** Markdown-tiedosto avautuu esikatselu vieressään; **Näytä › Näytä/kätke esikatselu vieressä** (⇧⌘V) sulkee ja avaa sen, ja **Asetukset › Yleiset** kytkee automaattisen esikatselun pois. Esikatselu piirtää kaavat, Mermaid-, Graphviz- ja PlantUML-kaaviot sekä kansion kuvat; verkosta tulevat kuvat latautuvat vain luotetussa kansiossa tai kun pyydät niitä esikatselussa.
- **Muoto-valikko:** lihavointi (⌘B), kursiivi (⌘I), yliviivaus (⇧⌘X), koodi (⌥⇧⌘C), linkki (⇧⌘K), otsikot 1–3 (⌥⌘1–⌥⌘3), luettelomerkityt ja numeroidut luettelot, tehtävät, lainaukset ja koodilohkot. Jokainen komento kytkee muotoilun päälle tai pois valinnalle tai riveille ja on yksi peruttava vaihe.
- **Liittäminen.** Word-dokumentista tai verkkosivulta tuleva teksti saapuu Markdown-muodossa ja sen kuvat tiedostoina dokumentin viereen; ⌥⇧⌘V liittää pelkän tekstin.
- **Vienti.** **Tiedosto › Vie** kirjoittaa PDF-tiedoston, yksittäisen HTML-tiedoston, jonka sisällä ovat sen kuvat, tai Word-dokumentin. Verkosta tulevat kuvat tulevat mukaan vain luotetusta kansiosta.
- **Avaa Splatissa.** Jos [Splat](https://github.com/sventmueller/splat) on asennettu, **Tiedosto › Avaa Splatissa** sekä tiedostopuun ja välilehtien kontekstivalikot avaavat dokumentin siinä tallennettuaan sen.

## Sanelu

Sanele AI-välilehden viestikenttään tai editoriin: siellä olevalla mikrofonipainikkeella, valinnalla **Muokkaa › Aloita sanelu** (⌃⌥D) tai fn-näppäimellä (pidä painettuna nauhoittaaksesi, kaksoisnapauta, jotta nauhoitus jatkuu). Puheentunnistus on sama kuin [Lowspeak](https://github.com/sventmueller/lowspeak)-sovelluksessa; Lowspeakia itseään ei tarvitse asentaa.

- **Asetukset › Sanelu** määrittää tilan ja mallin (verkossa OpenAI:lla tai Groqilla, paikallisesti Whisperillä tai Parakeetilla), kielen, jälkikäsittelyn, mikrofonin ja oman sanastosi.
- **Paikalliset mallit** ladataan Hugging Facesta, kun valitset sellaisen, ja lataus näyttää edistymisen ja on peruttavissa. Ne tallentuvat kansioihin, joita Lowspeak käyttää, joten kumpi tahansa sovellus käyttää sitä, minkä toinen latasi. Paikallinen malli pitää nauhoituksen tässä Macissa.
- **Verkkolitterointi** tarvitsee OpenAI- tai Groq-avaimen, jonka Dango säilyttää omassa Avainnipun kohteessaan.
- **Lowspeakista.** Jos Lowspeakin asetukset poikkeavat oletuksista, Sanelu-välilehti tarjoaa kerran sen asetusten käyttöönottoa ja pyynnöstä sen avaimia; macOS kysyy ennen kuin Dango lukee avaimen (valitse ”Salli”, älä ”Salli aina”).
- **Luvat.** macOS kysyy mikrofonin käyttölupaa ensimmäisellä sanelukerralla. fn-näppäin toimii, kun Dango on aktiivinen sovellus eikä Lowspeak ole käynnissä; se tarvitsee Käyttöapu-luvan, jota Dango pyytää ensimmäisen sanelun jälkeen, ei koskaan käynnistyksen yhteydessä.

## Luottamus

Kansio voi saada ohjelmia käyntiin heti, kun siinä käynnistetään jotain: Git-hookit ja repositorion asetukset, kuten `core.fsmonitor`, kielipalvelimet kansiosta `node_modules`, agenttien hookit, `.mcp.json`. Kun avaat kansion ensimmäistä kertaa, Dango kysyy, luotatko siihen.

- **Luotettu:** Git, kielipalvelimet, virheenjäljittimet, agentit ja CI toimivat tälle kansiolle ja kaikelle sen alla olevalle.
- **Rajoitettu:** voit lukea ja muokata; editorin muutosmerkit saadaan lukemalla `.git`-kansiota suoraan. Mitään kansion määrittämää ei käynnistetä. Git-, CI- ja AI-paneelit kertovat tästä ja tarjoavat kansioon luottamista.

Teksti, joka ei tule sinulta, voi sisältää ohjeita, joiden tarkoitus on ohjailla agenttia (kehoteinjektio, prompt injection). Siksi päätteen tuloste (`@terminal`), selainsivu (`@browser`), agentille annettu CI-loki, projektin ulkopuolinen tiedosto ja verkkosivulta liitetty pidempi teksti välitetään merkittyinä: merkinnässä lukee ”Ulkopuolinen sisältö”, ja agentti saa tekstin huomautuksella, että se on epäluotettavaa dataa, jonka ohjeita se ei saa noudattaa. Oman agentin verkkosivut tulevat samalla huomautuksella. Liitetty teksti katsotaan verkkosivulta tulleeksi, kun selain ilmoittaa sen leikepöydällä, kuten Safari, Chrome ja Firefox tekevät; muualta tuleva teksti ja muutaman sanan lyhyt fraasi pysyvät omana tekstinäsi.

## Hiekkalaatikko

Dango itse toimii ilman App Sandboxia, koska jokainen sen käynnistämä ohjelma perisi sen: päätteet, Git, kielipalvelimet ja agentit eivät voisi toimia. Sen sijaan jokaisessa AI-ketjussa on hiekkalaatikkokytkin, joka on oletuksena pois päältä. Se, mitä kytkin rajoittaa, riippuu agentista, ja kytkimen työkaluvihje kertoo sen:

| Agentti | Kun hiekkalaatikko on päällä |
|---|---|
| Claude | shell-komennot toimivat Clauden omassa hiekkalaatikossa; sen tiedosto- ja verkkotyökalut eivät |
| Codex | komennot ja muutokset kirjoittavat vain projektiin, ilman verkkoa |
| Oma agentti | tiedostot, verkkopyynnöt ja komennot pysyvät projektissa ja sallituissa kansioissa, ilman verkkoa (`sandbox-exec`) |
| Antigravity, OpenCode | ei hiekkalaatikkoa, jonka Dango voisi kytkeä päälle |

## Ulkoasu ja käyttöapu

- **Tumma tila** seuraa järjestelmää; **Lisää kontrastia** ja **Vähennä liikettä** otetaan huomioon.
- **VoiceOver** nimeää sivupalkin, välilehdet, AI-aikajanan kortit, Git- ja CI-rivit, virheenjäljittimen ja päätteen tekstin. Keskeytyskohtia ei koskaan erotella toisistaan pelkän värin perusteella.
- **Näppäimistö.** Jokaisella komennolla on pikanäppäin, ja kaikkia voi muuttaa tiedostossa `keybindings.json`; ⌥⌘\` siirtyy seuraavaan paneeliin.
- **Oikealta vasemmalle.** Arabiaksi ja hepreaksi ikkuna peilataan, mutta koodinäkymät pysyvät vasemmalta oikealle.

Mitä Dango lisää AppKitin päälle alue kerrallaan, kerrotaan tiedostossa [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Kielet

Käyttöliittymä on käännetty 35 kielelle: arabia, englanti, espanja, heprea, hindi, hollanti, indonesia, italia, japani, katalaani, kiina (yksinkertaistettu), kiina (perinteinen), korea, kreikka, kroatia, malaiji, norja, portugali (Brasilia), portugali (Portugali), puola, ranska, romania, ruotsi, saksa, slovakki, sloveeni, suomi, tanska, thai, turkki, tšekki, ukraina, unkari, venäjä ja vietnam.

Dango seuraa kielijärjestystä kohdassa **Järjestelmäasetukset › Yleiset › Kieli ja alue**; voit myös valita siellä kielen pelkästään Dangolle kohdassa **Apit**. Jos omalle kielellesi ei ole käännöstä, se käyttää englantia. Termit noudattavat yhtä sanastoa, [docs/glossary.md](../../../docs/glossary.md), ja Applen omia käännöksiä, joten Dango tuntuu samalta kuin muu Macisi.

## Pikanäppäimet

| Toiminto | Pikanäppäin |
|---|---|
| Avaa kansio, uusi ikkuna, sulje ikkuna | ⌘O, ⇧⌘N, ⇧⌘W |
| Uusi tiedosto, tallenna, tallenna nimellä | ⌘N, ⌘S, ⇧⌘S |
| Uusi välilehti, sulje välilehti, seuraava ja edellinen välilehti | ⌘T, ⌘W, ⇧⌘] ja ⇧⌘[ |
| Avaa nopeasti, etsi projektista | ⌘P, ⇧⌘F |
| Tiedostot, Haku, Agentit, Git, Virheenjäljitys, CI | ⌘1–⌘6 |
| Näytä tai kätke sivupalkki, seuraava paneeli | ⌃⌘S, ⌥⌘\` |
| Taakse- ja eteenpäin muokattujen kohtien läpi | ⌃- ja ⌃⇧- |
| Kommentoinnin vaihtaminen, siirry riville | ⌘/, ⌃G |
| Seuraava esiintymä, kohdistin ylle ja alle | ⌘D, ⌃⇧↑ ja ⌃⇧↓ |
| Laskosta, avaa laskos, laskosta kaikki, avaa kaikki laskokset | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Rivitys, Tarkastus | ⌥Z, ⌥⌘I |
| Täydennys, hover-tiedot, siirry määrittelyyn | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Viittaukset, kutsuhierarkia, nimeä uudelleen | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symbolit tiedostossa, projektissa | ⇧⌘O, ⌥⌘O |
| Pikakorjaus, muotoile | ⌥⌘A, ⌥⇧F |
| Seuraava ja edellinen ongelma | ⌘' ja ⇧⌘' |
| Markdown-esikatselu, liitä pelkkänä tekstinä | ⇧⌘V, ⌥⇧⌘V |
| Lihavointi, kursiivi, yliviivaus, koodi, linkki | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Otsikko 1–3 | ⌥⌘1–⌥⌘3 |
| Uusi pääte, tyhjennä pääte | ⌃\`, ⌘K |
| Kohdistus agentin viestikenttään, kommentoi suunnitelmaa | ⌘Esc, ⌥⌘K |
| Osoitekenttä, lataa uudelleen, takaisin, eteenpäin (selain) | ⌘L, ⌘R, ⌘[, ⌘] |
| Aloita tai lopeta sanelu | ⌃⌥D |
| Keskeytyskohta, kaikki keskeytyskohdat päälle tai pois | ⌘\\, ⌘Y |
| Jatka, askella yli, sisään, ulos, pysäytä | ⌃⌘Y, F6, F7, F8, ⌘. |
| Asetukset | ⌘, |

`keybindings.json` sovelluksen datakansiossa (katso [Missä tiedot ovat](#missä-tiedot-ovat)) muuttaa mitä tahansa niistä.

## Yksityisyys ja turvallisuus

- **Ei telemetriaa,** ei analytiikkaa eikä kaatumisraportteja minnekään. Virheet menevät järjestelmälokiin (Console.app, sovelluksen nimellä).
- **Verkkoa vain siihen, minkä itse käynnistät:** käyttämäsi agentit, kukin yhteydessä omaan palveluunsa; ACP-rekisteri sekä agenttien, kielipalvelimien, virheenjäljityssovittimien ja sanelumallien lataukset, kun pyydät niitä; CI-palvelimet, joille kirjaudut; Copilotin muokkausennusteet, jos valitsit sen; oman agentin palveluntarjoajat; verkkolitterointi, jos valitsit sen; selainvälilehdessä avaamasi sivut; sekä päivitysten etsintä, kun olet sallinut sen.
- **Koodi lähtee Macista** vain sinne, mihin olet valinnut sille palvelun: agentille, Copilotin ennusteisiin, AI:n kirjoittamaan commit-viestiin. Commit-viestiä varten erot lähtevät vasta, kun gitleaks ei ole löytänyt niistä salaisuuksia.
- **Avaimet ja tokenit** pysyvät macOS:n Avainnipussa, eivät koskaan asetustiedostossa. Muiden sovellusten avaimia, kuten Lowspeakin, luetaan vain suostumuksellasi ja vain kerran.
- **Kiinnitetyt lataukset.** Dangon asentamat agentit, kielipalvelimet ja virheenjäljityssovittimet on kiinnitetty testattuihin versioihin ja tarkistettu tarkistussummiaan vasten.
- **Koodin allekirjoitus** Hardened Runtimella; päivitykset asentuvat vain sovelluksen omalla allekirjoituksella.

## Missä tiedot ovat

`<id>` on paketin tunniste (bundle identifier), julkaisussa `local.agentide` (debug-käännöksissä `local.agentide.debug`). Se ei sisällä näyttönimeä, joten nimen muuttaminen säilyttää kaikki tiedot.

| Paikka | Mitä |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (AI-paneelin luettelo), `agent-threads/` (oman agentin keskustelut), `layouts/` (välilehdet projekteittain), `recovery/` (tallentamattomat editorit kaatumisen jälkeen), `checkpoints/` (kansioille ilman Gitiä), `worktrees/`, `agents/` ja `tools/` (asennetut agentit, kielipalvelimet ja virheenjäljityssovittimet), `task-output/` |
| `~/Library/Caches/<id>/` | liitetyt kuvat, komentotulkin integrointiskriptit, ACP-rekisteri; voi poistaa |
| `~/Library/Logs/<id>/dictation/` | sanelun lokitiedostot |
| `~/Library/Application Support/Lowspeak` ja `FluidAudio` | paikalliset sanelumallit, yhteiset Lowspeakin kanssa |
| `~/Library/WebKit/<id>/` | selainvälilehden evästeet ja verkkosivustojen tiedot |
| `<project>/.dango/settings.json` | projektin omat asetukset |
| projektin Git-repositorio | tarkistuspisteet commiteina polussa `refs/ide-checkpoints/`; tavallinen push tai fetch ei koskaan siirrä niitä |
| Avainnippu, palvelu `<id>` | oman agentin ja sanelun API-avaimet, CI-tokenit |
| `~/.local/bin/dango` | päätekomento, jos asensit sen |

Claude säilyttää omat istuntonsa kansiossa `~/.claude/projects`; Dango lukee niitä jatkaakseen istuntoa mutta ei muuta niitä.

## Tunnetut rajoitukset

- **Ei App Sandboxia** itse sovellukselle, koska päätteet, Git, kielipalvelimet ja agentit perisivät sen; kansioon on luotettava, ennen kuin mitään sen määrittämää ajetaan. Antigravityllä ja OpenCodella ei ole hiekkalaatikkoa, jonka Dango voisi kytkeä päälle.
- **Clauden käyttöluvut ja kontekstin erittely** perustuvat sen protokollan kokeellisiksi merkittyihin osiin, ja ne piilotetaan, kun niitä ei ole.
- **Keskustelun kelaus taaksepäin** toimii vain Claudella; muilla agenteilla kelataan taaksepäin vain koodi.
- **CI-kirjautuminen selaimen kautta** on pois päältä; käytä `gh`- tai `glab`-komentoa, tokenia tai ympäristöä.
- **Gemini CLI:tä ei tarjota:** Google lopetti sen kirjautumisen henkilökohtaisella tilillä ja ohjaa käyttämään Antigravityä. Aiemmat Gemini-ketjut pysyvät luettavina.

## Kääntäminen lähdekoodista

Tarvitset:

- macOS 14:n tai uudemman ja Xcode Command Line Tools -paketin (`xcode-select --install`); itse Xcodea ei tarvita.
- Rustin siinä versiossa, jonka `rust-toolchain.toml` nimeää. Homebrew’n `rust` kääntää tämän Macin suorittimelle; julkaisun universaali käännös tarvitsee rustupin ja kohteet `aarch64-apple-darwin` ja `x86_64-apple-darwin`.
- Kloonit repositorioista [splat](https://github.com/sventmueller/splat) ja [lowspeak](https://github.com/sventmueller/lowspeak) tämän viereen (`../splat`, `../lowspeak`), molemmat `main`-haarassaan; työtila ottaa niistä crateja polun kautta.
- cmaken jokaiseen käännökseen, jossa on sanelu (`make run`, `make build`, `make release`); se kääntää whisper.cpp:n. `make check` kääntää ilman sitä.

| Komento | Mitä se tekee |
|---|---|
| `make run` | kääntää debug-sovelluksen (`target/debug/Dango Debug.app`) ja avaa sen |
| `make build` | kääntää julkaisusovelluksen (`target/release/Dango.app`), allekirjoitettuna paikallisella varmenteella, jonka ensimmäinen käännös luo kirjautumisavainnippuun; kasvattaa käännösnumeroa tiedostossa `VERSION` |
| `make lint` | muotoilu ja clippy; varoitukset kaatavat sen |
| `make check` | `lint`, sitten kaikki testit; ei tarvitse avainnippua, ikkunaa eikä verkkoa |
| `make test` | `check`, sitten itsetesti `probe` ja saavutettavuustarkastus, vasemmalta oikealle ja oikealta vasemmalle, oikeassa sovelluksessa |
| `make perf` | ajoitusbudjetit julkaisukäännöksessä; mielekäs vain muuten joutilaassa Macissa |
| `make release` | julkaisun kuivaharjoitus kansioon `target/release-dry-run/`, allekirjoitettuna paikallisella varmenteella; ei lähetä mitään minnekään |
| `make release-notarized` | `check`, sitten itse julkaisu kansioon `dist/`: Developer ID -allekirjoitus, notarisointi, sovelluksen omalla avaimella allekirjoitettu päivityssyöte; kasvattaa käännösnumeroa |

Debug- ja julkaisuversio ovat erillisiä sovelluksia omine asetuksineen ja tietoineen. Kääntäminen, testaus, itsetestit, julkaisu ja koodin noudattamat säännöt kuvataan tiedostossa [docs/development.md](../../../docs/development.md), niiden taustalla olevat päätökset tiedostossa [docs/plan.md](../../../docs/plan.md). Julkaisutiedot ovat tiedostossa [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), käännettyinä kunkin README-tiedoston viereen kansioon `assets/readme/<language>/`.

## Lisenssi

Dangon lisenssi on jompikumpi seuraavista

- Apache-lisenssi, versio 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT-lisenssi ([LICENSE-MIT](../../../LICENSE-MIT))

valintasi mukaan.

Ellet nimenomaisesti toisin ilmoita, jokainen Dangoon sisällytettäväksi tarkoituksella toimittamasi lahjoitus, sellaisena kuin se on määritelty Apache-2.0-lisenssissä, lisensoidaan kaksoislisenssillä kuten yllä ilman lisäehtoja.
