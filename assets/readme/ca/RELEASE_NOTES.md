## Pestanyes d’IA
- “Mostra la sortida” d’una ordre en segon pla obre una pestanya que segueix la sortida mentre l’ordre s’executa, en
  lloc d’una còpia del moment del clic. La pestanya s’obre al final de la sortida i s’hi queda a mesura que arriben
  línies noves, fins que et desplaces cap amunt; si tornes a desplaçar-te fins al final, la torna a seguir. Si la
  pestanya ja és oberta, “Mostra la sortida” la porta al davant, al final.
  - Una ordre que escriu ella mateixa la sortida en un fitxer, com ara `make > build.log 2>&1`, mostra aquest fitxer a
    mesura que creix.
  - Quan l’ordre acaba, la pestanya en mostra tota la sortida; “Obre a l’editor” obre el fitxer que hi ha al darrere.
  - Una pestanya només llegeix la sortida mentre és al davant.
