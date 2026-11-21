## Sigurnost
- Poveznice u odgovorima, planovima i izvješćima agenta, u pregledu Markdowna i u PDF-ovima više ne otvaraju bilo koju
  aplikaciju jednim klikom. Web-adrese otvaraju se u kartici preglednika, a datoteke u projektu u kartici urednika; sve
  ostalo, poput datoteke negdje drugdje, mrežnog dijeljenja ili poveznice druge aplikacije, najprije pita i pokazuje
  kamo vodi.
- Adrese za prijavu i za Remote Control koje preda agent ili MCP poslužitelj otvaraju se samo ako su web-adrese.
- Mapama koje sadrže mnogo više od jednog projekta, poput vaše početne mape, Radne površine, Dokumenata, Preuzimanja ili
  korijena volumena, više se ne može vjerovati u cjelini; vjerujte njihovim projektima jednom po jednom. Pitanje o
  povjerenju sada kaže da ono vrijedi i za mape unutar mape.
- CI tokeni i prijave više nikad ne idu nešifriranim http-om na poslužitelj koji nije ovaj Mac; repozitorij čiji
  udaljeni repozitorij počinje s `http://` ispituje se preko https-a.
- Bez gitleaksa predviđanja prije slanja koda pružatelju izvan ovog Maca čekaju dok ne odaberete slanje bez provjere.
  Vlastite postavke gitleaksa u projektu više ne isključuju provjeru.
- Vlastiti agent više se ne može navesti da piše izvan projekta putem poveznice koja se pojavi nakon što je njegovo
  pisanje odobreno, a datoteke s dodatnim čvrstim poveznicama dobivaju vlastitu datoteku umjesto da se piše kroz njih.
- Dovršavanja jezičnog poslužitelja više ne mogu zamrznuti urednik, a njegova preimenovanja i brisanja povezane datoteke
  djeluju na poveznicu, ne na ono na što pokazuje.
- Slike koje priložite promptu dekodira izolirana usluga dekodiranja, kao i svugdje drugdje.
- Program u terminalu može zamijeniti sadržaj memorijskog pretinca samo dok je njegov terminal u prvom planu, a terminal
  to i kaže.
- rust-analyzer instalira se iz izdanja koje je fiksirano zajedno s kontrolnim zbrojem, umjesto iz trenutačno
  najnovijeg.

## Pretraga i navigacija
- Brzo otvaranje kaže kada nijedna datoteka ne odgovara i pokazuje pretražuje li datoteke ili simbole projekta.
- Simboli projekta obuhvaćaju TypeScript i JavaScript i kad nijedna njihova datoteka nije otvorena. Točni rezultati iz
  projekta i u jeziku u kojem radite dolaze prvi; simboli iz knjižnica i instaliranih paketa su označeni.
- Prikaz › Pretraga stavlja fokus u polje za pretragu.

## Urednik
- Urednik zadržava fokus kad se datoteka Markdowna otvori s pregledom pokraj sebe, pa izbornik Formatiraj odmah radi.
  Naredbe urednika rade i dok je fokus u rubnom stupcu.
- Prijedlog na kraju retka ili u okviru ispod njega kaže da ga tipka Tab prihvaća.
- Istoimene kartice u pregledu kartica mogu se razlikovati, na primjer datoteka i njezin pregled u pregledniku.
- Izbornik Prikaz više ne nudi macOS-ovu stavku “Sakrij traku s karticama”, koju kartice Danga ne koriste.

## Jezici i otklanjanje pogrešaka
- JSON poslužitelj sada provjerava datoteke, a poslužitelji koji svoju dijagnostiku registriraju tek nakon pokretanja,
  poput basedpyright i sourcekit-lsp, sada je prikazuju.
- Gdje pip odbija instalirati u Python, kao kod Pythona iz Homebrewa, program za otklanjanje pogrešaka nudi instalaciju
  debugpy u virtualno okruženje projekta.
