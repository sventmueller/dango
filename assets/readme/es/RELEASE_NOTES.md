Esta es la primera versión de Dango, un editor nativo de macOS para trabajar con agentes de programación. Editor,
terminales, un navegador, Git, CI, un depurador, hilos de IA y una Software Factory para órdenes de trabajo comparten
una ventana por carpeta. La app está escrita en Rust sobre AppKit; WebKit solo aparece en la pestaña del navegador. La
interfaz está disponible en 35 idiomas.

## Requisitos
- macOS 14 o posterior, en Apple silicon o Intel.
- Para los hilos de IA, el agente que elijas: Claude necesita el comando `claude` con la sesión iniciada; Codex necesita
  Node.js, y la app instala su paquete de versión fijada cuando lo pidas; Antigravity y OpenCode usan tu propio `agy` u
  `opencode`, o una descarga de versión fijada; el agente propio necesita una clave de API, o bien Ollama o LM Studio en
  este Mac.
- Los servidores de lenguaje y los adaptadores de depuración se instalan cuando lo pidas. gopls y `dlv` necesitan Go;
  los servidores de TypeScript, Python, JSON, YAML, CSS, HTML, ESLint y shell y el depurador de JavaScript necesitan
  Node.js; clangd, sourcekit-lsp y `lldb-dap` vienen con las Xcode Command Line Tools.
- Las predicciones de edición necesitan una suscripción a GitHub Copilot (basta con la gratuita), o un modelo
  fill-in-the-middle en Ollama o LM Studio.

## Ventana
- Cada archivo, terminal, página del navegador, hilo de IA, diff y vista previa es una pestaña de macOS en la ventana de
  la carpeta. El «+» al final de la barra de pestañas y ⌘T ofrecen un archivo nuevo, un terminal, el navegador y cada
  agente instalado, también en un worktree de Git propio.
- La barra lateral cambia entre Archivos, Búsqueda, Agentes, Git, Depurar, CI, Pruebas, Contenedores y la Software
  Factory; con un clic secundario ocultas los que no uses. La barra de herramientas reúne atrás y adelante por los
  lugares que has editado (⌃- y ⌃⇧-), la rama, Abrir rápidamente y el resumen de Actividad.
- Una ventana se divide en áreas una al lado de otra o una encima de otra, anidadas a la profundidad que quieras, como
  en Xcode; cada área guarda sus documentos uno detrás de otro con una barra de pestañas propia. «Abrir al lado» muestra
  un segundo documento junto al primero.
- Tras un reinicio, o un fallo, cada ventana vuelve con sus pestañas y sus áreas, y los editores con texto sin guardar
  vuelven con ese texto.
- Las imágenes, el audio, el vídeo, las tipografías y los documentos de Office se abren en una pestaña con Quick Look, y
  los PDF con PDFKit.

## Editor
- Colores de sintaxis, números de línea, marcas de cambio respecto al último commit, buscar y reemplazar, Abrir
  rápidamente (⌘P) y búsqueda en el proyecto (⇧⌘F). Los colores y los estilos de Markdown siguen el ritmo incluso en
  archivos de muchos miles de líneas sin frenar la escritura.
- Sangría automática, tabulaciones o espacios según `.editorconfig`, pares de corchetes, activar y desactivar
  comentarios, plegado, ir a una línea, espacios en blanco visibles, ajuste de líneas, un minimapa y encabezados fijos
  de los ámbitos que lo envuelven.
- Varios cursores: ⌥-clic, ⌥-arrastrar para una columna, ⌘D para la siguiente aparición.
- **Modo Vim:** se activa en el menú Editor, con los modos normal, inserción y visual, movimientos y operadores,
  registros, marcas, macros, búsqueda y comandos Ex; sus mensajes aparecen en el idioma de la interfaz.
- Abreviaturas de Emmet en el autocompletado de HTML, CSS y JSX, y los problemas de ESLint corregidos al guardar si
  quieres.
