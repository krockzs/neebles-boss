# N.E.E.B.L.E.S. Boss v2.0.1

## Aislamiento genérico de mundos Multi-RootFS

- Cada RootFS de un módulo con Construction `rootfs.<id>.domination.world` valida exclusivamente las rutas físicas de ese mundo. El catálogo autenticado de mundos puede seguir siendo compartido sin exigir los ejecutables de mundos ajenos.
- La selección del mundo proviene de la Construction vinculada al módulo; tanto el staging de Install como RuntimeLease validan el RootFS correspondiente.
- La validación global de la autoridad doméstica de Boss permanece disponible para los consumidores que realmente la requieran. Los contratos antiguos sin mundo explícito conservan su recorrido preexistente.
- Se mantienen intactas las reglas de Preinstall: Essential, almacenes acumulativos de paquetes, verificación SHA-256 y reutilización de `.deb` ya certificados.

## Registry y validación

- Se restaura Test Module 1.2.12 (`3d265bc62a7ff51ed1c42aecf0158abfa29b3223`) en el Registry, conservando NEEBLES Connect 1.0.0 (`295d48e74fb7f7bd78d75fde0980dd19f06a5b8c`).
- Las seis pruebas nuevas F10 y la regresión Rust pasaron en el Ryzen antes del versionado. La publicación del candidato debe certificar de nuevo los artefactos exactos con Rust 1.98.1 y Qt 6.8.2/Esbirro.
- La instalación y desinstalación real de ambos módulos queda pendiente hasta ejecutar la release publicada en el Ryzen; no se declara certificada por las pruebas de fuente.

## Actualización

- Se conserva el modal Release Notes con el botón **Reboot** siempre disponible. Guarda tu trabajo y cierra las aplicaciones abiertas antes de reiniciar.
- La versión 2.0.1 permanece **candidata local** hasta completar la compilación controlada, verificación de los 11 artefactos, publicación de los mismos bytes y aceptación real instalada.
