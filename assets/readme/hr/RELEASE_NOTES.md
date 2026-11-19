## AI kartice
- AI kartica koja tri minute nije u pogledu gasi svoj proces Claude dok sesija miruje. Razgovor i ono što ste upisali
  ostaju u kartici, a sljedeći prompt nastavlja sesiju. Kartica zadržava svoj proces dok Claude radi, dok prompt čeka,
  dok je neko pitanje otvoreno, dok podagent, zadatak u pozadini ili naredba još radi ili dok je Remote Control
  uključen.
- Završavanjem agenta završavaju i naredbe koje je pokrenuo u vlastitoj grupi procesa ili sesiji, a koje su ga prije
  mogle nadživjeti.
- Vremenska crta dodaje nove unose na svoj kraj umjesto da ponovno indeksira sve prethodne retke.

## Brzina
- Vraćeni prozori otvaraju se brže: kartica datoteke bez nespremljenih promjena učitava svoju datoteku tek kad dođe u
  prvi plan.
- Pretraga projekta čije je okno skriveno pauzira se i nastavlja kad se okno prikaže.
- Git oznake promjena u uredniku pokreću najviše jednu usporedbu istodobno po datoteci; izmjene napravljene u
  međuvremenu spajaju se u jednu naknadnu usporedbu, a tekst datoteke iz commita ponovno se čita samo kad se commit
  promijenio.
- Minikarta datoteke sa sažetim kodom drži jedan unos po nesažetom odsječku umjesto jednog po retku.

## Pretraga
- Strelica filtara poravnata je sa strelicom zamjene iznad nje.
