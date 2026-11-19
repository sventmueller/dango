## Hitrost in pomnilnik
- Vse, kar zažene agent, v katerem koli jeziku, na primer gradnje, testi, lupine in strežniki MCP, teče v stanju ozadja
  sistema macOS: z najnižjo prioriteto ter z omejenim dostopom do diska in omrežja. Sami agenti, skupaj s podagenti, ki
  jih zažene Claude, tečejo s srednjo prioriteto. Agenti pod polno obremenitvijo ne upočasnjujejo več programa Dango; to
  velja tudi za agente Software Factory.
- Obnovljena okna zaženejo le tisto, kar je vidno: zavihek AI za drugimi zavihki zažene svoj CLI, ko pride v ospredje
  ali dobi poziv, zavihki brskalnika, predogleda spletnega mesta in terminala pa naložijo svojo stran ali zaženejo svojo
  lupino, ko se prvič prikažejo.
- Dolgo iskanje po projektu med izvajanjem ne izrisuje več vseh svojih rezultatov znova. Software Factory, dnevniki CI
  in podokno Testi delajo manj, dokler niso vidni; testi in konfiguracije za odpravljanje napak se preberejo, ko se
  njihovo podokno prvič prikaže, skrita podokna CI in Vsebniki pa ne preberejo ničesar, dokler se ne prikažejo.
- Dolge seje porabijo manj pomnilnika: kartica s končanimi razlikami obdrži le vrstice, ki jih prikazuje, zahteva za
  dovoljenje, na katero ste odgovorili, sprosti svoj vnos, predpomnilnik glifov terminala ne raste več brez omejitve,
  podokni Datoteke in Git pa sprostita vrstice, ki jih ni več.
- Sam program je približno 40 % manjši.

## Okna in meniji
- Zapiranje območja premakne njegove zavihke v območje pred njim, sicer v območje za njim, namesto da bi jih zaprlo.
  Skupaj s svojimi zavihki se zapreta le območje predogleda in zadnje območje.
- Meni Prikaz navaja podokna v vrstnem redu ikon v stranski vrstici.
- »Upravljaj agente …« ni več v meniju +; ostaja pod »+ Nova seja« v podoknu Agenti.

## Popravki
- »Vključi in izključi datoteke« v podoknu Iskanje ob svoji puščici ne prikazuje več odvečne črke.
- Zapiranje pojavnega okna s komentarji k specifikaciji ne povzroči več sesutja.
