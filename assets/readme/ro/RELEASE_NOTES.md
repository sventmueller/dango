## Memorie
- Sesiunile lungi ale agenților au nevoie de mult mai puțină memorie:
  - La deschiderea unei sesiuni anterioare, transcrierea ei este citită linie cu linie; o sesiune cu o transcriere de
    100 MB ajunge acum la cel mult aproximativ 20 MB în loc de 65 MB.
  - Un apel de instrument încheiat păstrează doar ce arată cardul său: primele 400 de linii ale ieșirii, iar intrarea
    brută nu o mai păstrează odată ce cardul își arată câmpurile. Eticheta „linii suplimentare: N” numără în continuare
    fiecare linie.
  - O filă IA care rămâne în afara vederii timp de 30 de secunde își eliberează cardurile, iar când se arată din nou le
    construiește pe cele vizibile, în locul în care era.
  - După închiderea unei file sau deschiderea unei sesiuni anterioare, Dango returnează sistemului macOS memoria
    eliberată.
- Listele nu mai cresc în memorie când se reîmprospătează. Pe macOS 27, fiecare reîmprospătare a unei liste cu rânduri
  care se pot extinde lăsa în urmă definitiv un triunghi de extindere, iar fiecare reîmprospătare lăsa în urmă câte
  puțin din fiecare rând:
  - Listele CI, Containere, Software Factory, Teste, Depanare, Git și Căutare, precum și ierarhia apelurilor, își
    desenează singure săgețile.
  - Listele care nu sunt vizibile nu se mai reîmprospătează; recuperează când se arată.
  - Listele Sesiuni, CI, Software Factory și Containere păstrează rândurile care nu s-au schimbat, iar Git
    reîmprospătează doar rândurile fișierelor a căror stare s-a schimbat.
- Panourile unei ferestre închise sunt eliberate odată cu ea.

## File IA
- Pentru o comandă în fundal care își scrie singură ieșirea într-un fișier, cum ar fi `make > build.log 2>&1`, „Afișați
  ieșirea” afișează acel fișier când CLI nu are nimic din ieșire.
