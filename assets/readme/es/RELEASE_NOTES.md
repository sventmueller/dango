## Velocidad
- Lo que inicia un agente, como compilaciones, pruebas y shells, se ejecuta con la prioridad más baja en lugar de en el
  estado de segundo plano de macOS. Dango sigue pasando primero, y los agentes antes que lo que inician, pero sus
  compilaciones vuelven a usar los núcleos de rendimiento y toda la velocidad del disco; el estado de segundo plano
  había reducido su capacidad de trabajo a cerca de una décima parte incluso en un Mac por lo demás inactivo.

## Agentes
- El punto de una pestaña de IA sigue en verde mientras aún se ejecuta un comando en segundo plano de su sesión, y el
  aviso de que una sesión ha terminado no llega hasta que también ha acabado su trabajo en segundo plano.