- Los archivos conservan su codificación, su marca de orden de bytes y sus finales de línea. Un archivo que cambia en el
  disco se vuelve a cargar si el editor no tiene texto sin guardar; si no, una barra ofrece las diferencias y una
  fusión.
- **Predicciones de edición:** mientras escribes, aparece una propuesta en gris; la tecla Tab la acepta. Vienen de
  GitHub Copilot o de un modelo local en Ollama o LM Studio, según lo que elijas en los ajustes; un modelo local
  mantiene tu código en este Mac.

## Servidores de lenguaje
Go, Rust, TypeScript y JavaScript, Python, C, C++ y Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML,
Markdown y Lua, ESLint junto al servidor de TypeScript, además de servidores propios desde los ajustes. Diagnósticos,
también al final de la línea, información al pasar el puntero, autocompletado, ayuda de firmas, ir a la definición, a
la definición de tipo y a la implementación, referencias, jerarquías de llamadas y de tipos, cambio de nombre, formato,
colores semánticos, plegado y el esquema; tipos y nombres de parámetros dentro de la línea; correcciones rápidas y
refactorizaciones tras una bombilla (⌥⌘A); los símbolos del proyecto con ⌥⌘O o `#` en Abrir rápidamente. El `tsc`
propio de un proyecto TypeScript se usa cuando puede servir de servidor de lenguaje.

## Depurador
Puntos de interrupción, también con condiciones, ejecución paso a paso, hilos de ejecución con sus pilas de llamadas,
variables y expresiones vigiladas en el Inspector (⌥⌘I), valores junto al código mientras el programa está en pausa, y
una consola de depuración que evalúa en el frame en pausa. Rust, C, C++ y Swift mediante `lldb-dap`, Go mediante `dlv`,
JavaScript y TypeScript mediante el depurador de JavaScript, Python mediante debugpy. Se lee el `.vscode/launch.json`
del proyecto. El menú Depurar usa los atajos de Xcode.

## Pruebas y contenedores
- El panel Pruebas muestra las pruebas de los proyectos de Rust, Go, Python, Jest, Vitest y Bun antes de la primera
  ejecución, ejecuta una, un archivo o todas, muestra cada resultado en directo y lo marca en el margen del editor.
- El panel Contenedores muestra los contenedores, las imágenes, los volúmenes y los proyectos de Compose de Docker; los
  inicia, los detiene y los elimina, sigue sus logs y abre una shell en una pestaña de terminal.

## Terminal
Terminales en pestañas con el entorno de la shell de inicio de sesión, dibujados con Metal. Pegar o soltar una imagen la
entrega a un agente de programación que se ejecuta en el terminal, o pega la ruta de una copia guardada. ⌘-clic abre
enlaces y nombres de archivo. Un servidor de desarrollo que inicia el terminal se puede abrir en la pestaña del
navegador.

## Markdown
Un archivo Markdown se abre con su vista previa al lado (⇧⌘V), con fórmulas, diagramas de Mermaid, Graphviz y PlantUML y
las imágenes de la carpeta. Un menú Formato para negrita, cursiva, enlaces, encabezados, listas, citas y bloques de
código; el texto pegado desde Word o una página web llega como Markdown. **Archivo › Exportar** guarda en PDF, en un
único archivo HTML o en un documento de Word. Si Splat está instalado, **Archivo › Abrir en Splat** abre allí el
documento.

## Navegador
Una pestaña de navegador con campo de dirección, atrás y adelante, búsqueda, descargas que preguntan antes, y el Web
Inspector.

