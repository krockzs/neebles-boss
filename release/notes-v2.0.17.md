# N.E.E.B.L.E.S. Boss v2.0.17 — candidato para certificación

## Ajuste de aislamiento del proveedor OS

- El servicio `neebles-platform-operations.service` pasa de `ProtectHome=yes` a
  `ProtectHome=read-only`, manteniendo `/home` y `/root` inaccesibles mediante
  `InaccessiblePaths=/home /root`. Esto permite consultar el socket de Bridge en
  `/run/user/<uid>/neebles/proxy-client/bridge.sock` sin exponer directorios personales.
- Se preservan la autenticación del proveedor, su socket root `0600`, la política
  por módulo, aprobaciones SHA-256, `NoNewPrivileges=yes`, el confinamiento
  `ProtectSystem=strict`, `RestrictAddressFamilies=AF_UNIX` y los controles de identidad.
- No cambia el ejecutable ni el contrato de Proxy Client, Secrets o CUSTOM clásico.
- Se incorpora una regresión estática para el servicio y su socket.

## Puertas independientes de publicación

1. Gate de fuente y regresiones Rust/Python/shell.
2. Compilación y certificación de binarios Rust y Qt 6.8.2 en Esbirro con
   material CUSTOM clásico fijado, además de la construcción del runtime Boss.
3. Empaquetado, verificación de los 11 artefactos y sus SHA-256 exactos.
4. GitHub Release nueva sin recompilar los artefactos certificados.
5. Instalación controlada, pruebas de no-regresión y verificación de Activate
   en VivoProBook; recuperación si alguna prueba falla.

**Estado actual:** candidato de fuente únicamente. Ninguna de estas puertas
se considera aprobada antes de aportar su evidencia correspondiente.
