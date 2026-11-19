## AI-tabbladen
- 'Toon uitvoer' bij een achtergrondcommando opent een tabblad dat de uitvoer volgt zolang het commando loopt, in plaats
  van een kopie ervan op dat moment. Het tabblad opent aan het eind van de uitvoer en blijft daar terwijl er nieuwe
  regels bijkomen, totdat je omhoog scrolt; scrol je terug naar het eind, dan volgt het weer. 'Toon uitvoer' bij een
  tabblad dat al open is, haalt het naar voren, aan het eind van de uitvoer.
  - Een commando dat zijn uitvoer zelf naar een bestand schrijft, zoals `make > build.log 2>&1`, toont dat bestand
    terwijl het groeit.
  - Zodra het commando klaar is, toont het tabblad de volledige uitvoer; 'Open in editor' opent het bestand erachter.
  - Een tabblad leest de uitvoer alleen zolang het vooraan staat.
