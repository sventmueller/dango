## Velocidade e memória
- Tudo o que um agente inicia, em qualquer linguagem, como builds, testes, shells e servidores MCP, é executado no
  estado de segundo plano do macOS: com a prioridade mais baixa e com acesso limitado a disco e rede. Os próprios
  agentes, com os subagentes do Claude, são executados com prioridade média. Agentes em carga máxima não deixam mais o
  Dango lento; isso também vale para os agentes da Software Factory.
- Janelas restauradas iniciam apenas o que aparece: uma aba de IA atrás de outras inicia sua CLI quando vem para a
  frente ou recebe um prompt, e abas de navegador, de pré-visualização de site e de terminal carregam sua página ou
  iniciam seu shell quando aparecem pela primeira vez.
- Uma busca longa no projeto não redesenha mais todos os resultados enquanto é executada. A Software Factory, os logs de
  CI e o painel Testes fazem menos enquanto não estão visíveis; testes e configurações de depuração são lidos quando o
  painel deles aparece pela primeira vez, e os painéis CI e Contêineres ocultos não leem nada até aparecerem.
- Sessões longas precisam de menos memória: um diff concluído mantém apenas as linhas que o seu cartão mostra, uma
  solicitação de permissão respondida libera sua entrada, o cache de glifos do terminal não cresce mais sem limite, e os
  painéis Arquivos e Git liberam linhas que não existem mais.
- O programa em si está cerca de 40% menor.

## Janelas e menus
- Fechar uma área move as abas dela para a área anterior ou, se não houver, para a seguinte, em vez de fechá-las.
  Somente a área de uma pré-visualização e a última área fecham junto com suas abas.
- O menu Visualizar lista os painéis na ordem dos símbolos da barra lateral.
- “Gerenciar Agentes…” saiu do menu +; ele continua em “+ Nova Sessão” no painel Agentes.

## Correções
- “Incluir e Excluir Arquivos” no painel de busca não mostra mais uma letra perdida ao lado da seta.
- Fechar o popover de comentários de uma especificação não causa mais uma falha.
