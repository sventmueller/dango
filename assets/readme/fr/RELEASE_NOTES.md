Voici la première version de Dango, un éditeur natif pour macOS conçu pour travailler avec des agents de code.
L’éditeur, les terminaux, un navigateur, Git, CI, un débogueur, les fils IA et une Software Factory pour les ordres de
travail partagent une fenêtre par dossier. L’app est écrite en Rust sur AppKit ; WebKit n’apparaît que dans l’onglet du
navigateur. L’interface est disponible en 35 langues.

## Configuration requise
- macOS 14 ou ultérieur, sur Mac avec puce Apple ou processeur Intel.
- Pour les fils IA, l’agent de votre choix : Claude a besoin de la commande `claude`, avec votre compte connecté ; Codex
  a besoin de Node.js, et l’app installe son paquet en version figée à votre demande ; Antigravity et OpenCode utilisent
  votre propre `agy` ou `opencode`, ou un téléchargement en version figée ; l’agent intégré a besoin d’une clé d’API, ou
  d’Ollama ou de LM Studio sur ce Mac.
- Les serveurs de langage et les adaptateurs de débogage s’installent à votre demande. gopls et `dlv` nécessitent Go ;
  les serveurs TypeScript, Python, JSON, YAML, CSS, HTML, ESLint et shell ainsi que le débogueur JavaScript nécessitent
  Node.js ; clangd, sourcekit-lsp et `lldb-dap` sont fournis avec les Xcode Command Line Tools.
- Les prédictions de saisie nécessitent un abonnement GitHub Copilot (la formule gratuite suffit) ou un modèle
  fill-in-the-middle dans Ollama ou LM Studio.

## Fenêtre
- Chaque fichier, terminal, page web, fil IA, diff et aperçu est un onglet macOS dans la fenêtre du dossier. Le « + » au
  bout de la barre d’onglets et ⌘T proposent un nouveau fichier, un terminal, le navigateur et chaque agent installé, y
  compris dans son propre worktree Git.
- La barre latérale bascule entre Fichiers, Recherche, Agents, Git, Déboguer, CI, Tests, Conteneurs et la Software
  Factory ; un clic droit masque ceux dont vous ne vous servez pas. La barre d’outils contient Précédent et Suivant à
  travers les endroits que vous avez modifiés (⌃- et ⌃⇧-), la branche, Ouverture rapide et la vue d’ensemble Activité.
- Une fenêtre se divise en zones côte à côte ou l’une au-dessus de l’autre, imbriquées aussi profondément que vous le
  souhaitez, comme dans Xcode ; chaque zone garde ses documents les uns derrière les autres, avec sa propre barre
  d’onglets. « Ouvrir à côté » affiche un second document à côté du premier.
- Après un redémarrage, ou un plantage, chaque fenêtre revient avec ses onglets et ses zones, et les éditeurs contenant
  du texte non enregistré reviennent avec ce texte.
- Les images, l’audio, la vidéo, les polices et les documents Office s’ouvrent dans un onglet via Quick Look, les PDF
  via PDFKit.

## Éditeur
- Coloration syntaxique, numéros de ligne, marques de modification par rapport au dernier commit, rechercher et
  remplacer, Ouverture rapide (⌘P) et recherche dans le projet (⇧⌘F). Les couleurs et les styles Markdown suivent même
  dans des fichiers de plusieurs milliers de lignes, sans ralentir la saisie.
- Indentation automatique, tabulations ou espaces selon `.editorconfig`, paires de parenthèses, activation et
  désactivation des commentaires, repli, aller à la ligne, espaces visibles, retour à la ligne automatique, une
  minicarte et des en-têtes épinglés des portées englobantes.
- Plusieurs curseurs : ⌥-clic, ⌥-glisser pour une colonne, ⌘D pour l’occurrence suivante.
- **Mode Vim :** à activer dans le menu Éditeur, avec les modes normal, insertion et visuel, les mouvements et les
  opérateurs, les registres, les marques, les macros, la recherche et les commandes Ex ; ses messages s’affichent dans
  la langue de l’interface.
