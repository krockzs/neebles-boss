# N.E.E.B.L.E.S. Boss v2.0.15

## Corrección de identidad administrativa Qt / proveedor OS

- Corrige un defecto detectado al actualizar Proxy Client 1.0.1 mediante Boss 2.0.14: el proveedor OS rechazaba `--authority-supply <ruta OS> modules update` aunque el agente Qt hubiera autenticado a root por Polkit.
- La autenticación del proveedor OS reconoce ahora el prefijo global **solo** cuando coincide exactamente con `/usr/lib/neebles/platform/authority/authority-supply.json`.
- Conserva la autorización por SO_PEERCRED UID 0, identidad por inode del ejecutable Boss instalado, UID real/efectivo/guardado/FS todos 0 y comprobación de CLI administrativa o cgroup del servicio Boss.
- Los procesos de módulos no reciben privilegios. El preflight sigue verificando el recibo de aprobación OS por hashes del manifiesto, contrato y ELF antes del cierre de servicios.
- No altera CUSTOM clásico, CUSTOM V2, Secrets, Connect ni el código de Proxy Client.

## Verificación

- F47: 35/35 tests Python OS aprobados, incluidos 7 casos nuevos de invocación Qt y casos de rechazo.
- Los binarios deben compilarse y certificarse en Esbirro para esta versión.
- La publicación GitHub y la aceptación instalada son puertas separadas. La aprobación de Proxy Client 1.0.1 sigue siendo una operación administrativa OS independiente.
