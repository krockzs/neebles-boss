# N.E.E.B.L.E.S. Boss v2.0.5 — candidato

## Corrección de acceso a dependencias instaladas

- Incidente físico reproducido con Secrets 1.0.1: Install validó tres RootFS y activó MaterialBinding; Enable falló porque el proceso de escritorio no podía leer el snapshot `root:root 0600` de dependencias.
- La misma lectura afectaba el catálogo, los botones Enable/Disable y el preflight de Uninstall; no era un defecto de Secrets ni de su daemon.
- Boss crea el snapshot con modo privado 0600 mientras escribe y sincroniza, y solo al finalizar publica metadatos de Registry como root-owned read-only 0644; no contiene credenciales.
- El lector valida el propietario relativo al directorio del módulo, el tipo de archivo y la ausencia de permisos de escritura por grupo/otros; conserva fail-closed para snapshots inexistentes o inválidos.
- La CLI incorpora `modules migrate-requirements-access` (solo root mediante autorización existente). Verifica ANTES de cambiar permisos que cada snapshot antiguo es propiedad de root, válido para la versión instalada y modo exacto 0600. Jamás sintetiza historial desde el Registry.
- Preserva el control de dependencias de consumidores; ningún proveedor puede desinstalarse si un módulo instalado lo requiere.
- Sin cambios a NEEBLES Secrets 1.0.1, Connect ni CUSTOM V2.

## Certificación pendiente

- `cargo test --locked --bin neebles-backend boss205_` y `cargo check --locked` en el Ryzen.
- Compilación Rust/Qt con Esbirro y CI release 2.0.5 antes de publicar.
- Reinstalación física de Secrets 1.0.1, Enable/Open, publicación de variables, Disable/Enable, bloqueo de dependencias y Uninstall.
- Ningún resultado de pruebas físicas se presupone aprobado.
