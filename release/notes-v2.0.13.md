# N.E.E.B.L.E.S. Boss v2.0.13

## Surface Open — etiqueta opcional gobernada

- `other-label-open` es un booleano opcional del `manifest.json` de un módulo (schema 4). Ausente equivale a `false`, preservando la acción visual **Open** para módulos existentes.
- Cuando el módulo declara `true`, Boss presenta en la pestaña Módulos la etiqueta resuelta desde sus propias traducciones de Surface, según `ui.language` global y el fallback del manifiesto de idiomas del módulo.
- La etiqueta puede ser Config, Dashboard u otra declarada por el módulo. El nombre de la acción interna sigue siendo `governor.open`: el Governor, Lifecycle y RuntimeRegistry no se sustituyen por acciones especiales.
- Las declaraciones alternativas sin `governor.open` o sin etiqueta Surface traducida y coherente se rechazan antes de la instalación del módulo.
- No se impone el campo a Secrets, Connect ni a otros consumidores existentes.

## Surface: botones alternantes genéricos

- `toggle-button` funciona en Boss UI / Config, Launcher y Tray usando el mismo objeto Lifecycle y las acciones on/off declaradas.
- La presentación resuelve etiquetas de estado desde los diccionarios del módulo (`label_on_key`, `label_off_key`) sin incorporar textos de sus consumidores en el código de Boss.
- Toda acción continúa por `surface-action` y las verificaciones de requisitos y contratos existentes.
- Proxy Client 1.0.1 será el primer consumidor con `other-label-open: true`; esta release no publica ni instala ese módulo.

## Alcance, certificación y compatibilidad

- La fuente de Boss se versiona a 2.0.13, manteniendo el proveedor universal de operaciones OS incorporado en 2.0.12.
- CUSTOM clásico continúa como fuente certificada del runtime de Boss; CUSTOM V2 continúa separadamente como autoridad de materiales de módulos. No se añaden paquetes ni cambios a ambos corpora por esta capacidad de interfaz.
- El pipeline de release debe compilar Rust 1.98.1 y Qt 6.8.2 bajo Esbirro, generar exactamente los 11 artefactos, ejecutar `verify-release.py` y publicar esos mismos bytes certificados.
- La compilación local F25C sobre la fuente previa al versionado aprobó UI, Tray Host y los dos ELF del Launcher Plugin. La CI de 2.0.13 debe reconstruir/certificar el producto versionado completo; no se reutilizan binarios con número 2.0.12 como artefactos finales.
- La instalación, actualización, control de idioma y comportamiento en runtime real requieren aceptación aparte y no se presumen por la construcción de fuentes.
