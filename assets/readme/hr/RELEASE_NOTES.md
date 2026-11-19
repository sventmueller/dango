## Memorija
- Duge sesije agenata trebaju mnogo manje memorije:
  - Otvaranje prethodne sesije čita njezin transkript redak po redak; sesija s transkriptom od 100 MB sada doseže
    najviše oko 20 MB umjesto 65 MB.
  - Završeni poziv alata zadržava samo ono što prikazuje njegova kartica na vremenskoj crti: prvih 400 redaka izlaza, a
    sirovi ulaz više ne čim kartica prikaže svoja polja. “Još redaka: N” i dalje broji svaki redak.
  - AI kartica koja 30 sekundi ostane izvan pogleda oslobađa kartice svoje vremenske crte, a kad se ponovno prikaže,
    gradi one koje su u pogledu, na mjestu gdje je bila.
  - Nakon zatvaranja kartice ili otvaranja prethodne sesije Dango vraća oslobođenu memoriju sustavu macOS.
- Popisi više ne rastu u memoriji pri osvježavanju. U sustavu macOS 27 svako je osvježavanje popisa s recima koji se
  otvaraju trajno ostavljalo za sobom trokut za otvaranje, a svako je osvježavanje ostavljalo pomalo od svakog retka:
  - Popisi CI, Kontejneri, Software Factory, Testovi, Otklanjanje pogrešaka, Git i Pretraga te hijerarhija poziva sami
    crtaju svoje strelice.
  - Popisi koji nisu u pogledu više se ne osvježavaju; to nadoknade kad se prikažu.
  - Popisi Sesije, CI, Software Factory i Kontejneri zadržavaju retke koji se nisu promijenili, a Git osvježava samo
    retke datoteka čije se stanje promijenilo.
- Okna zatvorenog prozora oslobađaju se zajedno s njim.

## AI kartice
- “Prikaži izlaz” naredbe u pozadini koja sama zapisuje svoj izlaz u datoteku, poput `make > build.log 2>&1`, prikazuje
  tu datoteku kad CLI nema ništa od izlaza.
