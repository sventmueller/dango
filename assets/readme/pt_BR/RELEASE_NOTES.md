Esta é a primeira versão do Dango, um editor nativo para macOS feito para trabalhar com agentes de programação. Editor,
terminais, um navegador, Git, CI, um depurador, threads de IA e uma Software Factory para ordens de serviço compartilham
uma janela por pasta. O app é escrito em Rust sobre o AppKit; o WebKit aparece apenas na aba do navegador. A interface
está disponível em 35 idiomas.

## Requisitos
- macOS 14 ou posterior, em Apple silicon ou Intel.
- Para os threads de IA, o agente que você escolher: o Claude precisa do comando `claude`, com sessão iniciada; o Codex
  precisa do Node.js, e o app instala o pacote dele em versão fixada quando você pedir; o Antigravity e o OpenCode usam
  o seu próprio `agy` ou `opencode`, ou um download em versão fixada; o agente integrado precisa de uma chave de API, ou
  do Ollama ou do LM Studio neste Mac.
- Servidores de linguagem e adaptadores de depuração são instalados quando você pede. O gopls e o `dlv` precisam do Go;
  os servidores de TypeScript, Python, JSON, YAML, CSS, HTML, ESLint e shell e o depurador de JavaScript precisam do
  Node.js; clangd, sourcekit-lsp e `lldb-dap` vêm com as Xcode Command Line Tools.
- As previsões ao digitar precisam de uma assinatura do GitHub Copilot (a gratuita serve) ou de um modelo
  fill-in-the-middle no Ollama ou no LM Studio.

## Janela
- Cada arquivo, terminal, página do navegador, thread de IA, diff e pré-visualização é uma aba do macOS na janela da
  pasta. O “+” no fim da barra de abas e ⌘T oferecem um novo arquivo, um terminal, o navegador e cada agente instalado,
  também em um worktree Git próprio.
- A barra lateral alterna entre Arquivos, Busca, Agentes, Git, Depurar, CI, Testes, Contêineres e a Software Factory;
  um clique com o botão direito oculta os que você não usa. A barra de ferramentas tem voltar e avançar pelos lugares
  que você editou (⌃- e ⌃⇧-), a branch, Abrir Rapidamente e a visão geral de Atividade.
- Uma janela se divide em áreas lado a lado ou uma acima da outra, aninhadas com a profundidade que você quiser, como no
  Xcode; cada área mantém seus documentos um atrás do outro, com uma barra de abas própria. “Abrir ao Lado” mostra um
  segundo documento ao lado do primeiro.
- Depois de reiniciar, ou de uma falha, cada janela volta com suas abas e áreas, e editores com texto não salvo voltam
  com esse texto.
- Imagens, áudio, vídeo, fontes e documentos do Office abrem em uma aba pelo Quick Look, PDFs pelo PDFKit.

## Editor
- Cores de sintaxe, números de linha, marcas de alteração em relação ao último commit, buscar e substituir, Abrir
  Rapidamente (⌘P) e busca no projeto (⇧⌘F). Cores e estilos de Markdown acompanham mesmo em arquivos de muitos milhares
  de linhas, sem atrasar a digitação.
- Recuo automático, tabulações ou espaços conforme o `.editorconfig`, pares de colchetes, ativar e desativar
  comentários, dobra de código, ir para linha, espaços em branco visíveis, quebra de linhas, um minimapa e cabeçalhos
  fixos dos escopos que envolvem o código.
- Vários cursores: ⌥-clique, ⌥-arrastar para uma coluna, ⌘D para a próxima ocorrência.
- **Modo Vim:** ativado no menu Editor, com os modos normal, inserção e visual, movimentos e operadores, registradores,
  marcas, macros, busca e comandos Ex; suas mensagens aparecem no idioma da interface.
- Abreviações do Emmet no autocompletar de HTML, CSS e JSX, e os problemas do ESLint corrigidos ao salvar, se você
  quiser.
- Os arquivos mantêm a codificação, a marca de ordem de bytes e os finais de linha. Um arquivo alterado no disco é
  recarregado quando o editor não tem texto não salvo; caso contrário, uma barra oferece as diferenças e a mesclagem.
- **Previsões ao digitar:** enquanto você digita, uma sugestão aparece em cinza; Tab a aceita. As sugestões vêm do
  GitHub Copilot ou de um modelo local no Ollama ou no LM Studio, conforme você escolher nos Ajustes; um modelo local
  mantém o seu código neste Mac.

## Servidores de linguagem
Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML,
Markdown e Lua, ESLint ao lado do servidor de TypeScript, além de servidores próprios definidos nos Ajustes.
Diagnósticos, também no fim da linha, hover, conclusão de código, ajuda de assinatura, ir para definição, definição de
tipo e implementação, referências, hierarquias de chamadas e de tipos, renomear, formatação, cores semânticas, dobra de
código e estrutura; tipos e nomes de parâmetros dentro da linha; correções rápidas e refatorações atrás de uma lâmpada
(⌥⌘A); os símbolos do projeto com ⌥⌘O ou `#` em Abrir Rapidamente. O próprio `tsc` de um projeto TypeScript é usado
quando pode servir como servidor de linguagem.

