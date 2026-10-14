# N.E.E.B.L.E.S. Boss v1.0.54

## Puente Tray Surface gobernado

- El Tray Manager admite solicitudes tipadas `surface_query` y `surface_action` de un proveedor registrado y autenticado de su propio módulo.
- La consulta comprueba identidad del proveedor, Surface `tray` visible y proyección del estado de ejecución del módulo.
- La acción resuelve exclusivamente una acción declarada y delega su ejecución al dispatcher de Surface y al Governor de Boss. El proveedor no obtiene acceso administrativo libre.
- El Tray Manager realiza reconciliación genérica con reintentos al inicio de sesión, sin depender de servicios privados por módulo.
- Conserva el comportamiento existente de Settings, Tray y Launcher y los contratos de desinstalación de recursos propiedad de cada módulo.

## Compatibilidad

Test Module 1.2.10 utiliza el puente nuevo para abrir su UI desde el Tray y ya no necesita un controlador de sesión privado. CUSTOM V2 debe usar el commit autenticado del módulo y conceder al proveedor el socket de `neebles/tray.sock`, sin montar el antiguo socket privado. El catálogo de módulos debe seleccionar Test Module 1.2.10 solo después de publicar e instalar Boss 1.0.54.

## Certificación

El candidato de código superó previamente 974 pruebas (Python y Rust), sin fallos y con dos casos omitidos por condiciones de entorno. La release debe superar el workflow de build, las verificaciones de 11 artefactos y la publicación verificable antes de instalarse.

El ciclo físico Ryzen (Install, Enable, Open, Close, autostart, reinicio y Uninstall) sigue pendiente y no se declara aprobado. La auditoría de idiomas queda fuera de esta release.
