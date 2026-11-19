## Vitesse et mémoire
- Tout ce qu’un agent lance, dans n’importe quel langage, comme les builds, les tests, les shells et les serveurs MCP,
  s’exécute dans l’état d’arrière-plan de macOS : avec la priorité la plus basse et des accès au disque et au réseau
  bridés. Les agents eux-mêmes, avec les sous-agents de Claude, s’exécutent avec une priorité moyenne. Des agents à
  pleine charge ne ralentissent plus Dango ; cela vaut aussi pour les agents de la Software Factory.
- Les fenêtres restaurées ne lancent que ce qui s’affiche : un onglet IA placé derrière d’autres lance sa CLI lorsqu’il
  passe au premier plan ou reçoit un prompt, et les onglets de navigateur, d’aperçu de site et de terminal chargent leur
  page ou lancent leur shell la première fois qu’ils s’affichent.
- Une longue recherche dans le projet ne redessine plus tous ses résultats pendant qu’elle s’exécute. La
  Software Factory, les journaux CI et le panneau Tests en font moins tant qu’ils ne sont pas visibles ; les tests et
  les configurations de lancement sont lus la première fois que leur panneau s’affiche, et les panneaux CI et Conteneurs
  masqués ne lisent rien tant qu’ils ne s’affichent pas.
- Les longues sessions demandent moins de mémoire : un diff terminé ne garde que les lignes que montre sa carte, une
  demande d’autorisation qui a reçu sa réponse libère son entrée, le cache de glyphes du terminal ne grossit plus sans
  limite, et les panneaux Fichiers et Git libèrent les lignes qui n’existent plus.
- Le programme lui-même est environ 40 % plus petit.

## Fenêtres et menus
- Fermer une zone ne ferme plus ses onglets : ils passent dans la zone précédente, sinon dans la suivante. Seules la
  zone d’un aperçu et la dernière zone se ferment avec leurs onglets.
- Le menu Présentation liste les panneaux dans l’ordre des icônes de la barre latérale.
- « Gérer les agents… » a quitté le menu + ; il reste sous « + Nouvelle session » dans le panneau Agents.

## Corrections
- « Inclure et exclure des fichiers » dans le panneau Recherche n’affiche plus de lettre parasite à côté de sa flèche.
- Fermer la fenêtre contextuelle de commentaires d’une spécification ne provoque plus de plantage.