## Depurador
Pontos de interrupção, também com condições, execução passo a passo, threads com suas pilhas de chamadas, variáveis e
expressões monitoradas no Inspetor (⌥⌘I), valores ao lado do código enquanto o programa está pausado e um console de
depuração que avalia expressões no frame pausado. Rust, C, C++ e Swift pelo `lldb-dap`, Go pelo `dlv`, JavaScript e
TypeScript pelo depurador de JavaScript, Python pelo debugpy. O `.vscode/launch.json` do projeto é lido. O menu Depurar
usa os atalhos do Xcode.

## Testes e contêineres
- O painel Testes lista os testes de projetos Rust, Go, Python, Jest, Vitest e Bun antes da primeira execução, executa
  um, um arquivo ou todos, mostra cada resultado ao vivo e o marca na margem do editor.
- O painel Contêineres lista os contêineres, imagens, volumes e projetos Compose do Docker; ele os inicia, para e
  remove, acompanha seus logs e abre um shell em uma aba de terminal.

## Terminal
Terminais em abas com o ambiente do shell de login, desenhados com Metal. Uma imagem colada ou solta é entregue a um
agente de programação em execução no terminal; caso contrário, é colado o caminho de uma cópia salva. ⌘-clique abre
links e nomes de arquivo. Um servidor de desenvolvimento iniciado pelo terminal pode ser aberto na aba do navegador.

## Markdown
Um arquivo Markdown abre com a pré-visualização ao lado (⇧⌘V), com fórmulas, diagramas Mermaid, Graphviz e PlantUML e as
imagens da pasta. Um menu Formatar para negrito, itálico, links, títulos, listas, citações e blocos de código; texto
colado do Word ou de uma página da web chega como Markdown. **Arquivo › Exportar** gera PDF, um único arquivo HTML ou um
documento do Word. Onde o Splat estiver instalado, **Arquivo › Abrir no Splat** abre o documento nele.

## Navegador
Uma aba de navegador com campo de endereço, voltar e avançar, busca, downloads com confirmação prévia e o Inspetor Web.

## Git
Um painel Git com preparação e descarte de arquivos inteiros ou de blocos individuais, commits, commit e push ou
sincronização, branches, stashes, conflitos de mesclagem e um gráfico dos commits recentes. O Blame mostra quem alterou
por último a linha sob o cursor, ou uma coluna para o arquivo inteiro. Um botão escreve a mensagem de commit com IA; o
diff só é enviado depois que o gitleaks não encontra nenhum segredo nele, e cada commit também é verificado em busca de
segredos. Um thread de IA pode trabalhar em um worktree Git próprio; o painel mostra o que ele alterou e incorpora o
trabalho por mesclagem ou cherry-pick.

## CI
GitHub Actions e GitLab CI, também GitHub Enterprise Server e GitLab autogerenciado: execuções, jobs e logs da branch
atual; reexecutar jobs com falha, um único job ou (no GitHub) uma execução inteira, iniciar jobs manuais ou aprovar
implantações em espera, iniciar um novo pipeline ou um workflow que possa ser iniciado manualmente, e cancelar. Os merge
requests ou pull requests abertos e as issues aparecem no mesmo painel e abrem na aba do navegador. O log de um job com
falha pode ir para um agente, marcado como conteúdo externo. O token vem das Chaves, do `gh` ou do `glab`, do ambiente
do shell de login ou é digitado por você.

## IA
Cada agente trabalha em uma aba de IA com a mesma linha do tempo: respostas, chamadas de ferramentas com sua saída,
pedidos de permissão, perguntas, planos para aprovar e comentar, pendências, subagentes e tarefas em segundo plano; dois
botões no cabeçalho da aba listam os subagentes e os processos em segundo plano em execução. Antes de cada prompt, o app
cria um ponto de restauração; as alterações podem ser revisadas e retrocedidas por mensagem. Novos threads começam no
modo “Manual”, e comandos Git destrutivos nunca são permitidos de forma permanente. A visão geral de Atividade lista
tudo o que está em execução. Um ponto verde na aba indica uma sessão trabalhando, um vermelho uma sessão esperando por
você, e uma notificação avisa quando uma sessão termina ou fica esperando enquanto você está em outro lugar. A linha do
tempo, o campo de mensagem e a linha de status têm um tamanho de fonte próprio nos Ajustes.
- **Claude** pelo comando `claude`, sem alterações, com sessões, subagentes, tarefas em segundo plano, contexto, uso e
  Remote Control pelo celular.
- **Codex** pelo seu servidor de app e **OpenCode** pelo seu servidor, cada um em seu próprio protocolo, com seus
  threads, retrocesso no próprio agente e Remote Control para o Codex.
- **Agentes ACP:** Antigravity, e outros do registro ACP quando você pedir; os agentes instalados ficam fixados em
  versões testadas e são verificados pelas suas somas de verificação.