- Abréviations Emmet dans la complétion HTML, CSS et JSX, et correction des problèmes ESLint à l’enregistrement si vous
  le souhaitez.
- Les fichiers conservent leur encodage, leur indicateur d’ordre des octets et leurs fins de ligne. Un fichier modifié
  sur le disque est rechargé si l’éditeur ne contient aucun texte non enregistré ; sinon, une barre propose les
  différences et une fusion.
- **Prédictions de saisie :** pendant que vous tapez, une suggestion apparaît en gris ; la touche Tab l’accepte. Elles
  proviennent de GitHub Copilot ou d’un modèle local dans Ollama ou LM Studio, selon votre choix dans les réglages ; un
  modèle local garde votre code sur ce Mac.

## Serveurs de langage
Go, Rust, TypeScript et JavaScript, Python, C, C++ et Objective-C, Swift, scripts shell, JSON, YAML, CSS, HTML, TOML,
Markdown et Lua, ESLint à côté du serveur TypeScript, ainsi que vos propres serveurs depuis les réglages. Diagnostics,
y compris en fin de ligne, infobulles au survol, complétion, aide à la signature, accès à la définition, à la définition
de type et à l’implémentation, références, hiérarchies d’appels et de types, renommage, mise en forme, couleurs
sémantiques, repli et structure ; types et noms de paramètres dans la ligne ; corrections rapides et refactorisations
derrière une ampoule (⌥⌘A) ; les symboles du projet avec ⌥⌘O ou `#` dans Ouverture rapide. Le `tsc` propre à un projet
TypeScript est utilisé lorsqu’il peut servir de serveur de langage.

## Débogueur
Points d’arrêt, y compris conditionnels, exécution pas à pas, threads avec leurs piles d’appels, variables et
expressions espionnées dans l’Inspecteur (⌥⌘I), valeurs à côté du code tant que le programme est en pause, et une
console de débogage qui évalue dans le frame en pause. Rust, C, C++ et Swift via `lldb-dap`, Go via `dlv`, JavaScript et
TypeScript via le débogueur JavaScript, Python via debugpy. Le fichier `.vscode/launch.json` du projet est lu. Le menu
Déboguer reprend les raccourcis de Xcode.

## Tests et conteneurs
- Le panneau Tests liste les tests des projets Rust, Go, Python, Jest, Vitest et Bun avant la première exécution, en
  exécute un, un fichier ou tous, affiche chaque résultat en direct et le marque dans la marge de l’éditeur.
- Le panneau Conteneurs liste les conteneurs, les images, les volumes et les projets Compose de Docker ; il les démarre,
  les arrête et les supprime, suit leurs journaux et ouvre un shell dans un onglet de terminal.

## Terminal
Des terminaux en onglets avec l’environnement du shell de connexion, dessinés avec Metal. Coller ou déposer une image la
transmet à un agent de code qui tourne dans le terminal, ou colle le chemin d’une copie enregistrée. ⌘-clic ouvre les
liens et les noms de fichier. Un serveur de développement lancé par le terminal peut s’ouvrir dans l’onglet du
navigateur.

## Markdown
Un fichier Markdown s’ouvre avec son aperçu à côté (⇧⌘V), avec les formules, les diagrammes Mermaid, Graphviz et
PlantUML et les images du dossier. Un menu Format pour le gras, l’italique, les liens, les titres, les listes, les
citations et les blocs de code ; le texte collé depuis Word ou une page web arrive en Markdown. **Fichier › Exporter**
écrit un PDF, un fichier HTML unique ou un document Word. Si Splat est installé, **Fichier › Ouvrir dans Splat** y ouvre
le document.

## Navigateur
Un onglet de navigateur avec champ d’adresse, Précédent et Suivant, recherche, téléchargements soumis à confirmation, et
le Web Inspector.

## Git
Un panneau Git pour indexer et abandonner des fichiers entiers ou des blocs isolés, commiter, commiter et pousser ou
synchroniser, avec les branches, les stashs, les conflits de fusion et un graphe des commits récents. Blame indique qui
a modifié en dernier la ligne sous le curseur, ou affiche une colonne pour tout le fichier. Un bouton rédige le message
de commit par l’IA ; le diff ne part qu’une fois que gitleaks n’y a trouvé aucun secret, et chaque commit est également
vérifié à la recherche de secrets. Un fil IA peut travailler dans son propre worktree Git ; le panneau montre ce qu’il a
modifié et intègre le travail par fusion ou par cherry-pick.

