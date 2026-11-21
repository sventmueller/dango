## Varnost
- Povezave v odgovorih, načrtih in poročilih agenta, v predogledu Markdowna in v datotekah PDF z enim klikom ne odprejo
  več nobenega programa. Spletni naslovi se odprejo v zavihku brskalnika, datoteke v projektu v zavihku urejevalnika;
  vse drugo, na primer datoteka drugje, omrežna skupna raba ali povezava drugega programa, najprej vpraša in pokaže, kam
  vodi.
- Naslovi za vpis in za Remote Control, ki jih preda agent ali strežnik MCP, se odprejo le, če so spletni naslovi.
- Mapam, ki vsebujejo veliko več kot en projekt, na primer domači mapi, Namizju, Dokumentom, Prenosom ali korenu
  nosilca, ni več mogoče zaupati kot celoti; zaupajte jim projekt za projektom. Vprašanje o zaupanju zdaj pove, da
  zaupanje velja tudi za mape v njej.
- Žetoni in vpisi CI nikoli ne gredo prek navadnega http na strežnik, ki ni ta Mac; repozitorij, katerega oddaljeni
  repozitorij se začne z `http://`, se poizveduje prek https.
- Brez gitleaks napovedovanje med urejanjem počaka, preden gre koda ponudniku zunaj tega Maca, dokler se ne odločite, da
  jo pošljete nepreverjeno. Lastne nastavitve gitleaks v projektu preverjanja ne izklopijo več.
- Lastnega agenta ni več mogoče zavesti, da bi pisal zunaj projekta prek povezave, ki se pojavi po odobritvi njegovega
  pisanja, datoteke z dodatnimi trdimi povezavami pa dobijo lastno datoteko, namesto da bi se pisalo skoznje.
- Dopolnjevanje jezikovnega strežnika ne more več zamrzniti urejevalnika, njegova preimenovanja in brisanja datoteke, ki
  je povezava, pa delujejo na samo povezavo, ne na to, na kar kaže.
- Slike, ki jih priložite pozivu, dekodira storitev za dekodiranje v peskovniku, kot povsod drugje.
- Program v terminalu lahko zamenja odložišče le, ko je njegov terminal v ospredju, in terminal to sporoči.
- rust-analyzer se namesti iz izdaje, pripete z njeno kontrolno vsoto, namesto iz tiste, ki je bila tisti hip
  najnovejša.

## Iskanje in navigacija
- »Hitro odpri« pove, ko se nobena datoteka ne ujema, in pokaže, ali išče datoteke ali simbole projekta.
- Simboli projekta vključujejo TypeScript in JavaScript, tudi če ni odprta nobena njihova datoteka. Najprej so prikazana
  natančna ujemanja iz projekta in v jeziku, v katerem delate; simboli iz knjižnic in nameščenih paketov so označeni.
- Prikaz › Iskanje postavi fokus v iskalno polje.

## Urejevalnik
- Urejevalnik obdrži fokus, ko se datoteka Markdown odpre s predogledom ob sebi, zato meni Oblikovanje deluje takoj.
  Ukazi urejevalnika delujejo tudi, ko je fokus v stranski vrstici.
- Predlog na koncu vrstice ali v polju pod njo pove, da ga sprejme tipka Tab.
- Zavihke z enakim imenom v pregledu zavihkov je mogoče razločiti, na primer datoteko in njen predogled v brskalniku.
- Meni Prikaz ne ponuja več lastnega ukaza sistema macOS »Skrij vrstico z zavihki«, ki ga zavihki programa Dango ne
  uporabljajo.

## Jeziki in odpravljanje napak
- Strežnik JSON zdaj preverja datoteke, strežniki, ki svojo diagnostiko registrirajo po zagonu, na primer basedpyright
  in sourcekit-lsp, pa jo prikažejo.
- Kjer pip noče nameščati v Python, kot pri Pythonu iz Homebrew, orodje za odpravljanje napak ponudi namestitev debugpy
  v virtualno okolje projekta.
