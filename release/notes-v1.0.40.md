# N.E.E.B.L.E.S. Boss v1.0.40

Versión de prueba en NEEBLES OS basada en el backend 1.0.39.

- Introduce las defensas B8–B42 para actualización transaccional y diagnóstico de recuperación.
- Agrega `neebles modules recovery-diagnostics <module>` por IPC contra el Boss persistente.
- Conserva B8: **las actualizaciones con procesos extra activos continúan bloqueadas** hasta certificación E2E.
- No habilita recuperación automática destructiva ni altera el `governor.open` canónico.

Validación local previa: 807/807 pruebas unitarias exitosas en 1.0.39.
La publicación requiere superar el workflow de empaquetado y certificación de la versión 1.0.40.
La instalación en NEEBLES OS y sus pruebas reales son posteriores a esta publicación.
