<p><img src="../../readme-icon.png" alt="Icône de Dango" width="168" height="168"></p>

# Dango

**L’éditeur macOS natif pour travailler avec des agents de code.**

Dango réunit le code, les agents qui le modifient et tout ce qui les entoure dans une fenêtre par dossier : un éditeur avec serveurs de langage et prédictions pendant la saisie, des terminaux, un navigateur, Git, la CI, un débogueur et des fils IA avec Claude, Codex, Antigravity, OpenCode ou un agent intégré à l’app. Chaque agent travaille dans la même chronologie, demande avant de modifier des fichiers ou d’exécuter des commandes, et laisse un point de contrôle avant chaque prompt, pour que vous puissiez examiner et annuler ce qu’il a fait, message par message. Rien de ce qu’un dossier configure ne s’exécute avant que vous lui fassiez confiance, et le texte venu de l’extérieur parvient à un agent marqué comme non fiable. Dictez dans l’éditeur et dans le champ de message ; la reconnaissance vocale peut rester sur votre Mac. Dango est écrit en Rust sur de vrais contrôles AppKit, sans Electron, parle 35 langues et n’a aucune télémétrie.

**Pas encore publié.** La première version sera une image disque signée avec un Developer ID et notarisée par Apple, pour macOS 14 ou ultérieur sur Apple Silicon et Intel. D’ici là, [compilez Dango depuis les sources](#compiler-depuis-les-sources).

**Vous avez trouvé un bug ?** [Ouvrez une issue](https://github.com/sventmueller/dango/issues/new) et indiquez avec quel agent et dans quel type de projet il s’est produit.

[![Rust, édition 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 ou ultérieur](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universel : Apple Silicon et Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal : alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 langues d’interface](https://img.shields.io/badge/languages-35-4C9A2A)
![Aucune télémétrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signé avec le Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licence : MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licence)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
**Français** ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Sommaire

- [Fonctionnalités en bref](#fonctionnalités-en-bref)
- [Configuration requise](#configuration-requise)
- [Installation](#installation)
- [Premiers pas](#premiers-pas)
- [Agents IA](#agents-ia)
  - [La chronologie](#la-chronologie)
  - [Autorisations et modes](#autorisations-et-modes)
  - [Points de contrôle et worktrees](#points-de-contrôle-et-worktrees)
- [Éditeur](#éditeur)
- [Serveurs de langage](#serveurs-de-langage)
- [Tests](#tests)
- [Débogueur](#débogueur)
- [Terminal](#terminal)
- [Navigateur](#navigateur)
- [Git](#git)
- [CI](#ci)
- [Conteneurs](#conteneurs)
- [Markdown](#markdown)
- [Dictée](#dictée)
- [Confiance](#confiance)
- [Sandbox](#sandbox)
- [Apparence et accessibilité](#apparence-et-accessibilité)
- [Langues](#langues)
- [Raccourcis clavier](#raccourcis-clavier)
- [Confidentialité et sécurité](#confidentialité-et-sécurité)
- [Emplacement des données](#emplacement-des-données)
- [Limites connues](#limites-connues)
- [Compiler depuis les sources](#compiler-depuis-les-sources)
- [Licence](#licence)

## Fonctionnalités en bref

- **Tous les agents dans une seule chronologie.** Claude via sa propre commande `claude`, Codex, Antigravity et OpenCode via l’Agent Client Protocol, d’autres depuis le registre ACP, et un agent intégré à l’app pour Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama et LM Studio. Réponses, appels d’outils, demandes d’autorisation, plans, tâches, sous-agents et tâches d’arrière-plan ont le même aspect, quel que soit l’agent.
- **Examiner et annuler.** Avant chaque prompt, Dango conserve un point de contrôle. Ce qu’un agent a modifié peut être examiné et annulé message par message, et un fil peut travailler dans son propre worktree Git et livrer son travail par fusion ou cherry-pick.
- **Une fenêtre par dossier.** Fichiers, terminaux, pages web, fils IA, diffs et aperçus sont des onglets macOS de cette fenêtre. La barre latérale contient Fichiers, Recherche, Agents, Git, Tests, Déboguer, CI et Conteneurs.
- **Un vrai éditeur de code.** Couleurs syntaxiques, plusieurs points d’insertion, repli, `.editorconfig`, serveurs de langage pour 17 langages avec complétion, atteindre la définition, références, renommage, corrections rapides et hiérarchies, et prédictions pendant la saisie par GitHub Copilot ou un modèle local.
- **Débogueur, Git et CI intégrés.** Points d’arrêt et pas à pas pour Rust, C, C++, Swift, Go, JavaScript, TypeScript et Python ; indexation de blocs isolés, branches, stashes, conflits et graphe des commits ; exécutions, jobs et journaux de GitHub Actions et GitLab CI.
- **Prudent avec vos dossiers.** Un dossier auquel vous ne faites pas confiance ne lance rien : ni processus Git, ni serveurs de langage, ni hooks, ni agents. Les commandes Git destructrices ne sont jamais autorisées définitivement, et les commits sont vérifiés à la recherche de secrets.
- **Dictée.** Parlez dans l’éditeur et dans le champ de message, en ligne via OpenAI ou Groq, ou localement avec Whisper et Parakeet.
- **Natif jusqu’au bout.** Fenêtres, onglets, menus et barres d’outils AppKit, TextKit 2 pour l’éditeur, Metal pour le terminal, WebKit uniquement dans l’onglet du navigateur. Le mode sombre, Augmenter le contraste et VoiceOver fonctionnent comme dans toute app Mac.
- **Dans votre langue.** L’interface existe en 35 langues : allemand, anglais, arabe, catalan, chinois (simplifié), chinois (traditionnel), coréen, croate, danois, espagnol, finnois, français, grec, hébreu, hindi, hongrois, indonésien, italien, japonais, malais, néerlandais, norvégien, polonais, portugais (Brésil), portugais (Portugal), roumain, russe, slovaque, slovène, suédois, tchèque, thaï, turc, ukrainien et vietnamien.

## Configuration requise

- macOS 14 Sonoma ou ultérieur, sur Apple Silicon ou Intel. Dango est développé et testé sur macOS 27.
- Pour les fils IA, l’agent que vous voulez utiliser ; voir [Agents IA](#agents-ia).
- Pour les serveurs de langage et les adaptateurs de débogage, que Dango installe à votre demande : Go pour gopls et `dlv` ; Node.js pour les serveurs TypeScript, Python, JSON, YAML, CSS, HTML et shell et pour le débogueur JavaScript ; les Xcode Command Line Tools pour clangd, sourcekit-lsp et `lldb-dap`. rust-analyzer est téléchargé depuis ses versions GitHub.
- Pour les prédictions pendant la saisie, un abonnement GitHub Copilot (le gratuit suffit), ou un modèle fill-in-the-middle dans Ollama ou LM Studio.

## Installation

**Depuis l’image disque.** Chaque version est une image disque, `Dango-<version>.dmg`, avec une app universelle pour Apple Silicon et Intel, signée avec un Developer ID et notarisée par Apple. Ouvrez-la et faites glisser Dango sur le dossier Applications. Un Cask Homebrew installe la même image. Aucune version n’est encore publiée.

**Mises à jour.** Dango installé depuis l’image disque se tient à jour avec [Sparkle](https://sparkle-project.org). Au deuxième lancement, il demande s’il peut rechercher les mises à jour de lui-même ; tant que vous n’avez pas accepté, il n’envoie aucune requête. **Dango › Rechercher les mises à jour…** vérifie immédiatement. Les mises à jour sont signées avec la clé propre de l’app, que Sparkle vérifie avant d’installer quoi que ce soit. Une version compilée par vos soins n’a pas de mise à jour automatique.

**Depuis les sources.** Voir [Compiler depuis les sources](#compiler-depuis-les-sources), puis déplacez `target/release/Dango.app` dans `/Applications`.

**La commande de terminal.** **Dango › Installer la commande « dango »…** écrit un petit script dans `~/.local/bin/dango`, sans droits d’administrateur ; `~/.local/bin` doit figurer dans votre `PATH`. Ensuite, `dango .` ouvre le dossier actuel et `dango file.rs` un fichier. Dango ne remplace jamais à cet endroit un fichier qu’il n’a pas écrit lui-même.

## Premiers pas

1. **Ouvrez un dossier** avec ⌘O ou `dango <folder>`. La première fois, Dango vous demande si vous lui faites confiance (voir [Confiance](#confiance)).
2. **La barre latérale** passe de Fichiers (⌘1) à Recherche (⌘2), Agents (⌘3), Git (⌘4), Tests (⌘5), Déboguer (⌘6), CI (⌘7) et Conteneurs (⌘8) ; un clic droit masque ceux que vous n’utilisez pas, et ⌃⌘S replie la barre latérale.
3. **Chaque document est un onglet** de la fenêtre du dossier, les onglets propres à macOS. Le « + » au bout de la barre d’onglets et ⌘T proposent un nouveau fichier, un terminal, le navigateur et chaque agent installé, aussi dans son propre worktree Git ; maintenir le « + » enfoncé affiche la même chose sous forme de menu.
4. **Ouverture rapide** avec ⌘P trouve les fichiers, et avec `#` les symboles du projet ; ⇧⌘F recherche et remplace dans le projet.
5. **Parlez à un agent** dans un onglet IA. ⌘Esc place le focus clavier dans son champ de message et l’en ressort.
6. **Réglages** avec ⌘, contient les onglets Général, CI, Agents, Agent intégré, Serveur de langage, Dictée et Prédiction.

La barre d’outils contient retour et avance parmi les endroits que vous avez modifiés (⌃- et ⌃⇧-), la branche, Ouverture rapide et l’aperçu des activités, qui liste tout ce qui tourne. **Ouvrir à côté** dans le menu contextuel d’un fichier affiche un second document à côté du premier. Après un redémarrage ou un plantage, chaque fenêtre revient avec ses onglets, et les éditeurs avec du texte non enregistré reviennent avec ce texte. Images, audio, vidéo, polices et documents Office s’ouvrent dans un onglet via Coup d’œil, les PDF via PDFKit.

## Agents IA

| Agent | Fonctionnement | Ce qu’il faut |
|---|---|---|
| Claude | la commande `claude`, pilotée directement via son protocole stream-json | `claude` installé et connecté ; Dango cherche dans le `PATH` du shell de connexion, dans `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` et `/usr/local/bin`. La connexion se fait uniquement dans `claude` lui-même |
| Codex | `codex-acp` via l’Agent Client Protocol (ACP) | Node.js et npm ; Dango installe le paquet à version figée à votre demande |
| Antigravity | `agy acp` | votre propre `agy` dans le `PATH`, ou le téléchargement à version figée, installé à votre demande |
| OpenCode | `opencode acp` | votre propre `opencode` dans le `PATH`, ou le téléchargement à version figée, installé à votre demande |
| Autres agents ACP | depuis le registre ACP, récupéré uniquement à votre demande | ce que nomme l’entrée du registre ; les entrées sans somme de contrôle ne s’installent qu’après votre confirmation |
| Agent intégré | intégré à l’app, avec des outils pour les fichiers, la recherche, les commandes, les diagnostics et les requêtes web | une clé d’API pour Anthropic, OpenAI, Google Gemini ou OpenRouter, conservée dans le trousseau ; Ollama ou LM Studio sur ce Mac sans clé ; tout serveur qui parle l’API d’OpenAI ou d’Anthropic |

**Réglages › Agents** indique quels agents ce Mac possède, installe les autres et leurs mises à jour, et ouvre le registre ACP. Les agents que Dango installe sont figés sur des versions testées et vérifiés par rapport à leurs sommes de contrôle publiées (npm integrity ou SHA-256) ; Dango n’exécute aucun paquet `npx` sans version.

### La chronologie

Chaque agent travaille dans un onglet IA avec la même chronologie : réponses, appels d’outils avec leur sortie, demandes d’autorisation, questions, plans à approuver et à commenter (⌥⌘K), tâches, sous-agents et tâches d’arrière-plan. Deux boutons dans l’en-tête de l’onglet listent les sous-agents et processus d’arrière-plan en cours. Mentionnez un fichier, `@terminal` ou `@browser` pour transmettre son contenu à l’agent. Un point vert sur l’onglet signale une session au travail, un point rouge une session qui vous attend, et une notification indique quand elle a terminé ou attend pendant que vous regardez ailleurs. Pour Claude, la ligne d’état affiche le contexte et la consommation.

### Autorisations et modes

Les nouveaux fils démarrent en mode « Manuel » : l’agent demande avant de modifier des fichiers ou d’exécuter des commandes. Pour les commandes Git destructrices (`push --force`, `reset --hard`, `clean -fd`), Dango ne propose pas « Toujours autoriser ». L’agent intégré conserve ses règles « Toujours autoriser » par projet, les liste dans **Réglages › Agent intégré** et demande à chaque fois pour les commandes Git destructrices, quoi qu’il ait enregistré ; Claude et les agents ACP conservent ces règles dans leur propre configuration. Avec « Tout autoriser » et sans sandbox, le fil affiche un avertissement permanent.

### Points de contrôle et worktrees

Avant chaque prompt, Dango conserve un point de contrôle, sous forme de commit sous `refs/ide-checkpoints/` dans le dépôt du projet, ou dans son propre dossier pour un projet sans Git ; un push ou un fetch ordinaire ne les emporte jamais. Ce qu’un agent a modifié peut être examiné et annulé message par message. Un fil peut travailler dans son propre worktree Git : le panneau Git montre ce qu’il a modifié et reprend le travail par fusion ou cherry-pick.

## Éditeur

- **Couleurs syntaxiques, numéros de ligne et marques de modification** par rapport au dernier commit, rechercher et remplacer, Ouverture rapide (⌘P) et recherche dans le projet (⇧⌘F).
- **L’indentation** suit le fichier et `.editorconfig` : tabulations ou espaces, indentation automatique, paires de parenthèses.
- **Plusieurs points d’insertion :** ⌥-clic, ⌥-glisser pour une colonne, ⌃⇧↑ et ⌃⇧↓, ⌘D pour l’occurrence suivante.
- **Repli** (⌥⌘← et ⌥⌘→, avec ⇧ pour tout), activer ou désactiver un commentaire (⌘/), aller à la ligne (⌃G), espaces visibles et retour à la ligne automatique (⌥Z).
- **Les fichiers conservent leur encodage**, leur indicateur d’ordre des octets et leurs fins de ligne. Un fichier modifié sur le disque est rechargé quand l’éditeur n’a pas de texte non enregistré ; sinon, une barre propose les différences et une fusion.
- **Prédictions pendant la saisie.** Pendant que vous tapez, une suggestion apparaît en gris, et Tab l’accepte. Elle vient de GitHub Copilot ou d’un modèle local dans Ollama ou LM Studio, selon le choix fait dans **Réglages › Prédiction** ; un modèle local garde votre code sur ce Mac.
- **Minicarte et Défilement épinglé** (menu Éditeur) : une image étroite de tout le fichier à côté du texte, et les lignes d’en-tête des blocs qui contiennent la ligne du haut, épinglées au-dessus d’elle.
- **Diagnostics en fin de ligne** (menu Présentation) : le message le plus grave de chaque ligne après une seconde d’inactivité, avec un symbole pour sa gravité.
- **Aperçu en direct des pages statiques :** Éditeur › Ouvrir dans l’onglet navigateur sert les pages d’un dossier de confiance depuis Dango lui-même et recharge l’onglet quand un fichier est enregistré.
- **Prettier** met en forme JavaScript, TypeScript, CSS, HTML, Markdown et plus encore là où le projet le configure, avec le `node_modules/.bin/prettier` propre au projet ; le réglage `formatter` choisit le formateur par langage.
- **Mode Vim** (Éditeur › Mode Vim) : modes Normal, Insertion, Visuel et Remplacement, compteurs, opérateurs avec mouvements et objets textuels, `.`, registres, marques et macros. `u` et ⌃R utilisent l’annulation de l’éditeur, et un passage en mode Insertion compte pour une seule étape. `"+` et `"*` sont le presse-papiers ; le réglage `vimClipboard` fait aussi du registre sans nom le presse-papiers. `/`, `?`, `n`, `N`, `*`, `:s` et `:%s` cherchent avec les expressions régulières de Rust, pas avec les motifs de Vim : pas de `\v`, et `\b` à la place de `\<` et `\>`. Les commandes Ex sont `:w`, `:q`, `:wq`, `:x`, `:noh` et `:<line>`. En mode Normal, ⌃G revient à Vim au lieu d’aller à la ligne.
- **Synchronisation des réglages** via un dossier d’iCloud Drive (**Réglages › Général**) : vos réglages et raccourcis vous suivent sur vos autres Mac ; les chemins, les adresses locales et les consentements restent sur chacun.

## Serveurs de langage

Go, Rust, TypeScript et JavaScript, Python, C, C++ et Objective-C, Swift, scripts shell, JSON, YAML, CSS, HTML, TOML, Markdown et Lua, plus vos propres serveurs depuis **Réglages › Serveur de langage**. Dango installe un serveur à votre demande, dans son propre dossier d’outils, et utilise le `tsc` d’un projet TypeScript quand il peut servir de serveur de langage.

Ce qu’ils apportent : diagnostics (⌘' et ⇧⌘' passent de l’un à l’autre), survol (⌃⌘I), complétion (⌥Esc), aide aux signatures, atteindre la définition (⌃⌘J), la définition de type et l’implémentation, références (⌃⇧⌘F), hiérarchies d’appels et de types (⌃⇧⌘H), renommage (⌃⌘E), mise en forme (⌥⇧F), couleurs sémantiques, repli et la structure (⇧⌘O) ; types et noms de paramètres dans la ligne ; corrections rapides et refactorisations derrière une ampoule (⌥⌘A) ; les symboles du projet avec ⌥⌘O.

En plus du serveur propre à un fichier, ESLint vérifie le JavaScript et le TypeScript des projets qui l’utilisent, avec ses corrections rapides, et les abréviations Emmet apparaissent dans la complétion pour HTML, CSS et JSX. Dango installe les deux à votre demande ; **Réglages › Général** corrige les problèmes ESLint à l’enregistrement si vous le souhaitez.

## Tests

Le panneau Tests liste les tests d’un dossier par paquet et par fichier avec leur état : `cargo test` ou cargo-nextest, Go, pytest, Vitest et Jest. Un symbole à côté de chaque test dans la marge l’exécute, ⌥-clic le débogue, et une assertion en échec marque sa ligne. Le menu Déboguer exécute le test au point d’insertion (⌃⌥⌘U), à nouveau les derniers tests (⌃⌥⌘G) ou tous, et débogue le test au point d’insertion. Les tests ne s’exécutent que dans les dossiers de confiance, puisqu’ils exécutent le code du projet ; `cargo test` ne liste ses tests que lorsque vous le demandez, car les lister compile chaque programme de test.

## Débogueur

Points d’arrêt, aussi avec conditions et comme points de journalisation, pas à pas, fils d’exécution avec leurs piles d’appels, variables et expressions surveillées dans l’inspecteur (⌥⌘I), valeurs à côté du code tant que le programme est en pause, et une console de débogage qui évalue dans le cadre en pause.

| Langage | Adaptateur de débogage |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` des Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | le débogueur JavaScript, sur Node.js |
| Python | debugpy |

Le `.vscode/launch.json` d’un projet est lu. Le menu Déboguer reprend les raccourcis de Xcode : ⌘\\ pose un point d’arrêt, ⌘Y les active ou désactive tous, ⌃⌘Y continue, F6, F7 et F8 passent par-dessus, entrent et sortent, ⌘. arrête.

## Terminal

Des terminaux dans des onglets (⌃\`) avec l’environnement du shell de connexion, dessinés avec Metal. ⌘K efface l’écran. Une image collée ou déposée est transmise à un agent de code qui tourne dans le terminal, ou colle le chemin d’une copie enregistrée. ⌘-clic ouvre les liens et les noms de fichiers. Un serveur de développement lancé depuis le terminal peut s’ouvrir dans l’onglet du navigateur.

## Navigateur

Un onglet de navigateur avec champ d’adresse (⌘L), précédent et suivant (⌘[ et ⌘]), actualiser (⌘R), recherche, téléchargements qui demandent d’abord, et l’inspecteur web. `@browser` transmet la page à un agent, marquée comme texte venu de l’extérieur.

## Git

Le panneau Git indexe et abandonne des fichiers ou des blocs isolés, crée des commits, commite puis pousse ou synchronise, change de branche et en crée, garde des stashes, résout les conflits de fusion et dessine un graphe des commits récents. Blame indique qui a modifié en dernier la ligne sous le curseur, ou une colonne pour tout le fichier. Un bouton rédige le message de commit avec l’IA ; le diff ne part qu’une fois que gitleaks n’y a trouvé aucun secret, et chaque commit est aussi vérifié à la recherche de secrets.

## CI

GitHub Actions et GitLab CI, y compris GitHub Enterprise Server et GitLab auto-hébergé :

- exécutions, jobs et journaux de la branche actuelle ;
- relancer les jobs en échec, un seul job ou, sur GitHub, une exécution entière ; lancer des jobs manuels ou approuver des déploiements en attente ; lancer un nouveau pipeline ou un workflow qui peut être lancé à la main ; annuler ;
- les pull requests ou merge requests ouvertes et les issues, qui s’ouvrent dans l’onglet du navigateur ;
- le journal d’un job en échec transmis à un agent, marqué comme texte venu de l’extérieur.

Le jeton vient du trousseau, de `gh` ou `glab`, de l’environnement du shell de connexion, ou c’est celui que vous saisissez dans **Réglages › CI**.

## Conteneurs

Le panneau Conteneurs affiche les projets Compose de Docker avec leurs services, les conteneurs isolés, les images et les volumes, et suit les événements de Docker au fur et à mesure. Un fichier Compose du dossier apparaît comme un projet avant même qu’un de ses conteneurs existe. Les conteneurs démarrent, s’arrêtent et redémarrent ; leurs journaux et un shell s’ouvrent dans des onglets de terminal, et un port publié s’ouvre dans l’onglet du navigateur. Supprimer un conteneur, une image ou un volume, et nettoyer les données inutilisées, demandent d’abord confirmation. Les commandes up, down et pull de Compose ne s’exécutent que dans les dossiers de confiance, puisque Compose construit et exécute ce que décrit le projet. Le menu du panneau choisit le contexte Docker de la fenêtre sans changer celui qu’utilisent vos shells ; les mêmes commandes figurent dans le menu Conteneurs. Le panneau nécessite la ligne de commande `docker`, Docker 24.0.6 ou ultérieur.

## Markdown

- **Aperçu.** Un fichier Markdown s’ouvre avec son aperçu à côté ; **Présentation › Afficher/masquer l’aperçu à côté** (⇧⌘V) le ferme et l’ouvre, et **Réglages › Général** désactive l’aperçu automatique. L’aperçu dessine les formules, les diagrammes Mermaid, Graphviz et PlantUML et les images du dossier ; les images du web ne se chargent que dans un dossier de confiance ou quand vous les demandez dans l’aperçu.
- **Menu Format :** gras (⌘B), italique (⌘I), barré (⇧⌘X), code (⌥⇧⌘C), lien (⇧⌘K), titres 1 à 3 (⌥⌘1–⌥⌘3), listes à puces et numérotées, tâches, citations et blocs de code. Chaque commande bascule, sur la sélection ou les lignes, et constitue une seule étape d’annulation.
- **Coller.** Le texte venu de Word ou d’une page web arrive en Markdown, ses images comme fichiers à côté du document ; ⌥⇧⌘V colle du texte brut.
- **Exporter.** **Fichier › Exporter** écrit un PDF, un fichier HTML unique contenant ses images, ou un document Word. Les images du web ne sont incluses que depuis un dossier de confiance.
- **Ouvrir dans Splat.** Si [Splat](https://github.com/sventmueller/splat) est installé, **Fichier › Ouvrir dans Splat** et les menus contextuels de la liste des fichiers et des onglets y ouvrent le document, après l’avoir enregistré.

## Dictée

Dictez dans le champ de message d’un onglet IA ou dans l’éditeur : avec le bouton micro qui s’y trouve, **Édition › Démarrer la dictée** (⌃⌥D) ou la touche fn (maintenir pour enregistrer, toucher deux fois pour continuer à enregistrer). La reconnaissance vocale est celle de [Lowspeak](https://github.com/sventmueller/lowspeak) ; Lowspeak lui-même n’a pas besoin d’être installé.

- **Réglages › Dictée** définit le mode et le modèle (en ligne avec OpenAI ou Groq, localement avec Whisper ou Parakeet), la langue, le post-traitement, le micro et votre propre vocabulaire.
- **Les modèles locaux** sont téléchargés depuis Hugging Face quand vous en choisissez un, avec la progression et la possibilité d’annuler. Ils sont placés dans les dossiers qu’utilise Lowspeak, si bien que chaque app utilise ce que l’autre a téléchargé. Un modèle local garde l’enregistrement sur ce Mac.
- **La transcription en ligne** nécessite une clé pour OpenAI ou Groq, que Dango conserve dans son propre élément du trousseau.
- **Depuis Lowspeak.** Si Lowspeak est réglé autrement que par défaut, l’onglet Dictée propose une fois de reprendre ses réglages, et ses clés sur demande ; macOS demande avant que Dango n’en lise une (choisissez « Autoriser », pas « Toujours autoriser »).
- **Autorisations.** macOS demande l’accès au micro à la première dictée. La touche fn fonctionne tant que Dango est l’app active et que Lowspeak ne tourne pas ; elle nécessite l’autorisation d’accessibilité, que Dango ne demande qu’après la première dictée, jamais au lancement.

## Confiance

Un dossier peut faire exécuter des programmes dès que quelque chose y est lancé : hooks Git et réglages du dépôt comme `core.fsmonitor`, serveurs de langage depuis `node_modules`, hooks d’agents, `.mcp.json`. La première fois que vous ouvrez un dossier, Dango vous demande si vous lui faites confiance.

- **De confiance :** Git, les serveurs de langage, les débogueurs, les agents et la CI s’exécutent pour ce dossier et tout ce qu’il contient.
- **Restreint :** vous pouvez lire et modifier ; les marques de modification dans l’éditeur viennent de la lecture directe de `.git`. Rien de ce que le dossier configure n’est lancé. Les panneaux Git, CI et IA l’indiquent et proposent de faire confiance au dossier.

Un texte qui ne vient pas de vous peut contenir des instructions destinées à piloter un agent (injection de prompt). La sortie d’un terminal (`@terminal`), une page web (`@browser`), un journal de CI transmis à un agent, un fichier hors du projet et un texte plus long collé depuis une page web sont donc transmis marqués : la puce indique « Contenu externe », et l’agent reçoit le texte avec l’avis qu’il s’agit de données non fiables dont il ne doit pas suivre les instructions. Les pages web de l’agent intégré arrivent avec le même avis. Un texte collé compte comme venant d’une page web quand le navigateur l’indique dans le presse-papiers, comme le font Safari, Chrome et Firefox ; un texte venu d’ailleurs, et une courte phrase de quelques mots, restent les vôtres.

## Sandbox

Dango lui-même tourne sans App Sandbox, car chaque programme qu’il lance en hériterait : terminaux, Git, serveurs de langage et agents ne pourraient pas fonctionner. À la place, chaque fil IA a un interrupteur de sandbox, désactivé par défaut. Ce qu’il limite dépend de l’agent, et l’info-bulle de l’interrupteur le précise :

| Agent | Avec la sandbox activée |
|---|---|
| Claude | les commandes shell s’exécutent dans la sandbox propre à Claude ; ses outils pour les fichiers et le web, non |
| Codex | les commandes et les modifications n’écrivent que dans le projet, sans réseau |
| Agent intégré | fichiers, requêtes web et commandes restent dans le projet et les dossiers autorisés, sans réseau (`sandbox-exec`) |
| Antigravity, OpenCode | aucune sandbox que Dango puisse activer |

## Apparence et accessibilité

- **Le mode sombre** suit le système ; **Augmenter le contraste** et **Réduire les animations** sont respectés.
- **VoiceOver** nomme la barre latérale, les onglets, les cartes de la chronologie IA, les lignes de Git et de la CI, le débogueur et le texte du terminal. Les points d’arrêt ne se distinguent jamais par la seule couleur.
- **Clavier.** Chaque commande a un raccourci, et tous peuvent être modifiés dans `keybindings.json` ; ⌥⌘\` passe au panneau suivant.
- **De droite à gauche.** En arabe et en hébreu, la fenêtre est inversée, tandis que les vues de code restent de gauche à droite.

Ce que Dango ajoute à AppKit, domaine par domaine, se trouve dans [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Langues

L’interface est traduite en 35 langues : allemand, anglais, arabe, catalan, chinois (simplifié), chinois (traditionnel), coréen, croate, danois, espagnol, finnois, français, grec, hébreu, hindi, hongrois, indonésien, italien, japonais, malais, néerlandais, norvégien, polonais, portugais (Brésil), portugais (Portugal), roumain, russe, slovaque, slovène, suédois, tchèque, thaï, turc, ukrainien et vietnamien.

Dango suit l’ordre des langues de **Réglages Système › Général › Langue et région** ; vous pouvez aussi y choisir une langue pour Dango seul sous **Apps**. Sans traduction pour votre langue, il utilise l’anglais. Les termes suivent un glossaire, [docs/glossary.md](../../../docs/glossary.md), et les traductions d’Apple, pour que Dango se lise comme le reste de votre Mac.

## Raccourcis clavier

| Action | Raccourci |
|---|---|
| Ouvrir un dossier, nouvelle fenêtre, fermer la fenêtre | ⌘O, ⇧⌘N, ⇧⌘W |
| Nouveau fichier, enregistrer, enregistrer sous | ⌘N, ⌘S, ⇧⌘S |
| Nouvel onglet, fermer l’onglet, onglet suivant et précédent | ⌘T, ⌘W, ⇧⌘] et ⇧⌘[ |
| Ouverture rapide, rechercher dans le projet | ⌘P, ⇧⌘F |
| Fichiers, Recherche, Agents, Git, Tests, Déboguer, CI, Conteneurs | ⌘1 à ⌘8 |
| Afficher ou masquer la barre latérale, panneau suivant | ⌃⌘S, ⌥⌘\` |
| Retour et avance parmi les endroits modifiés | ⌃- et ⌃⇧- |
| Activer/désactiver le commentaire, aller à la ligne | ⌘/, ⌃G |
| Occurrence suivante, point d’insertion au-dessus et en dessous | ⌘D, ⌃⇧↑ et ⌃⇧↓ |
| Replier, déplier, tout replier, tout déplier | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Retour à la ligne automatique, inspecteur | ⌥Z, ⌥⌘I |
| Complétion, survol, atteindre la définition | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Références, hiérarchie d’appels, renommer | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symboles du fichier, du projet | ⇧⌘O, ⌥⌘O |
| Correction rapide, mise en forme | ⌥⌘A, ⌥⇧F |
| Problème suivant et précédent | ⌘' et ⇧⌘' |
| Aperçu Markdown, coller en texte brut | ⇧⌘V, ⌥⇧⌘V |
| Gras, italique, barré, code, lien | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Titre 1 à 3 | ⌥⌘1 à ⌥⌘3 |
| Nouveau terminal, effacer le terminal | ⌃\`, ⌘K |
| Focus sur le champ de message de l’agent, commenter un plan | ⌘Esc, ⌥⌘K |
| Champ d’adresse, actualiser, précédent, suivant (navigateur) | ⌘L, ⌘R, ⌘[, ⌘] |
| Démarrer ou arrêter la dictée | ⌃⌥D |
| Point d’arrêt, activer ou désactiver tous les points d’arrêt | ⌘\\, ⌘Y |
| Exécuter le test au point d’insertion, réexécuter les derniers tests | ⌃⌥⌘U, ⌃⌥⌘G |
| Continuer, pas par-dessus, entrer, sortir, arrêter | ⌃⌘Y, F6, F7, F8, ⌘. |
| Réglages | ⌘, |

`keybindings.json` dans le dossier de données de l’app (voir [Emplacement des données](#emplacement-des-données)) modifie n’importe lequel d’entre eux.

## Confidentialité et sécurité

- **Aucune télémétrie,** aucune analyse, aucun rapport de plantage envoyé où que ce soit. Les erreurs vont dans le journal système (Console, sous le nom de l’app).
- **Le réseau uniquement pour ce que vous lancez :** les agents que vous utilisez, chacun avec son propre service ; le registre ACP et les téléchargements d’agents, de serveurs de langage, d’adaptateurs de débogage et de modèles de dictée quand vous les demandez ; les hôtes de CI auxquels vous vous connectez ; les prédictions de Copilot si vous l’avez choisi ; les fournisseurs de l’agent intégré ; la transcription en ligne si vous l’avez choisie ; les pages que vous ouvrez dans l’onglet du navigateur ; et la recherche de mises à jour une fois que vous l’avez autorisée.
- **Le code ne quitte le Mac** que là où vous avez choisi un service pour cela : un agent, les prédictions de Copilot, un message de commit rédigé par l’IA. Le diff d’un message de commit ne part qu’une fois que gitleaks n’y a trouvé aucun secret.
- **Les clés et jetons** restent dans le trousseau macOS, jamais dans un fichier de réglages. Les clés d’autres apps, comme celles de Lowspeak, ne sont lues que si vous acceptez, et une seule fois.
- **Téléchargements figés.** Les agents, serveurs de langage et adaptateurs de débogage que Dango installe sont figés sur des versions testées et vérifiés par rapport à leurs sommes de contrôle.
- **Signature du code** avec le Hardened Runtime ; les mises à jour ne s’installent qu’avec la signature propre de l’app.

## Emplacement des données

`<id>` est l’identifiant de bundle, `local.agentide` pour la version publiée (`local.agentide.debug` pour les builds de débogage). Il ne contient pas le nom affiché, si bien qu’un changement de nom conserve toutes les données.

| Emplacement | Contenu |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json` ; `threads/` (la liste du panneau IA), `agent-threads/` (conversations de l’agent intégré), `layouts/` (onglets par projet), `recovery/` (éditeurs non enregistrés après un plantage), `checkpoints/` (pour les dossiers sans Git), `worktrees/`, `agents/` et `tools/` (agents, serveurs de langage et adaptateurs de débogage installés), `task-output/` |
| `~/Library/Caches/<id>/` | images collées, scripts d’intégration au shell, le registre ACP ; peut être supprimé |
| `~/Library/Logs/<id>/dictation/` | les fichiers journaux de la dictée |
| `~/Library/Application Support/Lowspeak` et `FluidAudio` | modèles de dictée locaux, partagés avec Lowspeak |
| `~/Library/WebKit/<id>/` | cookies et données de sites web de l’onglet du navigateur |
| `<project>/.dango/settings.json` | les réglages propres au projet |
| le dépôt Git du projet | points de contrôle, sous forme de commits sous `refs/ide-checkpoints/` ; un push ou un fetch ordinaire ne les emporte jamais |
| trousseau, service `<id>` | clés d’API de l’agent intégré et de la dictée, jetons de CI |
| `~/.local/bin/dango` | la commande de terminal, si vous l’avez installée |

Claude conserve ses propres sessions sous `~/.claude/projects` ; Dango les lit pour reprendre une session mais ne les modifie pas.

## Limites connues

- **Pas d’App Sandbox** pour l’app elle-même, car terminaux, Git, serveurs de langage et agents en hériteraient ; un dossier doit être de confiance avant que quoi que ce soit qu’il configure ne s’exécute. Antigravity et OpenCode n’ont pas de sandbox que Dango puisse activer.
- **Les chiffres de consommation et la répartition du contexte de Claude** reposent sur des parties de son protocole marquées comme expérimentales, et sont masqués quand elles manquent.
- **Revenir en arrière dans la conversation** ne fonctionne que pour Claude ; pour les autres agents, seul le code est restauré.
- **La connexion à la CI via le navigateur** est désactivée ; utilisez `gh`, `glab`, un jeton ou l’environnement.
- **Gemini CLI n’est pas proposé :** Google a mis fin à la connexion avec un compte personnel et renvoie vers Antigravity. Les fils Gemini antérieurs restent lisibles.

## Compiler depuis les sources

Il vous faut :

- macOS 14 ou ultérieur avec les Xcode Command Line Tools (`xcode-select --install`) ; Xcode lui-même n’est pas nécessaire.
- Rust tel que le nomme `rust-toolchain.toml`. Le `rust` de Homebrew compile pour le processeur de ce Mac ; la compilation universelle d’une version nécessite rustup avec les cibles `aarch64-apple-darwin` et `x86_64-apple-darwin`.
- Des copies de [splat](https://github.com/sventmueller/splat) et [lowspeak](https://github.com/sventmueller/lowspeak) à côté de celle-ci (`../splat`, `../lowspeak`), toutes deux sur leur branche `main` ; l’espace de travail en prend des crates par chemin.
- cmake, pour chaque compilation avec dictée (`make run`, `make build`, `make release`), qui compile whisper.cpp. `make check` compile sans.

| Commande | Ce qu’elle fait |
|---|---|
| `make run` | compile l’app de débogage (`target/debug/Dango Debug.app`) et l’ouvre |
| `make build` | compile l’app de version (`target/release/Dango.app`), signée avec un certificat local que la première compilation crée dans le trousseau de session ; incrémente le numéro de build dans `VERSION` |
| `make lint` | mise en forme et clippy ; les avertissements le font échouer |
| `make check` | `lint`, puis tous les tests ; ne nécessite ni trousseau, ni fenêtre, ni réseau |
| `make test` | `check`, puis l’autotest `probe` et l’audit d’accessibilité, de gauche à droite et de droite à gauche, dans la vraie app |
| `make perf` | les budgets de temps, dans une compilation de version ; n’a de sens que sur un Mac par ailleurs inactif |
| `make release` | une répétition de la publication dans `target/release-dry-run/`, signée avec le certificat local ; n’envoie rien nulle part |
| `make release-notarized` | `check`, puis la publication elle-même dans `dist/` : signature Developer ID, notarisation, flux de mises à jour signé avec la clé propre de l’app ; incrémente le numéro de build |

Débogage et version sont des apps distinctes avec leurs propres réglages et données. La compilation, les tests, les autotests, la publication et les règles que suit le code sont décrits dans [docs/development.md](../../../docs/development.md), les décisions qui les sous-tendent dans [docs/plan.md](../../../docs/plan.md). Les notes de version sont [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traduites à côté de chaque README dans `assets/readme/<language>/`.

## Licence

Dango est distribué sous l’une ou l’autre des licences suivantes

- Licence Apache, version 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licence MIT ([LICENSE-MIT](../../../LICENSE-MIT))

à votre choix.

Sauf indication contraire explicite de votre part, toute contribution que vous soumettez intentionnellement pour inclusion dans Dango, au sens de la licence Apache-2.0, sera soumise à la double licence ci-dessus, sans conditions supplémentaires.
