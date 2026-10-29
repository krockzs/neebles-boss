# N.E.E.B.L.E.S. Boss v2.0.10

## Operaciones privilegiadas
- Añade el proveedor OS de operaciones genéricas con IPC AF_UNIX y systemd socket activation.
- Autorización por política root explícita, sin comandos ni argumentos arbitrarios suministrados por módulos.
- Identidad del proceso ligada al ELF root-controlled y al cgroup del servicio `neebles-runtime.service`.
- Operaciones denegadas por defecto en ausencia de política.
- Los servicios del proveedor se despliegan separadamente después de instalar Boss; no están activados por la release misma.

## Compatibilidad
- Conserva Boss 2.0.9, Governor, middleware IPC y CUSTOM clásico / Esbirro.
- No añade lógica específica de Proxy ni Tailscale a Boss.
