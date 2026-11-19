## Velocitat i memòria
- Tot el que inicia un agent, en qualsevol llenguatge, com ara compilacions, proves, intèrprets d’ordres i servidors
  MCP, s’executa en l’estat de segon pla de macOS: amb la prioritat més baixa i amb l’accés al disc i a la xarxa
  limitat. Els mateixos agents, amb els subagents de Claude, s’executen amb prioritat mitjana. Els agents a plena
  càrrega ja no alenteixen Dango; això també val per als agents de la Software Factory.
- Les finestres restaurades només inicien el que es veu: una pestanya d’IA que és darrere d’altres inicia la seva CLI
  quan passa al davant o rep un prompt, i les pestanyes de navegador, de previsualització del lloc web i de terminal
  carreguen la pàgina o inicien l’intèrpret d’ordres la primera vegada que es mostren.
- Una cerca llarga al projecte ja no torna a dibuixar tots els resultats mentre s’executa. La Software Factory, els logs
  de CI i el tauler Proves fan menys feina mentre no es veuen; les proves i les configuracions d’inici es llegeixen la
  primera vegada que es mostra el seu tauler, i els taulers CI i Contenidors ocults no llegeixen res fins que es
  mostren.
- Les sessions llargues necessiten menys memòria: un diff acabat conserva només les línies que mostra la seva targeta,
  una sol·licitud de permís ja contestada allibera la seva entrada, la memòria cau de glifs del terminal ja no creix
  sense límit, i els taulers Fitxers i Git alliberen les files que ja no existeixen.
- El programa mateix ocupa al voltant d’un 40 % menys.

## Finestres i menús
- En tancar una àrea, les seves pestanyes ja no es tanquen: passen a l’àrea anterior o, si no n’hi ha, a la següent.
  Només l’àrea d’una previsualització i l’última àrea es tanquen amb les seves pestanyes.
- El menú Mostra llista els taulers en l’ordre de les icones de la barra lateral.
- “Gestiona els agents…” ja no és al menú +; continua sota “+ Nova sessió” al tauler Agents.

## Correccions
- “Inclou i exclou fitxers” al tauler Cerca ja no mostra una lletra solta al costat de la fletxa.
- Tancar la finestra emergent de comentaris d’una especificació ja no provoca cap fallada.
