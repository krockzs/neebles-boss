# N.E.E.B.L.E.S. Boss v2.0.0

## Selector multibanderas y traducciones

- Se incorporan 253 opciones de países y territorios al selector de idiomas de Boss, conservando `es_CL` como predeterminado y `en_US` como idioma alternativo.
- Cada opción tiene un JSON propio con las 149 claves exigidas por el contrato de Boss.
- Se reutilizan traducciones de lenguas compartidas entre países. Algunos mensajes complejos permanecen temporalmente en inglés; la paridad estructural no implica una certificación lingüística nativa completa.
- El selector existente, la persistencia de `ui.language`, Settings y la propagación genérica a módulos se conservan sin cambios de arquitectura.

## Construcción y certificación

- Versión de producto 2.0.0 alineada en Rust, UI, Installer, Auth Agent, Launcher, Tray Host, handoff e instalador.
- Se conservan Rust 1.98.1, Qt 6.8.2 en el mundo controlado de Esbirro y la certificación de los binarios Qt instalados desde `DESTDIR`.
- El workflow hereda el procedimiento 1.0.55, construye y verifica 11 artefactos y genera `bootstrap.json` y `SHA256SUMS` a partir de los resultados efectivos.
- Esta versión permanece **candidata** hasta que el workflow y las pruebas físicas de instalación y actualización superen sus controles.

## Compatibilidad y pruebas posteriores

- La traducción de cada módulo (ventanas, Tray y servicios) continúa siendo responsabilidad del propio módulo; Test Module se probará por separado.
- Verificar visualmente el selector, persistencia del idioma, Release Notes y reinicio en el sistema instalado.
- El botón **Reboot** sigue disponible; guarda tu trabajo antes de utilizarlo.
