Esta é a primeira versão do Dango, um editor nativo para macOS para trabalhar com agentes de programação. Editor,
terminais, um navegador, Git, CI, um depurador, threads de IA e uma Software Factory para ordens de trabalho partilham
uma janela por pasta. A aplicação está escrita em Rust sobre o AppKit; o WebKit surge apenas no separador do navegador.
A interface está disponível em 35 idiomas.

## Requisitos
- macOS 14 ou posterior, com Apple silicon ou Intel.
- Para os threads de IA, o agente escolhido: o Claude precisa do comando `claude`, com sessão iniciada; o Codex precisa
  do Node.js, e a aplicação instala o respetivo pacote numa versão fixa quando o pedir; o Antigravity e o OpenCode usam
  os seus próprios `agy` ou `opencode`, ou uma transferência numa versão fixa; o agente integrado precisa de uma chave
  da API, ou do Ollama ou do LM Studio neste Mac.
- Os servidores de linguagem e os adaptadores de depuração são instalados quando o pedir. O gopls e o `dlv` precisam do
  Go; os servidores de TypeScript, Python, JSON, YAML, CSS, HTML, ESLint e shell e o depurador de JavaScript precisam
  do Node.js; o clangd, o sourcekit-lsp e o `lldb-dap` vêm com as Xcode Command Line Tools.
- As previsões durante a escrita precisam de uma subscrição do GitHub Copilot (a gratuita é suficiente) ou de um modelo
  fill-in-the-middle no Ollama ou no LM Studio.

## Janela
- Cada ficheiro, terminal, página do navegador, thread de IA, diff e pré-visualização é um separador do macOS na janela
  da pasta. O “+” no fim da barra de separadores e ⌘T oferecem um novo ficheiro, um terminal, o navegador e cada agente
  instalado, também num worktree Git próprio.
- A barra lateral alterna entre Ficheiros, Pesquisa, Agentes, Git, Depurar, CI, Testes, Contentores e a Software
  Factory; um clique com o botão direito oculta os que não utiliza. A barra de ferramentas contém recuar e avançar pelos
  locais editados (⌃- e ⌃⇧-), o branch, Abrir rapidamente e a vista geral de Atividade.
- Uma janela divide-se em áreas lado a lado ou uma por cima da outra, aninhadas com a profundidade que quiser, como no
  Xcode; cada área mantém os seus documentos uns atrás dos outros, com uma barra de separadores própria. “Abrir ao lado”
  mostra um segundo documento ao lado do primeiro.
- Após um reinício, ou uma falha, cada janela regressa com os seus separadores e áreas, e os editores com texto por
  guardar regressam com esse texto.
- Imagens, áudio, vídeo, tipos de letra e documentos do Office abrem num separador através do Quick Look, os PDF através
  do PDFKit.

## Editor
- Cores de sintaxe, números de linha, marcas de alteração em relação ao último commit, procurar e substituir, Abrir
  rapidamente (⌘P) e pesquisa no projeto (⇧⌘F). As cores e os estilos de Markdown acompanham mesmo em ficheiros de
  muitos milhares de linhas, sem atrasar a escrita.
- Indentação automática, tabulações ou espaços conforme o `.editorconfig`, pares de parênteses, ativar e desativar
  comentários, dobragem, ir para a linha, espaços em branco visíveis, moldagem de linhas, um minimapa e cabeçalhos fixos
  dos âmbitos que envolvem o código.
- Vários cursores: ⌥-clique, ⌥-arrastar para uma coluna, ⌘D para a ocorrência seguinte.
- **Modo Vim:** ativado no menu Editor, com os modos normal, inserção e visual, movimentos e operadores, registos,
  marcas, macros, pesquisa e comandos Ex; as respetivas mensagens surgem no idioma da interface.
- Abreviaturas do Emmet no preenchimento automático de HTML, CSS e JSX, e os problemas do ESLint corrigidos ao guardar,
  se o desejar.
