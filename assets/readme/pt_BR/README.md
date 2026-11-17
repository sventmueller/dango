<p><img src="../../readme-icon.png" alt="Ícone do Dango" width="135" height="135"></p>

# Dango

**O editor nativo para macOS para trabalhar com agentes de programação.**

O Dango reúne o código, os agentes que o alteram e tudo ao redor em uma janela por pasta: um editor com servidores de linguagem e previsões enquanto você digita, terminais, um navegador, Git, CI, um depurador e threads de IA com Claude, Codex, Antigravity, OpenCode ou um agente integrado ao app. Cada agente trabalha na mesma linha do tempo, pergunta antes de alterar arquivos ou executar comandos e deixa um ponto de restauração antes de cada prompt, para que você possa revisar e retroceder o que ele fez, mensagem por mensagem. Nada que uma pasta configure é executado antes que você confie nela, e o texto de fora chega a um agente marcado como não confiável. Dite no editor e no campo de mensagem; o reconhecimento de fala pode ficar no seu Mac. O Dango é escrito em Rust sobre controles AppKit de verdade, sem Electron, fala 35 idiomas e não tem telemetria.

[![Baixar Dango.dmg](https://img.shields.io/badge/Baixar_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Baixar Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: assinado e notarizado pela Apple, para macOS 14 ou posterior em Apple Silicon e Intel. O link ficará disponível a partir da primeira publicação.

**Encontrou um bug?** [Abra uma issue](https://github.com/sventmueller/dango/issues/new) e diga com qual agente e em que tipo de projeto ele aconteceu.

[![Rust, edição 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 ou posterior](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon e Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 idiomas de interface](https://img.shields.io/badge/languages-35-4C9A2A)
![Sem telemetria](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Assinado com Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licença: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licença)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
**Português (Brasil)** ·
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

## Sumário

- [Recursos em resumo](#recursos-em-resumo)
- [Requisitos](#requisitos)
- [Instalação](#instalação)
- [Primeiros passos](#primeiros-passos)
- [Agentes de IA](#agentes-de-ia)
  - [A linha do tempo](#a-linha-do-tempo)
  - [Permissões e modos](#permissões-e-modos)
  - [Pontos de restauração e worktrees](#pontos-de-restauração-e-worktrees)
- [Editor](#editor)
- [Servidores de linguagem](#servidores-de-linguagem)
- [Testes](#testes)
- [Depurador](#depurador)
- [Terminal](#terminal)
- [Navegador](#navegador)
- [Git](#git)
- [CI](#ci)
- [Contêineres](#contêineres)
- [Markdown](#markdown)
- [Ditado](#ditado)
- [Confiança](#confiança)
- [Sandbox](#sandbox)
- [Aparência e acessibilidade](#aparência-e-acessibilidade)
- [Idiomas](#idiomas)
- [Atalhos de teclado](#atalhos-de-teclado)
- [Privacidade e segurança](#privacidade-e-segurança)
- [Onde ficam os dados](#onde-ficam-os-dados)
- [Limitações conhecidas](#limitações-conhecidas)
- [Compilar a partir do código-fonte](#compilar-a-partir-do-código-fonte)
- [Licença](#licença)

## Recursos em resumo

- **Todos os agentes em uma linha do tempo.** Claude pelo seu próprio comando `claude`, Codex, Antigravity e OpenCode pelo Agent Client Protocol, outros do registro ACP e um agente integrado ao app para Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama e LM Studio. Respostas, chamadas de ferramentas, pedidos de permissão, planos, pendências, subagentes e tarefas em segundo plano têm a mesma aparência, seja qual for o agente.
- **Revisar e retroceder.** Antes de cada prompt, o Dango guarda um ponto de restauração. O que um agente alterou pode ser revisado e retrocedido por mensagem, e uma thread pode trabalhar em uma worktree Git própria e entregar o trabalho por mesclagem ou cherry-pick.
- **Uma janela por pasta.** Arquivos, terminais, páginas web, threads de IA, diffs e pré-visualizações são abas do macOS dessa janela. A barra lateral contém Arquivos, Busca, Agentes, Git, Testes, Depurar, CI e Contêineres.
- **Um editor de código de verdade.** Cores de sintaxe, vários pontos de inserção, recolhimento, `.editorconfig`, servidores de linguagem para 17 linguagens com preenchimento, ir para a definição, referências, renomear, correções rápidas e hierarquias, e previsões enquanto você digita do GitHub Copilot ou de um modelo local.
- **Depurador, Git e CI integrados.** Pontos de interrupção e execução passo a passo para Rust, C, C++, Swift, Go, JavaScript, TypeScript e Python; preparar blocos isolados, branches, stashes, conflitos e um grafo de commits; execuções, jobs e logs do GitHub Actions e do GitLab CI.
- **Cuidadoso com as suas pastas.** Uma pasta em que você não confia não inicia nada: nem processos Git, nem servidores de linguagem, nem hooks, nem agentes. Comandos Git destrutivos nunca são permitidos para sempre, e os commits são verificados em busca de segredos.
- **Ditado.** Fale no editor e no campo de mensagem, online pela OpenAI ou pela Groq ou localmente com Whisper e Parakeet.
- **Nativo de ponta a ponta.** Janelas, abas, menus e barras de ferramentas do AppKit, TextKit 2 para o editor, Metal para o terminal, WebKit só na aba do navegador. Modo escuro, Aumentar contraste e VoiceOver funcionam como em qualquer app do Mac.
- **No seu idioma.** A interface está disponível em 35 idiomas: alemão, árabe, catalão, checo, chinês (simplificado), chinês (tradicional), coreano, croata, dinamarquês, eslovaco, esloveno, espanhol, finlandês, francês, grego, hebraico, hindi, holandês, húngaro, indonésio, inglês, italiano, japonês, malaio, norueguês, polonês, português (Brasil), português (Portugal), romeno, russo, sueco, tailandês, turco, ucraniano e vietnamita.

## Requisitos

- macOS 14 Sonoma ou posterior, em Apple Silicon ou Intel. O Dango é desenvolvido e testado no macOS 27.
- Para as threads de IA, o agente que você quer usar; veja [Agentes de IA](#agentes-de-ia).
- Para os servidores de linguagem e os adaptadores de depuração, que o Dango instala quando você pede: Go para gopls e `dlv`; Node.js para os servidores de TypeScript, Python, JSON, YAML, CSS, HTML e shell e para o depurador de JavaScript; as Xcode Command Line Tools para clangd, sourcekit-lsp e `lldb-dap`. O rust-analyzer é baixado das versões dele no GitHub.
- Para as previsões enquanto você digita, um plano do GitHub Copilot (o gratuito serve) ou um modelo fill-in-the-middle no Ollama ou no LM Studio.

## Instalação

**Pela imagem de disco.** Cada versão é uma imagem de disco, `Dango-<version>.dmg`, com um app universal para Apple Silicon e Intel, assinado com um Developer ID e notarizado pela Apple. Abra-a e arraste o Dango para a pasta Aplicativos. Um Cask do Homebrew instala a mesma imagem. Nenhuma versão foi lançada ainda.

**Atualizações.** O Dango instalado pela imagem de disco se mantém atualizado com o [Sparkle](https://sparkle-project.org). Na segunda abertura, ele pergunta se pode buscar atualizações sozinho; até você concordar, não faz nenhuma solicitação. **Dango › Buscar Atualizações…** busca na hora. As atualizações são assinadas com a chave própria do app, que o Sparkle verifica antes de instalar qualquer coisa. Uma versão compilada por você não tem atualizador.

**Pelo código-fonte.** Veja [Compilar a partir do código-fonte](#compilar-a-partir-do-código-fonte) e depois mova `target/release/Dango.app` para `/Applications`.

**O comando de terminal.** **Dango › Instalar Comando “dango”…** grava um pequeno script em `~/.local/bin/dango`, sem privilégios de administrador; `~/.local/bin` precisa estar no seu `PATH`. Depois, `dango .` abre a pasta atual e `dango file.rs` um arquivo. O Dango nunca substitui ali um arquivo que ele mesmo não tenha gravado.

## Primeiros passos

1. **Abra uma pasta** com ⌘O ou `dango <folder>`. Na primeira vez, o Dango pergunta se você confia nela (veja [Confiança](#confiança)).
2. **A barra lateral** alterna entre Arquivos (⌘1), Busca (⌘2), Agentes (⌘3), Git (⌘4), Testes (⌘5), Depurar (⌘6), CI (⌘7) e Contêineres (⌘8); um clique com o botão direito oculta os que você não usa, e ⌃⌘S recolhe a barra lateral.
3. **Cada documento é uma aba** da janela da pasta, as abas do próprio macOS. O “+” no fim da barra de abas e ⌘T oferecem um arquivo novo, um terminal, o navegador e cada agente instalado, também em uma worktree Git própria; mantendo o “+” pressionado, aparece o mesmo como menu.
4. **Abrir Rapidamente** com ⌘P encontra arquivos, e com `#` os símbolos do projeto; ⇧⌘F busca e substitui no projeto.
5. **Converse com um agente** em uma aba de IA. ⌘Esc leva o foco do teclado ao campo de mensagem dele e de volta.
6. **Ajustes** com ⌘, contém as abas Geral, CI, Agentes, Agente Integrado, Servidor de linguagem, Ditado e Previsão.

A barra de ferramentas contém voltar e avançar pelos lugares que você editou (⌃- e ⌃⇧-), a branch, Abrir Rapidamente e o resumo de Atividade, que lista tudo o que está em execução. **Abrir ao Lado** no menu de contexto de um arquivo mostra um segundo documento ao lado do primeiro. Depois de uma reinicialização ou de uma falha, cada janela volta com suas abas, e editores com texto não salvo voltam com esse texto. Imagens, áudio, vídeo, fontes e documentos do Office abrem em uma aba pela Visualização Rápida, PDFs pelo PDFKit.

## Agentes de IA

| Agente | Como é executado | Do que precisa |
|---|---|---|
| Claude | o comando `claude`, com o qual o Dango fala diretamente pelo protocolo stream-json | `claude` instalado e com sessão iniciada; o Dango procura no `PATH` do shell de login, em `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` e `/usr/local/bin`. O login acontece só no próprio `claude` |
| Codex | `codex-acp` pelo Agent Client Protocol (ACP) | Node.js e npm; o Dango instala o pacote de versão fixada quando você pede |
| Antigravity | `agy acp` | o seu próprio `agy` no `PATH`, ou o download de versão fixada, instalado quando você pede |
| OpenCode | `opencode acp` | o seu próprio `opencode` no `PATH`, ou o download de versão fixada, instalado quando você pede |
| Outros agentes ACP | do registro ACP, buscado só quando você pede | o que a entrada do registro indicar; entradas sem checksum só são instaladas depois que você confirma |
| Agente integrado | integrado ao app, com ferramentas para arquivos, busca, comandos, diagnósticos e requisições web | uma chave de API para Anthropic, OpenAI, Google Gemini ou OpenRouter, guardada nas Chaves; Ollama ou LM Studio neste Mac sem chave; qualquer servidor que fale a API da OpenAI ou da Anthropic |

**Ajustes › Agentes** mostra quais agentes este Mac tem, instala os outros e suas atualizações e abre o registro ACP. Os agentes que o Dango instala ficam fixados em versões testadas e são verificados pelos checksums publicados (npm integrity ou SHA-256); o Dango não executa pacotes `npx` sem versão.

### A linha do tempo

Cada agente trabalha em uma aba de IA com a mesma linha do tempo: respostas, chamadas de ferramentas com a saída delas, pedidos de permissão, perguntas, planos para aprovar e comentar (⌥⌘K), pendências, subagentes e tarefas em segundo plano. Dois botões no cabeçalho da aba listam os subagentes e processos em segundo plano em execução. Mencione um arquivo, `@terminal` ou `@browser` para passar o conteúdo ao agente. Um ponto verde na aba indica uma sessão trabalhando, um vermelho uma sessão esperando por você, e uma notificação avisa quando ela termina ou espera enquanto você olha para outro lugar. Para o Claude, a linha de status mostra o contexto e o uso.

### Permissões e modos

Threads novas começam no modo “Manual”: o agente pergunta antes de alterar arquivos ou executar comandos. Para comandos Git destrutivos (`push --force`, `reset --hard`, `clean -fd`), o Dango não oferece “Permitir Sempre”. O agente integrado guarda suas regras de “Permitir Sempre” por projeto, lista-as em **Ajustes › Agente Integrado** e pergunta toda vez nos comandos Git destrutivos, seja o que for que esteja guardado; o Claude e os agentes ACP guardam essas regras na própria configuração. Com “Permitir Tudo” e sem sandbox, a thread mostra um aviso permanente.

### Pontos de restauração e worktrees

Antes de cada prompt, o Dango guarda um ponto de restauração, como commit em `refs/ide-checkpoints/` no repositório do projeto, ou em uma pasta própria para um projeto sem Git; um push ou fetch comum nunca os leva junto. O que um agente alterou pode ser revisado e retrocedido por mensagem. Uma thread pode trabalhar em uma worktree Git própria: o painel Git mostra o que ela alterou e incorpora o trabalho por mesclagem ou cherry-pick.

## Editor

- **Cores de sintaxe, números de linha e marcas de alteração** em relação ao último commit, buscar e substituir, Abrir Rapidamente (⌘P) e busca no projeto (⇧⌘F).
- **O recuo** segue o arquivo e o `.editorconfig`: tabulações ou espaços, recuo automático, pares de parênteses.
- **Vários pontos de inserção:** ⌥-clique, ⌥-arrastar para uma coluna, ⌃⇧↑ e ⌃⇧↓, ⌘D para a próxima ocorrência.
- **Recolhimento** (⌥⌘← e ⌥⌘→, com ⇧ para tudo), ativar ou desativar comentário (⌘/), ir para a linha (⌃G), espaços visíveis e quebrar linhas (⌥Z).
- **Os arquivos mantêm a codificação**, a marca de ordem de bytes e os finais de linha. Um arquivo alterado no disco é recarregado quando o editor não tem texto não salvo; caso contrário, uma barra oferece as diferenças e uma mesclagem.
- **Previsões enquanto você digita.** Enquanto você digita, aparece uma sugestão em cinza, e Tab a aceita. Ela vem do GitHub Copilot ou de um modelo local no Ollama ou no LM Studio, conforme a escolha em **Ajustes › Previsão**; um modelo local mantém o seu código neste Mac.
- **Minimapa e Rolagem Fixa** (menu Editor): uma imagem estreita do arquivo inteiro ao lado do texto, e as linhas de cabeçalho dos blocos em que está a linha do topo, fixadas acima dela.
- **Diagnósticos no Fim da Linha** (menu Visualizar): a mensagem mais grave de cada linha após um segundo de pausa, com um símbolo para a sua gravidade.
- **Pré-visualização ao vivo de páginas estáticas:** Editor › Abrir em Aba do Navegador serve as páginas de uma pasta confiável pelo próprio Dango e recarrega a aba quando um arquivo é salvo.
- **Prettier** formata JavaScript, TypeScript, CSS, HTML, Markdown e mais onde o projeto o configura, com o próprio `node_modules/.bin/prettier` do projeto; o ajuste `formatter` escolhe o formatador por linguagem.
- **Modo Vim** (Editor › Modo Vim): modos Normal, Inserção, Visual e Substituição, contagens, operadores com movimentos e objetos de texto, `.`, registradores, marcas e macros. `u` e ⌃R usam o desfazer do editor, e cada inserção conta como um único passo. `"+` e `"*` são a área de transferência; o ajuste `vimClipboard` faz do registrador sem nome também a área de transferência. `/`, `?`, `n`, `N`, `*`, `:s` e `:%s` buscam com as expressões regulares de Rust, não com os padrões do Vim: sem `\v`, e `\b` no lugar de `\<` e `\>`. Os comandos Ex são `:w`, `:q`, `:wq`, `:x`, `:noh` e `:<line>`. No modo Normal, ⌃G fica com o Vim em vez de ir para a linha.
- **Sincronização de ajustes** por uma pasta no iCloud Drive (**Ajustes › Geral**): seus ajustes e atalhos acompanham você nos seus outros Macs; caminhos, endereços locais e consentimentos ficam em cada um.

## Servidores de linguagem

Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML, Markdown e Lua, além de servidores próprios em **Ajustes › Servidor de linguagem**. O Dango instala um servidor quando você pede, na própria pasta de ferramentas, e usa o `tsc` de um projeto TypeScript quando ele pode servir como servidor de linguagem.

O que eles oferecem: diagnósticos (⌘' e ⇧⌘' passam de um para outro), informações do símbolo (⌃⌘I), preenchimento (⌥Esc), ajuda de assinaturas, ir para a definição (⌃⌘J), para a definição de tipo e para a implementação, referências (⌃⇧⌘F), hierarquias de chamadas e de tipos (⌃⇧⌘H), renomear (⌃⌘E), formatar (⌥⇧F), cores semânticas, recolhimento e a estrutura (⇧⌘O); tipos e nomes de parâmetros dentro da linha; correções rápidas e refatorações atrás de uma lâmpada (⌥⌘A); os símbolos do projeto com ⌥⌘O.

Além do servidor próprio de um arquivo, o ESLint verifica o JavaScript e o TypeScript dos projetos que o usam, com suas correções rápidas, e as abreviações do Emmet aparecem no preenchimento para HTML, CSS e JSX. O Dango instala os dois quando você pede; **Ajustes › Geral** corrige os problemas do ESLint ao salvar, se você quiser.

## Testes

O painel Testes lista os testes de uma pasta por pacote e arquivo com o estado de cada um: `cargo test` ou cargo-nextest, Go, pytest, Vitest e Jest. Um símbolo ao lado de cada teste na margem o executa, ⌥-clique o depura, e uma asserção com falha marca a linha dela. O menu Depurar executa o teste no ponto de inserção (⌃⌥⌘U), os últimos testes novamente (⌃⌥⌘G) ou todos, e depura o teste no ponto de inserção. Os testes só são executados em pastas confiáveis, já que executam o código do projeto; o `cargo test` lista seus testes quando você pede, pois listar compila todos os programas de teste.

## Depurador

Pontos de interrupção, também com condições e como logpoints, execução passo a passo, threads com suas pilhas de chamadas, variáveis e expressões monitoradas no Inspetor (⌥⌘I), valores ao lado do código enquanto o programa está pausado, e um console de depuração que avalia no frame pausado.

| Linguagem | Adaptador de depuração |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` das Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | o depurador de JavaScript, no Node.js |
| Python | debugpy |

O `.vscode/launch.json` de um projeto é lido. O menu Depurar usa os atalhos do Xcode: ⌘\\ define um ponto de interrupção, ⌘Y ativa ou desativa todos, ⌃⌘Y continua, F6, F7 e F8 são Contornar, Intervir e Sair, ⌘. para.

## Terminal

Terminais em abas (⌃\`) com o ambiente do shell de login, desenhados com Metal. ⌘K limpa a tela. Uma imagem colada ou arrastada vai para um agente de programação em execução no terminal, ou cola o caminho de uma cópia salva. ⌘-clique abre links e nomes de arquivos. Um servidor de desenvolvimento iniciado pelo terminal pode ser aberto na aba do navegador.

## Navegador

Uma aba de navegador com campo de endereço (⌘L), voltar e avançar (⌘[ e ⌘]), recarregar (⌘R), busca, downloads que perguntam antes e o inspetor web. `@browser` passa a página a um agente, marcada como texto de fora.

## Git

O painel Git prepara e descarta arquivos ou blocos isolados, faz commits, faz commit e push ou sincroniza, troca e cria branches, guarda stashes, resolve conflitos de mesclagem e desenha um grafo dos commits recentes. O blame mostra quem alterou por último a linha sob o cursor, ou uma coluna para o arquivo inteiro. Um botão escreve a mensagem de commit com IA; o diff só sai depois que o gitleaks não encontrou nenhum segredo nele, e cada commit também é verificado em busca de segredos.

## CI

GitHub Actions e GitLab CI, também GitHub Enterprise Server e GitLab auto-hospedado:

- execuções, jobs e logs da branch atual;
- reexecutar jobs com falha, um único job ou, no GitHub, uma execução inteira; iniciar jobs manuais ou aprovar implantações em espera; iniciar um pipeline novo ou um workflow que pode ser iniciado manualmente; cancelar;
- os pull requests ou merge requests abertos e as issues, que abrem na aba do navegador;
- o log de um job com falha entregue a um agente, marcado como texto de fora.

O token vem das Chaves, do `gh` ou do `glab`, do ambiente do shell de login, ou é um que você digita em **Ajustes › CI**.

## Contêineres

O painel Contêineres mostra os projetos Compose do Docker com seus serviços, contêineres avulsos, imagens e volumes, e acompanha os eventos do Docker à medida que acontecem. Um arquivo Compose da pasta aparece como projeto antes mesmo de existir qualquer um de seus contêineres. Os contêineres iniciam, param e reiniciam; seus logs e um shell abrem em abas do terminal, e uma porta publicada abre na aba do navegador. Apagar um contêiner, uma imagem ou um volume, e limpar dados não usados, pedem confirmação antes. Os comandos up, down e pull do Compose só são executados em pastas confiáveis, já que o Compose cria e executa o que o projeto descreve. O menu do painel escolhe o contexto do Docker da janela sem mudar o que os seus shells usam; os mesmos comandos estão no menu Contêineres. O painel precisa da linha de comando `docker`, Docker 24.0.6 ou posterior.

## Markdown

- **Pré-visualização.** Um arquivo Markdown abre com a pré-visualização ao lado; **Visualizar › Mostrar/Ocultar Pré-visualização ao Lado** (⇧⌘V) a fecha e a abre, e **Ajustes › Geral** desativa a pré-visualização automática. A pré-visualização desenha fórmulas, diagramas Mermaid, Graphviz e PlantUML e as imagens da pasta; imagens da web só carregam em uma pasta confiável ou quando você as pede na pré-visualização.
- **Menu Formatar:** negrito (⌘B), itálico (⌘I), tachado (⇧⌘X), código (⌥⇧⌘C), link (⇧⌘K), títulos 1–3 (⌥⌘1–⌥⌘3), listas com marcadores e numeradas, tarefas, citações e blocos de código. Cada comando alterna, na seleção ou nas linhas, e é uma única etapa de desfazer.
- **Colar.** Texto do Word ou de uma página web chega como Markdown, e as imagens como arquivos ao lado do documento; ⌥⇧⌘V cola texto simples.
- **Exportar.** **Arquivo › Exportar** grava um PDF, um único arquivo HTML com as imagens dentro ou um documento do Word. Imagens da web só vão junto a partir de uma pasta confiável.
- **Abrir no Splat.** Onde o [Splat](https://github.com/sventmueller/splat) estiver instalado, **Arquivo › Abrir no Splat** e os menus de contexto da lista de arquivos e das abas abrem o documento lá, depois de salvá-lo.

## Ditado

Dite no campo de mensagem de uma aba de IA ou no editor: com o botão de microfone ali, **Editar › Iniciar Ditado** (⌃⌥D) ou a tecla fn (mantenha pressionada para gravar, toque duas vezes para continuar gravando). O reconhecimento de fala é o do [Lowspeak](https://github.com/sventmueller/lowspeak); o Lowspeak em si não precisa estar instalado.

- **Ajustes › Ditado** define o modo e o modelo (online com OpenAI ou Groq, localmente com Whisper ou Parakeet), o idioma, o pós-processamento, o microfone e o seu próprio vocabulário.
- **Modelos locais** são baixados do Hugging Face quando você escolhe um, com progresso e a opção de cancelar. Eles ficam nas pastas que o Lowspeak usa, de modo que cada app usa o que o outro baixou. Um modelo local mantém a gravação neste Mac.
- **A transcrição online** precisa de uma chave da OpenAI ou da Groq, que o Dango guarda em um item próprio das Chaves.
- **Do Lowspeak.** Se o Lowspeak estiver configurado de forma diferente do padrão, a aba Ditado oferece uma vez adotar os ajustes dele, e as chaves se você pedir; o macOS pergunta antes que o Dango leia uma delas (escolha “Permitir”, não “Permitir Sempre”).
- **Permissões.** O macOS pede o microfone no primeiro ditado. A tecla fn funciona enquanto o Dango é o app ativo e o Lowspeak não está em execução; ela precisa da permissão de acessibilidade, que o Dango só pede depois do primeiro ditado, nunca ao abrir.

## Confiança

Uma pasta pode fazer programas serem executados assim que algo é iniciado nela: hooks do Git e ajustes do repositório como `core.fsmonitor`, servidores de linguagem de `node_modules`, hooks de agentes, `.mcp.json`. Na primeira vez que você abre uma pasta, o Dango pergunta se você confia nela.

- **Confiável:** Git, servidores de linguagem, depuradores, agentes e CI são executados para esta pasta e tudo o que há dentro dela.
- **Restrita:** você pode ler e editar; as marcas de alteração no editor vêm da leitura direta de `.git`. Nada do que a pasta configura é iniciado. Os painéis Git, CI e IA informam isso e oferecem confiar na pasta.

Um texto que não vem de você pode trazer instruções feitas para conduzir um agente (injeção de prompt). A saída de um terminal (`@terminal`), uma página web (`@browser`), um log de CI entregue a um agente, um arquivo fora do projeto e um texto mais longo colado de uma página web vão, por isso, marcados: a etiqueta diz “Conteúdo Externo”, e o agente recebe o texto com o aviso de que são dados não confiáveis cujas instruções ele não deve seguir. As páginas web do agente integrado chegam com o mesmo aviso. Um texto colado conta como vindo de uma página web quando o navegador informa isso na área de transferência, como fazem o Safari, o Chrome e o Firefox; texto de outros lugares, e uma frase curta de poucas palavras, continua sendo seu.

## Sandbox

O próprio Dango é executado sem App Sandbox, porque todos os programas que ele inicia o herdariam: terminais, Git, servidores de linguagem e agentes não conseguiriam funcionar. Em vez disso, cada thread de IA tem um botão de sandbox, desativado por padrão. O que ele limita depende do agente, e a dica do botão explica:

| Agente | Com o sandbox ativado |
|---|---|
| Claude | os comandos de shell são executados no sandbox próprio do Claude; as ferramentas dele para arquivos e web, não |
| Codex | comandos e alterações gravam só no projeto, sem rede |
| Agente integrado | arquivos, requisições web e comandos ficam dentro do projeto e das pastas permitidas, sem rede (`sandbox-exec`) |
| Antigravity, OpenCode | nenhum sandbox que o Dango possa ativar |

## Aparência e acessibilidade

- **O modo escuro** segue o sistema; **Aumentar contraste** e **Reduzir movimento** são respeitados.
- **O VoiceOver** nomeia a barra lateral, as abas, os cartões da linha do tempo de IA, as linhas do Git e da CI, o depurador e o texto do terminal. Pontos de interrupção nunca são diferenciados só pela cor.
- **Teclado.** Cada comando tem um atalho, e todos podem ser alterados em `keybindings.json`; ⌥⌘\` vai para o próximo painel.
- **Da direita para a esquerda.** Em árabe e hebraico, a janela é espelhada, enquanto as visualizações de código continuam da esquerda para a direita.

O que o Dango acrescenta ao AppKit, área por área, está em [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Idiomas

A interface está traduzida para 35 idiomas: alemão, árabe, catalão, checo, chinês (simplificado), chinês (tradicional), coreano, croata, dinamarquês, eslovaco, esloveno, espanhol, finlandês, francês, grego, hebraico, hindi, holandês, húngaro, indonésio, inglês, italiano, japonês, malaio, norueguês, polonês, português (Brasil), português (Portugal), romeno, russo, sueco, tailandês, turco, ucraniano e vietnamita.

O Dango segue a ordem de idiomas em **Ajustes › Geral › Idioma e Região**; ali você também pode escolher um idioma só para o Dango em **Aplicativos**. Sem tradução para o seu idioma, ele usa o inglês. Os termos seguem um glossário, [docs/glossary.md](../../../docs/glossary.md), e as traduções da Apple, para que o Dango se leia como o resto do seu Mac.

## Atalhos de teclado

| Ação | Atalho |
|---|---|
| Abrir pasta, nova janela, fechar janela | ⌘O, ⇧⌘N, ⇧⌘W |
| Novo arquivo, salvar, salvar como | ⌘N, ⌘S, ⇧⌘S |
| Nova aba, fechar aba, próxima aba e aba anterior | ⌘T, ⌘W, ⇧⌘] e ⇧⌘[ |
| Abrir Rapidamente, buscar no projeto | ⌘P, ⇧⌘F |
| Arquivos, Busca, Agentes, Git, Testes, Depurar, CI, Contêineres | ⌘1 a ⌘8 |
| Mostrar ou ocultar a barra lateral, próximo painel | ⌃⌘S, ⌥⌘\` |
| Voltar e avançar pelos lugares editados | ⌃- e ⌃⇧- |
| Ativar ou desativar comentário, ir para a linha | ⌘/, ⌃G |
| Próxima ocorrência, ponto de inserção acima e abaixo | ⌘D, ⌃⇧↑ e ⌃⇧↓ |
| Recolher, expandir, recolher tudo, expandir tudo | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Quebrar linhas, Inspetor | ⌥Z, ⌥⌘I |
| Preenchimento, informações do símbolo, ir para a definição | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referências, hierarquia de chamadas, renomear | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Símbolos no arquivo, no projeto | ⇧⌘O, ⌥⌘O |
| Correção rápida, formatar | ⌥⌘A, ⌥⇧F |
| Próximo problema e problema anterior | ⌘' e ⇧⌘' |
| Pré-visualização de Markdown, colar como texto simples | ⇧⌘V, ⌥⇧⌘V |
| Negrito, itálico, tachado, código, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Título 1 a 3 | ⌥⌘1 a ⌥⌘3 |
| Novo terminal, limpar o terminal | ⌃\`, ⌘K |
| Foco no campo de mensagem do agente, comentar um plano | ⌘Esc, ⌥⌘K |
| Campo de endereço, recarregar, voltar, avançar (navegador) | ⌘L, ⌘R, ⌘[, ⌘] |
| Iniciar ou parar o ditado | ⌃⌥D |
| Ponto de interrupção, ativar ou desativar todos | ⌘\\, ⌘Y |
| Executar o teste no ponto de inserção, executar novamente os últimos testes | ⌃⌥⌘U, ⌃⌥⌘G |
| Continuar, contornar, intervir, sair, parar | ⌃⌘Y, F6, F7, F8, ⌘. |
| Ajustes | ⌘, |

O `keybindings.json` na pasta de dados do app (veja [Onde ficam os dados](#onde-ficam-os-dados)) altera qualquer um deles.

## Privacidade e segurança

- **Sem telemetria,** sem análises, sem relatórios de falha enviados para lugar algum. Os erros vão para o registro do sistema (Console, com o nome do app).
- **A rede só para o que você inicia:** os agentes que você usa, cada um com o próprio serviço; o registro ACP e os downloads de agentes, servidores de linguagem, adaptadores de depuração e modelos de ditado quando você os pede; os hosts de CI em que você inicia sessão; as previsões do Copilot, se você o escolheu; os provedores do agente integrado; a transcrição online, se você a escolheu; as páginas que você abre na aba do navegador; e a busca de atualizações depois que você a permitiu.
- **O código só sai do Mac** onde você escolheu um serviço para isso: um agente, as previsões do Copilot, uma mensagem de commit escrita com IA. O diff para uma mensagem de commit só sai depois que o gitleaks não encontrou nenhum segredo nele.
- **Chaves e tokens** ficam nas Chaves do macOS, nunca em um arquivo de ajustes. Chaves de outros apps, como as do Lowspeak, só são lidas se você concordar, e só uma vez.
- **Downloads fixados.** Agentes, servidores de linguagem e adaptadores de depuração que o Dango instala ficam fixados em versões testadas e são verificados pelos checksums.
- **Assinatura de código** com o Hardened Runtime; as atualizações só são instaladas com a assinatura própria do app.

## Onde ficam os dados

`<id>` é o identificador do pacote, `local.agentide` para a versão lançada (`local.agentide.debug` para as compilações de depuração). Ele não contém o nome exibido, então uma mudança de nome mantém todos os dados.

| Local | O quê |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (a lista do painel de IA), `agent-threads/` (conversas do agente integrado), `layouts/` (abas por projeto), `recovery/` (editores não salvos após uma falha), `checkpoints/` (para pastas sem Git), `worktrees/`, `agents/` e `tools/` (agentes, servidores de linguagem e adaptadores de depuração instalados), `task-output/` |
| `~/Library/Caches/<id>/` | imagens coladas, scripts de integração com o shell, o registro ACP; pode ser apagado |
| `~/Library/Logs/<id>/dictation/` | os arquivos de log do ditado |
| `~/Library/Application Support/Lowspeak` e `FluidAudio` | modelos de ditado locais, compartilhados com o Lowspeak |
| `~/Library/WebKit/<id>/` | cookies e dados de sites da aba do navegador |
| `<project>/.dango/settings.json` | os ajustes próprios do projeto |
| o repositório Git do projeto | pontos de restauração, como commits em `refs/ide-checkpoints/`; um push ou fetch comum nunca os leva junto |
| Chaves, serviço `<id>` | chaves de API do agente integrado e do ditado, tokens de CI |
| `~/.local/bin/dango` | o comando de terminal, se você o instalou |

O Claude guarda as próprias sessões em `~/.claude/projects`; o Dango as lê para retomar uma sessão, mas não as altera.

## Limitações conhecidas

- **Sem App Sandbox** para o próprio app, porque terminais, Git, servidores de linguagem e agentes o herdariam; uma pasta precisa ser confiável antes que qualquer coisa que ela configure seja executada. O Antigravity e o OpenCode não têm um sandbox que o Dango possa ativar.
- **Os números de uso e a divisão do contexto do Claude** dependem de partes do protocolo dele marcadas como experimentais, e ficam ocultos quando elas faltam.
- **Retroceder a conversa** só funciona com o Claude; com os outros agentes, só o código é retrocedido.
- **O login na CI pelo navegador** está desativado; use `gh`, `glab`, um token ou o ambiente.
- **O Gemini CLI não é oferecido:** o Google encerrou o login com uma conta pessoal e indica o Antigravity. Threads do Gemini anteriores continuam legíveis.

## Compilar a partir do código-fonte

Você precisa de:

- macOS 14 ou posterior com as Xcode Command Line Tools (`xcode-select --install`); o Xcode em si não é necessário.
- Rust conforme indicado em `rust-toolchain.toml`. O `rust` do Homebrew compila para o processador deste Mac; a compilação universal de uma versão precisa do rustup com os alvos `aarch64-apple-darwin` e `x86_64-apple-darwin`.
- Cópias de [splat](https://github.com/sventmueller/splat) e [lowspeak](https://github.com/sventmueller/lowspeak) ao lado desta (`../splat`, `../lowspeak`), ambas na branch `main`; o workspace usa crates delas por caminho.
- cmake, para cada compilação com ditado (`make run`, `make build`, `make release`), que compila o whisper.cpp. `make check` compila sem.

| Comando | O que faz |
|---|---|
| `make run` | compila o app de depuração (`target/debug/Dango Debug.app`) e o abre |
| `make build` | compila o app de versão (`target/release/Dango.app`), assinado com um certificado local que a primeira compilação cria nas Chaves de início de sessão; mantém `VERSION` inalterado |
| `make lint` | formatação e clippy; avisos fazem falhar |
| `make check` | `lint` e depois todos os testes; não precisa de Chaves, janela nem rede |
| `make test` | `check` e depois o autoteste `probe` e a auditoria de acessibilidade, da esquerda para a direita e da direita para a esquerda, no app real |
| `make perf` | os orçamentos de tempo, em uma compilação de versão; só faz sentido em um Mac que esteja ocioso de resto |
| `make release` | um ensaio da publicação em `target/release-dry-run/`, assinado com o certificado local; não envia nada a lugar nenhum |
| `make release-notarized` | `check` e depois a publicação em si em `dist/`: assinatura com Developer ID, notarização, o feed de atualizações assinado com a chave própria do app; mantém `VERSION` inalterado |
| `make deploy` | em um `main` limpo e com selo verde de CI: notariza a próxima versão, atualiza e faz commit de `VERSION`, envia os arquivos da publicação e faz push do código, do selo de CI e da tag de versão; o GitHub publica a versão automaticamente |

Depuração e versão são apps separados, com ajustes e dados próprios. A compilação, os testes, os autotestes, a publicação e as regras que o código segue estão descritos em [docs/development.md](../../../docs/development.md), e as decisões por trás deles em [docs/plan.md](../../../docs/plan.md). As notas da versão são o [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traduzidas ao lado de cada README em `assets/readme/<language>/`.

## Licença

O Dango é licenciado sob qualquer uma das duas

- Licença Apache, versão 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licença MIT ([LICENSE-MIT](../../../LICENSE-MIT))

à sua escolha.

Salvo declaração explícita em contrário, qualquer contribuição enviada intencionalmente por você para inclusão no Dango, conforme definida na licença Apache-2.0, terá licença dupla como acima, sem termos ou condições adicionais.
