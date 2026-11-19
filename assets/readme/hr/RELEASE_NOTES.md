## AI kartice
- “Prikaži izlaz” kod naredbe u pozadini otvara karticu koja prati izlaz dok naredba radi, umjesto njegove kopije iz tog
  trenutka. Kartica se otvara na kraju izlaza i ostaje ondje dok stižu novi retci, sve dok ne pomaknete prikaz prema
  gore; kad ga ponovno pomaknete do kraja, kartica opet prati izlaz. “Prikaži izlaz” za karticu koja je već otvorena
  dovodi je u prvi plan, na kraju izlaza.
  - Naredba koja svoj izlaz sama zapisuje u datoteku, poput `make > build.log 2>&1`, prikazuje tu datoteku dok ona
    raste.
  - Kad naredba završi, kartica prikazuje cijeli njezin izlaz; “Otvori u uredniku” otvara datoteku iza nje.
  - Kartica čita izlaz samo dok je u prvom planu.
