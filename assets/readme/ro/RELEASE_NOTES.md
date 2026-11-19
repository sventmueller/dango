## Viteză și memorie
- Tot ce pornește un agent, în orice limbaj, cum ar fi build-uri, teste, shell-uri și servere MCP, rulează în starea de
  fundal a macOS: cu cea mai mică prioritate și cu acces limitat la disc și la rețea. Agenții înșiși, împreună cu
  subagenții lui Claude, rulează cu prioritate medie. Agenții la încărcare maximă nu mai încetinesc Dango; acest lucru
  este valabil și pentru agenții din Software Factory.
- Ferestrele restaurate pornesc doar ce se vede: o filă IA aflată în spatele altora își pornește CLI-ul când ajunge în
  față sau primește un prompt, iar filele de browser, de previzualizare a site-ului și de terminal își încarcă pagina
  sau își pornesc shell-ul când sunt afișate pentru prima dată.
- O căutare lungă în proiect nu își mai redesenează toate rezultatele cât timp rulează. Software Factory, jurnalele CI
  și panoul Teste fac mai puțin cât timp nu sunt vizibile; testele și configurațiile de depanare sunt citite când panoul
  lor este afișat pentru prima dată, iar panourile CI și Containere ascunse nu citesc nimic până nu sunt afișate.
- Sesiunile lungi au nevoie de mai puțină memorie: un diff încheiat păstrează doar rândurile pe care le arată cardul
  său, o cerere de permisiune la care s-a răspuns își eliberează datele introduse, memoria cache de glife a terminalului
  nu mai crește nelimitat, iar panourile Fișiere și Git eliberează rândurile care nu mai există.
- Programul în sine este cu aproximativ 40% mai mic.

## Ferestre și meniuri
- Închiderea unei zone îi mută filele în zona dinaintea ei, altfel în cea de după ea, în loc să le închidă. Doar zona
  unei previzualizări și ultima zonă se închid împreună cu filele lor.
- Meniul Vizualizare listează panourile în ordinea simbolurilor din bara laterală.
- „Gestionați agenții…” nu mai apare în meniul +; rămâne sub „+ Sesiune nouă” în panoul Agenți.

## Remedieri
- „Includeți și excludeți fișiere” din panoul de căutare nu mai afișează o literă rătăcită lângă săgeata sa.
- Închiderea popoverului de comentarii al unei specificații nu mai provoacă închiderea neașteptată a aplicației.
