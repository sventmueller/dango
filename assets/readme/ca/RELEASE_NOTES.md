## Seguretat
- Els enllaços de les respostes, els plans i els informes d’un agent, de la previsualització de Markdown i dels PDF ja
  no obren qualsevol app amb un sol clic. Les adreces web s’obren en una pestanya de navegador, i els fitxers del
  projecte, en una pestanya d’editor; tota la resta, com ara un fitxer d’un altre lloc, un recurs compartit de xarxa o
  l’enllaç d’una altra app, pregunta abans i mostra on porta.
- Les adreces d’inici de sessió i de Remote Control que lliura un agent o un servidor MCP només s’obren si són adreces
  web.
- Ja no es pot confiar en conjunt en les carpetes que contenen molt més que un projecte, com ara la carpeta d’inici,
  l’Escriptori, Documents, Baixades o l’arrel d’un volum; confia‑hi projecte per projecte. La pregunta de confiança ara
  diu que també inclou les subcarpetes.
- Els tokens de CI i els inicis de sessió ja no surten mai per http sense xifrar cap a un servidor que no sigui aquest
  Mac; a un repositori el remot del qual comença per `http://` se li pregunta per https.
- Sense gitleaks, les prediccions esperen, abans que el codi vagi a un proveïdor de fora d’aquest Mac, fins que tries
  enviar sense comprovar. La configuració de gitleaks pròpia d’un projecte ja no desactiva la comprovació.
- Ja no es pot induir l’agent propi a escriure fora del projecte mitjançant un enllaç que apareix després que se n’hagi
  aprovat l’escriptura, i els fitxers amb més enllaços físics reben un fitxer propi en lloc de ser escrits a través
  d’ells.
- Les complecions d’un servidor de llenguatge ja no poden penjar l’editor, i els seus canvis de nom i eliminacions d’un
  fitxer enllaçat actuen sobre l’enllaç, no sobre la destinació.
- Les imatges que adjuntes a un prompt les descodifica el servei de descodificació aïllat, com a tot arreu.
- Un programa del terminal només pot substituir el porta-retalls mentre el seu terminal és al davant, i el terminal ho
  indica.
- rust-analyzer s’instal·la a partir d’una versió fixada amb la seva suma de verificació en lloc de la més recent del
  moment.

## Cerca i navegació
- Obertura ràpida indica quan no hi coincideix cap fitxer i mostra si cerca fitxers o els símbols del projecte.
- Els símbols del projecte inclouen TypeScript i JavaScript sense que cap dels seus fitxers estigui obert. Els resultats
  exactes del projecte i del llenguatge en què treballes apareixen primer; els símbols de biblioteques i paquets
  instal·lats estan marcats.
- Mostra › Cerca posa el focus al camp de cerca.

## Editor
- L’editor conserva el focus quan un fitxer Markdown s’obre amb la previsualització al costat, de manera que el menú
  Format funciona de seguida. Les ordres de l’editor també funcionen mentre el focus és a la barra lateral.
- Un suggeriment al final d’una línia o en un quadre a sota indica que la tecla Tab l’accepta.
- Les pestanyes amb el mateix nom a la visió general de pestanyes es distingeixen, per exemple un fitxer i la seva
  previsualització al navegador.
- El menú Mostra ja no ofereix “Oculta la barra de pestanyes” de macOS, que les pestanyes de Dango no fan servir.

## Llenguatges i depuració
- El servidor de JSON ara comprova els fitxers, i els servidors que registren els diagnòstics després d’iniciar‑se, com
  ara basedpyright i sourcekit-lsp, els mostren.
- On pip es nega a instal·lar a Python, com fa el de Homebrew, el depurador ofereix instal·lar debugpy en un entorn
  virtual del projecte.