- **Agente integrado:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio e servidores compatíveis com a
  API da OpenAI ou da Anthropic, com ferramentas para arquivos, busca, comandos, diagnósticos e requisições web; as
  chaves ficam nas Chaves.
- Os comandos próprios de um agente abrem como sobreposições da aba, iguais para todos os agentes: estado, exportação,
  servidores MCP, permissões, hooks, memória, configuração, skills, plug-ins, modelo, conta e feedback.
- Um seletor de sandbox por thread limita o que os comandos de um agente podem fazer, na medida em que o agente oferece
  suporte a isso.
- Conteúdo externo, como saída do terminal, uma página da web ou um log de CI, vai para o agente marcado como não
  confiável.

## Software Factory
Ordens de serviço em vez de prompts avulsos: um objetivo com critérios de aceitação, escrito à mão ou tirado de uma
issue. Cada ordem é planejada, construída e verificada em um worktree Git próprio, por agentes de fabricantes
diferentes em funções fixas: um planejador escreve a especificação, um construtor faz o trabalho e um revisor de outro
fabricante verifica cada critério. Os comandos de verificação do projeto, como build e testes, são executados em uma
sandbox, e o construtor corrige o que eles encontrarem.
- Você decide em dois momentos: aprova a especificação e aceita o resultado com sua evidência, por inteiro ou seção por
  seção. Nada é incorporado sem a sua aceitação.
- O painel e a aba da Software Factory mostram cada ordem com sua estação, seu custo e o que ela aguarda; a aba de uma
  ordem mostra sua linha do tempo, sua evidência e o próximo passo, e abre a sessão em execução como aba de IA.
- Orçamentos por ordem e por dia, limites de turnos, uma pausa ao encerrar o app, no repouso e com bateria fraca, e um
  log escrito antes de cada efeito, para que as ordens sejam retomadas depois de reiniciar.
- Issues com um rótulo escolhido podem entrar como ordens; uma ordem aceita pode sair como merge request com um
  comentário na sua issue, com um clique seu. Ordens com escopos de caminhos separados rodam lado a lado e são
  incorporadas uma após a outra.
- Métricas por agente e fabricante, roteamento pela taxa de primeira passagem, propostas de regras para `AGENTS.md` e
  `CLAUDE.md` a partir de constatações recorrentes, e ordens recorrentes do arquivo de workflow do projeto.
- O arquivo de workflow `.dango/factory.toml` de uma pasta contém seus gates, classes de risco e escopos de caminhos;
  comandos de verificação alterados e tudo o que afrouxe os limites aguardam a sua confirmação.

## Ditado
Ditado no campo de mensagem da aba de IA e no editor pelo reconhecimento de fala do Lowspeak: o botão do microfone, ⌃⌥D
ou a tecla fn enquanto o Dango é o app ativo. Ele usa os ajustes e modelos do Lowspeak; a transcrição online precisa de
uma chave da OpenAI ou da Groq.

## Atualizações
O app se atualiza pelo Sparkle. Na segunda abertura, ele pergunta se pode buscar atualizações por conta própria; até
você concordar, ele não faz nenhuma requisição. **Dango › Buscar Atualizações…** busca na hora.

## Acessibilidade
Nomes do VoiceOver para a barra lateral, as abas, os cartões da linha do tempo de IA, as linhas de Git e CI, a Software
Factory, o depurador e o texto do terminal; controle total pelo teclado com atalhos que você pode alterar em
`keybindings.json`; Aumentar Contraste e Reduzir Movimento; as visualizações de código continuam da esquerda para a
direita em interfaces da direita para a esquerda. Detalhes em ACCESSIBILITY.md.

## Privacidade
Sem telemetria, sem análises e sem relatórios de falhas. O app só se conecta para o que você inicia: os agentes que você
usa, os downloads que você pede, os hosts de CI em que você inicia sessão, as previsões do Copilot se você as escolheu,
a transcrição online se você a escolheu, as páginas que você abre e a busca de atualizações depois que você a permitiu.
Chaves e tokens ficam nas Chaves do macOS, e os agentes da Software Factory nunca os veem.

## Limitações conhecidas
- O início de sessão no CI pelo navegador está desativado; use `gh`, `glab`, um token ou o ambiente.
- O app é executado sem o App Sandbox, pois terminais, Git, servidores de linguagem e agentes o herdariam; uma pasta
  precisa ser confiável antes que qualquer coisa configurada nela seja executada. O Antigravity e o OpenCode não têm
  sandbox que o app possa ativar.
- Na Software Factory, um construtor ou revisor do Codex pode ler fora do seu worktree, pois o protocolo do Codex não
  tem limites de leitura; o que ele escreve fica dentro do worktree.
- Os números de uso e o detalhamento do contexto do Claude dependem de partes do protocolo dele marcadas como
  experimentais e ficam ocultos quando elas não estão presentes. Retroceder a conversa funciona com o Claude, o Codex e
  o OpenCode; nos outros agentes, apenas o código é retrocedido.
- O Gemini CLI não é oferecido: o Google encerrou o início de sessão com conta pessoal e indica o Antigravity. Threads
  anteriores do Gemini continuam legíveis.
