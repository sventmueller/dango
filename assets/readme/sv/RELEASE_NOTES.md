## Hastighet och minne
- Allt som en agent startar, i vilket språk som helst, till exempel byggen, tester, skal och MCP-servrar, körs i macOS:s
  bakgrundsläge: med lägsta prioritet och med begränsad disk- och nätverksåtkomst. Själva agenterna, tillsammans med
  Claudes underagenter, körs med medelhög prioritet. Agenter under full belastning gör inte längre Dango långsammare;
  det gäller även agenterna i Software Factory.
- Återställda fönster startar bara det som syns: en AI-flik bakom andra startar sitt CLI först när den hamnar längst
  fram eller får en prompt, och flikar för webbläsare, webbplatsförhandsvisning och terminal läser in sin sida eller
  startar sitt skal först när de visas för första gången.
- En lång sökning i projektet ritar inte längre om alla sina träffar medan den pågår. Software Factory, CI-loggar och
  panelen Tester gör mindre medan de inte syns; tester och felsökningskonfigurationer läses in när deras panel visas för
  första gången, och dolda paneler för CI och Containrar läser ingenting förrän de visas.
- Långa sessioner behöver mindre minne: en färdig diff behåller bara de rader som dess kort visar, en besvarad
  behörighetsförfrågan släpper sin indata, terminalens glyfcache växer inte längre obegränsat, och panelerna Filer och
  Git släpper rader som inte längre finns.
- Själva programmet är ungefär 40 % mindre.

## Fönster och menyer
- När en ruta stängs flyttas dess flikar till rutan före den, annars till rutan efter den, i stället för att stängas.
  Bara rutan för en förhandsvisning och den sista rutan stängs tillsammans med sina flikar.
- Menyn Innehåll listar panelerna i samma ordning som symbolerna i sidofältet.
- ”Hantera agenter…” finns inte längre i +-menyn; det finns kvar under ”+ Ny session” i panelen Agenter.

## Felrättningar
- ”Inkludera och exkludera filer” i panelen Sök visar inte längre en lös bokstav bredvid sin pil.
- Att stänga kommentarspopovern för en specifikation leder inte längre till en krasch.
