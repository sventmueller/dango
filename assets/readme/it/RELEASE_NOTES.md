## Sicurezza
- I link nelle risposte, nei piani e nei resoconti di un agente, nell’anteprima Markdown e nei PDF non aprono più
  alcuna app con un solo clic. Gli indirizzi web si aprono in un pannello del browser, i file all’interno del progetto
  in un pannello dell’editor; tutto il resto, come un file altrove, una condivisione di rete o il link di un’altra app,
  chiede prima conferma e mostra dove porta.
- Gli indirizzi di accesso e di Remote Control che un agente o un server MCP consegna si aprono solo se sono indirizzi
  web.
- Le cartelle che contengono molto più di un progetto, come la cartella Inizio, la Scrivania, Documenti, Download o la
  radice di un volume, non possono più essere considerate attendibili nel loro insieme; fidati dei progetti uno per
  uno. La domanda sulla fiducia ora dice che questa vale anche per le cartelle al suo interno.
- I token della CI e gli accessi non passano mai in semplice http verso un server diverso da questo Mac; a un
  repository il cui remote inizia con `http://` la richiesta arriva via https.
- Senza gitleaks, le previsioni di modifica aspettano prima che il codice vada a un provider esterno a questo Mac,
  finché non scegli di inviarlo senza controllo. Le impostazioni di gitleaks di un progetto non disattivano più il
  controllo.
- L’agente integrato non può più essere indotto a scrivere fuori dal progetto tramite un link che compare dopo che la
  sua scrittura è stata approvata, e i file con altri hard link ricevono un file tutto loro invece di essere scritti
  attraverso il link.
- I completamenti di un server di linguaggio non possono più bloccare l’editor, e le sue rinomine ed eliminazioni di un
  file collegato agiscono sul link, non su ciò a cui punta.
- Le immagini che alleghi a un prompt vengono decodificate dal servizio di decodifica in sandbox, come ovunque.
- Un programma nel terminale può sostituire gli appunti solo mentre il suo terminale è in primo piano, e il terminale
  lo segnala.
- rust-analyzer si installa da una release fissata con il suo checksum, invece che dall’ultima disponibile.

## Ricerca e navigazione
- Apertura rapida dice quando nessun file corrisponde e mostra se cerca tra i file o tra i simboli del progetto.
- I simboli del progetto includono TypeScript e JavaScript senza che uno dei loro file sia aperto. Le corrispondenze
  esatte del progetto e del linguaggio in cui lavori vengono per prime; i simboli delle librerie e dei pacchetti
  installati sono contrassegnati.
- Vista › Ricerca porta il focus nel campo di ricerca.

## Editor
- L’editor mantiene il focus quando un file Markdown si apre con l’anteprima accanto, così il menu Formato funziona
  subito. I comandi dell’editor funzionano anche mentre il focus è nella barra laterale.
- Una proposta alla fine di una riga o in un riquadro sotto di essa indica che Tab la accetta.
- I pannelli con lo stesso nome nella panoramica dei pannelli si distinguono, per esempio un file e la sua anteprima
  nel browser.
- Il menu Vista non offre più “Nascondi barra dei pannelli” di macOS, che i pannelli di Dango non usano.

## Linguaggi e debug
- Il server JSON ora controlla i file, e i server che registrano la loro diagnostica dopo l’avvio, come basedpyright e
  sourcekit-lsp, la mostrano.
- Dove pip si rifiuta di installare in Python, come fa quello di Homebrew, il debugger propone di installare debugpy
  in un ambiente virtuale del progetto.