## CI
GitHub Actions et GitLab CI, y compris GitHub Enterprise Server et GitLab autogéré : exécutions, jobs et journaux de la
branche actuelle ; relance des jobs en échec, d’un seul job ou (sur GitHub) d’une exécution entière, démarrage des jobs
manuels ou approbation des déploiements en attente, lancement d’un nouveau pipeline ou d’un workflow démarrable à la
main, et annulation. Les merge requests ou pull requests et les issues ouvertes s’affichent dans le même panneau et
s’ouvrent dans l’onglet du navigateur. Le journal d’un job en échec peut être transmis à un agent, marqué comme texte
externe. Le jeton provient du trousseau, de `gh` ou `glab`, de l’environnement du shell de connexion, ou vous le
saisissez.

## IA
Chaque agent travaille dans un onglet IA doté de la même chronologie : réponses, appels d’outils avec leur sortie,
demandes d’autorisation, questions, plans à approuver et à commenter, tâches à faire, sous-agents et tâches en
arrière-plan ; deux boutons dans l’en-tête de l’onglet listent les sous-agents et les processus en arrière-plan en
cours. Avant chaque prompt, l’app crée un point de contrôle ; les modifications peuvent être examinées et annulées par
retour en arrière, message par message. Les nouveaux fils démarrent dans le mode « Manuel », et les commandes Git
destructrices ne sont jamais autorisées définitivement. La vue d’ensemble Activité liste tout ce qui tourne. Un point
vert sur l’onglet signale une session au travail, un point rouge une session qui vous attend, et une notification vous
prévient lorsqu’elle se termine ou attend pendant que vous regardez ailleurs. La chronologie, le champ de message et la
ligne d’état ont leur propre taille de police dans les réglages.
- **Claude** via la commande `claude`, telle quelle, avec les sessions, les sous-agents, les tâches en arrière-plan, le
  contexte, l’utilisation et Remote Control depuis le téléphone.
- **Codex** via son serveur d’app et **OpenCode** via son serveur, chacun dans son propre protocole, avec leurs fils,
  le retour en arrière dans l’agent lui-même, et Remote Control pour Codex.
- **Agents ACP :** Antigravity, et d’autres depuis le registre ACP à votre demande ; les agents installés sont figés sur
  des versions testées et vérifiés par rapport à leurs sommes de contrôle.
- **Agent intégré :** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio et les serveurs qui parlent l’API
  d’OpenAI ou d’Anthropic, avec des outils pour les fichiers, la recherche, les commandes, les diagnostics et les
  requêtes web ; les clés restent dans le trousseau.
- Les commandes propres à un agent s’ouvrent en superposition dans l’onglet, de la même manière pour chaque agent :
  état, export, serveurs MCP, autorisations, hooks, mémoire, configuration, skills, plug-ins, modèle, compte et
  commentaires.
- Un interrupteur de sandbox par fil limite ce que les commandes d’un agent peuvent faire, dans la mesure où l’agent le
  prend en charge.
- Le texte externe, comme la sortie d’un terminal, une page web ou un journal CI, parvient à l’agent marqué comme non
  approuvé.

## Software Factory
Des ordres de travail au lieu de prompts isolés : un objectif avec des critères d’acceptation, rédigé à la main ou
repris d’une issue. Chaque ordre est planifié, construit et contrôlé dans son propre worktree Git, par des agents de
fabricants différents dans des rôles fixes : un planificateur rédige la spécification, un builder fait le travail, et
un relecteur d’un autre fabricant vérifie chaque critère. Les commandes de contrôle du projet, comme le build et les
tests, s’exécutent dans une sandbox, et le builder corrige ce qu’elles trouvent.
- Vous décidez à deux moments : vous approuvez la spécification, et vous acceptez le résultat avec ses preuves, en
  entier ou section par section. Rien n’est intégré sans votre acceptation.
