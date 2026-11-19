## Velocidade e memória
- Tudo o que um agente inicia, em qualquer linguagem, como builds, testes, shells e servidores MCP, é executado no
  estado de segundo plano do macOS: com a prioridade mais baixa e com acesso limitado ao disco e à rede. Os próprios
  agentes, com os subagentes do Claude, são executados com prioridade média. Agentes em carga máxima já não tornam o
  Dango mais lento; o mesmo se aplica aos agentes da Software Factory.
- As janelas restauradas só iniciam o que está visível: um separador de IA atrás de outros inicia a respetiva CLI quando
  passa para a frente ou recebe um prompt, e os separadores de navegador, de pré-visualização de site e de terminal
  carregam a página ou iniciam a shell quando são mostrados pela primeira vez.
- Uma pesquisa longa no projeto já não volta a desenhar todos os resultados enquanto decorre. A Software Factory, os
  logs de CI e o painel Testes fazem menos enquanto não estão visíveis; os testes e as configurações de depuração são
  lidos quando o respetivo painel é mostrado pela primeira vez, e os painéis CI e Contentores ocultos não leem nada até
  serem mostrados.
- As sessões longas precisam de menos memória: um diff concluído guarda apenas as linhas que o respetivo cartão mostra,
  um pedido de permissão respondido liberta os dados introduzidos, a cache de glifos do terminal já não cresce sem
  limite e os painéis Ficheiros e Git libertam as linhas que já não existem.
- O próprio programa é cerca de 40% mais pequeno.

## Janelas e menus
- Fechar uma área move os respetivos separadores para a área anterior ou, se não existir, para a seguinte, em vez de os
  fechar. Só a área de uma pré-visualização e a última área fecham com os respetivos separadores.
- O menu Visualização apresenta os painéis pela ordem dos símbolos da barra lateral.
- “Gerir agentes…” saiu do menu +; continua em “+ Nova sessão” no painel Agentes.

## Correções
- “Incluir e excluir ficheiros” no painel de pesquisa já não mostra uma letra perdida junto à seta.
- Fechar o popover de comentários de uma especificação já não provoca uma falha.
