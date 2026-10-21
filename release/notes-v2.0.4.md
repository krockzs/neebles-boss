# N.E.E.B.L.E.S. Boss v2.0.4 — candidato

## Integración genérica de servicios y módulos

- Se incorpora la sesión autenticada `RegisterService` del IPC, exclusiva para servicios propios declarados como `systemd --user`, independiente del runtime `Open`.
- Boss verifica identidad del proceso, pertenencia al cgroup de la unidad declarada, UID y ejecutable antes de admitir la publicación de variables.
- El canal de servicio solo permite publicar variables y hacer Ping/Unregister; no entrega privilegios para Invoke, Settings ni administración.
- La interfaz Boss se suscribe a cambios `module.lifecycle` y refresca el catálogo para actualizar condiciones `depends_install` como las de Secrets y Connect.
- Enable/Disable gobierna la activación de los servicios declarados, sin retirar sus unidades ni borrar datos persistentes.
- Se mantienen intactas la gestión `Keep/Replace/Add/Retire` incorporada en 2.0.3 y la separación entre datos privados y archivos instalados del módulo.

## Certificación pendiente

- Esta versión requiere validación del protocolo de servicio, compilación Qt controlada de Esbirro y prueba instalada antes de considerarse aceptada.
- El modal de Release Notes conserva la advertencia de guardar el trabajo y el botón Reboot.
- Esta nota describe el candidato; no afirma publicación ni pruebas Live que todavía no se han realizado.
