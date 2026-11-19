## Memória
- Sessões longas de agentes precisam de muito menos memória:
  - Abrir uma sessão anterior lê a transcrição dela uma linha por vez; uma sessão com uma transcrição de 100 MB agora
    atinge no máximo cerca de 20 MB em vez de 65 MB.
  - Uma chamada de ferramenta concluída guarda apenas o que seu cartão mostra: as primeiras 400 linhas da saída, e não
    mais a entrada bruta assim que o cartão mostra seus campos. “Mais N linhas” continua contando todas as linhas.
  - Uma aba de IA que fica fora de vista por 30 segundos libera seus cartões e, quando volta a aparecer, constrói os que
    estão à vista, no ponto em que estava.
  - Depois que uma aba é fechada ou uma sessão anterior é aberta, o Dango devolve ao macOS a memória que liberou.
- As listas não crescem mais na memória ao serem atualizadas. No macOS 27, cada atualização de uma lista com linhas
  expansíveis deixava para trás, para sempre, um triângulo de expansão, e cada atualização deixava para trás um pouco de
  cada linha:
  - As listas de CI, Contêineres, Software Factory, Testes, Depurar, Git e Busca e a hierarquia de chamadas desenham as
    próprias setas.
  - Listas fora de vista não são mais atualizadas; elas se atualizam quando aparecem.
  - As listas Sessões, CI, Software Factory e Contêineres mantêm as linhas que não mudaram, e o Git atualiza apenas as
    linhas dos arquivos cujo estado mudou.
- Os painéis de uma janela fechada são liberados com ela.

## Abas de IA
- “Mostrar Saída” de um comando em segundo plano que grava ele mesmo a saída em um arquivo, como
  `make > build.log 2>&1`, mostra esse arquivo quando a CLI não tem nada da saída.
