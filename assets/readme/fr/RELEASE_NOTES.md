## Mémoire
- Les longues sessions d’agent demandent beaucoup moins de mémoire :
  - L’ouverture d’une session passée lit sa transcription une ligne à la fois ; une session dont la transcription pèse
    100 MB culmine désormais à environ 20 MB au lieu de 65 MB.
  - Un appel d’outil terminé ne garde que ce qu’affiche sa carte : les 400 premières lignes de sa sortie, et non plus
    l’entrée brute dès que la carte affiche ses champs. « N lignes de plus » compte toujours chaque ligne.
  - Un onglet IA qui reste hors de vue pendant 30 secondes libère ses cartes et, lorsqu’il réapparaît, construit celles
    qui sont visibles, à l’endroit où il en était.
  - Après la fermeture d’un onglet ou l’ouverture d’une session passée, Dango rend à macOS la mémoire qu’il a libérée.
- Les listes ne grossissent plus en mémoire lorsqu’elles s’actualisent. Sous macOS 27, chaque actualisation d’une liste
  aux lignes dépliables laissait derrière elle un triangle d’affichage pour de bon, et chaque actualisation laissait
  derrière elle un peu de chaque ligne :
  - Les listes CI, Conteneurs, Software Factory, Tests, Déboguer, Git et Recherche ainsi que la hiérarchie d’appels
    dessinent leurs propres chevrons.
  - Les listes hors de vue ne s’actualisent plus ; elles rattrapent leur retard lorsqu’elles s’affichent.
  - Les listes Sessions, CI, Software Factory et Conteneurs gardent les lignes qui n’ont pas changé, et Git n’actualise
    que les lignes des fichiers dont l’état a changé.
- Les panneaux d’une fenêtre fermée sont libérés avec elle.

## Onglets IA
- « Afficher la sortie » d’une commande en arrière-plan qui écrit elle-même sa sortie dans un fichier, comme
  `make > build.log 2>&1`, affiche ce fichier lorsque la CLI n’a rien de la sortie.
