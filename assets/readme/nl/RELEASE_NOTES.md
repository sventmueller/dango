## Geheugen
- Lange agentsessies hebben veel minder geheugen nodig:
  - Bij het openen van een eerdere sessie wordt het transcript regel voor regel gelezen; een sessie met een transcript
    van 100 MB piekt nu op ongeveer 20 MB in plaats van 65 MB.
  - Een afgeronde toolaanroep bewaart alleen wat de kaart ervan toont: de eerste 400 regels van de uitvoer, en niet
    langer de ruwe invoer zodra de kaart haar velden toont. De melding 'nog N regels' telt nog altijd elke regel.
  - Een AI-tabblad dat 30 seconden uit beeld blijft, laat zijn kaarten los en bouwt bij het opnieuw tonen de kaarten in
    beeld op, op de plek waar het was.
  - Na het sluiten van een tabblad of het openen van een eerdere sessie geeft Dango het vrijgekomen geheugen terug aan
    macOS.
- Lijsten groeien niet meer in het geheugen als ze worden ververst. Op macOS 27 liet elke verversing van een lijst met
  uitklapbare rijen voorgoed een openklapdriehoekje achter, en liet elke verversing een beetje van elke rij achter:
  - De lijsten CI, Containers, Software Factory, Tests, Debug, Git en Zoeken en de aanroephiërarchie tekenen hun eigen
    pijltjes.
  - Lijsten die niet in beeld zijn, worden niet meer ververst; ze halen hun achterstand in zodra ze worden getoond.
  - De lijsten Sessies, CI, Software Factory en Containers behouden de rijen die niet zijn veranderd, en Git ververst
    alleen de rijen van bestanden waarvan de status is veranderd.
- De panelen van een gesloten venster worden samen met het venster vrijgegeven.

## AI-tabbladen
- 'Toon uitvoer' van een achtergrondcommando dat zijn uitvoer zelf naar een bestand schrijft, zoals
  `make > build.log 2>&1`, toont dat bestand als de CLI niets van de uitvoer heeft.
