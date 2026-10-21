# N.E.E.B.L.E.S. Boss v2.0.2

## Registry declarativo y dependencias

- Registry schema 3: `short_name`, `depends_install` y mensajes localizados en archivos independientes por modulo.
- El catalogo solo contiene NEEBLES Secrets y NEEBLES Connect; Test Module permanece como repositorio de referencia pero se retira del catalogo.
- Connect declara dependencia de instalacion por presencia de Secrets; mientras Secrets no este instalado, Boss oculta Install y muestra una advertencia en ingles o espanol.
- Install comprueba la dependencia tambien en el backend; no instala requisitos en cascada.
- El grafo de dependencias instaladas protege a los proveedores requeridos frente a Uninstall y Disable.

## Variables y servicios

- Se incorpora infraestructura generica de publicacion de variables por IPC gobernado y consulta interna del estado persistente.
- Esta release no exige aun que Secrets publique variables de clave maestra o desbloqueo: esa integracion corresponde al trabajo posterior entre Secrets y Connect.
- El ciclo de Update de servicios persistentes conserva las restricciones de seguridad existentes hasta que su integracion transaccional este completa; no se declara aqui certificada una actualizacion real de esos servicios.

## Instalacion y aceptacion

- Compilacion de fuente Rust verificada en Ryzen; el paquete de release debe certificarse por separado con el workflow versionado, sus artefactos e instalacion real.
- El modal Release Notes conserva el boton Reboot; guarda tu trabajo antes de reiniciar.
- Esta nota describe el candidato v2.0.2; no afirma publicacion ni aceptacion instaladas por si sola.
