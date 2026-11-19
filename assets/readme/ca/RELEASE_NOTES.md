## Memòria
- Les sessions llargues d’agents necessiten molta menys memòria:
  - En obrir una sessió anterior, se’n llegeix la transcripció línia a línia; una sessió amb una transcripció de 100 MB
    ara arriba com a màxim a uns 20 MB en lloc de 65 MB.
  - Una crida d’eina acabada només conserva el que mostra la seva targeta: les primeres 400 línies de la sortida, i ja
    no l’entrada en brut un cop la targeta mostra els seus camps. “N línies més” continua comptant cada línia.
  - Una pestanya d’IA que queda fora de la vista durant 30 segons allibera les seves targetes i, quan torna a
    mostrar-se, construeix les que són a la vista, al lloc on era.
  - Després de tancar una pestanya o d’obrir una sessió anterior, Dango torna a macOS la memòria que ha alliberat.
- Les llistes ja no creixen en memòria quan s’actualitzen. A macOS 27, cada actualització d’una llista amb files
  desplegables deixava enrere per sempre un triangle desplegable, i cada actualització deixava enrere una mica de cada
  fila:
  - Les llistes de CI, Contenidors, Software Factory, Proves, Depura, Git i Cerca i la jerarquia de crides dibuixen les
    seves pròpies fletxes.
  - Les llistes fora de la vista ja no s’actualitzen; es posen al dia quan es mostren.
  - Les llistes Sessions, CI, Software Factory i Contenidors conserven les files que no han canviat, i Git només
    actualitza les files dels fitxers l’estat dels quals ha canviat.
- Els taulers d’una finestra tancada s’alliberen amb ella.

## Pestanyes d’IA
- “Mostra la sortida” d’una ordre en segon pla que escriu ella mateixa la sortida en un fitxer, com ara
  `make > build.log 2>&1`, mostra aquest fitxer quan la CLI no té res de la sortida.
