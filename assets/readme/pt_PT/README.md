<p><img src="../../readme-icon.png" alt="Ícone do Dango" width="147" height="147"></p>

# Dango

**O editor nativo para macOS para trabalhar com agentes de programação.**

O Dango reúne o código, os agentes que o alteram e tudo o que os rodeia numa janela por pasta: um editor com servidores de linguagem e previsões durante a escrita, terminais, um navegador, Git, CI, um depurador e threads de IA com Claude, Codex, Antigravity, OpenCode ou um agente integrado na aplicação. Cada agente trabalha na mesma cronologia, pergunta antes de alterar ficheiros ou executar comandos e deixa um ponto de restauro antes de cada prompt, para que possa rever e retroceder o que fez, mensagem a mensagem. Nada do que uma pasta configura é executado antes de confiar nela, e o texto vindo de fora chega a um agente marcado como não fidedigno. Dite no editor e no campo de mensagem; o reconhecimento de voz pode ficar no seu Mac. O Dango está escrito em Rust sobre controlos AppKit verdadeiros, sem Electron, fala 35 idiomas e não tem telemetria.

[![Descarregar Dango.dmg](https://img.shields.io/badge/Descarregar_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Descarregar Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: assinado e notarizado pela Apple, para macOS 14 ou posterior em Apple Silicon e Intel. A ligação ficará disponível a partir da primeira publicação.

**Encontrou um erro?** [Abra uma issue](https://github.com/sventmueller/dango/issues/new) e indique com que agente e em que tipo de projeto aconteceu.

[![Rust, edição 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 ou posterior](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![Universal: Apple Silicon e Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![AppKit através de objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![Terminal: Tapa](https://img.shields.io/badge/terminal-Tapa-F46D01)](https://github.com/sventmueller/tapa) ![35 idiomas da interface](https://img.shields.io/badge/languages-35-4C9A2A) ![Sem telemetria](https://img.shields.io/badge/telemetry-none-4C9A2A) ![Assinado com Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![Licença: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licença)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · [Hrvatski](../hr/README.md) · [Italiano](../it/README.md) · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · [Polski](../pl/README.md) · [Português (Brasil)](../pt_BR/README.md) · **Português (Portugal)** · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · [简体中文](../zh-Hans/README.md) · [繁體中文](../zh-Hant/README.md)

## Índice

- [Funcionalidades em resumo](#funcionalidades-em-resumo)
- [Requisitos](#requisitos)
- [Instalação](#instalação)
- [Primeiros passos](#primeiros-passos)
- [Agentes de IA](#agentes-de-ia)
  - [A cronologia](#a-cronologia)
  - [Permissões e modos](#permissões-e-modos)
  - [Pontos de restauro e worktrees](#pontos-de-restauro-e-worktrees)
- [Editor](#editor)
- [Servidores de linguagem](#servidores-de-linguagem)
- [Testes](#testes)
- [Depurador](#depurador)
- [Terminal](#terminal)
- [Navegador](#navegador)
- [Git](#git)
- [CI](#ci)
- [Contentores](#contentores)
- [Markdown](#markdown)
- [Ditado](#ditado)
- [Confiança](#confiança)
- [Sandbox](#sandbox)
- [Aspeto e acessibilidade](#aspeto-e-acessibilidade)
- [Idiomas](#idiomas)
- [Atalhos de teclado](#atalhos-de-teclado)
- [Privacidade e segurança](#privacidade-e-segurança)
- [Onde ficam os dados](#onde-ficam-os-dados)
- [Limitações conhecidas](#limitações-conhecidas)
- [Compilar a partir do código-fonte](#compilar-a-partir-do-código-fonte)
- [Licença](#licença)

## Funcionalidades em resumo

- **Todos os agentes numa cronologia.** Claude através do seu próprio comando `claude`, Codex, Antigravity e OpenCode através do Agent Client Protocol, outros a partir do registo ACP e um agente integrado na aplicação para Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama e LM Studio. Respostas, chamadas de ferramentas, pedidos de permissão, planos, tarefas pendentes, subagentes e tarefas em segundo plano têm o mesmo aspeto, seja qual for o agente.
- **Rever e retroceder.** Antes de cada prompt, o Dango guarda um ponto de restauro. O que um agente alterou pode ser revisto e retrocedido por mensagem, e uma thread pode trabalhar num worktree Git próprio e entregar o trabalho por fusão ou cherry-pick.
- **Uma janela por pasta.** Ficheiros, terminais, páginas web, threads de IA, diffs e pré-visualizações são separadores do macOS dessa janela. A barra lateral contém Ficheiros, Pesquisa, Agentes, Git, Testes, Depurar, CI e Contentores.
- **Um verdadeiro editor de código.** Cores de sintaxe, vários pontos de inserção, recolha de blocos, `.editorconfig`, servidores de linguagem para 17 linguagens com preenchimento, ir para a definição, referências, alterar o nome, correções rápidas e hierarquias, e previsões durante a escrita do GitHub Copilot ou de um modelo local.
- **Depurador, Git e CI integrados.** Pontos de interrupção e execução passo a passo para Rust, C, C++, Swift, Go, JavaScript, TypeScript e Python; preparar blocos isolados, branches, stashes, conflitos e um grafo de commits; execuções, jobs e logs do GitHub Actions e do GitLab CI.
- **O terminal do Tapa.** Blocos de comando cuja saída copia, guarda ou entrega a um agente, um campo de entrada com propostas do seu histórico e do projeto, pesquisa em todo o histórico, e notificações e progresso dos programas que nele correm.
- **Cuidadoso com as suas pastas.** Uma pasta em que não confia não inicia nada: nem processos Git, nem servidores de linguagem, nem hooks, nem agentes. Os comandos Git destrutivos nunca são permitidos para sempre, e os commits são verificados à procura de segredos.
- **Ditado.** Fale no editor e no campo de mensagem, online através da OpenAI ou da Groq ou localmente com Whisper e Parakeet.
- **Nativo de raiz.** Janelas, separadores, menus e barras de ferramentas AppKit, TextKit 2 para o editor, Metal para o terminal, WebKit apenas no separador do navegador. O modo escuro, Aumentar contraste e o VoiceOver funcionam como em qualquer aplicação do Mac.
- **No seu idioma.** A interface existe em 35 idiomas: alemão, árabe, catalão, checo, chinês (simplificado), chinês (tradicional), coreano, croata, dinamarquês, eslovaco, esloveno, espanhol, finlandês, francês, grego, hebraico, hindi, húngaro, indonésio, inglês, italiano, japonês, malaio, neerlandês, norueguês, polaco, português (Brasil), português (Portugal), romeno, russo, sueco, tailandês, turco, ucraniano e vietnamita.

## Requisitos

- macOS 14 Sonoma ou posterior, em Apple Silicon ou Intel. O Dango é desenvolvido e testado no macOS 27.
- Para as threads de IA, o agente que pretende utilizar; consulte [Agentes de IA](#agentes-de-ia).
- Para os servidores de linguagem e os adaptadores de depuração, que o Dango instala quando o pede: Go para gopls e `dlv`; Node.js para os servidores de TypeScript, Python, JSON, YAML, CSS, HTML e shell e para o depurador de JavaScript; as Xcode Command Line Tools para clangd, sourcekit-lsp e `lldb-dap`. O rust-analyzer é transferido a partir das suas versões no GitHub.
- Para as previsões durante a escrita, um plano do GitHub Copilot (o gratuito é suficiente) ou um modelo fill-in-the-middle no Ollama ou no LM Studio.

## Instalação

**A partir da imagem de disco.** Cada versão é uma imagem de disco, `Dango-<version>.dmg`, com uma aplicação universal para Apple Silicon e Intel, assinada com um Developer ID e notarizada pela Apple. Abra-a e arraste o Dango para a pasta Aplicações. Um Cask do Homebrew instala a mesma imagem. Ainda não foi publicada nenhuma versão.

**Atualizações.** O Dango instalado a partir da imagem de disco mantém-se atualizado com o [Sparkle](https://sparkle-project.org). No segundo arranque, pergunta se pode procurar atualizações por si; até concordar, não faz nenhum pedido. **Dango › Procurar atualizações…** procura de imediato. As atualizações estão assinadas com a chave própria da aplicação, que o Sparkle verifica antes de instalar seja o que for. Uma versão compilada por si não tem atualizador.

**A partir do código-fonte.** Consulte [Compilar a partir do código-fonte](#compilar-a-partir-do-código-fonte) e depois mova `target/release/Dango.app` para `/Applications`.

**O comando de terminal.** **Dango › Instalar o comando “dango”…** grava um pequeno script em `~/.local/bin/dango`, sem privilégios de administrador; `~/.local/bin` tem de estar no seu `PATH`. Depois, `dango .` abre a pasta atual e `dango file.rs` um ficheiro. O Dango nunca substitui aí um ficheiro que não tenha gravado ele próprio.

## Primeiros passos

1. **Abra uma pasta** com ⌘O ou `dango <folder>`. Da primeira vez, o Dango pergunta se confia nela (consulte [Confiança](#confiança)).
2. **A barra lateral** alterna entre Ficheiros (⌘1), Pesquisa (⌘2), Agentes (⌘3), Git (⌘4), Testes (⌘5), Depurar (⌘6), CI (⌘7) e Contentores (⌘8); um clique com o botão direito oculta os que não utiliza, e ⌃⌘S recolhe a barra lateral.
3. **Cada documento é um separador** da janela da pasta, os separadores do próprio macOS. O “+” no fim da barra de separadores e ⌘T oferecem um ficheiro novo, um terminal, o navegador e cada agente instalado, também num worktree Git próprio; mantendo o “+” premido, aparece o mesmo como menu.
4. **Abrir rapidamente** com ⌘P encontra ficheiros, e com `#` os símbolos do projeto; ⇧⌘F pesquisa e substitui no projeto.
5. **Fale com um agente** num separador de IA. ⌘Esc leva o foco do teclado para o campo de mensagem e de volta.
6. **Definições** com ⌘, contém os separadores Geral, Terminal, CI, Agentes, Software Factory, Agente integrado, Servidor de linguagem, Ditado e Previsão.

A barra de ferramentas contém recuar e avançar pelos locais que editou (⌃- e ⌃⇧-), o branch, Abrir rapidamente e a vista geral de Atividade, que lista tudo o que está em execução. **Abrir ao lado** no menu de contexto de um ficheiro mostra um segundo documento ao lado do primeiro. Após um reinício ou uma falha, cada janela regressa com os seus separadores, e os editores com texto não guardado regressam com esse texto. Imagens, áudio, vídeo, tipos de letra e documentos do Office abrem num separador através da Vista rápida, os PDF através do PDFKit.

## Agentes de IA

| Agente | Como é executado | Do que precisa |
|---|---|---|
| Claude | o comando `claude`, com o qual o Dango comunica diretamente através do protocolo stream-json | `claude` instalado e com sessão iniciada; o Dango procura no `PATH` da shell de início de sessão, em `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` e `/usr/local/bin`. A sessão só é iniciada no próprio `claude` |
| Codex | `codex-acp` através do Agent Client Protocol (ACP) | Node.js e npm; o Dango instala o pacote de versão fixada quando o pede |
| Antigravity | `agy acp` | o seu próprio `agy` no `PATH`, ou a transferência de versão fixada, instalada quando o pede |
| OpenCode | `opencode acp` | o seu próprio `opencode` no `PATH`, ou a transferência de versão fixada, instalada quando o pede |
| Outros agentes ACP | a partir do registo ACP, obtido apenas quando o pede | o que a entrada do registo indicar; as entradas sem checksum só são instaladas depois de confirmar |
| Agente integrado | integrado na aplicação, com ferramentas para ficheiros, pesquisa, comandos, diagnósticos e pedidos web | uma chave de API para Anthropic, OpenAI, Google Gemini ou OpenRouter, guardada no Porta-chaves; Ollama ou LM Studio neste Mac sem chave; qualquer servidor que fale a API da OpenAI ou da Anthropic |

**Definições › Agentes** mostra que agentes este Mac tem, instala os outros e as respetivas atualizações e abre o registo ACP. Os agentes que o Dango instala estão fixados em versões testadas e são verificados com os checksums publicados (npm integrity ou SHA-256); o Dango não executa pacotes `npx` sem versão.

### A cronologia

Cada agente trabalha num separador de IA com a mesma cronologia: respostas, chamadas de ferramentas com a respetiva saída, pedidos de permissão, perguntas, planos para aprovar e comentar (⌥⌘K), tarefas pendentes, subagentes e tarefas em segundo plano. Dois botões no cabeçalho do separador listam os subagentes e processos em segundo plano em execução. Mencione um ficheiro, `@terminal` ou `@browser` para passar o conteúdo ao agente. Um ponto verde no separador indica uma sessão a trabalhar, um vermelho uma sessão à sua espera, e uma notificação avisa quando termina ou espera enquanto está a olhar para outro sítio. Com o Claude, a linha de estado mostra o contexto e a utilização.

### Permissões e modos

As threads novas começam no modo “Manual”: o agente pergunta antes de alterar ficheiros ou executar comandos. Para comandos Git destrutivos (`push --force`, `reset --hard`, `clean -fd`), o Dango não oferece “Permitir sempre”. O agente integrado guarda as suas regras de “Permitir sempre” por projeto, lista-as em **Definições › Agente integrado** e pergunta sempre nos comandos Git destrutivos, seja o que for que esteja guardado; o Claude e os agentes ACP guardam essas regras na sua própria configuração. Com “Permitir tudo” e sem sandbox, a thread mostra um aviso permanente.

### Pontos de restauro e worktrees

Antes de cada prompt, o Dango guarda um ponto de restauro, como commit em `refs/ide-checkpoints/` no repositório do projeto, ou numa pasta própria para um projeto sem Git; um push ou fetch normal nunca os leva. O que um agente alterou pode ser revisto e retrocedido por mensagem. Uma thread pode trabalhar num worktree Git próprio: o painel Git mostra o que alterou e incorpora o trabalho por fusão ou cherry-pick.

## Editor

- **Cores de sintaxe, números de linha e marcas de alteração** em relação ao último commit, procurar e substituir, Abrir rapidamente (⌘P) e pesquisa no projeto (⇧⌘F).
- **A indentação** segue o ficheiro e o `.editorconfig`: tabulações ou espaços, indentação automática, pares de parênteses.
- **Vários pontos de inserção:** ⌥-clique, ⌥-arrastar para uma coluna, ⌃⇧↑ e ⌃⇧↓, ⌘D para a ocorrência seguinte.
- **Recolha de blocos** (⌥⌘← e ⌥⌘→, com ⇧ para tudo), ativar ou desativar comentário (⌘/), ir para a linha (⌃G), espaços visíveis e quebra de linha (⌥Z).
- **Os ficheiros mantêm a codificação**, a marca de ordem de bytes e as quebras de linha. Um ficheiro alterado no disco é recarregado quando o editor não tem texto por guardar; caso contrário, uma barra oferece as diferenças e uma fusão.
- **Previsões durante a escrita.** Enquanto escreve, aparece uma sugestão a cinzento, e Tab aceita-a. Vem do GitHub Copilot ou de um modelo local no Ollama ou no LM Studio, conforme a escolha em **Definições › Previsão**; um modelo local mantém o seu código neste Mac.
- **Minimapa e Deslocamento fixo** (menu Editor): uma imagem estreita do ficheiro inteiro ao lado do texto, e as linhas de cabeçalho dos blocos em que se encontra a linha do topo, fixadas por cima dela.
- **Diagnósticos no fim da linha** (menu Visualização): a mensagem mais grave de cada linha após um segundo de pausa, com um símbolo para a sua gravidade.
- **Pré-visualização em direto de páginas estáticas:** Editor › Abrir num separador do navegador serve as páginas de uma pasta de confiança a partir do próprio Dango e recarrega o separador quando um ficheiro é guardado.
- **Prettier** formata JavaScript, TypeScript, CSS, HTML, Markdown e mais onde o projeto o configura, com o próprio `node_modules/.bin/prettier` do projeto; a definição `formatter` escolhe o formatador por linguagem.
- **Modo Vim** (Editor › Modo Vim): modos Normal, Insert, Visual e Replace, contagens, operadores com movimentos e objetos de texto, `.`, registos, marcas e macros. `u` e ⌃R utilizam o desfazer do editor, e cada passagem pelo modo Insert conta como um único passo. `"+` e `"*` são o clipboard; a definição `vimClipboard` faz do registo sem nome também o clipboard. `/`, `?`, `n`, `N`, `*`, `:s` e `:%s` procuram com as expressões regulares de Rust, não com os padrões do Vim: sem `\v`, e `\b` em vez de `\<` e `\>`. Os comandos Ex são `:w`, `:q`, `:wq`, `:x`, `:noh` e `:<line>`. No modo Normal, ⌃G fica com o Vim em vez de ir para a linha.
- **Sincronização das definições** através de uma pasta no iCloud Drive (**Definições › Geral**): as suas definições e atalhos seguem consigo para os seus outros Mac; caminhos, endereços locais e consentimentos ficam em cada um.

## Servidores de linguagem

Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML, Markdown e Lua, além de servidores próprios em **Definições › Servidor de linguagem**. O Dango instala um servidor quando o pede, na sua própria pasta de ferramentas, e utiliza o `tsc` de um projeto TypeScript quando este pode servir de servidor de linguagem.

O que oferecem: diagnósticos (⌘' e ⇧⌘' passam de um para outro), informação ao passar o ponteiro (⌃⌘I), preenchimento (⌥Esc), ajuda de assinaturas, ir para a definição (⌃⌘J), para a definição de tipo e para a implementação, referências (⌃⇧⌘F), hierarquias de chamadas e de tipos (⌃⇧⌘H), alterar o nome (⌃⌘E), formatar (⌥⇧F), cores semânticas, recolha de blocos e a estrutura (⇧⌘O); tipos e nomes de parâmetros dentro da linha; correções rápidas e refatorações atrás de uma lâmpada (⌥⌘A); os símbolos do projeto com ⌥⌘O.

Além do servidor próprio de um ficheiro, o ESLint verifica o JavaScript e o TypeScript dos projetos que o utilizam, com as suas correções rápidas, e as abreviaturas do Emmet aparecem no preenchimento para HTML, CSS e JSX. O Dango instala ambos quando o pede; **Definições › Geral** corrige os problemas do ESLint ao guardar, se assim o desejar.

## Testes

O painel Testes lista os testes de uma pasta por pacote e ficheiro com o respetivo estado: `cargo test` ou cargo-nextest, Go, pytest, Vitest e Jest. Um símbolo junto a cada teste na margem executa-o, ⌥-clique depura-o, e uma asserção falhada marca a respetiva linha. O menu Depurar executa o teste no ponto de inserção (⌃⌥⌘U), os últimos testes novamente (⌃⌥⌘G) ou todos, e depura o teste no ponto de inserção. Os testes só são executados em pastas de confiança, uma vez que executam o código do projeto; o `cargo test` lista os respetivos testes quando o pedir, pois listá-los compila todos os programas de teste.

## Depurador

Pontos de interrupção, também com condições e como logpoints, execução passo a passo, threads com as respetivas pilhas de chamadas, variáveis e expressões vigiadas no inspetor (⌥⌘I), valores ao lado do código enquanto o programa está em pausa, e uma consola de depuração que avalia no quadro em pausa.

| Linguagem | Adaptador de depuração |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` das Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | o depurador de JavaScript, sobre Node.js |
| Python | debugpy |

O `.vscode/launch.json` de um projeto é lido. O menu Depurar utiliza os atalhos do Xcode: ⌘\\ define um ponto de interrupção, ⌘Y ativa ou desativa todos, ⌃⌘Y continua, F6, F7 e F8 passam por cima, entram e saem, ⌘. para.

## Terminal

Os terminais abrem como separadores (**Ficheiro › Novo terminal**, ⌃\`) com a shell de início de sessão e o respetivo ambiente. O terminal é o do [Tapa](https://github.com/sventmueller/tapa), com núcleo VT próprio, desenhado com Metal: xterm e VT520 com o protocolo de teclado do Kitty, ligações OSC 8, saída sincronizada, truecolor e comunicação do rato, no tamanho de texto do editor e nas cores claras e escuras do Tapa. A integração com a shell para zsh, bash e fish configura-se por si, sem tocar nos seus ficheiros de configuração, e segue consigo quando usa `ssh` para outro anfitrião. Após um reinício, um terminal regressa como uma shell nova na pasta em que estava, apenas em pastas de confiança; o que mostrava não regressa.

- **Blocos de comando.** Cada comando e a respetiva saída formam um bloco. ⌘↑ e ⌘↓ saltam entre os prompts. **Selecionar bloco** (⇧⌘A), **Copiar saída** (⌥⌘C), **Guardar saída…** (⌥⌘S) e **Recolher/expandir bloco** (⇧⌘↩) atuam sobre o bloco atual; **Adicionar/remover marcador** (⇧⌘M) assinala uma linha, e ⇧⌘↑ e ⇧⌘↓ saltam entre marcadores. Com **Marcar comandos como blocos** ativado, uma barra na margem mostra cada bloco, os comandos que falharam ficam assinalados, o comando em execução mantém-se no topo enquanto a saída se desloca, e há botões sobre o bloco que está sob o ponteiro que mostram o estado, a duração e as mesmas ações.
- **Enviar bloco para o agente** (menu Edição e menu de contexto do terminal) abre um novo separador de IA com a linha de comandos e a saída do bloco atual anexadas como Conteúdo externo (consulte [Confiança](#confiança)), e um rascunho que pergunta pela causa quando o comando falhou, como faz o log de um job de CI falhado. Não tem atalho até atribuir um a `terminal.blockToAgent` no `keybindings.json`.
- **O campo de entrada.** **Visualização › Mostrar/ocultar campo de entrada** (⌘I) transforma a linha de comandos num campo de texto no fundo do terminal, com a pasta e o branch do Git por cima; os novos terminais seguem a sua última escolha. Return envia todo o campo para a shell. Precisa da integração com a shell e afasta-se perante programas de ecrã inteiro como vim, htop ou claude.
- **As propostas** aparecem a cinzento logo a seguir ao que escreve, no campo de entrada e sem ele: do seu histórico, com a pasta, o branch e o código de saída com que cada comando foi executado, de especificações para mais de 700 ferramentas de linha de comandos e do projeto, como branches do git, contentores do docker, scripts do `package.json` e alvos do make, do just e do cargo. → aceita uma proposta; Tab completa e, com vários candidatos, abre um menu que a escrita filtra. Antes de escrever, o campo pode propor o comando que costuma vir a seguir. Os comandos são coloridos por partes, e um que a sua shell não consegue executar recebe um sublinhado vermelho tracejado. As propostas nunca utilizam a rede e só iniciam programas do `PATH` da shell de início de sessão.
- **Procurar** (⌘F, ⌘G e ⇧⌘G enquanto um terminal tem o foco) pesquisa todo o histórico, com expressões regulares, maiúsculas e minúsculas e palavras inteiras, e conta os resultados; a barra de deslocamento assinala os resultados, os prompts e os comandos que falharam.
- **Selecionar e copiar.** Um duplo clique seleciona por inteiro um URL, um caminho, um hash ou um endereço IP, e arrastar com ⌥ seleciona um retângulo. **Selecionar com o teclado** (⇧⌘C) move uma seleção com as teclas do Vim, **Copiar com formatação** (⇧⌥⌘C) mantém cores e estilos. Um programa só pode copiar por OSC 52 enquanto o respetivo terminal tem o foco do teclado, e o terminal indica-o; nenhum programa lê a área de transferência.
- **Colar.** Um texto que executaria um comando na quebra de linha, ou que contém carateres de controlo, aparece primeiro numa caixa de diálogo, a menos que o programa o leia como colagem delimitada (bracketed paste). Colar uma imagem entrega-a a um agente de programação em execução no terminal, que a lê ele próprio da área de transferência, ou cola o caminho de uma cópia guardada nas caches; por `ssh`, o terminal diz porque não o faz. Os ficheiros largados num terminal, imagens incluídas, chegam como caminhos entre aspas; uma imagem largada a partir de outro sítio, como uma página web, segue o caminho de uma imagem colada.
- **Ligações.** ⌘-clique abre um endereço web no separador do navegador, uma ligação `mailto:` no Mail, e um nome de ficheiro, também com linha e coluna, no editor do Dango; as ligações são encontradas mesmo sem OSC 8. As ligações de outros tipos não abrem.
- **Servidores de desenvolvimento.** Quando um programa no terminal inicia um servidor de desenvolvimento, uma barra por cima do terminal indica o respetivo endereço, e **Abrir num separador do navegador** abre-o aí.
- **Notificações e progresso.** A campainha e uma notificação que um programa peça (OSC 9 e OSC 777) colocam o ponto vermelho no separador do terminal enquanto não o vê; enquanto outra aplicação está à frente, a notificação vai também para o macOS, e clicar nela traz o terminal de volta. O progresso que um programa comunica (OSC 9;4) aparece como um anel no respetivo separador e como uma barra no ícone na Dock.
- **Entrada de teclado segura** (**Dango › Entrada de teclado segura**) impede que outras aplicações leiam o que escreve num terminal; também se ativa por si enquanto um programa pede uma palavra-passe.
- **Teclas.** ⌘K limpa o terminal (**Edição › Limpar terminal**); ⌃C e Esc vão para o programa que nele corre. Os comandos do terminal estão nos menus Edição e Visualização, e os respetivos atalhos só atuam enquanto um terminal tem o foco do teclado, pelo que ⌘I, ⌘↑ ou ⌘F mantêm o seu significado no editor. O menu de contexto também abre um **Novo terminal aqui** e fecha o terminal com **Fechar área**.

**Definições › Terminal** ativa e desativa o campo de entrada para os novos terminais, as respetivas teclas Vim, as propostas, as cores dos comandos, o sublinhado dos desconhecidos, os blocos de comando, copiar a seleção imediatamente, mostrar antes as colagens de várias linhas e ⌥ como tecla meta.

## Navegador

Um separador de navegador com campo de endereço (⌘L), recuar e avançar (⌘[ e ⌘]), recarregar (⌘R), pesquisa, transferências que perguntam primeiro e o inspetor web. `@browser` passa a página a um agente, marcada como texto vindo de fora.

## Git

O painel Git prepara e descarta ficheiros ou blocos isolados, faz commits, faz commit e push ou sincroniza, muda e cria branches, guarda stashes, resolve conflitos de fusão e desenha um grafo dos commits recentes. O blame mostra quem alterou por último a linha sob o cursor, ou uma coluna para o ficheiro inteiro. Um botão escreve a mensagem de commit com IA; o diff só sai depois de o gitleaks não ter encontrado nenhum segredo, e cada commit é também verificado à procura de segredos.

## CI

GitHub Actions e GitLab CI, também GitHub Enterprise Server e GitLab autoalojado:

- execuções, jobs e logs do branch atual;
- voltar a executar jobs falhados, um único job ou, no GitHub, uma execução inteira; iniciar jobs manuais ou aprovar implementações em espera; iniciar um pipeline novo ou um workflow que pode ser iniciado manualmente; cancelar;
- os pull requests ou merge requests abertos e as issues, que abrem no separador do navegador;
- o log de um job falhado entregue a um agente, marcado como texto vindo de fora.

O token vem do Porta-chaves, do `gh` ou do `glab`, do ambiente da shell de início de sessão, ou é um que introduz em **Definições › CI**.

## Contentores

O painel Contentores mostra os projetos Compose do Docker com os respetivos serviços, contentores isolados, imagens e volumes, e acompanha os eventos do Docker à medida que acontecem. Um ficheiro Compose da pasta aparece como projeto mesmo antes de existir qualquer um dos seus contentores. Os contentores iniciam, param e reiniciam; os respetivos logs e uma shell abrem em separadores do terminal, e uma porta publicada abre no separador do navegador. Apagar um contentor, uma imagem ou um volume, e limpar dados não utilizados, pedem confirmação antes. Os comandos up, down e pull do Compose só são executados em pastas de confiança, uma vez que o Compose cria e executa o que o projeto descreve. O menu do painel escolhe o contexto do Docker da janela sem alterar o que as suas shells utilizam; os mesmos comandos estão no menu Contentores. O painel precisa da linha de comandos `docker`, Docker 24.0.6 ou posterior.

## Markdown

- **Pré-visualização.** Um ficheiro Markdown abre com a pré-visualização ao lado; **Visualização › Mostrar/ocultar pré-visualização ao lado** (⇧⌘V) fecha-a e abre-a, e **Definições › Geral** desativa a pré-visualização automática. A pré-visualização desenha fórmulas, diagramas Mermaid, Graphviz e PlantUML e as imagens da pasta; as imagens da web só são carregadas numa pasta de confiança ou quando as pede na pré-visualização.
- **Menu Formatação:** negrito (⌘B), itálico (⌘I), rasurado (⇧⌘X), código (⌥⇧⌘C), ligação (⇧⌘K), títulos 1–3 (⌥⌘1–⌥⌘3), listas com marcas e numeradas, tarefas, citações e blocos de código. Cada comando alterna, na seleção ou nas linhas, e é um único passo de anulação.
- **Colar.** Texto do Word ou de uma página web chega como Markdown, e as imagens como ficheiros ao lado do documento; ⌥⇧⌘V cola texto simples.
- **Exportar.** **Ficheiro › Exportar** grava um PDF, um único ficheiro HTML com as imagens lá dentro ou um documento do Word. As imagens da web só são incluídas a partir de uma pasta de confiança.
- **Abrir no Splat.** Se o [Splat](https://github.com/sventmueller/splat) estiver instalado, **Ficheiro › Abrir no Splat** e os menus de contexto da lista de ficheiros e dos separadores abrem lá o documento, depois de o guardar.

## Ditado

Dite no campo de mensagem de um separador de IA ou no editor: com o botão do microfone aí presente, **Edição › Iniciar ditado** (⌃⌥D) ou a tecla fn (mantenha premida para gravar, toque duas vezes para continuar a gravar). O reconhecimento de voz é o do [Lowspeak](https://github.com/sventmueller/lowspeak); o Lowspeak propriamente dito não precisa de estar instalado.

- **Definições › Ditado** define o modo e o modelo (online com OpenAI ou Groq, localmente com Whisper ou Parakeet), o idioma, o pós-processamento, o microfone e o seu próprio vocabulário.
- **Os modelos locais** são transferidos do Hugging Face quando escolhe um, com o progresso e a opção de cancelar. Ficam nas pastas que o Lowspeak utiliza, pelo que cada aplicação utiliza o que a outra transferiu. Um modelo local mantém a gravação neste Mac.
- **A transcrição online** precisa de uma chave da OpenAI ou da Groq, que o Dango guarda num elemento próprio do Porta-chaves.
- **A partir do Lowspeak.** Se o Lowspeak estiver configurado de forma diferente da predefinida, o separador Ditado oferece uma vez adotar as definições dele, e as chaves se o pedir; o macOS pergunta antes de o Dango ler uma delas (escolha “Permitir”, não “Permitir sempre”).
- **Permissões.** O macOS pede acesso ao microfone no primeiro ditado. A tecla fn funciona enquanto o Dango é a aplicação ativa e o Lowspeak não está em execução; precisa da permissão de acessibilidade, que o Dango só pede depois do primeiro ditado, nunca no arranque.

## Confiança

Uma pasta pode fazer com que sejam executados programas assim que algo é iniciado nela: hooks do Git e definições do repositório como `core.fsmonitor`, servidores de linguagem de `node_modules`, hooks de agentes, `.mcp.json`. Da primeira vez que abre uma pasta, o Dango pergunta se confia nela.

- **De confiança:** Git, servidores de linguagem, depuradores, agentes e CI são executados para esta pasta e tudo o que contém.
- **Restrita:** pode ler e editar; as marcas de alteração no editor vêm da leitura direta de `.git`. Nada do que a pasta configura é iniciado. Os painéis Git, CI e IA indicam-no e oferecem confiar na pasta.

Um texto que não vem de si pode trazer instruções destinadas a conduzir um agente (injeção de prompts). A saída de um terminal (`@terminal`, ou um bloco de comando entregue a um agente), uma página web (`@browser`), um log de CI entregue a um agente, um ficheiro fora do projeto e um texto mais longo colado de uma página web seguem, por isso, marcados: a etiqueta diz “Conteúdo externo”, e o agente recebe o texto com o aviso de que são dados não fidedignos cujas instruções não deve seguir. As páginas web do agente integrado chegam com o mesmo aviso. Um texto colado conta como vindo de uma página web quando o navegador o indica na área de transferência, como fazem o Safari, o Chrome e o Firefox; o texto vindo de outros sítios, e uma frase curta de poucas palavras, continua a ser seu.

## Sandbox

O próprio Dango é executado sem App Sandbox, porque todos os programas que inicia a herdariam: terminais, Git, servidores de linguagem e agentes não conseguiriam funcionar. Em vez disso, cada thread de IA tem um interruptor de sandbox, desativado por predefinição. O que limita depende do agente, e a dica do interruptor explica-o:

| Agente | Com a sandbox ativada |
|---|---|
| Claude | os comandos de shell são executados na sandbox própria do Claude; as ferramentas dele para ficheiros e web, não |
| Codex | os comandos e as alterações só escrevem no projeto, sem rede |
| Agente integrado | ficheiros, pedidos web e comandos ficam dentro do projeto e das pastas permitidas, sem rede (`sandbox-exec`) |
| Antigravity, OpenCode | nenhuma sandbox que o Dango possa ativar |

## Aspeto e acessibilidade

- **O modo escuro** segue o sistema; **Aumentar contraste** e **Reduzir movimento** são respeitados.
- **O VoiceOver** dá nome à barra lateral, aos separadores, aos cartões da cronologia de IA, às linhas do Git e da CI e ao depurador, lê as linhas do terminal e anuncia a nova saída. Os pontos de interrupção nunca se distinguem apenas pela cor.
- **Teclado.** Cada comando tem um atalho, e todos podem ser alterados em `keybindings.json`; ⌥⌘\` passa para o painel seguinte.
- **Da direita para a esquerda.** Em árabe e hebraico, a janela é espelhada, enquanto as vistas de código e a grelha do terminal se mantêm da esquerda para a direita.

O que o Dango acrescenta ao AppKit, área a área, está em [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Idiomas

A interface está traduzida em 35 idiomas: alemão, árabe, catalão, checo, chinês (simplificado), chinês (tradicional), coreano, croata, dinamarquês, eslovaco, esloveno, espanhol, finlandês, francês, grego, hebraico, hindi, húngaro, indonésio, inglês, italiano, japonês, malaio, neerlandês, norueguês, polaco, português (Brasil), português (Portugal), romeno, russo, sueco, tailandês, turco, ucraniano e vietnamita.

O Dango segue a ordem de idiomas em **Definições do Sistema › Geral › Idioma e região**; também pode escolher ali um idioma só para o Dango, em **Aplicações**. Sem tradução para o seu idioma, usa o inglês. Os termos seguem um glossário, [docs/glossary.md](../../../docs/glossary.md), e as traduções da Apple, para que o Dango se leia como o resto do seu Mac.

## Atalhos de teclado

| Ação | Atalho |
|---|---|
| Abrir pasta, nova janela, fechar janela | ⌘O, ⇧⌘N, ⇧⌘W |
| Novo ficheiro, guardar, guardar como | ⌘N, ⌘S, ⇧⌘S |
| Novo separador, fechar separador, separador seguinte e anterior | ⌘T, ⌘W, ⇧⌘] e ⇧⌘[ |
| Abrir rapidamente, pesquisar no projeto | ⌘P, ⇧⌘F |
| Ficheiros, Pesquisa, Agentes, Git, Testes, Depurar, CI, Contentores | ⌘1 a ⌘8 |
| Mostrar ou ocultar a barra lateral, painel seguinte | ⌃⌘S, ⌥⌘\` |
| Recuar e avançar pelos locais editados | ⌃- e ⌃⇧- |
| Ativar ou desativar comentário, ir para a linha | ⌘/, ⌃G |
| Ocorrência seguinte, ponto de inserção acima e abaixo | ⌘D, ⌃⇧↑ e ⌃⇧↓ |
| Recolher, expandir, recolher tudo, expandir tudo | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Quebra de linha, inspetor | ⌥Z, ⌥⌘I |
| Preenchimento, informação, ir para a definição | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referências, hierarquia de chamadas, alterar o nome | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Símbolos no ficheiro, no projeto | ⇧⌘O, ⌥⌘O |
| Correção rápida, formatar | ⌥⌘A, ⌥⇧F |
| Problema seguinte e anterior | ⌘' e ⇧⌘' |
| Pré-visualização de Markdown, colar como texto simples | ⇧⌘V, ⌥⇧⌘V |
| Negrito, itálico, rasurado, código, ligação | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Título 1 a 3 | ⌥⌘1 a ⌥⌘3 |
| Novo terminal, limpar o terminal | ⌃\`, ⌘K |
| Num terminal: procurar, procurar seguinte, procurar anterior | ⌘F, ⌘G, ⇧⌘G |
| Num terminal: prompt anterior e seguinte, marcador anterior e seguinte, adicionar ou remover um marcador | ⌘↑ e ⌘↓, ⇧⌘↑ e ⇧⌘↓, ⇧⌘M |
| Num terminal: selecionar bloco, copiar saída, guardar saída, recolher bloco | ⇧⌘A, ⌥⌘C, ⌥⌘S, ⇧⌘↩ |
| Num terminal: campo de entrada, selecionar com o teclado, copiar com formatação | ⌘I, ⇧⌘C, ⇧⌥⌘C |
| Foco no campo de mensagem do agente, comentar um plano | ⌘Esc, ⌥⌘K |
| Campo de endereço, recarregar, recuar, avançar (navegador) | ⌘L, ⌘R, ⌘[, ⌘] |
| Iniciar ou parar o ditado | ⌃⌥D |
| Ponto de interrupção, ativar ou desativar todos | ⌘\\, ⌘Y |
| Executar o teste no ponto de inserção, executar novamente os últimos testes | ⌃⌥⌘U, ⌃⌥⌘G |
| Continuar, passar por cima, entrar, sair, parar | ⌃⌘Y, F6, F7, F8, ⌘. |
| Definições | ⌘, |

O `keybindings.json` na pasta de dados da aplicação (consulte [Onde ficam os dados](#onde-ficam-os-dados)) altera qualquer um deles.

## Privacidade e segurança

- **Sem telemetria,** sem análises, sem relatórios de falhas enviados para lado nenhum. Os erros vão para o registo do sistema (Consola, com o nome da aplicação).
- **A rede só para o que inicia:** os agentes que utiliza, cada um com o seu próprio serviço; o registo ACP e as transferências de agentes, servidores de linguagem, adaptadores de depuração e modelos de ditado quando os pede; os anfitriões de CI em que inicia sessão; as previsões do Copilot, se o escolheu; os fornecedores do agente integrado; a transcrição online, se a escolheu; as páginas que abre no separador do navegador; e a procura de atualizações depois de a ter permitido.
- **O código só sai do Mac** onde escolheu um serviço para isso: um agente, as previsões do Copilot, uma mensagem de commit escrita com IA. O diff para uma mensagem de commit só sai depois de o gitleaks não ter encontrado nenhum segredo.
- **As chaves e os tokens** ficam no Porta-chaves do macOS, nunca num ficheiro de definições. As chaves de outras aplicações, como as do Lowspeak, só são lidas se concordar, e só uma vez.
- **Transferências fixadas.** Os agentes, servidores de linguagem e adaptadores de depuração que o Dango instala estão fixados em versões testadas e são verificados com os respetivos checksums.
- **Assinatura de código** com o Hardened Runtime; as atualizações só são instaladas com a assinatura própria da aplicação.

## Onde ficam os dados

`<id>` é o identificador do pacote, `local.agentide` para a versão publicada (`local.agentide.debug` para as compilações de depuração). Não contém o nome apresentado, pelo que uma mudança de nome mantém todos os dados.

| Local | O quê |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`, `terminal-history.sqlite` (o histórico de comandos dos terminais); `threads/` (a lista do painel de IA), `agent-threads/` (conversas do agente integrado), `layouts/` (separadores por projeto), `recovery/` (editores por guardar após uma falha), `checkpoints/` (para pastas sem Git), `worktrees/`, `agents/` e `tools/` (agentes, servidores de linguagem e adaptadores de depuração instalados), `task-output/` |
| `~/Library/Caches/<id>/` | imagens coladas, scripts de integração com a shell, o registo ACP; pode ser apagado |
| `~/Library/Logs/<id>/dictation/` | os ficheiros de registo do ditado |
| `~/Library/Application Support/Lowspeak` e `FluidAudio` | modelos de ditado locais, partilhados com o Lowspeak |
| `~/Library/WebKit/<id>/` | cookies e dados de sites do separador do navegador |
| `<project>/.dango/settings.json` | as definições próprias do projeto |
| o repositório Git do projeto | pontos de restauro, como commits em `refs/ide-checkpoints/`; um push ou fetch normal nunca os leva |
| Porta-chaves, serviço `<id>` | chaves de API do agente integrado e do ditado, tokens de CI |
| `~/.local/bin/dango` | o comando de terminal, se o instalou |

O Claude guarda as suas próprias sessões em `~/.claude/projects`; o Dango lê-as para retomar uma sessão, mas não as altera.

## Limitações conhecidas

- **Sem App Sandbox** para a própria aplicação, porque terminais, Git, servidores de linguagem e agentes a herdariam; uma pasta tem de ser de confiança antes de ser executado o que quer que configure. O Antigravity e o OpenCode não têm uma sandbox que o Dango possa ativar.
- **Os números de utilização e a repartição do contexto do Claude** dependem de partes do protocolo marcadas como experimentais, e ficam ocultos quando estas faltam.
- **Retroceder a conversa** só funciona com o Claude; com os outros agentes, só o código é retrocedido.
- **O início de sessão na CI através do navegador** está desativado; utilize `gh`, `glab`, um token ou o ambiente.
- **Sem imagens dentro do terminal:** os gráficos do Kitty, o Sixel e as imagens do iTerm2 não aparecem.
- **O Gemini CLI não é oferecido:** a Google terminou o início de sessão com uma conta pessoal e remete para o Antigravity. As threads Gemini anteriores continuam legíveis.

## Compilar a partir do código-fonte

Precisa de:

- macOS 14 ou posterior com as Xcode Command Line Tools (`xcode-select --install`); o Xcode propriamente dito não é necessário.
- Rust tal como indicado em `rust-toolchain.toml`. O `rust` do Homebrew compila para o processador deste Mac; a compilação universal de uma versão precisa do rustup com os destinos `aarch64-apple-darwin` e `x86_64-apple-darwin`.
- Cópias de [splat](https://github.com/sventmueller/splat), [lowspeak](https://github.com/sventmueller/lowspeak) e [tapa](https://github.com/sventmueller/tapa) ao lado desta (`../splat`, `../lowspeak`, `../tapa`), cada uma no respetivo branch `main`; o espaço de trabalho obtém crates delas por caminho, e o terminal vem do Tapa.
- cmake, para cada compilação com ditado (`make run`, `make build`, `make release`), que compila o whisper.cpp. `make check` compila sem ele.

| Comando | O que faz |
|---|---|
| `make run` | compila a aplicação de depuração (`target/debug/Dango Debug.app`) e abre-a |
| `make build` | compila a aplicação de versão (`target/release/Dango.app`), assinada com um certificado local que a primeira compilação cria no Porta-chaves de início de sessão; mantém `Cargo.toml` inalterado |
| `make lint` | formatação e clippy; os avisos fazem-no falhar |
| `make check` | `lint` e depois todos os testes; não precisa de Porta-chaves, janela nem rede |
| `make test` | `check` e depois o autoteste `probe` e a auditoria de acessibilidade, da esquerda para a direita e da direita para a esquerda, na aplicação real |
| `make perf` | os orçamentos de tempo, numa compilação de versão; só faz sentido num Mac que de resto esteja inativo |
| `make release` | um ensaio da publicação em `target/release-dry-run/`, assinado com o certificado local; não envia nada para lado nenhum |
| `make release-notarized` | `check` e depois a publicação propriamente dita em `dist/`: assinatura com Developer ID, notarização, o feed de atualizações assinado com a chave própria da aplicação; mantém `Cargo.toml` inalterado |
| `make deploy` | num `main` limpo, com ou sem selo de CI: notariza a próxima versão, faz commit de `Cargo.toml`, carrega os ficheiros da publicação, faz push do código e da etiqueta de versão, com o selo verde de CI se existir, e depois publica a versão ele próprio |

Depuração e versão são aplicações separadas com definições e dados próprios. A compilação, os testes, os autotestes, a publicação e as regras que o código segue estão descritos em [docs/development.md](../../../docs/development.md), e as decisões por detrás em [docs/plan.md](../../../docs/plan.md). As notas da versão são o [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traduzidas ao lado de cada README em `assets/readme/<language>/`.

## Licença

O Dango está licenciado ao abrigo de uma das seguintes licenças

- Licença Apache, Versão 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licença MIT ([LICENSE-MIT](../../../LICENSE-MIT))

à sua escolha.

Salvo indicação explícita em contrário, qualquer contribuição intencionalmente submetida por si para inclusão no Dango, tal como definida na licença Apache-2.0, será licenciada em licença dupla como acima, sem quaisquer termos ou condições adicionais.
