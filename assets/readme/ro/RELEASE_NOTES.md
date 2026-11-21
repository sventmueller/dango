## Securitate
- Linkurile din răspunsurile, planurile și rapoartele unui agent, din previzualizarea Markdown și din PDF-uri nu mai
  deschid nicio aplicație cu un singur clic. Adresele web se deschid într-o filă de browser, fișierele din proiect
  într-o filă de editor; orice altceva, cum ar fi un fișier din altă parte, o partajare de rețea sau linkul unei alte
  aplicații, întreabă mai întâi și arată unde duce.
- Adresele de autentificare și de Remote Control pe care le transmite un agent sau un server MCP se deschid doar dacă
  sunt adrese web.
- Dosarelor care conțin mult mai mult decât un proiect, cum ar fi dosarul de utilizator, Birou, Documente, Descărcări
  sau rădăcina unui volum, nu li se mai poate acorda încredere în ansamblu; acordați încredere fiecărui proiect în
  parte. Întrebarea despre încredere spune acum că aceasta se aplică și dosarelor din interior.
- Tokenurile CI și datele de autentificare nu pleacă niciodată prin http simplu către un alt server decât acest Mac;
  un depozit al cărui remote începe cu `http://` este interogat prin https.
- Fără gitleaks, predicțiile de editare așteaptă înainte ca un cod să plece către un furnizor din afara acestui Mac,
  până când alegeți să trimiteți neverificat. Configurările gitleaks proprii ale unui proiect nu mai dezactivează
  verificarea.
- Agentul propriu nu mai poate fi determinat să scrie în afara proiectului printr-un link care apare după ce scrierea
  sa a fost aprobată, iar fișierele cu alte legături fixe primesc un fișier propriu în loc să fie scrise prin legătură.
- Completările unui server de limbaj nu mai pot bloca editorul, iar redenumirile și ștergerile unui fișier legat făcute
  de acesta acționează asupra legăturii, nu asupra țintei ei.
- Imaginile pe care le atașați unui prompt sunt decodificate de serviciul de decodificare din sandbox, ca peste tot.
- Un program din terminal poate înlocui clipboardul doar cât timp terminalul său este în față, iar terminalul spune
  acest lucru.
- rust-analyzer se instalează dintr-o versiune fixată cu suma sa de control, în locul celei care era cea mai nouă.

## Căutare și navigare
- Deschidere rapidă spune când niciun fișier nu se potrivește și arată dacă se caută fișiere sau simbolurile
  proiectului.
- Simbolurile proiectului includ TypeScript și JavaScript fără ca vreunul dintre fișierele lor să fie deschis.
  Potrivirile exacte din proiect și din limbajul în care lucrați apar primele; simbolurile din biblioteci și din
  pachetele instalate sunt marcate.
- Vizualizare › Căutare mută focalizarea în câmpul de căutare.

## Editor
- Editorul păstrează focalizarea când un fișier Markdown se deschide cu previzualizarea alături, așa că meniul Format
  funcționează imediat. Comenzile editorului funcționează și cât timp focalizarea este în bara laterală.
- O propunere de la capătul unui rând sau dintr-o casetă de sub el spune că Tab o acceptă.
- Filele cu același nume din prezentarea generală a filelor se pot deosebi, de exemplu un fișier și previzualizarea sa
  în browser.
- Meniul Vizualizare nu mai oferă „Ascundeți bara de file” a macOS, pe care filele Dango nu o folosesc.

## Limbaje și depanare
- Serverul JSON verifică acum fișierele, iar serverele care își înregistrează diagnosticele după pornire, cum ar fi
  basedpyright și sourcekit-lsp, le afișează.
- Acolo unde pip refuză să instaleze în Python, cum face cel din Homebrew, depanatorul se oferă să instaleze debugpy
  într-un mediu virtual al proiectului.
