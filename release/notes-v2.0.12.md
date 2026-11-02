# N.E.E.B.L.E.S. Boss v2.0.12

## Proveedor OS universal de operaciones privilegiadas

- Contrato opcional por módulo `manifest.platform_operations` y `contracts/platform-operations.json`; sin ramas ni comandos específicos de módulos en Boss.
- `require_privilege` omitido o `false` conserva operaciones normales de Lifecycle; `true` solo se admite para `boss.platform_operation`.
- Install/Update solicita reconciliación de política únicamente para módulos con contrato privilegiado. **Una solicitud o instalación no concede aprobación root.**
- El OS exige aprobación administrativa independiente, vinculada a los SHA-256 exactos de `manifest.json`, contrato de operaciones y ELF certificado. Modificarlos obliga a renovar autorización.
- Uninstall y rollback gobiernan la revocación o reconciliación. El proveedor autentica identidad, ámbito de operación y política; el journal bloquea operaciones ante estado incierto.
- El proveedor root se integra solamente cuando OS suministra AuthoritySupply; si falta, Boss y los módulos ordinarios funcionan y las operaciones privilegiadas fallan de forma cerrada.
- Connect 1.0.2 y Secrets 1.0.7 no requieren `platform_operations`, ni `require_privilege=true`; no se les eleva ni se les exige proveedor root.

## Certificación y publicación

- Rust 1.98.1 y los componentes Qt 6.8.2 deben compilarse y certificarse por los workflows controlados de Boss/CUSTOM classic/Esbirro.
- La fuente CUSTOM classic se fija por SHA inmutable en el workflow; los hashes de material físico certificado no se regeneran por cambios solo documentales.
- El workflow conserva **11 artefactos y `SHA256SUMS`**. La release publicada debe reutilizar exactamente los bytes certificados, sin recompilación.
- La prueba fuente y la generación de candidato no sustituyen aceptación instalada, actualización, servicios reales, rollback ni autorización OS efectiva.
- No incluye código, política ni tecnología privada de Proxy Client, Proxy Server o Tailscale.
