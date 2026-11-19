## Pannelli AI
- “Mostra output” di un comando in background apre un pannello che segue l’output finché il comando è in esecuzione,
  invece di una sua copia di quel momento. Il pannello si apre alla fine dell’output e ci resta mentre arrivano nuove
  righe, finché non scorri verso l’alto; se torni a scorrere fino alla fine, riprende a seguirlo. “Mostra output” su un
  pannello già aperto lo porta in primo piano, alla fine dell’output.
  - Un comando che scrive da sé il proprio output in un file, come `make > build.log 2>&1`, mostra quel file man mano
    che cresce.
  - Quando il comando termina, il pannello mostra tutto il suo output; “Apri nell’editor” apre il file che c’è dietro.
  - Un pannello legge l’output solo mentre è in primo piano.
