## Terminal
- Terminal sada dolazi iz aplikacije Tapa, sestrinske aplikacije za terminal, sa svime što ona donosi, a ono što je terminal aplikacije Dango dosad mogao ostaje: lijepljenje slika agentima, razvojni poslužitelji u kartici preglednika, ⌘-klik u urednik.
- **Prikaz › Prikaži/sakrij polje za unos** (⌘I dok terminal ima fokus) pretvara naredbeni redak u tekstno polje s mapom i Git granom iznad njega. Naredbe su obojene po svojim dijelovima, a ona koju vaša ljuska ne može pokrenuti dobiva crveno isprekidano podcrtavanje.
- Prijedlozi se prikazuju sivo dok tipkate, u polju i bez njega: iz vaše povijesti, iz specifikacija za više od 700 alata naredbenog retka i iz projekta, kao što su git grane, skripte iz `package.json` te make, just i cargo ciljevi. → prihvaća prijedlog; Tab dovršava, a uz više kandidata otvara izbornik koji tipkanje filtrira. Prijedlozi nikad ne koriste mrežu.
- Blokovi naredbi: ⌘↑ i ⌘↓ skaču između odzivnika, izbornik **Uredi** odabire, kopira i sprema izlaz naredbe, a oznake obilježavaju retke. Kad je uključeno **Označi naredbe kao blokove**, traka prikazuje svaki blok, neuspjele naredbe su označene, a gumbi iznad bloka prikazuju njegovo stanje i trajanje.
- **Pošalji blok agentu** (izbornik Uredi i kontekstni izbornik terminala) otvara AI karticu s naredbenim retkom i izlazom trenutačnog bloka priloženima kao Vanjski sadržaj.
- **Pronađi** (⌘F dok terminal ima fokus) pretražuje cijelu povijest s regularnim izrazima, razlikovanjem velikih i malih slova i cijelim riječima; traka za pomicanje označava rezultate.
- Obavijest programa (OSC 9 i OSC 777) i njegovo zvono stavljaju crvenu točku na karticu terminala dok je ne vidite, a dok je sprijeda druga aplikacija, obavijest ide u macOS. Napredak koji program javlja (OSC 9;4) prikazuje se kao prsten na njegovoj kartici i kao traka na ikoni u Docku.
- Dvoklik odabire cijeli URL, putanju ili hash, povlačenje s ⌥ odabire pravokutnik, **Kopiraj s formatiranjem** zadržava boje i stilove, a **Odabir tipkovnicom** pomiče odabir Vim tipkama.
- Lijepljenje više redaka koje bi se odmah izvršilo najprije se prikazuje, a **Dango › Zaštiti unos putem tipkovnice** sprječava druge aplikacije da čitaju ono što tipkate u terminal.
- Poveznice drugih vrsta osim web adresa, `mailto:` i datoteka više se ne otvaraju iz terminala.

## Postavke
- **Postavke › Terminal** uključuje i isključuje polje za unos za nove terminale, njegove Vim tipke, prijedloge, bojenje naredbi, podcrtavanje nepoznatih naredbi, blokove naredbi, trenutačno kopiranje odabira, prikaz višerednog lijepljenja unaprijed i ⌥ kao meta tipku, koja je ovamo premještena iz kartice Općenito.
