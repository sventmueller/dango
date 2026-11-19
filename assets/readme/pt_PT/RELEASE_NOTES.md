## Memória
- As sessões longas de agentes precisam de muito menos memória:
  - Ao abrir uma sessão anterior, a respetiva transcrição é lida uma linha de cada vez; uma sessão com uma transcrição
    de 100 MB atinge agora, no máximo, cerca de 20 MB em vez de 65 MB.
  - Uma chamada de ferramenta concluída guarda apenas o que o respetivo cartão mostra: as primeiras 400 linhas da saída,
    e deixa de guardar a entrada em bruto assim que o cartão mostra os seus campos. “Mais N linhas” continua a contar
    todas as linhas.
  - Um separador de IA que fique fora de vista durante 30 segundos liberta os seus cartões e, quando volta a ser
    mostrado, constrói os que estão à vista, no ponto onde estava.
  - Depois de um separador ser fechado ou de uma sessão anterior ser aberta, o Dango devolve ao macOS a memória que
    libertou.
- As listas já não crescem na memória ao serem atualizadas. No macOS 27, cada atualização de uma lista com linhas
  expansíveis deixava para trás, de vez, um triângulo de expansão, e cada atualização deixava para trás um pouco de cada
  linha:
  - As listas de CI, Contentores, Software Factory, Testes, Depurar, Git e Pesquisa e a hierarquia de chamadas desenham
    as suas próprias setas.
  - As listas fora de vista já não são atualizadas; põem-se em dia quando são mostradas.
  - As listas Sessões, CI, Software Factory e Contentores mantêm as linhas que não mudaram, e o Git atualiza apenas as
    linhas dos ficheiros cujo estado mudou.
- Os painéis de uma janela fechada são libertados com ela.

## Separadores de IA
- “Mostrar saída” de um comando em segundo plano que escreve ele próprio a saída num ficheiro, como
  `make > build.log 2>&1`, mostra esse ficheiro quando a CLI não tem nada da saída.
