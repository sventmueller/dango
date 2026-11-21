## Velocità
- Ciò che un agente avvia, come build, test e shell, viene eseguito con la priorità più bassa invece che nello stato in
  background di macOS. Dango ha sempre la precedenza e gli agenti vengono prima di ciò che avviano, ma le loro build
  usano di nuovo i core ad alte prestazioni e tutta la velocità del disco; lo stato in background aveva ridotto il loro
  throughput a circa un decimo, anche su un Mac altrimenti inattivo.

## Agenti
- Il pallino di un pannello AI resta verde finché un comando in background della sua sessione è ancora in esecuzione, e
  l’avviso che una sessione è terminata arriva solo quando è terminato anche il suo lavoro in background.
