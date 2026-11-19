## File IA
- O filă IA care stă trei minute în afara vederii își eliberează procesul Claude cât timp sesiunea este inactivă.
  Discuția și ce ați tastat rămân în filă, iar următorul prompt reia sesiunea. O filă își păstrează procesul cât timp
  Claude lucrează, un prompt așteaptă, o întrebare este deschisă, un subagent, o sarcină în fundal sau o comandă încă
  rulează ori Remote Control este activat.
- Oprirea unui agent oprește și comenzile pe care acesta le-a pornit într-un grup de procese sau într-o sesiune proprie,
  care înainte puteau să-i supraviețuiască.
- Cronologia adaugă intrările noi la sfârșit, în loc să reindexeze toate rândurile anterioare.

## Viteză
- Ferestrele restaurate se deschid mai repede: o filă de fișier fără modificări nesalvate își încarcă fișierul abia când
  ajunge în față.
- O căutare în proiect al cărei panou este ascuns se întrerupe și continuă când panoul se afișează.
- Marcajele de modificări Git din editor rulează pentru fiecare fișier cel mult o comparație în același timp; editările
  făcute între timp sunt reunite într-o singură comparație ulterioară, iar textul fișierului din commit este recitit
  doar când s-a schimbat commitul.
- Miniharta unui fișier cu cod restrâns păstrează câte o intrare pentru fiecare porțiune extinsă, în loc de câte una
  pentru fiecare linie.

## Căutare
- Săgeata filtrelor este acum aliniată cu săgeata de la Înlocuiți de deasupra.
