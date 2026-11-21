## Vitesse
- Ce qu’un agent lance, comme les builds, les tests et les shells, s’exécute avec la priorité la plus basse au lieu de
  s’exécuter dans l’état d’arrière-plan de macOS. Dango passe toujours en premier, et les agents avant ce qu’ils
  lancent, mais leurs builds utilisent de nouveau les cœurs de performance et toute la vitesse du disque ; l’état
  d’arrière-plan avait réduit leur débit à environ un dixième, même sur un Mac par ailleurs inactif.

## Agents
- Le point d’un onglet IA reste vert tant qu’une commande en arrière-plan de sa session s’exécute encore, et l’avis
  indiquant qu’une session est terminée n’arrive qu’une fois que son travail en arrière-plan a lui aussi pris fin.
