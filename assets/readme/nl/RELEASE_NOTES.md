## Snelheid en geheugen
- Alles wat een agent start, in welke taal ook, zoals builds, tests, shells en MCP-servers, draait in de
  achtergrondstatus van macOS: met de laagste prioriteit en met beperkte schijf- en netwerktoegang. De agents zelf, met
  de subagents van Claude, draaien met gemiddelde prioriteit. Agents die volop werken, vertragen Dango niet meer; dat
  geldt ook voor de agents van de Software Factory.
- Herstelde vensters starten alleen wat zichtbaar is: een AI-tabblad achter andere start zijn CLI zodra het naar voren
  komt of een prompt krijgt, en browser-, sitevoorvertonings- en terminaltabbladen laden hun pagina of starten hun shell
  zodra ze voor het eerst te zien zijn.
- Een lange zoekopdracht in het project tekent niet meer telkens al zijn resultaten opnieuw terwijl hij loopt. De
  Software Factory, CI-logs en het paneel Tests doen minder zolang ze niet in beeld zijn; tests en debugconfiguraties
  worden gelezen zodra hun paneel voor het eerst verschijnt, en verborgen panelen CI en Containers lezen niets totdat ze
  verschijnen.
- Lange sessies hebben minder geheugen nodig: een afgeronde diff bewaart alleen de regels die zijn kaart toont, een
  beantwoord toestemmingsverzoek laat zijn invoer los, de glyphcache van de terminal groeit niet meer onbeperkt, en de
  panelen Bestanden en Git laten regels los die er niet meer zijn.
- Het programma zelf is ongeveer 40% kleiner.

## Vensters en menu's
- Als je een deelvenster sluit, gaan de tabbladen ervan naar het deelvenster ervoor, anders naar het deelvenster erna,
  in plaats van dat ze sluiten. Alleen het deelvenster van een voorvertoning en het laatste deelvenster sluiten samen
  met hun tabbladen.
- Het menu Weergave toont de panelen in de volgorde van de symbolen in de navigatiekolom.
- 'Beheer agents…' staat niet meer in het +-menu; het blijft onder '+ Nieuwe sessie' in het paneel Agents.

## Opgeloste problemen
- 'Neem bestanden op en sluit ze uit' in het zoekpaneel toont geen verdwaalde letter meer naast de pijl.
- Het sluiten van de opmerkingenpopover van een specificatie leidt niet meer tot een crash.
