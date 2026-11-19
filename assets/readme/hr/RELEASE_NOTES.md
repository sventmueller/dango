## Brzina i memorija
- Sve što agent pokrene, u bilo kojem jeziku, poput buildova, testova, ljuski i MCP poslužitelja, radi u pozadinskom
  stanju macOS-a: s najnižim prioritetom i usporenim pristupom disku i mreži. Sami agenti, zajedno s Claudeovim
  podagentima, rade sa srednjim prioritetom. Agenti pod punim opterećenjem više ne usporavaju Dango; to vrijedi i za
  agente Software Factoryja.
- Obnovljeni prozori pokreću samo ono što se vidi: AI kartica iza drugih kartica pokreće svoj CLI kad dođe u prvi plan
  ili dobije prompt, a kartice preglednika, pregleda web-mjesta i terminala učitavaju svoju stranicu ili pokreću svoju
  ljusku tek kad se prvi put prikažu.
- Duga pretraga projekta više ne iscrtava iznova sve svoje rezultate dok traje. Software Factory, CI zapisnici i okno
  Testovi rade manje dok nisu vidljivi; testovi i konfiguracije za otklanjanje pogrešaka čitaju se kad se njihovo okno
  prvi put prikaže, a skrivena okna CI i Kontejneri ne čitaju ništa dok se ne prikažu.
- Duge sesije trebaju manje memorije: dovršeni prikaz razlika zadržava samo retke koje pokazuje njegova pločica, zahtjev
  za dozvolu na koji je odgovoreno otpušta svoj unos, predmemorija glifova terminala više ne raste bez ograničenja, a
  okna Datoteke i Git otpuštaju retke kojih više nema.
- Sam program manji je za oko 40 %.

## Prozori i izbornici
- Zatvaranje područja premješta njegove kartice u prethodno područje, a ako ga nema, u sljedeće, umjesto da ih zatvori.
  Samo područje pregleda i posljednje područje zatvaraju se zajedno sa svojim karticama.
- Izbornik Prikaz navodi okna redoslijedom simbola u rubnom stupcu.
- “Upravljaj agentima…” više nije u izborniku +; ostaje pod “+ Nova sesija” u oknu Agenti.

## Ispravci
- “Uključi i isključi datoteke” u oknu Pretraga više ne prikazuje zalutalo slovo pokraj svoje strelice.
- Zatvaranje skočnog prozora s komentarima specifikacije više ne uzrokuje rušenje.
