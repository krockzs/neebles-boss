# N.E.E.B.L.E.S. Boss v2.0.9

## Cambios
- Artillería genérica `boss.platform_operation` para solicitar operaciones de plataforma a un proveedor autorizado.
- Protección declarativa `require_privilege` y denegación por defecto cuando falta la autorización.
- Cliente IPC Unix para proveedor externo, separado del manejo de tecnologías específicas.
- Candidato de proveedor de operaciones con políticas explícitas, límites de salida y validaciones de identidad de proceso; su despliegue privilegiado requiere certificación independiente.

## Compatibilidad y límites
- Conserva el middleware IPC y la semántica de solicitudes de 2.0.8.
- No incorpora operaciones específicas de Tailscale ni de Proxy Server a Boss.
- El proveedor privilegiado no se activa automáticamente por el mero hecho de publicar el ejecutable Boss.

## Gate de publicación
- `python3 scripts/check-release-version.py --version 2.0.9`
- Compilación y pruebas Rust del workflow sobre el commit publicado.
- Qt 6.8.2 construido desde CUSTOM clásico y certificado por Esbirro según el workflow fijado.
- Artefactos únicamente desde los directorios de staging certificados.
