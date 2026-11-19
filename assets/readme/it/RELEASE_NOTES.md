## Velocità e memoria
- Tutto ciò che un agente avvia, in qualsiasi linguaggio, come build, test, shell e server MCP, viene eseguito nello
  stato in background di macOS: con la priorità più bassa e con l’accesso a disco e rete limitato. Gli agenti stessi,
  insieme ai subagenti di Claude, vengono eseguiti con priorità media. Gli agenti a pieno carico non rallentano più
  Dango; questo vale anche per gli agenti della Software Factory.
- Le finestre ripristinate avviano solo ciò che si vede: un pannello AI dietro ad altri avvia la sua CLI quando passa in
  primo piano o riceve un prompt, e i pannelli del browser, dell’anteprima del sito e del terminale caricano la loro
  pagina o avviano la loro shell la prima volta che vengono mostrati.
- Una lunga ricerca nel progetto non ridisegna più tutti i suoi risultati mentre è in corso. La Software Factory, i log
  della CI e il riquadro Test lavorano meno quando non sono visibili; test e configurazioni di debug vengono letti la
  prima volta che il loro riquadro viene mostrato, e i riquadri CI e Container nascosti non leggono nulla finché non
  vengono mostrati.
- Le sessioni lunghe richiedono meno memoria: una scheda delle differenze conclusa conserva solo le righe che mostra,
  una richiesta di permesso a cui hai risposto rilascia il suo input, la cache dei glifi del terminale non cresce più
  senza limiti, e i riquadri File e Git rilasciano le righe che non esistono più.
- Il programma stesso è più piccolo di circa il 40%.

## Finestre e menu
- Chiudere un’area sposta i suoi pannelli nell’area precedente, o altrimenti in quella successiva, invece di chiuderli.
  Solo l’area di un’anteprima e l’ultima area si chiudono insieme ai loro pannelli.
- Il menu Vista elenca i riquadri nell’ordine dei simboli della barra laterale.
- “Gestisci agenti…” non è più nel menu +; resta sotto “+ Nuova sessione” nel riquadro Agenti.

## Correzioni
- “Includi ed escludi file” nel riquadro Ricerca non mostra più una lettera di troppo accanto alla sua freccia.
- Chiudere il popover dei commenti di una specifica non causa più un arresto anomalo.
