## Segurança
- As hiperligações nas respostas, planos e relatórios de um agente, na pré-visualização de Markdown e nos PDF já não
  abrem nenhuma aplicação com um único clique. Os endereços web abrem num separador do navegador, os ficheiros dentro
  do projeto num separador do editor; tudo o resto, como um ficheiro noutro local, uma partilha de rede ou a
  hiperligação de outra aplicação, pergunta primeiro e mostra para onde leva.
- Os endereços de início de sessão e de Remote Control que um agente ou um servidor MCP entrega só abrem quando são
  endereços web.
- As pastas que contêm muito mais do que um projeto, como a pasta pessoal, a Secretária, Documentos, Descargas ou a
  raiz de um volume, já não podem ser consideradas de confiança como um todo; confie nelas projeto a projeto. A pergunta
  de confiança indica agora que esta também se aplica às pastas no seu interior.
- Os tokens de CI e os inícios de sessão nunca saem por http simples para um servidor que não seja este Mac; um
  repositório cujo remoto começa por `http://` é consultado por https.
- Sem o gitleaks, as previsões de edição esperam antes de o código ir para um fornecedor fora deste Mac, até que opte
  por enviar sem verificação. As definições do gitleaks do próprio projeto já não desativam a verificação.
- O agente integrado já não pode ser levado a escrever fora do projeto através de uma hiperligação que aparece depois
  de a sua escrita ter sido aprovada, e os ficheiros com outras ligações físicas recebem um ficheiro próprio em vez de
  serem escritos através da ligação.
- As conclusões de um servidor de linguagem já não podem bloquear o editor, e as alterações de nome e eliminações de um
  ficheiro ligado feitas por ele atuam sobre a ligação, não sobre aquilo para onde aponta.
- As imagens que anexa a um prompt são descodificadas pelo serviço de descodificação em sandbox, como em todo o lado.
- Um programa no terminal só pode substituir o clipboard enquanto o respetivo terminal está à frente, e o terminal
  indica-o.
- O rust-analyzer é instalado a partir de uma versão fixada com o respetivo checksum, em vez da que fosse a mais
  recente.

## Pesquisa e navegação
- Abrir rapidamente indica quando nenhum ficheiro corresponde e mostra se está a procurar ficheiros ou os símbolos do
  projeto.
- Os símbolos do projeto incluem TypeScript e JavaScript sem que nenhum dos seus ficheiros esteja aberto. As
  correspondências exatas do projeto e na linguagem em que trabalha aparecem primeiro; os símbolos de bibliotecas e de
  pacotes instalados são assinalados.
- Visualização › Pesquisa coloca o foco no campo de pesquisa.

## Editor
- O editor mantém o foco quando um ficheiro Markdown abre com a pré-visualização ao lado, pelo que o menu Formatação
  funciona de imediato. Os comandos do editor também funcionam enquanto o foco está na barra lateral.
- Uma proposta no fim de uma linha ou numa caixa por baixo dela indica que a tecla Tab a aceita.
- Os separadores com o mesmo nome na vista geral dos separadores distinguem-se, por exemplo um ficheiro e a respetiva
  pré-visualização no navegador.
- O menu Visualização já não oferece o “Ocultar barra de separadores” do próprio macOS, que os separadores do Dango não
  usam.

## Linguagens e depuração
- O servidor JSON verifica agora os ficheiros, e os servidores que registam os seus diagnósticos depois de arrancar,
  como o basedpyright e o sourcekit-lsp, mostram-nos.
- Quando o pip se recusa a instalar no Python, como acontece com o do Homebrew, o depurador propõe instalar o debugpy
  num ambiente virtual do projeto.
