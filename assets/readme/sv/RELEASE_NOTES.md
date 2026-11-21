## Säkerhet
- Länkar i en agents svar, planer och rapporter, i Markdown-förhandsvisningen och i PDF:er öppnar inte längre någon app
  med ett enda klick. Webbadresser öppnas i en webbläsarflik, filer i projektet i en redigerarflik; allt annat, till
  exempel en fil någon annanstans, en nätverksdelning eller en annan apps länk, frågar först och visar vart det leder.
- Adresser för inloggning och Remote Control som en agent eller en MCP-server lämnar över öppnas bara när de är
  webbadresser.
- Mappar som innehåller mycket mer än ett projekt, till exempel hemmappen, Skrivbord, Dokument, Hämtade filer eller
  roten av en volym, kan inte längre betros som helhet; lita på dem projekt för projekt. Frågan om förtroende säger nu
  att förtroendet även gäller mapparna i den.
- CI-tokens och inloggningar skickas aldrig över okrypterad http till en annan server än den här Macen; ett repo vars
  remote börjar med `http://` frågas över https.
- Utan gitleaks väntar förutsägelser vid redigering innan kod går till en leverantör utanför den här Macen, tills du
  väljer att skicka okontrollerat. Ett projekts egna gitleaks-inställningar stänger inte längre av kontrollen.
- Den egna agenten kan inte längre ledas att skriva utanför projektet genom en länk som dyker upp efter att dess
  skrivning godkänts, och filer med fler hårda länkar får en egen fil i stället för att skrivas igenom.
- En språkservers kodkomplettering kan inte längre låsa redigeraren, och när den byter namn på eller raderar en länkad
  fil påverkas länken, inte det den pekar på.
- Bilder som du bifogar till en prompt avkodas av avkodningstjänsten i sandlådan, precis som överallt annars.
- Ett program i terminalen kan bara ersätta urklippet medan dess terminal ligger längst fram, och terminalen säger det.
- rust-analyzer installeras från en version som är låst med sin kontrollsumma i stället för den som råkade vara senast.

## Sök och navigering
- ”Öppna snabbt” säger när ingen fil matchar och visar om den söker bland filer eller projektets symboler.
- Projektets symboler omfattar TypeScript och JavaScript utan att någon av deras filer är öppen. Exakta träffar från
  projektet och i det språk du arbetar i kommer först; symboler från bibliotek och installerade paket är markerade.
- Innehåll › Sök sätter fokus i sökfältet.

## Redigerare
- Redigeraren behåller fokus när en Markdown-fil öppnas med sin förhandsvisning bredvid, så menyn Format fungerar
  direkt. Redigerarens kommandon fungerar också medan fokus är i sidofältet.
- Ett förslag i slutet av en rad eller i en ruta under den säger att Tab tar emot det.
- Flikar med samma namn i fliköversikten går att skilja åt, till exempel en fil och dess förhandsvisning i webbläsaren.
- Menyn Innehåll erbjuder inte längre macOS:s egna ”Göm flikfältet”, som Dangos flikar inte använder.

## Språk och felsökning
- JSON-servern kontrollerar nu filer, och servrar som registrerar sin diagnostik efter att de startat, till exempel
  basedpyright och sourcekit-lsp, visar den.
- Där pip vägrar installera i Python, som Homebrews Python gör, erbjuder felsökaren att installera debugpy i en virtuell
  miljö för projektet.
