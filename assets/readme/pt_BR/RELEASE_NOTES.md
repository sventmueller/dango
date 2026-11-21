## Segurança
- Links nas respostas, planos e relatórios de um agente, na pré-visualização de Markdown e em PDFs não abrem mais
  nenhum app com um único clique. Endereços web abrem em uma aba do navegador, arquivos dentro do projeto em uma aba do
  editor; qualquer outra coisa, como um arquivo em outro lugar, um compartilhamento de rede ou o link de outro app,
  pergunta antes e mostra aonde leva.
- Endereços de início de sessão e de Remote Control que um agente ou um servidor MCP entrega só abrem quando são
  endereços web.
- Pastas que contêm muito mais do que um projeto, como sua pasta pessoal, Mesa, Documentos, Downloads ou a raiz de um
  volume, não podem mais receber confiança como um todo; confie nelas projeto por projeto. A pergunta de confiança
  agora diz que ela também vale para as pastas dentro.
- Tokens de CI e inícios de sessão nunca saem por http simples para um servidor que não seja este Mac; um repositório
  cujo remoto começa com `http://` é consultado por https.
- Sem o gitleaks, as previsões de edição esperam antes que o código vá para um provedor fora deste Mac, até que você
  escolha enviar sem verificação. As configurações próprias do gitleaks em um projeto não desativam mais a verificação.
- O agente integrado não pode mais ser levado a escrever fora do projeto por meio de um link que aparece depois que sua
  escrita foi aprovada, e arquivos com outros links físicos ganham um arquivo próprio em vez de serem escritos através
  do link.
- As conclusões de um servidor de linguagem não podem mais travar o editor, e as renomeações e exclusões de um arquivo
  vinculado feitas por ele atuam sobre o link, não sobre aquilo para onde ele aponta.
- Imagens que você anexa a um prompt são decodificadas pelo serviço de decodificação em sandbox, como em todos os
  outros lugares.
- Um programa no terminal só pode substituir a área de transferência enquanto seu terminal está na frente, e o terminal
  avisa isso.
- O rust-analyzer é instalado a partir de uma versão fixada com sua soma de verificação, em vez da que fosse a mais
  recente.

## Busca e navegação
- Abrir Rapidamente avisa quando nenhum arquivo corresponde e mostra se está buscando arquivos ou os símbolos do
  projeto.
- Os símbolos do projeto incluem TypeScript e JavaScript sem que um arquivo deles esteja aberto. Correspondências
  exatas do projeto e na linguagem em que você trabalha vêm primeiro; símbolos de bibliotecas e de pacotes instalados
  são marcados.
- Visualizar › Busca coloca o foco no campo de busca.

## Editor
- O editor mantém o foco quando um arquivo Markdown abre com a pré-visualização ao lado, então o menu Formatar funciona
  na hora. Os comandos do editor também funcionam enquanto o foco está na barra lateral.
- Uma sugestão no fim de uma linha ou em uma caixa abaixo dela informa que a tecla Tab a aceita.
- Abas com o mesmo nome na visão geral das abas são diferenciadas, por exemplo um arquivo e sua pré-visualização no
  navegador.
- O menu Visualizar não oferece mais o “Ocultar Barra de Abas” do próprio macOS, que as abas do Dango não usam.

## Linguagens e depuração
- O servidor JSON agora verifica arquivos, e servidores que registram seus diagnósticos depois de iniciar, como o
  basedpyright e o sourcekit-lsp, os mostram.
- Onde o pip se recusa a instalar no Python, como faz o do Homebrew, o depurador oferece instalar o debugpy em um
  ambiente virtual do projeto.
