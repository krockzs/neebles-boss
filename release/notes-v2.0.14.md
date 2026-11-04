# N.E.E.B.L.E.S. Boss v2.0.14 — candidato F41

## Operaciones privilegiadas: ruta administrativa y aprobación previa

- El proveedor OS admite exclusivamente la identidad exacta del ELF root-owned de Boss,
  con credenciales UID 0, desde su cgroup de servicio o desde los subcomandos
  administrativos `modules install`, `modules update` y `modules uninstall`.
- Los módulos y los procesos de usuario no obtienen permiso por esta compatibilidad.
  Las operaciones de plataforma siguen siendo responsabilidad del proveedor OS.
- `platform.policy.preflight` es una consulta **de solo lectura** para verificar
  que el administrador OS aprobó los hashes del manifiesto, contrato y ELF del
  candidato antes de que Install/Update altere la instalación o detenga servicios.
- La aprobación sigue siendo una acción administrativa root separada, no un
  efecto de descargar, instalar o ejecutar Lifecycle.
- Si no existe autorización exacta para los bytes de la nueva versión, el
  preflight debe fallar antes del quiesce y de la transacción de publicación.
- El administrador OS conserva las aprobaciones anteriores en recibos
  históricos root-owned vinculados a hashes, para que una versión anterior
  restaurada pueda recuperar su autorización durante rollback.

## Puertas de certificación

- Fuente candidata. No es release publicada ni prueba de aceptación en VivoProBook.
- Requiere tests Python del proveedor OS, Rust fmt/check/tests, CI controlada de
  Esbirro y auditoría de rollback/upgrade con aprobación independiente antes de publicar.
- CUSTOM clásico, CUSTOM V2, Client, Secrets y Connect no se alteran.
