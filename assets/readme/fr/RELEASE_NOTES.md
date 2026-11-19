## Onglets IA
- « Afficher la sortie » d’une commande en arrière-plan ouvre un onglet qui suit la sortie tant que la commande
  s’exécute, au lieu d’une copie figée à ce moment-là. L’onglet s’ouvre à la fin de la sortie et y reste à mesure que de
  nouvelles lignes arrivent, jusqu’à ce que vous fassiez défiler vers le haut ; si vous revenez à la fin, il suit de
  nouveau. Si l’onglet est déjà ouvert, « Afficher la sortie » le fait passer au premier plan, à la fin.
  - Une commande qui écrit elle-même sa sortie dans un fichier, comme `make > build.log 2>&1`, affiche ce fichier au fur
    et à mesure qu’il grandit.
  - Une fois la commande terminée, l’onglet affiche toute sa sortie ; « Ouvrir dans l’éditeur » ouvre le fichier qui se
    trouve derrière.
  - Un onglet ne lit la sortie que tant qu’il est au premier plan.
