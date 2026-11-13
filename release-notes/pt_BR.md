<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Esta é a primeira versão do Dango, um editor nativo para macOS feito para trabalhar com agentes de programação. Editor,
terminais, um navegador, Git, CI, um depurador e threads de IA compartilham uma janela por pasta. O app é escrito em
Rust sobre o AppKit; o WebKit aparece apenas na aba do navegador. A interface está disponível em 35 idiomas.

## Requisitos
- macOS 14 ou posterior, em Apple silicon ou Intel.
- Para os threads de IA, o agente que você escolher: o Claude precisa do comando `claude`, com sessão iniciada; o Codex
  precisa do Node.js, e o app instala o pacote dele em versão fixada quando você pedir; o Antigravity e o OpenCode usam
  o seu próprio `agy` ou `opencode`, ou um download em versão fixada; o agente integrado precisa de uma chave de API, ou
  do Ollama ou do LM Studio neste Mac.
- Servidores de linguagem e adaptadores de depuração são instalados quando você pede. O gopls e o `dlv` precisam do Go;
  os servidores de TypeScript, Python, JSON, YAML, CSS, HTML e shell e o depurador de JavaScript precisam do Node.js;
  clangd, sourcekit-lsp e `lldb-dap` vêm com as Xcode Command Line Tools.
- As previsões ao digitar precisam de uma assinatura do GitHub Copilot (a gratuita serve) ou de um modelo
  fill-in-the-middle no Ollama ou no LM Studio.

## Janela
- Cada arquivo, terminal, página do navegador, thread de IA, diff e pré-visualização é uma aba do macOS na janela da
  pasta. O “+” no fim da barra de abas e ⌘T oferecem um novo arquivo, um terminal, o navegador e cada agente instalado,
  também em um worktree Git próprio.
- A barra lateral alterna entre Arquivos, Busca, Agentes, Git, CI e Depurar (⌘1–⌘6); um clique com o botão direito
  oculta os que você não usa. A barra de ferramentas tem voltar e avançar pelos lugares que você editou (⌃- e ⌃⇧-), a
  branch, Abrir Rapidamente e a visão geral de Atividade.
- “Abrir ao Lado” mostra um segundo documento ao lado do primeiro. Depois de reiniciar, ou de uma falha, cada janela
  volta com suas abas, e editores com texto não salvo voltam com esse texto.
- Imagens, áudio, vídeo, fontes e documentos do Office abrem em uma aba pelo Quick Look, PDFs pelo PDFKit.

## Editor
- Cores de sintaxe, números de linha, marcas de alteração em relação ao último commit, buscar e substituir, Abrir
  Rapidamente (⌘P) e busca no projeto (⇧⌘F).
- Recuo automático, tabulações ou espaços conforme o `.editorconfig`, pares de colchetes, ativar e desativar
  comentários, dobra de código, ir para linha, espaços em branco visíveis e quebra de linhas.
- Vários cursores: ⌥-clique, ⌥-arrastar para uma coluna, ⌘D para a próxima ocorrência.
- Os arquivos mantêm a codificação, a marca de ordem de bytes e os finais de linha. Um arquivo alterado no disco é
  recarregado quando o editor não tem texto não salvo; caso contrário, uma barra oferece as diferenças e a mesclagem.
- **Previsões ao digitar:** enquanto você digita, uma sugestão aparece em cinza; Tab a aceita. As sugestões vêm do
  GitHub Copilot ou de um modelo local no Ollama ou no LM Studio, conforme você escolher nos Ajustes; um modelo local
  mantém o seu código neste Mac.

## Servidores de linguagem
Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML,
Markdown e Lua, além de servidores próprios definidos nos Ajustes. Diagnósticos, hover, conclusão de código, ajuda de
assinatura, ir para definição, definição de tipo e implementação, referências, hierarquias de chamadas e de tipos,
renomear, formatação, cores semânticas, dobra de código e estrutura; tipos e nomes de parâmetros dentro da linha;
correções rápidas e refatorações atrás de uma lâmpada (⌥⌘A); os símbolos do projeto com ⌥⌘O ou `#` em Abrir Rapidamente.
O próprio `tsc` de um projeto TypeScript é usado quando pode servir como servidor de linguagem.

