## Terminal
- O terminal agora é o do Tapa, o app de terminal irmão do Dango, com tudo o que ele traz, enquanto o Dango mantém o que o terminal dele já fazia: colar imagens para agentes, servidores de desenvolvimento na aba do navegador, ⌘-clique que abre no editor.
- **Visualizar › Mostrar/Ocultar Campo de Entrada** (⌘I enquanto um terminal tem o foco) transforma a linha de comando em um campo de texto com a pasta e a branch do Git acima dele. Os comandos são coloridos por partes, e um que o seu shell não consegue executar ganha um sublinhado vermelho tracejado.
- As sugestões aparecem em cinza enquanto você digita, no campo e sem ele: do seu histórico, de especificações para mais de 700 ferramentas de linha de comando e do projeto, como branches do git, scripts do `package.json` e alvos do make, do just e do cargo. → aceita uma; Tab completa e, com vários candidatos, abre um menu que a digitação filtra. Elas nunca usam a rede.
- Blocos de comando: ⌘↑ e ⌘↓ pulam entre os prompts, o menu **Editar** seleciona, copia e salva a saída de um comando, e marcadores marcam linhas. Com **Marcar comandos como blocos** ativado, uma barra mostra cada bloco, comandos que falharam ficam marcados e botões sobre um bloco mostram o status e a duração dele.
- **Enviar Bloco para o Agente** (menu Editar e menu de contexto do terminal) abre uma aba de IA com a linha de comando e a saída do bloco atual anexadas como Conteúdo Externo.
- **Buscar** (⌘F enquanto um terminal tem o foco) pesquisa todo o histórico, com expressões regulares, maiúsculas e minúsculas e palavras inteiras; a barra de rolagem marca os resultados.
- A notificação de um programa (OSC 9 e OSC 777) e a campainha dele põem o ponto vermelho na aba do terminal enquanto você não o vê, e a notificação vai para o macOS enquanto outro app está em primeiro plano. O progresso que um programa informa (OSC 9;4) aparece como um anel na aba dele e como uma barra no ícone do Dock.
- Um clique duplo seleciona por inteiro uma URL, um caminho ou um hash, arrastar com ⌥ seleciona um retângulo, **Copiar com Formatação** mantém cores e estilos, e **Selecionar com o Teclado** move uma seleção com as teclas do Vim.
- Colagens de várias linhas que seriam executadas de imediato aparecem primeiro, e **Dango › Entrada de Teclado Segura** impede que outros apps leiam o que você digita em um terminal.
- O terminal não abre mais links de tipos que não sejam endereços web, `mailto:` e arquivos.

## Ajustes
- **Ajustes › Terminal** liga e desliga o campo de entrada para novos terminais, as teclas do Vim dele, as sugestões, as cores dos comandos, o sublinhado dos desconhecidos, os blocos de comando, copiar a seleção imediatamente, mostrar antes colagens de várias linhas e ⌥ como Metatecla, que veio de Geral para cá.