- Le panneau et l’onglet Software Factory montrent chaque ordre avec son étape, son coût et ce qu’il attend ; l’onglet
  d’un ordre montre sa chronologie, ses preuves et la prochaine action, et ouvre sa session en cours dans un onglet IA.
- Des budgets par ordre et par jour, des limites de tours, une pause à la fermeture de l’app, à la mise en veille et
  en cas de batterie faible, et un journal écrit avant chaque effet, pour que les ordres reprennent après un
  redémarrage.
- Les issues portant une étiquette choisie peuvent arriver comme ordres ; un ordre accepté peut partir comme merge
  request avec un commentaire sur son issue, d’un clic de votre part. Les ordres aux périmètres des chemins distincts
  s’exécutent côte à côte et sont intégrés l’un après l’autre.
- Des indicateurs par agent et par fabricant, un routage selon le taux de premier passage, des propositions de règles
  pour `AGENTS.md` et `CLAUDE.md` à partir de constats récurrents, et des ordres récurrents issus du fichier de workflow
  du projet.
- Le fichier de workflow `.dango/factory.toml` d’un dossier contient ses contrôles, ses classes de risque et ses
  périmètres des chemins ; les commandes de contrôle modifiées et tout ce qui assouplit les limites attendent votre
  confirmation.

## Dictée
Dictée dans le champ de message IA et dans l’éditeur grâce à la reconnaissance vocale de Lowspeak : le bouton du micro,
⌃⌥D, ou la touche fn tant que Dango est l’app active. Elle utilise les réglages et les modèles de Lowspeak ; la
transcription en ligne nécessite une clé OpenAI ou Groq.

## Mises à jour
L’app se met à jour via Sparkle. Au deuxième lancement, elle vous demande si elle peut rechercher les mises à jour
d’elle-même ; tant que vous n’avez pas accepté, elle n’envoie aucune requête. **Dango › Rechercher les mises à jour…**
lance la recherche immédiatement.

## Accessibilité
Des noms VoiceOver pour la barre latérale, les onglets, les cartes de la chronologie IA, les lignes Git et CI, la
Software Factory, le débogueur et le texte du terminal ; un contrôle complet au clavier avec des raccourcis modifiables
dans `keybindings.json` ; Augmenter le contraste et Réduire les animations ; les vues de code restent de gauche à droite
dans les interfaces de droite à gauche. Les détails figurent dans ACCESSIBILITY.md.

## Confidentialité
Ni télémétrie, ni statistiques d’utilisation, ni rapports de plantage. L’app ne se connecte que pour ce que vous
lancez : les agents que vous utilisez, les téléchargements que vous demandez, les hôtes CI auxquels vous vous connectez,
les prédictions de Copilot si vous les avez choisies, la transcription en ligne si vous l’avez choisie, les pages que
vous ouvrez, et la recherche de mises à jour une fois que vous l’avez autorisée. Les clés et les jetons restent dans le
trousseau macOS, et les agents de la Software Factory ne les voient jamais.

## Limites connues
- La connexion à CI via le navigateur est désactivée ; utilisez `gh`, `glab`, un jeton ou l’environnement.
- L’app s’exécute sans l’App Sandbox, car les terminaux, Git, les serveurs de langage et les agents en hériteraient ;
  vous devez faire confiance à un dossier avant que quoi que ce soit qu’il configure ne s’exécute. Antigravity et
  OpenCode n’ont pas de sandbox que l’app puisse activer.
- Dans la Software Factory, un builder ou un relecteur Codex peut lire en dehors de son worktree, car le protocole de
  Codex ne connaît aucune limite de lecture ; ses écritures restent à l’intérieur.
- Les chiffres d’utilisation de Claude et la répartition de son contexte reposent sur des parties de son protocole
  marquées comme expérimentales, et sont masqués lorsqu’ils manquent. Le retour en arrière dans la conversation
  fonctionne avec Claude, Codex et OpenCode ; pour les autres agents, seul le code revient en arrière.
- Gemini CLI n’est pas proposé : Google a mis fin à sa connexion avec un compte personnel et renvoie vers Antigravity.
  Les anciens fils Gemini restent lisibles.
