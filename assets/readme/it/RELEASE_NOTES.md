## Memoria
- Le sessioni lunghe degli agenti richiedono molta meno memoria:
  - All’apertura di una sessione passata, la sua trascrizione viene letta una riga alla volta; una sessione con una
    trascrizione di 100 MB ora raggiunge al massimo circa 20 MB invece di 65 MB.
  - Una chiamata a uno strumento conclusa conserva solo ciò che mostra la sua scheda: le prime 400 righe del suo output,
    e non più l’input grezzo una volta che la scheda mostra i suoi campi. “Altre N righe” conta ancora ogni riga.
  - Un pannello AI che resta fuori dalla vista per 30 secondi rilascia le sue schede e, quando torna visibile,
    costruisce quelle in vista, nel punto in cui si trovava.
  - Dopo la chiusura di un pannello o l’apertura di una sessione passata, Dango restituisce a macOS la memoria liberata.
- Gli elenchi non crescono più in memoria quando si aggiornano. Su macOS 27 ogni aggiornamento di un elenco con righe
  espandibili lasciava dietro di sé per sempre un triangolo di espansione, e ogni aggiornamento lasciava dietro di sé un
  po’ di ogni riga:
  - Gli elenchi CI, Container, Software Factory, Test, Debug, Git e Ricerca e la gerarchia delle chiamate disegnano da
    sé le proprie frecce.
  - Gli elenchi fuori dalla vista non si aggiornano più; si mettono in pari quando vengono mostrati.
  - Gli elenchi Sessioni, CI, Software Factory e Container conservano le righe che non sono cambiate, e Git aggiorna
    solo le righe dei file il cui stato è cambiato.
- I riquadri di una finestra chiusa vengono rilasciati con essa.

## Pannelli AI
- “Mostra output” di un comando in background che scrive da sé il proprio output in un file, come
  `make > build.log 2>&1`, mostra quel file quando la CLI non ha nulla dell’output.