- Os ficheiros mantêm a codificação, a marca de ordem de bytes e os finais de linha. Um ficheiro alterado no disco é
  recarregado quando o editor não tem texto por guardar; caso contrário, uma barra oferece as diferenças e a fusão.
- **Previsões durante a escrita:** enquanto escreve, surge uma proposta a cinzento; a tecla Tab aceita-a. As propostas
  vêm do GitHub Copilot ou de um modelo local no Ollama ou no LM Studio, conforme a escolha nas Definições; um modelo
  local mantém o código neste Mac.

## Servidores de linguagem
Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML,
Markdown e Lua, o ESLint ao lado do servidor de TypeScript, e servidores próprios definidos nas Definições.
Diagnósticos, também no fim da linha, hover, conclusão, ajuda de assinatura, ir para a definição, definição de tipo e
implementação, referências, hierarquias de chamadas e de tipos, mudar o nome, formatação, cores semânticas, dobragem e
estrutura; tipos e nomes de parâmetros dentro da linha; correções rápidas e refatorizações atrás de uma lâmpada (⌥⌘A);
os símbolos do projeto com ⌥⌘O ou `#` em Abrir rapidamente. O próprio `tsc` de um projeto TypeScript é usado quando
pode servir de servidor de linguagem.

## Depurador
Pontos de interrupção, também com condições, execução passo a passo, threads com as respetivas pilhas de chamadas,
variáveis e expressões observadas no Inspetor (⌥⌘I), valores junto ao código enquanto o programa está em pausa e uma
consola de depuração que avalia expressões no frame em pausa. Rust, C, C++ e Swift através do `lldb-dap`, Go através do
`dlv`, JavaScript e TypeScript através do depurador de JavaScript, Python através do debugpy. O `.vscode/launch.json` do
projeto é lido. O menu Depurar usa os atalhos do Xcode.

## Testes e contentores
- O painel Testes lista os testes de projetos Rust, Go, Python, Jest, Vitest e Bun antes da primeira execução, executa
  um, um ficheiro ou todos, mostra cada resultado em tempo real e assinala-o na margem do editor.
- O painel Contentores lista os contentores, as imagens, os volumes e os projetos Compose do Docker; inicia-os, para-os
  e remove-os, acompanha os respetivos logs e abre uma shell num separador de terminal.

## Terminal
Terminais em separadores com o ambiente da shell de início de sessão, desenhados com Metal. Uma imagem colada ou largada
é entregue a um agente de programação em execução no terminal; caso contrário, é colado o caminho de uma cópia guardada.
⌘-clique abre hiperligações e nomes de ficheiros. Um servidor de desenvolvimento iniciado pelo terminal pode ser aberto
no separador do navegador.

## Markdown
Um ficheiro Markdown abre com a pré-visualização ao lado (⇧⌘V), com fórmulas, diagramas Mermaid, Graphviz e PlantUML e
as imagens da pasta. Um menu Formatação para negrito, itálico, hiperligações, cabeçalhos, listas, citações e blocos de
código; o texto colado do Word ou de uma página web chega como Markdown. **Ficheiro › Exportar** escreve PDF, um único
ficheiro HTML ou um documento Word. Onde o Splat estiver instalado, **Ficheiro › Abrir no Splat** abre lá o documento.

## Navegador
Um separador de navegador com campo de endereço, recuar e avançar, pesquisa, transferências com confirmação prévia e o
Inspetor web.

## Git
Um painel Git com preparação e descarte de ficheiros inteiros ou de blocos individuais, commits, commit e push ou
sincronização, branches, stashes, conflitos de fusão e um gráfico dos commits recentes. O Blame mostra quem alterou por
último a linha sob o cursor, ou uma coluna para o ficheiro inteiro. Um botão escreve a mensagem de commit com IA; o diff
só sai depois de o gitleaks não ter encontrado nele nenhum segredo, e cada commit também é verificado quanto a segredos.
Um thread de IA pode trabalhar num worktree Git próprio; o painel mostra o que alterou e incorpora o trabalho por fusão
ou cherry-pick.

