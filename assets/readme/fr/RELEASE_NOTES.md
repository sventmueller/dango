## Sécurité
- Les liens dans les réponses, les plans et les rapports d’un agent, dans l’aperçu Markdown et dans les PDF n’ouvrent
  plus n’importe quelle app d’un simple clic. Les adresses web s’ouvrent dans un onglet de navigateur, les fichiers du
  projet dans un onglet d’éditeur ; tout le reste, comme un fichier situé ailleurs, un partage réseau ou le lien d’une
  autre app, demande d’abord et montre où il mène.
- Les adresses de connexion et de Remote Control qu’un agent ou un serveur MCP transmet ne s’ouvrent plus que s’il
  s’agit d’adresses web.
- Il n’est plus possible de faire confiance dans leur ensemble aux dossiers qui contiennent bien plus qu’un projet,
  comme votre dossier de départ, le Bureau, Documents, Téléchargements ou la racine d’un volume ; faites confiance à
  leurs projets un par un. La question de confiance précise désormais que la confiance s’étend aussi aux sous-dossiers.
- Les jetons CI et les connexions ne passent plus jamais en http non chiffré vers un autre serveur que ce Mac ; un dépôt
  dont le dépôt distant commence par `http://` est interrogé en https.
- Sans gitleaks, les prédictions attendent, avant que du code parte vers un fournisseur extérieur à ce Mac, que vous
  choisissiez d’envoyer sans vérification. Les réglages gitleaks propres à un projet ne désactivent plus la
  vérification.
- L’agent intégré ne peut plus être amené à écrire hors du projet par un lien qui apparaît après l’autorisation de son
  écriture, et les fichiers qui ont d’autres liens physiques reçoivent leur propre fichier au lieu d’être modifiés à
  travers eux.
- Les complétions d’un serveur de langage ne peuvent plus bloquer l’éditeur, et ses renommages et suppressions d’un
  fichier lié agissent sur le lien, et non sur sa cible.
- Les images que vous joignez à un prompt sont décodées par le service de décodage isolé, comme partout ailleurs.
- Un programme dans le terminal ne peut remplacer le presse-papiers que tant que son terminal est au premier plan, et le
  terminal le signale.
- rust-analyzer s’installe à partir d’une version figée avec sa somme de contrôle, au lieu de la plus récente du moment.

## Recherche et navigation
- Ouverture rapide indique quand aucun fichier ne correspond et montre si elle recherche des fichiers ou les symboles du
  projet.
- Les symboles du projet incluent TypeScript et JavaScript sans qu’un de leurs fichiers soit ouvert. Les résultats
  exacts du projet et du langage dans lequel vous travaillez viennent en premier ; les symboles des bibliothèques et des
  paquets installés sont signalés.
- Présentation › Recherche place le focus dans le champ de recherche.

## Éditeur
- L’éditeur garde le focus quand un fichier Markdown s’ouvre avec son aperçu à côté, si bien que le menu Format
  fonctionne aussitôt. Les commandes de l’éditeur fonctionnent aussi quand le focus est dans la barre latérale.
- Une suggestion en fin de ligne ou dans un encadré en dessous indique que la touche Tab l’accepte.
- Les onglets de même nom dans la vue d’ensemble des onglets se distinguent, par exemple un fichier et son aperçu dans
  le navigateur.
- Le menu Présentation ne propose plus « Masquer la barre d’onglets » de macOS, que les onglets de Dango n’utilisent
  pas.

## Langages et débogage
- Le serveur JSON vérifie désormais les fichiers, et les serveurs qui enregistrent leurs diagnostics après leur
  démarrage, comme basedpyright et sourcekit-lsp, les affichent.
- Là où pip refuse d’installer dans Python, comme dans celui de Homebrew, le débogueur propose d’installer debugpy dans
  un environnement virtuel du projet.
