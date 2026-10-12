# N.E.E.B.L.E.S. Boss v1.0.51

## Open en las superficies

- El Launcher actualiza la disponibilidad visual de Open con lecturas canónicas recientes de `boss surface-model`, incluidos `closed`, `opening` y `open`.
- El botón Open del Launcher se presenta deshabilitado cuando su acción no está disponible, y falla cerrado si no se puede refrescar el estado.
- El botón Open de Boss UI deja de mostrar el diálogo Module Transaction; la operación sigue pasando por Governor.
- Se preservan las transacciones, los diálogos y las notas de Install, Update, Uninstall y Critical Update.
- No se modifica RuntimeRegistry, la ejecución de Governor ni la defensa contra instancias duplicadas.

## Validación pendiente

Requiere candidato compilado por Esbirro/Qt, verificación de assets, instalación real 1.0.50 -> 1.0.51 y prueba visual de Open/Close desde Boss UI, Launcher y Test Module Tray.
