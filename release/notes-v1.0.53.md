# N.E.E.B.L.E.S. Boss v1.0.53

## TXN V5 — Endurecimiento transaccional de desinstalación

Esta versión continúa el trabajo de Uninstall TXN V5 incorporado en v1.0.52.

- Retira recursos de arranque declarados por cada módulo (systemd-user y XDG autostart) mediante autoridad autenticada, no por descubrimiento arbitrario.
- Valida identidad, propiedad, ubicación y ExecStart antes de actuar sobre recursos del usuario.
- Durante la fase posterior al commit irreversible, intenta las retiradas independientes y acumula errores críticos sin fingir rollback de una instalación ya retirada.
- La desactivación del runtime intenta por separado el cierre de Module IPC, del Tray y sus verificaciones finales; un fallo individual no omite las otras acciones autorizadas.
- Refuerza la recuperación reversible: antes de reactivar servicios originalmente activos vuelve a verificar la integridad de la instalación.
- Conserva la separación de responsabilidades entre Lifecycle/Arsenal, Boss Runtime, Esbirro, CUSTOM clásico y CUSTOM V2.

## Estado de certificación

El build debe pasar el workflow de release y la verificación de sus 11 artefactos antes de publicarse.
Las pruebas en un sistema NEEBLES OS instalado, incluida la desinstalación y reinstalación de Test Module 1.2.8 con ambas opciones de Settings, son una validación de integración posterior a la publicación. Esta nota no declara esas pruebas como aprobadas.
