## Velocidad y memoria
- Todo lo que inicia un agente, en cualquier lenguaje, como compilaciones, pruebas, shells y servidores MCP, se ejecuta
  en el estado de segundo plano de macOS: con la prioridad más baja y con el acceso al disco y a la red limitado. Los
  propios agentes, junto con los subagentes de Claude, se ejecutan con prioridad media. Los agentes a plena carga ya no
  ralentizan Dango; esto también vale para los agentes de la Software Factory.
- Las ventanas restauradas solo inician lo que se ve: una pestaña de IA que está detrás de otras inicia su CLI cuando
  pasa al primer plano o recibe un prompt, y las pestañas de navegador, de vista previa del sitio y de terminal cargan
  su página o inician su shell la primera vez que se muestran.
- Una búsqueda larga en el proyecto ya no vuelve a dibujar todos sus resultados mientras se ejecuta. La
  Software Factory, los logs de CI y el panel Pruebas trabajan menos mientras no están a la vista; las pruebas y las
  configuraciones de inicio se leen la primera vez que se muestra su panel, y los paneles CI y Contenedores ocultos no
  leen nada hasta que se muestran.
- Las sesiones largas necesitan menos memoria: un diff terminado conserva solo las líneas que muestra su tarjeta, una
  solicitud de permiso ya respondida libera su entrada, la caché de glifos del terminal ya no crece sin límite, y los
  paneles Archivos y Git liberan las filas que ya no existen.
- El propio programa ocupa alrededor de un 40 % menos.

## Ventanas y menús
- Al cerrar un área, sus pestañas ya no se cierran: pasan al área anterior o, si no la hay, a la siguiente. Solo el área
  de una vista previa y la última área se cierran con sus pestañas.
- El menú Visualización muestra los paneles en el orden de los iconos de la barra lateral.
- «Gestionar agentes…» ya no está en el menú +; sigue bajo «+ Nueva sesión» en el panel Agentes.

## Correcciones
- «Incluir y excluir archivos» en el panel Búsqueda ya no muestra una letra suelta junto a su flecha.
- Cerrar la ventana emergente de comentarios de una especificación ya no provoca un fallo.
