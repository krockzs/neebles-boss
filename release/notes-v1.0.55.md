# N.E.E.B.L.E.S. Boss v1.0.55

## Idioma global mediante Settings de módulos

- Boss sincroniza `ui.language` en los Settings de cada módulo instalado que declare el campo como String en su contrato.
- Utiliza el mecanismo existente `module_setting_set`, que guarda la preferencia y publica los cambios ya contemplados por Settings; los módulos cerrados conservan el idioma seleccionado al volver a abrirse.
- La sincronización es genérica y no conoce el lenguaje, la interfaz, el Tray ni otros extras de cada módulo.
- Los módulos anteriores que no declaran `ui.language` no reciben una escritura de un campo desconocido.
- El módulo sigue siendo responsable de adaptar sus propias ventanas, servicios y Trays al idioma de sus Settings.

## Inicio de los proveedores Tray

- El Tray Manager fortalece la reconciliación genérica durante el arranque de sesión para cubrir carreras temporales en la disponibilidad del servicio.
- No se restauran los antiguos controladores privados de sesión de Test Module.

## Construcción y compatibilidad

- Boss 1.0.55 conserva la compilación Rust 1.98.1 y el mundo Qt 6.8.2 controlado por Esbirro.
- Los binarios Qt deben proceder de `DESTDIR` y superar la certificación del workflow; no se aceptan binarios del árbol de build ni Qt del host.
- Test Module 1.2.11 será el consumidor de referencia del contrato `ui.language`.
- La versión nueva debe superar las pruebas de release, materiales y 11 artefactos, seguidas de las pruebas reales en Ryzen.

## Estado de certificación

El candidato local fue compilado y pasó 171 pruebas de librería Rust (1 omitida) y Test Module 1.2.11 pasó 33 pruebas Python. Esto todavía no certifica el ciclo completo de instalación/reinicio, idiomas en vivo o autostart del Tray en el sistema instalado.

El botón **Reboot** del modal de Release Notes debe permanecer disponible y advertir que se guarde el trabajo antes de utilizarlo.