## CI
GitHub Actions e GitLab CI, também GitHub Enterprise Server e GitLab autogerido: execuções, jobs e logs do branch atual;
voltar a executar jobs falhados, um único job ou (no GitHub) uma execução inteira, iniciar jobs manuais ou aprovar
implementações em espera, iniciar um novo pipeline ou um workflow que possa ser iniciado manualmente, e cancelar. Os
merge requests ou pull requests abertos e as issues aparecem no mesmo painel e abrem no separador do navegador. O log de
um job falhado pode seguir para um agente, marcado como conteúdo externo. O token vem do Porta-chaves, do `gh` ou do
`glab`, do ambiente da shell de início de sessão, ou é introduzido manualmente.

## IA
Cada agente trabalha num separador de IA com a mesma cronologia: respostas, chamadas de ferramentas com a respetiva
saída, pedidos de permissão, perguntas, planos para aprovar e comentar, tarefas pendentes, subagentes e tarefas em
segundo plano; dois botões no cabeçalho do separador listam os subagentes e os processos em segundo plano em execução.
Antes de cada prompt, a aplicação cria um ponto de restauro; as alterações podem ser revistas e retrocedidas por
mensagem. Os novos threads começam no modo “Manual”, e os comandos Git destrutivos nunca são permitidos de forma
definitiva. A vista geral de Atividade lista tudo o que está em execução. Um ponto verde no separador indica uma sessão
a trabalhar, um vermelho uma sessão à sua espera, e uma notificação avisa quando uma sessão termina ou fica à espera
enquanto está a ver outra coisa. A cronologia, o campo de mensagem e a linha de estado têm um tamanho de tipo de letra
próprio nas Definições.
- **Claude** através do comando `claude`, sem alterações, com sessões, subagentes, tarefas em segundo plano, contexto,
  utilização e Remote Control a partir do telemóvel.
- **Codex** através do respetivo servidor de aplicação e **OpenCode** através do respetivo servidor, cada um no seu
  próprio protocolo, com os seus threads, o retrocesso no próprio agente e Remote Control para o Codex.
- **Agentes ACP:** Antigravity, e outros do registo ACP quando o pedir; os agentes instalados estão fixados em versões
  testadas e são verificados com os respetivos checksums.
- **Agente integrado:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio e servidores compatíveis com a
  API da OpenAI ou da Anthropic, com ferramentas para ficheiros, pesquisa, comandos, diagnósticos e pedidos web; as
  chaves ficam no Porta-chaves.
- Os comandos próprios de um agente abrem como sobreposições do separador, iguais para todos os agentes: estado,
  exportação, servidores MCP, permissões, hooks, memória, configuração, skills, plug-ins, modelo, conta e feedback.
- Um interruptor de sandbox por thread limita o que os comandos de um agente podem fazer, na medida em que o agente o
  suporte.
- O conteúdo externo, como a saída do terminal, uma página web ou um log de CI, segue para o agente marcado como não
  fidedigno.

## Software Factory
Ordens de trabalho em vez de prompts avulsos: um objetivo com critérios de aceitação, escrito à mão ou retirado de uma
issue. Cada ordem é planeada, construída e verificada num worktree Git próprio, por agentes de fabricantes diferentes
em funções fixas: um planeador escreve a especificação, um construtor faz o trabalho e um revisor de outro fabricante
verifica cada critério. Os comandos de verificação do projeto, como a compilação e os testes, são executados numa
sandbox, e o construtor corrige o que encontrarem.
- Decide em dois momentos: aprova a especificação e aceita o resultado com a respetiva evidência, na íntegra ou secção
  a secção. Nada é incorporado sem a sua aceitação.
