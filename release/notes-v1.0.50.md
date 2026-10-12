# N.E.E.B.L.E.S. Boss v1.0.50

## Corrección de actualización

- El instalador acepta la transición solo cuando la versión candidata es mayor que la instalada, sin origen fijo; bloquea un downgrade antes de comenzar staging.
- Instalador, recibo de Critical Update y handoff usan el destino 1.0.50. La misma versión no se declara actualización.
- El gate de versionado verifica esos contratos.
- Nightmare, motor Rust de actualización, módulos y Esbirro/Qt 6.8.2 se conservan sin cambios.

Pendiente: prueba de Critical Update en NEEBLES OS instalado.