## Git
Un panel de Git para preparar y descartar archivos o fragmentos sueltos, commits, hacer commit y push o sincronizar,
ramas, stashes, conflictos de fusión y un gráfico de los commits recientes. Blame muestra quién cambió por última vez la
línea del cursor, o una columna para todo el archivo. Un botón escribe el mensaje del commit con IA; el diff solo sale
después de que gitleaks no haya encontrado ningún secreto en él, y cada commit también se revisa en busca de secretos.
Un hilo de IA puede trabajar en un worktree de Git propio; el panel muestra lo que ha cambiado e incorpora el trabajo
con una fusión o un cherry-pick.

## CI
GitHub Actions y GitLab CI, también GitHub Enterprise Server y GitLab autogestionado: ejecuciones, jobs y logs de la
rama actual; volver a ejecutar los jobs fallidos, un solo job o (en GitHub) una ejecución entera, iniciar jobs manuales
o aprobar despliegues en espera, iniciar una pipeline nueva o un workflow que se puede iniciar a mano, y cancelar. Los
merge requests o pull requests y los issues abiertos aparecen en el mismo panel y se abren en la pestaña del navegador.
El log de un job fallido se puede enviar a un agente, marcado como texto externo. El token sale del llavero, de `gh` o
`glab`, del entorno de la shell de inicio de sesión, o lo introduces tú.

## IA
Cada agente trabaja en una pestaña de IA con el mismo historial: respuestas, llamadas a herramientas con su salida,
solicitudes de permiso, preguntas, planes para aprobar y comentar, pendientes, subagentes y tareas en segundo plano; dos
botones en la cabecera de la pestaña muestran los subagentes y los procesos en segundo plano en ejecución. Antes de cada
prompt, la app guarda un punto de control; los cambios se pueden revisar y rebobinar por mensaje. Los hilos nuevos
empiezan en el modo «Manual», y los comandos de Git destructivos nunca se permiten de forma permanente. El resumen de
Actividad muestra todo lo que se está ejecutando. Un punto verde en la pestaña indica una sesión trabajando, uno rojo
una sesión que te espera, y una notificación te avisa cuando termina o espera mientras miras otra cosa. El historial,
el campo de mensaje y la línea de estado tienen su propio tamaño de letra en los ajustes.
- **Claude** mediante el comando `claude`, sin cambios, con sesiones, subagentes, tareas en segundo plano, contexto, uso
  y Remote Control desde el teléfono.
- **Codex** mediante su servidor de app y **OpenCode** mediante su servidor, cada uno en su propio protocolo, con sus
  hilos, el rebobinado en el propio agente y Remote Control para Codex.
- **Agentes ACP:** Antigravity, y más del registro de ACP cuando lo pidas; los agentes instalados están fijados a
  versiones probadas y se comprueban con sus sumas de comprobación.
- **Agente propio:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio y servidores que hablan la API de
  OpenAI o de Anthropic, con herramientas para archivos, búsqueda, comandos, diagnósticos y solicitudes web; las claves
  se quedan en el llavero.
- Los comandos propios de un agente se abren como superposiciones de la pestaña, igual para todos los agentes: estado,
  exportación, servidores MCP, permisos, hooks, memoria, configuración, skills, plugins, modelo, cuenta y comentarios.
- Un interruptor de sandbox por hilo limita lo que pueden hacer los comandos de un agente, en la medida en que el agente
  lo admita.
- El texto externo, como la salida del terminal, una página web o un log de CI, llega al agente marcado como no fiable.

## Software Factory
Órdenes de trabajo en lugar de prompts sueltos: un objetivo con criterios de aceptación, escrito a mano o tomado de un
issue. Cada orden se planifica, se construye y se comprueba en un worktree de Git propio, a cargo de agentes de
distintos fabricantes en roles fijos: un planificador escribe la especificación, un constructor hace el trabajo y un
revisor de otro fabricante comprueba cada criterio. Los comandos de control del proyecto, como la compilación y las
pruebas, se ejecutan en un sandbox, y el constructor corrige lo que encuentren.
- Decides en dos momentos: apruebas la especificación y aceptas el resultado con su evidencia, entero o sección por
  sección. No se incorpora nada sin tu aceptación.
