## Zavihki AI
- Zavihek AI, ki tri minute ni viden, sprosti svoj proces Claude, dokler seja miruje. Pogovor in to, kar ste natipkali,
  ostaneta v zavihku, naslednji poziv pa sejo nadaljuje. Zavihek obdrži svoj proces, dokler Claude dela, čaka poziv, je
  odprto vprašanje, še teče podagent, opravilo v ozadju ali ukaz ali je vklopljen Remote Control.
- Ko se agent konča, se končajo tudi ukazi, ki jih je zagnal v lastni skupini procesov ali lastni seji; prej so ga lahko
  preživeli.
- Časovnica doda nove vnose na konec, namesto da bi znova indeksirala vse prejšnje vrstice.

## Hitrost
- Obnovljena okna se odpirajo hitreje: zavihek datoteke brez neshranjenih sprememb naloži svojo datoteko šele, ko pride
  v ospredje.
- Iskanje po projektu, katerega podokno je skrito, se zaustavi in nadaljuje, ko se podokno prikaže.
- Oznake sprememb Git v urejevalniku za vsako datoteko hkrati izvajajo največ eno primerjavo; spremembe med njo se
  združijo v eno samo naknadno primerjavo, besedilo datoteke iz commita pa se znova prebere le, če se je commit
  spremenil.
- Mini zemljevid datoteke s strnjeno kodo hrani en vnos za vsak nestrnjen del namesto enega za vsako vrstico.

## Iskanje
- Puščica filtrov je poravnana s puščico Zamenjaj nad njo.
