## Memoria
- Las sesiones largas de agentes necesitan mucha menos memoria:
  - Al abrir una sesión anterior, su transcripción se lee línea a línea; una sesión con una transcripción de 100 MB
    alcanza ahora como máximo unos 20 MB en lugar de 65 MB.
  - Una llamada a herramienta terminada solo conserva lo que muestra su tarjeta: las primeras 400 líneas de su salida, y
    ya no la entrada sin procesar una vez que la tarjeta muestra sus campos. «N líneas más» sigue contando cada línea.
  - Una pestaña de IA que permanece fuera de la vista durante 30 segundos libera sus tarjetas y, cuando vuelve a
    mostrarse, construye las que están a la vista, en el lugar donde estaba.
  - Después de cerrar una pestaña o abrir una sesión anterior, Dango devuelve a macOS la memoria que ha liberado.
- Las listas ya no crecen en memoria al actualizarse. En macOS 27, cada actualización de una lista con filas
  desplegables dejaba atrás para siempre un triángulo desplegable, y cada actualización dejaba atrás un poco de cada
  fila:
  - Las listas de CI, Contenedores, Software Factory, Pruebas, Depurar, Git y Búsqueda y la jerarquía de llamadas
    dibujan sus propias flechas.
  - Las listas fuera de la vista ya no se actualizan; se ponen al día cuando se muestran.
  - Las listas Sesiones, CI, Software Factory y Contenedores conservan las filas que no han cambiado, y Git solo
    actualiza las filas de los archivos cuyo estado ha cambiado.
- Los paneles de una ventana cerrada se liberan con ella.

## Pestañas de IA
- «Mostrar salida» de un comando en segundo plano que escribe él mismo su salida en un archivo, como
  `make > build.log 2>&1`, muestra ese archivo cuando la CLI no tiene nada de la salida.
