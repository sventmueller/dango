## Seguridad
- Los enlaces de las respuestas, los planes y los informes de un agente, de la vista previa de Markdown y de los PDF ya
  no abren cualquier app con un solo clic. Las direcciones web se abren en una pestaña de navegador, y los archivos del
  proyecto, en una pestaña de editor; todo lo demás, como un archivo de otro lugar, un recurso compartido de red o el
  enlace de otra app, pregunta primero y muestra adónde lleva.
- Las direcciones de inicio de sesión y de Remote Control que entrega un agente o un servidor MCP solo se abren si son
  direcciones web.
- Ya no se puede confiar en su conjunto en las carpetas que contienen mucho más que un proyecto, como tu carpeta de
  inicio, Escritorio, Documentos, Descargas o la raíz de un volumen; confía en ellas proyecto a proyecto. La pregunta de
  confianza dice ahora que también abarca las subcarpetas.
- Los tokens de CI y los inicios de sesión ya nunca salen por http sin cifrar hacia un servidor que no sea este Mac; a
  un repositorio cuyo remoto empieza por `http://` se le pregunta por https.
- Sin gitleaks, las predicciones esperan, antes de que el código vaya a un proveedor fuera de este Mac, hasta que eliges
  enviar sin comprobar. Los ajustes de gitleaks propios de un proyecto ya no desactivan la comprobación.
- Ya no se puede llevar al agente propio a escribir fuera del proyecto mediante un enlace que aparece después de
  aprobarse su escritura, y los archivos con más enlaces físicos reciben un archivo propio en lugar de escribirse a
  través de ellos.
- El autocompletado de un servidor de lenguaje ya no puede bloquear el editor, y sus cambios de nombre y eliminaciones
  de un archivo enlazado actúan sobre el enlace, no sobre su destino.
- Las imágenes que adjuntas a un prompt las decodifica el servicio de decodificación aislado, como en todas partes.
- Un programa del terminal solo puede reemplazar el portapapeles mientras su terminal está delante, y el terminal lo
  indica.
- rust-analyzer se instala desde una versión fijada con su suma de comprobación en lugar de la más reciente del momento.

## Búsqueda y navegación
- Abrir rápidamente indica cuándo no coincide ningún archivo y muestra si busca archivos o los símbolos del proyecto.
- Los símbolos del proyecto incluyen TypeScript y JavaScript sin que esté abierto ningún archivo suyo. Los resultados
  exactos del proyecto y del lenguaje en el que trabajas aparecen primero; los símbolos de bibliotecas y paquetes
  instalados están marcados.
- Visualización › Búsqueda pone el foco en el campo de búsqueda.

## Editor
- El editor conserva el foco cuando un archivo Markdown se abre con su vista previa al lado, de modo que el menú Formato
  funciona enseguida. Los comandos del editor también funcionan mientras el foco está en la barra lateral.
- Una propuesta al final de una línea o en un recuadro debajo indica que la tecla Tab la acepta.
- Las pestañas con el mismo nombre en la vista general de pestañas se distinguen, por ejemplo un archivo y su vista
  previa en el navegador.
- El menú Visualización ya no ofrece «Ocultar barra de pestañas» de macOS, que las pestañas de Dango no usan.

## Lenguajes y depuración
- El servidor de JSON ahora comprueba los archivos, y los servidores que registran sus diagnósticos después de
  iniciarse, como basedpyright y sourcekit-lsp, los muestran.
- Donde pip se niega a instalar en Python, como en el de Homebrew, el depurador ofrece instalar debugpy en un entorno
  virtual del proyecto.
