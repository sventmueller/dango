## Separadores de IA
- “Mostrar saída” de um comando em segundo plano abre um separador que acompanha a saída enquanto o comando está em
  execução, em vez de uma cópia dela nesse momento. O separador abre no fim da saída e mantém-se aí à medida que chegam
  novas linhas, até deslocar para cima; ao deslocar de novo até ao fim, volta a acompanhá-la. “Mostrar saída” num
  separador que já está aberto trá-lo para a frente, no fim da saída.
  - Um comando que escreve ele próprio a saída num ficheiro, como `make > build.log 2>&1`, mostra esse ficheiro à medida
    que cresce.
  - Quando o comando termina, o separador mostra toda a saída; “Abrir no editor” abre o ficheiro que está por trás dele.
  - Um separador só lê a saída enquanto está à frente.
