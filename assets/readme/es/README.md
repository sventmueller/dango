<p><img src="../../readme-icon.png" alt="Icono de Dango" width="147" height="147"></p>

# Dango

**El editor nativo de macOS para trabajar con agentes de programación.**

Dango reúne el código, los agentes que lo cambian y todo lo que los rodea en una ventana por carpeta: un editor con servidores de lenguaje y predicciones mientras escribes, terminales, un navegador, Git, CI, un depurador e hilos de IA con Claude, Codex, Antigravity, OpenCode o un agente integrado en la app. Cada agente trabaja en la misma línea de tiempo, pregunta antes de cambiar archivos o ejecutar comandos y deja un punto de control antes de cada prompt, para que puedas revisar y rebobinar lo que ha hecho, mensaje a mensaje. Nada de lo que configura una carpeta se ejecuta antes de que confíes en ella, y el texto de fuera llega a un agente marcado como no fiable. Dicta en el editor y en el campo de mensaje; el reconocimiento de voz puede quedarse en tu Mac. Dango está escrito en Rust sobre controles reales de AppKit, sin Electron, habla 35 idiomas y no tiene telemetría.

[![Descargar Dango.dmg](https://img.shields.io/badge/Descargar_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Descargar Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: firmado y notarizado por Apple, para macOS 14 o posterior en Apple Silicon e Intel. El enlace estará disponible con la primera publicación.

**¿Has encontrado un error?** [Abre un issue](https://github.com/sventmueller/dango/issues/new) e indica con qué agente y en qué tipo de proyecto ocurrió.

[![Rust, edición 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 o posterior](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon e Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit mediante objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 idiomas de interfaz](https://img.shields.io/badge/languages-35-4C9A2A)
![Sin telemetría](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Firmado con Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licencia: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licencia)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
**Español** ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Contenido

- [Funciones de un vistazo](#funciones-de-un-vistazo)
- [Requisitos](#requisitos)
- [Instalación](#instalación)
- [Primeros pasos](#primeros-pasos)
- [Agentes de IA](#agentes-de-ia)
  - [La línea de tiempo](#la-línea-de-tiempo)
  - [Permisos y modos](#permisos-y-modos)
  - [Puntos de control y worktrees](#puntos-de-control-y-worktrees)
- [Editor](#editor)
- [Servidores de lenguaje](#servidores-de-lenguaje)
- [Pruebas](#pruebas)
- [Depurador](#depurador)
- [Terminal](#terminal)
- [Navegador](#navegador)
- [Git](#git)
- [CI](#ci)
- [Contenedores](#contenedores)
- [Markdown](#markdown)
- [Dictado](#dictado)
- [Confianza](#confianza)
- [Sandbox](#sandbox)
- [Apariencia y accesibilidad](#apariencia-y-accesibilidad)
- [Idiomas](#idiomas)
- [Atajos de teclado](#atajos-de-teclado)
- [Privacidad y seguridad](#privacidad-y-seguridad)
- [Dónde se guardan los datos](#dónde-se-guardan-los-datos)
- [Limitaciones conocidas](#limitaciones-conocidas)
- [Compilar desde el código fuente](#compilar-desde-el-código-fuente)
- [Licencia](#licencia)

## Funciones de un vistazo

- **Todos los agentes en una línea de tiempo.** Claude mediante su propio comando `claude`, Codex, Antigravity y OpenCode mediante el Agent Client Protocol, más desde el registro ACP, y un agente integrado en la app para Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama y LM Studio. Respuestas, llamadas a herramientas, solicitudes de permiso, planes, tareas pendientes, subagentes y tareas en segundo plano tienen el mismo aspecto, sea cual sea el agente.
- **Revisar y rebobinar.** Antes de cada prompt, Dango guarda un punto de control. Lo que un agente ha cambiado se puede revisar y rebobinar por mensaje, y un hilo puede trabajar en su propio worktree de Git y entregar su trabajo fusionando o con cherry-pick.
- **Una ventana por carpeta.** Archivos, terminales, páginas web, hilos de IA, diffs y vistas previas son pestañas de macOS de esa ventana. La barra lateral contiene Archivos, Búsqueda, Agentes, Git, Pruebas, Depurar, CI y Contenedores.
- **Un editor de código de verdad.** Colores de sintaxis, varios puntos de inserción, plegado, `.editorconfig`, servidores de lenguaje para 17 lenguajes con autocompletado, ir a la definición, referencias, renombrar, correcciones rápidas y jerarquías, y predicciones mientras escribes de GitHub Copilot o de un modelo local.
- **Depurador, Git y CI integrados.** Puntos de interrupción y ejecución paso a paso para Rust, C, C++, Swift, Go, JavaScript, TypeScript y Python; preparar fragmentos sueltos, ramas, stashes, conflictos y un grafo de commits; ejecuciones, jobs y logs de GitHub Actions y GitLab CI.
- **Cuidadoso con tus carpetas.** Una carpeta en la que no confías no inicia nada: ni procesos de Git, ni servidores de lenguaje, ni hooks, ni agentes. Los comandos destructivos de Git nunca se permiten para siempre, y los commits se revisan en busca de secretos.
- **Dictado.** Habla en el editor y en el campo de mensaje, en línea con OpenAI o Groq o en local con Whisper y Parakeet.
- **Nativo hasta la médula.** Ventanas, pestañas, menús y barras de herramientas de AppKit, TextKit 2 para el editor, Metal para el terminal, WebKit solo en la pestaña del navegador. El modo oscuro, Aumentar contraste y VoiceOver funcionan como en cualquier app del Mac.
- **En tu idioma.** La interfaz está en 35 idiomas: alemán, árabe, catalán, checo, chino (simplificado), chino (tradicional), coreano, croata, danés, eslovaco, esloveno, español, finés, francés, griego, hebreo, hindi, húngaro, indonesio, inglés, italiano, japonés, malayo, neerlandés, noruego, polaco, portugués (Brasil), portugués (Portugal), rumano, ruso, sueco, tailandés, turco, ucraniano y vietnamita.

## Requisitos

- macOS 14 Sonoma o posterior, en Apple Silicon o Intel. Dango se desarrolla y se prueba en macOS 27.
- Para los hilos de IA, el agente que quieras usar; consulta [Agentes de IA](#agentes-de-ia).
- Para los servidores de lenguaje y los adaptadores de depuración, que Dango instala cuando se lo pides: Go para gopls y `dlv`; Node.js para los servidores de TypeScript, Python, JSON, YAML, CSS, HTML y shell y para el depurador de JavaScript; las Xcode Command Line Tools para clangd, sourcekit-lsp y `lldb-dap`. rust-analyzer se descarga de sus versiones de GitHub.
- Para las predicciones mientras escribes, un plan de GitHub Copilot (el gratuito sirve) o un modelo fill-in-the-middle en Ollama o LM Studio.

## Instalación

**Desde la imagen de disco.** Cada versión es una imagen de disco, `Dango-<version>.dmg`, con una app universal para Apple Silicon e Intel, firmada con un Developer ID y notarizada por Apple. Ábrela y arrastra Dango a la carpeta Aplicaciones. Un Cask de Homebrew instala la misma imagen. Todavía no se ha publicado ninguna versión.

**Actualizaciones.** Dango instalado desde la imagen de disco se mantiene al día con [Sparkle](https://sparkle-project.org). En el segundo arranque pregunta si puede buscar actualizaciones por su cuenta; hasta que aceptes, no hace ninguna solicitud. **Dango › Buscar actualizaciones…** busca al momento. Las actualizaciones están firmadas con la clave propia de la app, que Sparkle comprueba antes de instalar nada. Una versión compilada por ti no tiene actualizador.

**Desde el código fuente.** Consulta [Compilar desde el código fuente](#compilar-desde-el-código-fuente) y después mueve `target/release/Dango.app` a `/Applications`.

**El comando de terminal.** **Dango › Instalar el comando «dango»…** escribe un pequeño script en `~/.local/bin/dango`, sin permisos de administrador; `~/.local/bin` debe estar en tu `PATH`. Después, `dango .` abre la carpeta actual y `dango file.rs` un archivo. Dango nunca sustituye ahí un archivo que no haya escrito él mismo.

## Primeros pasos

1. **Abre una carpeta** con ⌘O o `dango <folder>`. La primera vez, Dango pregunta si confías en ella (consulta [Confianza](#confianza)).
2. **La barra lateral** cambia entre Archivos (⌘1), Búsqueda (⌘2), Agentes (⌘3), Git (⌘4), Pruebas (⌘5), Depurar (⌘6), CI (⌘7) y Contenedores (⌘8); con un clic derecho ocultas los que no usas, y ⌃⌘S pliega la barra lateral.
3. **Cada documento es una pestaña** de la ventana de la carpeta, las pestañas propias de macOS. El «+» al final de la barra de pestañas y ⌘T ofrecen un archivo nuevo, un terminal, el navegador y cada agente instalado, también en su propio worktree de Git; si mantienes pulsado el «+», aparece lo mismo como menú.
4. **Abrir rápidamente** con ⌘P encuentra archivos, y con `#` los símbolos del proyecto; ⇧⌘F busca y reemplaza en el proyecto.
5. **Habla con un agente** en una pestaña de IA. ⌘Esc lleva el foco del teclado a su campo de mensaje y de vuelta.
6. **Ajustes** con ⌘, contiene las pestañas General, CI, Agentes, Agente propio, Servidor de lenguaje, Dictado y Predicción.

La barra de herramientas contiene atrás y adelante por los lugares que has editado (⌃- y ⌃⇧-), la rama, Abrir rápidamente y el resumen de Actividad, que lista todo lo que se está ejecutando. **Abrir al lado** en el menú contextual de un archivo muestra un segundo documento junto al primero. Tras un reinicio o un fallo, cada ventana vuelve con sus pestañas, y los editores con texto sin guardar vuelven con ese texto. Imágenes, audio, vídeo, tipos de letra y documentos de Office se abren en una pestaña mediante Vista Rápida, los PDF mediante PDFKit.

## Agentes de IA

| Agente | Cómo funciona | Qué necesita |
|---|---|---|
| Claude | el comando `claude`, al que se habla directamente mediante su protocolo stream-json | `claude` instalado y con la sesión iniciada; Dango busca en el `PATH` del shell de inicio de sesión, en `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` y `/usr/local/bin`. La sesión solo se inicia en el propio `claude` |
| Codex | `codex-acp` mediante el Agent Client Protocol (ACP) | Node.js y npm; Dango instala el paquete de versión fijada cuando se lo pides |
| Antigravity | `agy acp` | tu propio `agy` en el `PATH`, o la descarga de versión fijada, instalada cuando lo pides |
| OpenCode | `opencode acp` | tu propio `opencode` en el `PATH`, o la descarga de versión fijada, instalada cuando lo pides |
| Otros agentes ACP | del registro ACP, que solo se descarga cuando lo pides | lo que indique la entrada del registro; las entradas sin suma de comprobación solo se instalan después de que confirmes |
| Agente propio | integrado en la app, con herramientas para archivos, búsqueda, comandos, diagnósticos y solicitudes web | una clave de API para Anthropic, OpenAI, Google Gemini u OpenRouter, guardada en el llavero; Ollama o LM Studio en este Mac sin clave; cualquier servidor que hable la API de OpenAI o de Anthropic |

**Ajustes › Agentes** muestra qué agentes tiene este Mac, instala los demás y sus actualizaciones, y abre el registro ACP. Los agentes que instala Dango están fijados a versiones probadas y se comprueban con sus sumas de comprobación publicadas (npm integrity o SHA-256); Dango no ejecuta paquetes `npx` sin versión.

### La línea de tiempo

Cada agente trabaja en una pestaña de IA con la misma línea de tiempo: respuestas, llamadas a herramientas con su salida, solicitudes de permiso, preguntas, planes para aprobar y comentar (⌥⌘K), tareas pendientes, subagentes y tareas en segundo plano. Dos botones en la cabecera de la pestaña listan los subagentes y procesos en segundo plano en curso. Menciona un archivo, `@terminal` o `@browser` para darle su contenido al agente. Un punto verde en la pestaña indica una sesión trabajando, uno rojo una sesión que te espera, y una notificación avisa cuando termina o espera mientras miras otra cosa. Con Claude, la línea de estado muestra el contexto y el uso.

### Permisos y modos

Los hilos nuevos empiezan en el modo «Manual»: el agente pregunta antes de cambiar archivos o ejecutar comandos. Para los comandos destructivos de Git (`push --force`, `reset --hard`, `clean -fd`), Dango no ofrece «Permitir siempre». El agente propio guarda sus reglas de «Permitir siempre» por proyecto, las lista en **Ajustes › Agente propio** y pregunta cada vez por los comandos destructivos de Git, tenga lo que tenga guardado; Claude y los agentes ACP guardan esas reglas en su propia configuración. Con «Permitir todo» y sin sandbox, el hilo muestra un aviso permanente.

### Puntos de control y worktrees

Antes de cada prompt, Dango guarda un punto de control, como commit bajo `refs/ide-checkpoints/` en el repositorio del proyecto, o en su propia carpeta para un proyecto sin Git; un push o fetch normal nunca se los lleva. Lo que un agente ha cambiado se puede revisar y rebobinar por mensaje. Un hilo puede trabajar en su propio worktree de Git: el panel de Git muestra lo que ha cambiado e incorpora el trabajo fusionando o con cherry-pick.

## Editor

- **Colores de sintaxis, números de línea y marcas de cambios** respecto al último commit, buscar y reemplazar, Abrir rápidamente (⌘P) y búsqueda en el proyecto (⇧⌘F).
- **La sangría** sigue el archivo y `.editorconfig`: tabuladores o espacios, sangría automática, pares de paréntesis.
- **Varios puntos de inserción:** ⌥-clic, ⌥-arrastrar para una columna, ⌃⇧↑ y ⌃⇧↓, ⌘D para la siguiente aparición.
- **Plegado** (⌥⌘← y ⌥⌘→, con ⇧ para todo), comentar/descomentar (⌘/), ir a la línea (⌃G), espacios visibles y ajustar líneas (⌥Z).
- **Los archivos conservan su codificación**, marca de orden de bytes y finales de línea. Un archivo que cambia en el disco se vuelve a cargar si el editor no tiene texto sin guardar; si no, una barra ofrece las diferencias y una fusión.
- **Predicciones mientras escribes.** Mientras escribes aparece una propuesta en gris, y Tab la acepta. Viene de GitHub Copilot o de un modelo local en Ollama o LM Studio, según lo elegido en **Ajustes › Predicción**; un modelo local mantiene tu código en este Mac.
- **Minimapa y Desplazamiento fijo** (menú Editor): una imagen estrecha de todo el archivo junto al texto, y las líneas de cabecera de los bloques en los que está la línea superior, fijadas encima de ella.
- **Diagnósticos al final de la línea** (menú Visualización): el mensaje más grave de cada línea tras un segundo de reposo, con un símbolo para su gravedad.
- **Vista previa en directo de páginas estáticas:** Editor › Abrir en una pestaña del navegador sirve las páginas de una carpeta de confianza desde el propio Dango y recarga la pestaña cuando se guarda un archivo.
- **Prettier** formatea JavaScript, TypeScript, CSS, HTML, Markdown y más donde el proyecto lo configura, con el propio `node_modules/.bin/prettier` del proyecto; el ajuste `formatter` elige el formateador para cada lenguaje.
- **Modo Vim** (Editor › Modo Vim): modos Normal, Insertar, Visual y Reemplazar, contadores, operadores con movimientos y objetos de texto, `.`, registros, marcas y macros. `u` y ⌃R usan el deshacer del editor, y una pasada de inserción es un solo paso. `"+` y `"*` son el portapapeles; el ajuste `vimClipboard` hace que el registro sin nombre también sea el portapapeles. `/`, `?`, `n`, `N`, `*`, `:s` y `:%s` buscan con las expresiones regulares de Rust, no con los patrones de Vim: sin `\v`, y `\b` en lugar de `\<` y `\>`. Los comandos Ex son `:w`, `:q`, `:wq`, `:x`, `:noh` y `:<line>`. En modo Normal, ⌃G pasa a Vim en lugar de ir a la línea.
- **Sincronización de ajustes** a través de una carpeta de iCloud Drive (**Ajustes › General**): tus ajustes y atajos te siguen a tus otros Mac; las rutas, las direcciones locales y los consentimientos se quedan en cada uno.

## Servidores de lenguaje

Go, Rust, TypeScript y JavaScript, Python, C, C++ y Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML, Markdown y Lua, y servidores propios desde **Ajustes › Servidor de lenguaje**. Dango instala un servidor cuando lo pides, en su propia carpeta de herramientas, y usa el `tsc` de un proyecto de TypeScript cuando puede servir como servidor de lenguaje.

Lo que aportan: diagnósticos (⌘' y ⇧⌘' pasan de uno a otro), información del símbolo (⌃⌘I), autocompletado (⌥Esc), ayuda de firmas, ir a la definición (⌃⌘J), a la definición de tipo y a la implementación, referencias (⌃⇧⌘F), jerarquías de llamadas y de tipos (⌃⇧⌘H), renombrar (⌃⌘E), formatear (⌥⇧F), colores semánticos, plegado y el esquema (⇧⌘O); tipos y nombres de parámetros dentro de la línea; correcciones rápidas y refactorizaciones tras una bombilla (⌥⌘A); los símbolos del proyecto con ⌥⌘O.

Además del servidor propio de un archivo, ESLint revisa el JavaScript y el TypeScript de los proyectos que lo usan, con sus correcciones rápidas, y las abreviaturas de Emmet aparecen en el autocompletado para HTML, CSS y JSX. Dango instala ambos cuando lo pides; **Ajustes › General** corrige los problemas de ESLint al guardar, si quieres.

## Pruebas

El panel de Pruebas lista las pruebas de una carpeta por paquete y archivo con su estado: `cargo test` o cargo-nextest, Go, pytest, Vitest y Jest. Un símbolo junto a cada prueba en el margen la ejecuta, ⌥-clic la depura, y una aserción fallida marca su línea. El menú Depurar ejecuta la prueba en el punto de inserción (⌃⌥⌘U), las últimas pruebas de nuevo (⌃⌥⌘G) o todas, y depura la prueba en el punto de inserción. Las pruebas solo se ejecutan en carpetas de confianza, ya que ejecutan el código del proyecto; `cargo test` lista sus pruebas cuando se lo pides, porque listarlas compila cada programa de pruebas.

## Depurador

Puntos de interrupción, también con condiciones y como logpoints, ejecución paso a paso, hilos de ejecución con sus pilas de llamadas, variables y expresiones vigiladas en el Inspector (⌥⌘I), valores junto al código mientras el programa está en pausa, y una consola de depuración que evalúa en el frame en pausa.

| Lenguaje | Adaptador de depuración |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` de las Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | el depurador de JavaScript, sobre Node.js |
| Python | debugpy |

Se lee el `.vscode/launch.json` de un proyecto. El menú Depurar usa los atajos de Xcode: ⌘\\ pone un punto de interrupción, ⌘Y los activa o desactiva todos, ⌃⌘Y continúa, F6, F7 y F8 hacen paso a paso por procedimientos, por instrucciones y para salir, ⌘. detiene.

## Terminal

Terminales en pestañas (⌃\`) con el entorno del shell de inicio de sesión, dibujados con Metal. ⌘K limpia la pantalla. Una imagen pegada o arrastrada va a un agente de programación que se ejecute en el terminal, o pega la ruta de una copia guardada. ⌘-clic abre enlaces y nombres de archivo. Un servidor de desarrollo que inicia el terminal puede abrirse en la pestaña del navegador.

## Navegador

Una pestaña de navegador con campo de dirección (⌘L), atrás y adelante (⌘[ y ⌘]), volver a cargar (⌘R), búsqueda, descargas que preguntan primero y el inspector web. `@browser` le da la página a un agente, marcada como texto de fuera.

## Git

El panel de Git prepara y descarta archivos o fragmentos sueltos, hace commits, hace commit y push o sincroniza, cambia y crea ramas, guarda stashes, resuelve conflictos de fusión y dibuja un grafo de los commits recientes. Blame muestra quién cambió por última vez la línea bajo el cursor, o una columna para todo el archivo. Un botón escribe el mensaje de commit con IA; el diff solo sale después de que gitleaks no haya encontrado ningún secreto en él, y cada commit también se revisa en busca de secretos.

## CI

GitHub Actions y GitLab CI, también GitHub Enterprise Server y GitLab autogestionado:

- ejecuciones, jobs y logs de la rama actual;
- volver a ejecutar jobs fallidos, un solo job o, en GitHub, una ejecución completa; iniciar jobs manuales o aprobar despliegues en espera; iniciar un pipeline nuevo o un workflow que se puede iniciar a mano; cancelar;
- las pull requests o merge requests abiertas y los issues, que se abren en la pestaña del navegador;
- el log de un job fallido entregado a un agente, marcado como texto de fuera.

El token viene del llavero, de `gh` o `glab`, del entorno del shell de inicio de sesión, o es uno que introduces en **Ajustes › CI**.

## Contenedores

El panel de Contenedores muestra los proyectos Compose de Docker con sus servicios, los contenedores sueltos, las imágenes y los volúmenes, y sigue los eventos de Docker a medida que ocurren. Un archivo Compose de la carpeta aparece como proyecto incluso antes de que exista alguno de sus contenedores. Los contenedores se inician, se detienen y se reinician; sus logs y un shell se abren en pestañas de terminal, y un puerto publicado se abre en la pestaña del navegador. Eliminar un contenedor, una imagen o un volumen, y limpiar los datos sin usar, piden confirmación antes. Los comandos up, down y pull de Compose solo se ejecutan en carpetas de confianza, ya que Compose compila y ejecuta lo que describe el proyecto. El menú del panel elige el contexto de Docker de la ventana sin cambiar el que usan tus shells; los mismos comandos están en el menú Contenedores. Necesita la línea de comandos `docker`, Docker 24.0.6 o posterior.

## Markdown

- **Vista previa.** Un archivo Markdown se abre con su vista previa al lado; **Visualización › Mostrar/ocultar vista previa al lado** (⇧⌘V) la cierra y la abre, y **Ajustes › General** desactiva la vista previa automática. La vista previa dibuja fórmulas, diagramas de Mermaid, Graphviz y PlantUML y las imágenes de la carpeta; las imágenes de la web solo se cargan en una carpeta de confianza o cuando las pides en la vista previa.
- **Menú Formato:** negrita (⌘B), cursiva (⌘I), tachado (⇧⌘X), código (⌥⇧⌘C), enlace (⇧⌘K), encabezamientos 1–3 (⌥⌘1–⌥⌘3), listas con viñetas y numeradas, tareas, citas y bloques de código. Cada comando activa o desactiva, sobre la selección o las líneas, y es un solo paso de deshacer.
- **Pegar.** El texto de Word o de una página web llega como Markdown, y sus imágenes como archivos junto al documento; ⌥⇧⌘V pega texto sin formato.
- **Exportar.** **Archivo › Exportar** escribe un PDF, un único archivo HTML con sus imágenes dentro o un documento de Word. Las imágenes de la web solo se incluyen desde una carpeta de confianza.
- **Abrir en Splat.** Si [Splat](https://github.com/sventmueller/splat) está instalado, **Archivo › Abrir en Splat** y los menús contextuales de la lista de archivos y de las pestañas abren allí el documento, después de guardarlo.

## Dictado

Dicta en el campo de mensaje de una pestaña de IA o en el editor: con el botón del micrófono que hay allí, **Edición › Iniciar dictado** (⌃⌥D) o la tecla fn (mantenla pulsada para grabar, púlsala dos veces para seguir grabando). El reconocimiento de voz es el de [Lowspeak](https://github.com/sventmueller/lowspeak); no hace falta tener Lowspeak instalado.

- **Ajustes › Dictado** establece el modo y el modelo (en línea con OpenAI o Groq, en local con Whisper o Parakeet), el idioma, el posprocesado, el micrófono y tu propio vocabulario.
- **Los modelos locales** se descargan de Hugging Face cuando eliges uno, con el progreso y la opción de cancelar. Se guardan en las carpetas que usa Lowspeak, de modo que cada app usa lo que la otra ha descargado. Un modelo local mantiene la grabación en este Mac.
- **La transcripción en línea** necesita una clave de OpenAI o Groq, que Dango guarda en un ítem propio del llavero.
- **Desde Lowspeak.** Si Lowspeak está configurado de forma distinta a la predeterminada, la pestaña Dictado ofrece una vez adoptar sus ajustes, y sus claves si lo pides; macOS pregunta antes de que Dango lea una (elige «Permitir», no «Permitir siempre»).
- **Permisos.** macOS pide acceso al micrófono en el primer dictado. La tecla fn funciona mientras Dango es la app activa y Lowspeak no se está ejecutando; necesita el permiso de accesibilidad, que Dango solo pide después del primer dictado, nunca al arrancar.

## Confianza

Una carpeta puede hacer que se ejecuten programas en cuanto se inicia algo en ella: hooks de Git y ajustes del repositorio como `core.fsmonitor`, servidores de lenguaje de `node_modules`, hooks de agentes, `.mcp.json`. La primera vez que abres una carpeta, Dango pregunta si confías en ella.

- **De confianza:** Git, los servidores de lenguaje, los depuradores, los agentes y la CI se ejecutan para esta carpeta y todo lo que contiene.
- **Restringida:** puedes leer y editar; las marcas de cambios del editor salen de leer `.git` directamente. No se inicia nada de lo que configura la carpeta. Los paneles de Git, CI e IA lo indican y ofrecen confiar en la carpeta.

Un texto que no viene de ti puede llevar instrucciones pensadas para dirigir a un agente (inyección de prompts). La salida de un terminal (`@terminal`), una página web (`@browser`), un log de CI entregado a un agente, un archivo fuera del proyecto y un texto más largo pegado desde una página web se envían por eso marcados: la etiqueta dice «Contenido externo», y el agente recibe el texto con el aviso de que son datos no fiables cuyas instrucciones no debe seguir. Las páginas web del agente propio llegan con el mismo aviso. Un texto pegado cuenta como procedente de una página web cuando el navegador lo indica en el portapapeles, como hacen Safari, Chrome y Firefox; el texto de otras fuentes, y una frase corta de pocas palabras, sigue siendo tuyo.

## Sandbox

El propio Dango se ejecuta sin App Sandbox, porque todos los programas que inicia lo heredarían: terminales, Git, servidores de lenguaje y agentes no podrían funcionar. En su lugar, cada hilo de IA tiene un interruptor de sandbox, desactivado por omisión. Lo que limita depende del agente, y la ayuda del interruptor lo explica:

| Agente | Con el sandbox activado |
|---|---|
| Claude | los comandos de shell se ejecutan en el sandbox propio de Claude; sus herramientas de archivos y web, no |
| Codex | los comandos y los cambios solo escriben en el proyecto, sin red |
| Agente propio | archivos, solicitudes web y comandos se quedan dentro del proyecto y de las carpetas permitidas, sin red (`sandbox-exec`) |
| Antigravity, OpenCode | ningún sandbox que Dango pueda activar |

## Apariencia y accesibilidad

- **El modo oscuro** sigue al sistema; se respetan **Aumentar contraste** y **Reducir movimiento**.
- **VoiceOver** nombra la barra lateral, las pestañas, las tarjetas de la línea de tiempo de IA, las filas de Git y CI, el depurador y el texto del terminal. Los puntos de interrupción nunca se distinguen solo por el color.
- **Teclado.** Cada comando tiene un atajo, y todos se pueden cambiar en `keybindings.json`; ⌥⌘\` pasa al panel siguiente.
- **De derecha a izquierda.** En árabe y hebreo, la ventana se refleja, mientras que las vistas de código siguen de izquierda a derecha.

Lo que Dango añade a AppKit, área por área, está en [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Idiomas

La interfaz está traducida a 35 idiomas: alemán, árabe, catalán, checo, chino (simplificado), chino (tradicional), coreano, croata, danés, eslovaco, esloveno, español, finés, francés, griego, hebreo, hindi, húngaro, indonesio, inglés, italiano, japonés, malayo, neerlandés, noruego, polaco, portugués (Brasil), portugués (Portugal), rumano, ruso, sueco, tailandés, turco, ucraniano y vietnamita.

Dango sigue el orden de idiomas de **Ajustes del Sistema › General › Idioma y región**; allí también puedes elegir un idioma solo para Dango en **Aplicaciones**. Si no hay traducción para tu idioma, usa el inglés. Los términos siguen un glosario, [docs/glossary.md](../../../docs/glossary.md), y las traducciones de Apple, para que Dango se lea como el resto de tu Mac.

## Atajos de teclado

| Acción | Atajo |
|---|---|
| Abrir carpeta, nueva ventana, cerrar ventana | ⌘O, ⇧⌘N, ⇧⌘W |
| Nuevo archivo, guardar, guardar como | ⌘N, ⌘S, ⇧⌘S |
| Nueva pestaña, cerrar pestaña, pestaña siguiente y anterior | ⌘T, ⌘W, ⇧⌘] y ⇧⌘[ |
| Abrir rápidamente, buscar en el proyecto | ⌘P, ⇧⌘F |
| Archivos, Búsqueda, Agentes, Git, Pruebas, Depurar, CI, Contenedores | ⌘1 a ⌘8 |
| Mostrar u ocultar la barra lateral, panel siguiente | ⌃⌘S, ⌥⌘\` |
| Atrás y adelante por los lugares editados | ⌃- y ⌃⇧- |
| Comentar/descomentar, ir a la línea | ⌘/, ⌃G |
| Siguiente aparición, punto de inserción arriba y abajo | ⌘D, ⌃⇧↑ y ⌃⇧↓ |
| Plegar, desplegar, plegar todo, desplegar todo | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Ajustar líneas, Inspector | ⌥Z, ⌥⌘I |
| Autocompletado, información del símbolo, ir a la definición | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referencias, jerarquía de llamadas, renombrar | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Símbolos del archivo, del proyecto | ⇧⌘O, ⌥⌘O |
| Corrección rápida, formatear | ⌥⌘A, ⌥⇧F |
| Problema siguiente y anterior | ⌘' y ⇧⌘' |
| Vista previa de Markdown, pegar como texto sin formato | ⇧⌘V, ⌥⇧⌘V |
| Negrita, cursiva, tachado, código, enlace | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Encabezamiento 1 a 3 | ⌥⌘1 a ⌥⌘3 |
| Nuevo terminal, limpiar terminal | ⌃\`, ⌘K |
| Foco en el campo de mensaje del agente, comentar un plan | ⌘Esc, ⌥⌘K |
| Campo de dirección, volver a cargar, atrás, adelante (navegador) | ⌘L, ⌘R, ⌘[, ⌘] |
| Iniciar o detener el dictado | ⌃⌥D |
| Punto de interrupción, activar o desactivar todos | ⌘\\, ⌘Y |
| Ejecutar la prueba en el punto de inserción, volver a ejecutar las últimas pruebas | ⌃⌥⌘U, ⌃⌥⌘G |
| Continuar, paso a paso por procedimientos, por instrucciones, para salir, detener | ⌃⌘Y, F6, F7, F8, ⌘. |
| Ajustes | ⌘, |

`keybindings.json` en la carpeta de datos de la app (consulta [Dónde se guardan los datos](#dónde-se-guardan-los-datos)) cambia cualquiera de ellos.

## Privacidad y seguridad

- **Sin telemetría,** sin analíticas, sin informes de fallos enviados a ninguna parte. Los errores van al registro del sistema (Consola, con el nombre de la app).
- **La red solo para lo que tú inicias:** los agentes que usas, cada uno con su propio servicio; el registro ACP y las descargas de agentes, servidores de lenguaje, adaptadores de depuración y modelos de dictado cuando los pides; los servidores de CI en los que inicias sesión; las predicciones de Copilot si lo has elegido; los proveedores del agente propio; la transcripción en línea si la has elegido; las páginas que abres en la pestaña del navegador; y la búsqueda de actualizaciones una vez que la has permitido.
- **El código solo sale del Mac** donde has elegido un servicio para ello: un agente, las predicciones de Copilot, un mensaje de commit escrito con IA. El diff para un mensaje de commit solo sale después de que gitleaks no haya encontrado ningún secreto en él.
- **Las claves y los tokens** se quedan en el llavero de macOS, nunca en un archivo de ajustes. Las claves de otras apps, como las de Lowspeak, solo se leen si aceptas, y solo una vez.
- **Descargas fijadas.** Los agentes, servidores de lenguaje y adaptadores de depuración que instala Dango están fijados a versiones probadas y se comprueban con sus sumas de comprobación.
- **Firma de código** con el Hardened Runtime; las actualizaciones solo se instalan con la firma propia de la app.

## Dónde se guardan los datos

`<id>` es el identificador del paquete, `local.agentide` para la versión publicada (`local.agentide.debug` para las compilaciones de depuración). No contiene el nombre visible, así que un cambio de nombre conserva todos los datos.

| Lugar | Qué |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (la lista del panel de IA), `agent-threads/` (conversaciones del agente propio), `layouts/` (pestañas por proyecto), `recovery/` (editores sin guardar tras un fallo), `checkpoints/` (para carpetas sin Git), `worktrees/`, `agents/` y `tools/` (agentes, servidores de lenguaje y adaptadores de depuración instalados), `task-output/` |
| `~/Library/Caches/<id>/` | imágenes pegadas, scripts de integración con el shell, el registro ACP; se puede eliminar |
| `~/Library/Logs/<id>/dictation/` | los archivos de registro del dictado |
| `~/Library/Application Support/Lowspeak` y `FluidAudio` | modelos de dictado locales, compartidos con Lowspeak |
| `~/Library/WebKit/<id>/` | cookies y datos de sitios web de la pestaña del navegador |
| `<project>/.dango/settings.json` | los ajustes propios del proyecto |
| el repositorio de Git del proyecto | puntos de control, como commits bajo `refs/ide-checkpoints/`; un push o fetch normal nunca se los lleva |
| llavero, servicio `<id>` | claves de API del agente propio y del dictado, tokens de CI |
| `~/.local/bin/dango` | el comando de terminal, si lo has instalado |

Claude guarda sus propias sesiones en `~/.claude/projects`; Dango las lee para reanudar una sesión, pero no las cambia.

## Limitaciones conocidas

- **Sin App Sandbox** para la app en sí, ya que terminales, Git, servidores de lenguaje y agentes lo heredarían; una carpeta debe ser de confianza antes de que se ejecute nada de lo que configura. Antigravity y OpenCode no tienen un sandbox que Dango pueda activar.
- **Las cifras de uso y el desglose del contexto de Claude** dependen de partes de su protocolo marcadas como experimentales, y se ocultan cuando faltan.
- **Rebobinar la conversación** solo funciona con Claude; con los demás agentes solo se rebobina el código.
- **El inicio de sesión en la CI mediante el navegador** está desactivado; usa `gh`, `glab`, un token o el entorno.
- **Gemini CLI no se ofrece:** Google ha dejado de permitir el inicio de sesión con una cuenta personal y remite a Antigravity. Los hilos de Gemini anteriores siguen pudiendo leerse.

## Compilar desde el código fuente

Necesitas:

- macOS 14 o posterior con las Xcode Command Line Tools (`xcode-select --install`); Xcode en sí no hace falta.
- Rust tal como lo indica `rust-toolchain.toml`. El `rust` de Homebrew compila para el procesador de este Mac; la compilación universal de una versión necesita rustup con los destinos `aarch64-apple-darwin` y `x86_64-apple-darwin`.
- Copias de [splat](https://github.com/sventmueller/splat) y [lowspeak](https://github.com/sventmueller/lowspeak) junto a esta (`../splat`, `../lowspeak`), ambas en su rama `main`; el espacio de trabajo toma crates de ellas por ruta.
- cmake, para cada compilación con dictado (`make run`, `make build`, `make release`), que compila whisper.cpp. `make check` compila sin él.

| Comando | Qué hace |
|---|---|
| `make run` | compila la app de depuración (`target/debug/Dango Debug.app`) y la abre |
| `make build` | compila la app de versión (`target/release/Dango.app`), firmada con un certificado local que crea la primera compilación en el llavero de inicio de sesión; deja `VERSION` sin cambios |
| `make lint` | formato y clippy; los avisos lo hacen fallar |
| `make check` | `lint` y después todas las pruebas; no necesita llavero, ventana ni red |
| `make test` | `check` y después la autoprueba `probe` y la auditoría de accesibilidad, de izquierda a derecha y de derecha a izquierda, en la app real |
| `make perf` | los presupuestos de tiempo, en una compilación de versión; solo tiene sentido en un Mac por lo demás inactivo |
| `make release` | un ensayo de la publicación en `target/release-dry-run/`, firmado con el certificado local; no envía nada a ninguna parte |
| `make release-notarized` | `check` y después la publicación en sí en `dist/`: firma con Developer ID, notarización, el feed de actualizaciones firmado con la clave propia de la app; deja `VERSION` sin cambios |
| `make deploy` | en un `main` limpio, con o sin sello de CI: notariza la siguiente versión, hace commit de `VERSION`, sube los archivos de la publicación, envía el código y la etiqueta de versión, con el sello verde de CI si lo hay, y después publica la versión él mismo |

Depuración y versión son apps separadas con sus propios ajustes y datos. La compilación, las pruebas, las autopruebas, la publicación y las reglas que sigue el código se describen en [docs/development.md](../../../docs/development.md), y las decisiones que hay detrás, en [docs/plan.md](../../../docs/plan.md). Las notas de la versión son [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traducidas junto a cada README en `assets/readme/<language>/`.

## Licencia

Dango se distribuye bajo cualquiera de las dos licencias siguientes:

- Licencia Apache, versión 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licencia MIT ([LICENSE-MIT](../../../LICENSE-MIT))

a tu elección.

Salvo que declares expresamente lo contrario, toda contribución enviada intencionadamente para su inclusión en Dango por tu parte, tal como se define en la licencia Apache-2.0, tendrá doble licencia como arriba, sin términos ni condiciones adicionales.
