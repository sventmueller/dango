## Sikkerhet
- Lenker i svarene, planene og rapportene til en agent, i Markdown-forhåndsvisningen og i PDF-er åpner ikke lenger en
  app med ett klikk. Nettadresser åpnes i en nettleserfane, filer i prosjektet i en redigeringsfane; alt annet, som en
  fil et annet sted, en nettverksdeling eller lenken til en annen app, spør først og viser hvor den fører.
- Påloggings- og Remote Control-adresser som en agent eller en MCP-server gir videre, åpnes bare når de er
  nettadresser.
- Mapper som inneholder mye mer enn ett prosjekt, som hjemmemappen, Skrivebord, Dokumenter, Nedlastinger eller roten av
  et volum, kan ikke lenger klareres som en helhet; stol på dem prosjekt for prosjekt. Spørsmålet om tillit sier nå at
  den også gjelder mappene i den.
- CI-tokener og pålogginger går aldri over ren http til en annen tjener enn denne Macen; et repository med en remote
  som begynner med `http://`, blir spurt over https.
- Uten gitleaks venter redigeringsforutsigelser før koden går til en leverandør utenfor denne Macen, til du velger å
  sende ukontrollert. Et prosjekts egne gitleaks-innstillinger slår ikke lenger av kontrollen.
- Den innebygde agenten kan ikke lenger lures til å skrive utenfor prosjektet gjennom en lenke som dukker opp etter at
  skrivingen ble godkjent, og filer med flere harde lenker får en egen fil i stedet for å bli skrevet gjennom.
- Fullføringene til en språktjener kan ikke lenger få editoren til å henge, og når den gir nytt navn til eller sletter
  en lenket fil, gjelder det lenken, ikke det den peker på.
- Bilder du legger ved en prompt, dekodes av dekodingstjenesten i sandkassen, som overalt ellers.
- Et program i terminalen kan bare erstatte utklippstavlen mens terminalen er foran, og terminalen sier fra om det.
- rust-analyzer installeres fra en utgivelse som er festet med sjekksummen sin, i stedet for den som tilfeldigvis var
  nyest.

## Søk og navigasjon
- Åpne raskt sier fra når ingen fil passer, og viser om det søker i filer eller i prosjektets symboler.
- Prosjektets symboler omfatter TypeScript og JavaScript uten at noen av filene deres er åpne. Eksakte treff fra
  prosjektet og i språket du jobber i, kommer først; symboler fra biblioteker og installerte pakker er merket.
- Vis › Søk setter fokus i søkefeltet.

## Editor
- Editoren beholder fokus når en Markdown-fil åpnes med forhåndsvisningen ved siden av, så menyen Format virker med en
  gang. Editorkommandoer virker også mens fokus er i sidepanelet.
- Et forslag på slutten av en linje eller i en boks under den sier at Tab tar det.
- Faner med samme navn i faneoversikten skilles fra hverandre, for eksempel en fil og forhåndsvisningen av den i
  nettleseren.
- Menyen Vis tilbyr ikke lenger macOS’ egen «Skjul fanelinje», som fanene i Dango ikke bruker.

## Språk og feilsøking
- JSON-tjeneren sjekker nå filer, og tjenere som registrerer diagnosene sine etter at de har startet, som basedpyright
  og sourcekit-lsp, viser dem.
- Der pip nekter å installere i Python, slik Homebrew sin gjør, tilbyr feilsøkeren å installere debugpy i et virtuelt
  miljø for prosjektet.
