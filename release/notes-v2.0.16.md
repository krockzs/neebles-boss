# N.E.E.B.L.E.S. Boss v2.0.16 — candidato para certificación

## Aprobación administrativa de operaciones OS

- El proveedor OS permite revisar las operaciones privilegiadas declaradas y sus
  rutas/argumentos sin emitir un permiso por revisar.
- La autorización root independiente requiere una decisión expresa del
  administrador, ejecutada mediante el agente Polkit de Boss.
- El recibo OS se vincula al módulo y a los SHA-256 exactos de manifiesto,
  contrato y ELF certificado; se vuelve a validar antes de registrar autoridad.
- Install/Update conserva temporalmente un candidato root para su revisión y
  vuelve a comprobar la transacción completa después de aprobar, antes de
  detener servicios o publicar una nueva instalación.
- Se mantienen los controles de denegación, revocación, transacciones y rollback.

## Puertas de calidad

- F61: 76 pruebas Python, controles Rust, y compilación/certificación DESTDIR de
  Boss UI y auth-agent en el entorno Esbirro Qt 6.8.2.
- F62: alineación de metadatos de versión con controles de fuente.
- **Pendiente antes de publicar:** generación/certificación del paquete completo
  y aceptación de actualización real en equipo objetivo con permisos OS.

La instalación de Boss no concede automáticamente privilegios a módulos.