## Depurador
Pontos de interrupção, também com condições, execução passo a passo, threads com suas pilhas de chamadas, variáveis e
expressões monitoradas no Inspetor (⌥⌘I), valores ao lado do código enquanto o programa está pausado e um console de
depuração que avalia expressões no frame pausado. Rust, C, C++ e Swift pelo `lldb-dap`, Go pelo `dlv`, JavaScript e
TypeScript pelo depurador de JavaScript, Python pelo debugpy. O `.vscode/launch.json` do projeto é lido. O menu Depurar
usa os atalhos do Xcode.

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
você, e uma notificação avisa quando uma sessão termina ou fica esperando enquanto você está em outro lugar.
- **Claude** pelo comando `claude`, sem alterações, com sessões, subagentes, tarefas em segundo plano, contexto e uso.
- **Agentes ACP:** Codex, Antigravity e OpenCode, e outros do registro ACP quando você pedir; os agentes instalados
  ficam fixados em versões testadas e são verificados pelas suas somas de verificação.
- **Agente integrado:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio e servidores compatíveis com a
  API da OpenAI ou da Anthropic, com ferramentas para arquivos, busca, comandos, diagnósticos e requisições web; as
  chaves ficam nas Chaves.
- Um seletor de sandbox por thread limita o que os comandos de um agente podem fazer, na medida em que o agente oferece
  suporte a isso.
- Conteúdo externo, como saída do terminal, uma página da web ou um log de CI, vai para o agente marcado como não
  confiável.

## Ditado
Ditado no campo de mensagem da aba de IA e no editor pelo reconhecimento de fala do Lowspeak: o botão do microfone, ⌃⌥D
ou a tecla fn enquanto o Dango é o app ativo. Ele usa os ajustes e modelos do Lowspeak; a transcrição online precisa de
uma chave da OpenAI ou da Groq.

## Atualizações
O app se atualiza pelo Sparkle. Na segunda abertura, ele pergunta se pode buscar atualizações por conta própria; até
você concordar, ele não faz nenhuma requisição. **Dango › Buscar Atualizações…** busca na hora.

## Acessibilidade
Nomes do VoiceOver para a barra lateral, as abas, os cartões da linha do tempo de IA, as linhas de Git e CI, o depurador
e o texto do terminal; controle total pelo teclado com atalhos que você pode alterar em `keybindings.json`; Aumentar
Contraste e Reduzir Movimento; as visualizações de código continuam da esquerda para a direita em interfaces da direita
para a esquerda. Detalhes em ACCESSIBILITY.md.

## Privacidade
Sem telemetria, sem análises e sem relatórios de falhas. O app só se conecta para o que você inicia: os agentes que você
usa, os downloads que você pede, os hosts de CI em que você inicia sessão, as previsões do Copilot se você as escolheu,
a transcrição online se você a escolheu, as páginas que você abre e a busca de atualizações depois que você a permitiu.
Chaves e tokens ficam nas Chaves do macOS.

## Limitações conhecidas
- O início de sessão no CI pelo navegador está desativado; use `gh`, `glab`, um token ou o ambiente.
- O app é executado sem o App Sandbox, pois terminais, Git, servidores de linguagem e agentes o herdariam; uma pasta
  precisa ser confiável antes que qualquer coisa configurada nela seja executada. O Antigravity e o OpenCode não têm
  sandbox que o app possa ativar.
- Os números de uso e o detalhamento do contexto do Claude dependem de partes do protocolo dele marcadas como
  experimentais e ficam ocultos quando elas não estão presentes. Retroceder a conversa funciona apenas com o Claude; nos
  outros agentes, apenas o código é retrocedido.
- O Gemini CLI não é oferecido: o Google encerrou o início de sessão com conta pessoal e indica o Antigravity. Threads
  anteriores do Gemini continuam legíveis.
