## Pestañas de IA
- «Mostrar salida» de un comando en segundo plano abre una pestaña que sigue la salida mientras el comando se ejecuta,
  en lugar de una copia de ese momento. La pestaña se abre al final de la salida y se queda ahí a medida que llegan
  líneas nuevas, hasta que te desplazas hacia arriba; si vuelves a desplazarte hasta el final, vuelve a seguirla. Si la
  pestaña ya está abierta, «Mostrar salida» la trae al primer plano, al final.
  - Un comando que escribe él mismo su salida en un archivo, como `make > build.log 2>&1`, muestra ese archivo a medida
    que crece.
  - Cuando el comando termina, la pestaña muestra toda su salida; «Abrir en el editor» abre el archivo que hay detrás.
  - Una pestaña solo lee la salida mientras está en primer plano.
