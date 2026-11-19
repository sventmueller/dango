## Minne
- Långa agentsessioner behöver mycket mindre minne:
  - När en tidigare session öppnas läses dess transkript en rad i taget; en session med ett transkript på 100 MB når nu
    som mest ungefär 20 MB i stället för 65 MB.
  - Ett avslutat verktygsanrop behåller bara det som dess kort visar: de första 400 raderna av utdata, och inte längre
    rå indata när kortet visar sina fält. ”N rader till” räknar fortfarande varje rad.
  - En AI-flik som är utom synhåll i 30 sekunder släpper sina kort, och när den visas igen bygger den upp dem som syns,
    på samma ställe som den var.
  - När en flik stängs eller en tidigare session öppnas lämnar Dango tillbaka det frigjorda minnet till macOS.
- Listor växer inte längre i minnet när de uppdateras. I macOS 27 lämnade varje uppdatering av en lista med rader som
  kan öppnas kvar en visningstriangel för gott, och varje uppdatering lämnade kvar lite av varje rad:
  - Listorna CI, Containrar, Software Factory, Tester, Felsök, Git och Sök samt anropshierarkin ritar sina egna pilar.
  - Listor som inte syns uppdateras inte längre; de kommer ikapp när de visas.
  - Listorna Sessioner, CI, Software Factory och Containrar behåller raderna som inte har ändrats, och Git uppdaterar
    bara raderna för filer vars status har ändrats.
- Panelerna i ett stängt fönster släpps tillsammans med det.

## AI-flikar
- ”Visa utdata” för ett bakgrundskommando som självt skriver sina utdata till en fil, till exempel
  `make > build.log 2>&1`, visar den filen när CLI inte har något av utdata.
