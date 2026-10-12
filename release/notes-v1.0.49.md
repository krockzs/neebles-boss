# N.E.E.B.L.E.S. Boss v1.0.49

## Cambios de esta versión

- Unifica Release Notes de Boss y de los módulos en un solo diálogo, con advertencia de guardar el trabajo y botón Reiniciar disponible en ambos contextos. La solicitud de reinicio ocurre después del cierre del diálogo.
- Conserva el recibo de Critical Update de Boss y su confirmación persistente; las notas del módulo se muestran solamente después de verificar un cambio real de versión instalada.
- Corrige el identificador del módulo en los eventos de actualización y muestra la versión remota en el botón Critical Update de cada módulo, utilizando la ruta de actualización transaccional existente.
- Incorpora validación preventiva del `critical-update/manifest.json` de módulos: `[]` y ausencia heredada son compatibles; entradas inválidas o instrucciones sin ejecutor certificado se rechazan antes de desactivar la instalación vigente.
- Preserva Nightmare, el ejecutor Critical Update de Boss, Lifecycle, MaterialBinding, Multi-RootFS y rollback existentes.

## Evidencia de desarrollo (no equivale a certificación)

- B5/R3 en Ryzen, 2026-10-12: pruebas Rust del binario 714/714; biblioteca 171 aprobadas, una ignorada.
- Build Rust release (`cargo build --release --locked --bins`, Rust 1.98.1) GREEN.
- Idiomas es_CL/en_US: 149 claves en paridad, `git diff --check` GREEN antes del versionado.

## Puertas de aceptación que siguen pendientes

- Pipeline real de release de Boss con Qt 6.8.2 controlado por Esbirro, staging DESTDIR, certificaciones, materialización doméstica y `verify-release.py` sobre 11 assets.
- Publicación explícita después de verificar el candidato y actualización real de Boss instalado; pruebas visuales de diálogo, recibo y reboot.
- Prueba de actualización real de Test Module 1.2.3 a 1.2.4, con versión instalada verificada y notas localizadas. No se reemplaza 1.2.3 antes de esta prueba.

No se anuncia ejecución de instrucciones críticas no vacías en módulos: ese contrato permanece bloqueado hasta que exista ejecutor propio certificado.