- El panel y la pestaña de la Software Factory muestran cada orden con su etapa, su coste y lo que espera; la pestaña de
  una orden muestra su historial, su evidencia y el siguiente paso, y abre su sesión en curso como pestaña de IA.
- Presupuestos por orden y por día, límites de turnos, una pausa al salir, en reposo y con la batería baja, y un log
  escrito antes de cada efecto, de modo que las órdenes se reanudan tras un reinicio.
- Los issues con una etiqueta elegida pueden entrar como órdenes; una orden aceptada puede salir como merge request con
  un comentario en su issue, con un clic tuyo. Las órdenes con ámbitos de rutas separados se ejecutan en paralelo y se
  incorporan una tras otra.
- Métricas por agente y fabricante, enrutamiento según la tasa de primera pasada, propuestas de reglas para `AGENTS.md`
  y `CLAUDE.md` a partir de hallazgos recurrentes, y órdenes recurrentes desde el archivo de flujo de trabajo del
  proyecto.
- El archivo de flujo de trabajo `.dango/factory.toml` de una carpeta contiene sus controles, sus clases de riesgo y sus
  ámbitos de rutas; los comandos de control cambiados y todo lo que afloje los límites esperan tu confirmación.

## Dictado
Dictado en el campo de mensaje de IA y en el editor con el reconocimiento de voz de Lowspeak: el botón del micrófono,
⌃⌥D o la tecla fn mientras Dango es la app activa. Usa los ajustes y los modelos de Lowspeak; la transcripción en línea
necesita una clave de OpenAI o de Groq.

## Actualizaciones
La app se actualiza sola con Sparkle. En el segundo inicio pregunta si puede buscar actualizaciones por su cuenta; hasta
que aceptes, no hace ninguna solicitud. **Dango › Buscar actualizaciones…** las busca al momento.

## Accesibilidad
Nombres de VoiceOver para la barra lateral, las pestañas, las tarjetas del historial de IA, las filas de Git y CI, la
Software Factory, el depurador y el texto del terminal; control completo con el teclado con atajos que puedes cambiar
en `keybindings.json`; Aumentar contraste y Reducir movimiento; las vistas de código siguen de izquierda a derecha en
interfaces de derecha a izquierda. Los detalles están en ACCESSIBILITY.md.

## Privacidad
Sin telemetría, sin analíticas y sin informes de fallos. La app solo se conecta para lo que tú inicias: los agentes que
usas, las descargas que pides, los hosts de CI en los que inicias sesión, las predicciones de edición de Copilot si las
elegiste, la transcripción en línea si la elegiste, las páginas que abres y la búsqueda de actualizaciones una vez que
la hayas permitido. Las claves y los tokens se quedan en el llavero de macOS, y los agentes de la Software Factory
nunca los ven.

## Limitaciones conocidas
- El inicio de sesión en CI a través del navegador está desactivado; usa `gh`, `glab`, un token o el entorno.
- La app funciona sin App Sandbox, ya que los terminales, Git, los servidores de lenguaje y los agentes lo heredarían;
  una carpeta debe ser de confianza antes de que se ejecute nada de lo que configura. Antigravity y OpenCode no tienen
  ningún sandbox que la app pueda activar.
- En la Software Factory, un constructor o un revisor de Codex puede leer fuera de su worktree, ya que el protocolo de
  Codex no tiene límites de lectura; lo que escribe se queda dentro.
- Las cifras de uso de Claude y el desglose del contexto dependen de partes de su protocolo marcadas como experimentales
  y se ocultan cuando faltan. Rebobinar la conversación funciona con Claude, Codex y OpenCode; con los demás agentes
  solo se rebobina el código.
- Gemini CLI no se ofrece: Google dejó de permitir el inicio de sesión con una cuenta personal y remite a Antigravity.
  Los hilos de Gemini anteriores siguen siendo legibles.
