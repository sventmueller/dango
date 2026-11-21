## Beveiliging
- Links in de antwoorden, plannen en verslagen van een agent, in de Markdown-voorvertoning en in pdf's openen niet meer
  met één klik een app. Webadressen openen in een browsertabblad, bestanden in het project in een editortabblad; al het
  andere, zoals een bestand elders, een netwerkschijf of de link van een andere app, vraagt eerst en toont waar het
  heen leidt.
- Inlog- en Remote Control-adressen die een agent of een MCP-server doorgeeft, openen alleen als het webadressen zijn.
- Mappen die veel meer dan één project bevatten, zoals je thuismap, Bureaublad, Documenten, Downloads of de hoofdmap
  van een volume, kunnen niet meer als geheel worden vertrouwd; vertrouw ze per project. De vraag om vertrouwen zegt nu
  dat het ook voor de mappen erin geldt.
- CI-tokens en inloggegevens gaan nooit via gewoon http naar een andere server dan deze Mac; een repository waarvan de
  remote met `http://` begint, wordt via https bevraagd.
- Zonder gitleaks wachten bewerkingsvoorspellingen voordat code naar een aanbieder buiten deze Mac gaat, totdat je
  kiest om ongecontroleerd te versturen. De eigen gitleaks-instellingen van een project schakelen de controle niet meer
  uit.
- De ingebouwde agent kan niet meer buiten het project laten schrijven via een link die verschijnt nadat het schrijven
  is goedgekeurd, en bestanden met nog andere harde links krijgen een eigen bestand in plaats van dat erdoorheen wordt
  geschreven.
- De aanvullingen van een taalserver kunnen de editor niet meer laten vastlopen, en het wijzigen van de naam en het
  verwijderen van een gekoppeld bestand door de taalserver werken op de link, niet op waar die naar verwijst.
- Afbeeldingen die je aan een prompt toevoegt, worden gedecodeerd door de decodeerdienst in de sandbox, net als overal.
- Een programma in de terminal kan het klembord alleen vervangen zolang zijn terminal vooraan staat, en de terminal
  meldt dat.
- rust-analyzer wordt geïnstalleerd vanuit een release die met zijn checksum is vastgelegd, in plaats van wat toen het
  nieuwst was.

## Zoeken en navigeren
- Open snel meldt wanneer geen bestand overeenkomt en toont of het naar bestanden of naar de symbolen van het project
  zoekt.
- De symbolen van het project omvatten TypeScript en JavaScript zonder dat een bestand ervan open is. Exacte
  overeenkomsten uit het project en in de taal waarin je werkt, komen eerst; symbolen uit bibliotheken en
  geïnstalleerde pakketten zijn gemarkeerd.
- Weergave › Zoeken zet de focus in het zoekveld.

## Editor
- De editor houdt de focus wanneer een Markdown-bestand opent met de voorvertoning ernaast, zodat het menu Opmaak meteen
  werkt. Editorcommando's werken ook terwijl de focus in de navigatiekolom staat.
- Een voorstel aan het eind van een regel of in een vak eronder zegt dat Tab het overneemt.
- Tabbladen met dezelfde naam in het tabbladoverzicht zijn van elkaar te onderscheiden, bijvoorbeeld een bestand en zijn
  voorvertoning in de browser.
- Het menu Weergave biedt niet meer het eigen 'Verberg tabbladbalk' van macOS aan, dat de tabbladen van Dango niet
  gebruiken.

## Talen en debuggen
- De JSON-server controleert nu bestanden, en servers die hun diagnoses pas na het starten registreren, zoals
  basedpyright en sourcekit-lsp, tonen ze.
- Waar pip weigert in Python te installeren, zoals bij die van Homebrew, biedt de debugger aan debugpy in een virtuele
  omgeving van het project te installeren.