- O painel e o separador da Software Factory mostram cada ordem com a respetiva estação, o custo e aquilo de que está à
  espera; o separador de uma ordem mostra a cronologia, a evidência e o passo seguinte, e abre a sessão em curso como
  separador de IA.
- Orçamentos por ordem e por dia, limites de turnos, uma pausa ao sair, em repouso e com bateria fraca, e um log
  escrito antes de cada efeito, para que as ordens sejam retomadas após um reinício.
- As issues com uma etiqueta escolhida podem entrar como ordens; uma ordem aceite pode sair como merge request com um
  comentário na respetiva issue, com um clique seu. As ordens com âmbitos de caminhos separados são executadas lado a
  lado e incorporadas uma após a outra.
- Métricas por agente e fabricante, encaminhamento pela taxa de primeira passagem, propostas de regras para `AGENTS.md`
  e `CLAUDE.md` a partir de constatações recorrentes, e ordens recorrentes do ficheiro de workflow do projeto.
- O ficheiro de workflow `.dango/factory.toml` de uma pasta contém os respetivos gates, classes de risco e âmbitos de
  caminhos; os comandos de verificação alterados e tudo o que afrouxe os limites aguardam a sua confirmação.

## Ditado
Ditado no campo de mensagem do separador de IA e no editor através do reconhecimento de voz do Lowspeak: o botão do
microfone, ⌃⌥D ou a tecla fn enquanto o Dango é a aplicação ativa. Usa as definições e os modelos do Lowspeak; a
transcrição online precisa de uma chave da OpenAI ou da Groq.

## Atualizações
A aplicação atualiza-se através do Sparkle. No segundo arranque, pergunta se pode procurar atualizações por si própria;
enquanto não concordar, não faz nenhum pedido. **Dango › Procurar atualizações…** procura de imediato.

## Acessibilidade
Nomes do VoiceOver para a barra lateral, os separadores, os cartões da cronologia de IA, as linhas de Git e CI, a
Software Factory, o depurador e o texto do terminal; controlo total pelo teclado com atalhos que pode alterar em
`keybindings.json`; Aumentar contraste e Reduzir movimento; as vistas de código mantêm-se da esquerda para a direita em
interfaces da direita para a esquerda. Mais detalhes em ACCESSIBILITY.md.

## Privacidade
Sem telemetria, sem análises e sem relatórios de falhas. A aplicação só estabelece ligação para o que iniciar: os
agentes que utiliza, as transferências que pede, os anfitriões de CI em que inicia sessão, as previsões do Copilot se as
tiver escolhido, a transcrição online se a tiver escolhido, as páginas que abre e a procura de atualizações depois de a
ter permitido. As chaves e os tokens ficam no Porta-chaves do macOS, e os agentes da Software Factory nunca os veem.

## Limitações conhecidas
- O início de sessão no CI através do navegador está desativado; use `gh`, `glab`, um token ou o ambiente.
- A aplicação é executada sem a App Sandbox, porque os terminais, o Git, os servidores de linguagem e os agentes a
  herdariam; uma pasta tem de ser de confiança antes de ser executada qualquer coisa que ela configure. O Antigravity e
  o OpenCode não têm uma sandbox que a aplicação possa ativar.
- Na Software Factory, um construtor ou revisor do Codex pode ler fora do respetivo worktree, porque o protocolo do
  Codex não tem limites de leitura; o que escreve fica lá dentro.
- Os valores de utilização e a repartição do contexto do Claude dependem de partes do respetivo protocolo marcadas como
  experimentais e ficam ocultos quando faltam. Retroceder a conversa funciona com o Claude, o Codex e o OpenCode; nos
  outros agentes, só o código é retrocedido.
- O Gemini CLI não é oferecido: a Google terminou o início de sessão com conta pessoal e remete para o Antigravity. Os
  threads anteriores do Gemini continuam legíveis.
