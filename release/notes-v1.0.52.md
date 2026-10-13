# N.E.E.B.L.E.S. Boss v1.0.52

## Desinstalación transaccional de módulos — TXN V5

- Incorpora limpieza gobernada de recursos declarados, configuraciones,
  material autenticado y servicios systemd-user.
- Verifica la identidad y el ExecStart efectivo de los servicios antes
  de intervenirlos, rechazando sustituciones y hooks no autorizados.
- Mantiene recursos recuperables durante la fase reversible.
- Ante un fallo previo al commit, restaura el estado transaccional y
  recupera los servicios previamente activos cuando es seguro hacerlo.
- Separa el rollback reversible de la limpieza posterior al commit,
  sin intentar restaurar recursos eliminados irreversiblemente.
- Conserva la sincronización entre inventario instalado, archivos
  del módulo y MaterialBinding.

## Correcciones adicionales

- Aísla la prueba del marcador de identidad Tray en un entorno temporal,
  evitando modificar permisos del runtime productivo.
