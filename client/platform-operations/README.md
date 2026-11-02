# N.E.E.B.L.E.S. OS — operaciones privilegiadas universales

Este directorio contiene el contrato de distribución de proveedor OS para Boss.

El motor es global: `neebles.platform.operation` autentica la presencia del proveedor. El proveedor root usa `/etc/neebles/platform-operations/<id>.json` para dar permisos por módulo y operación. Ningún nombre, tecnología o comando privado de un módulo está en el código de Boss.

## Declaración de cada módulo

El manifiesto schema 4 agrega el campo opcional:

```json
"platform_operations": "contracts/platform-operations.json"
```

Solo se admite esa ubicación fija. Su contenido es:

```json
{
  "schema": "1",
  "module": "ejemplo-modulo",
  "operations": {
    "operacion": {
      "executable": "/usr/lib/neebles/module-operations/ejemplo-modulo-platform",
      "argv": ["--accion"]
    }
  }
}
```

El mismo checkout autenticado del módulo debe incluir su ELF nativo en `runtime/<id>-platform`. La fuente se certifica con su SHA/commit de Registry y los mundos/ELF mediante CUSTOM V2 y Esbirro según las reglas de cada módulo.

### Autorización

El módulo expresa una *solicitud*. Un Install/Update autorizado por el Gobernador toma la fuente ya autenticada y solicita a OS reconciliar una política estrecha: módulo, operación, ELF y argumentos fijos. Antes de sincronizar, OS exige una aprobación root independiente vinculada al SHA-256 exacto de `manifest.json`, `contracts/platform-operations.json` y `runtime/<id>-platform`. Install/Update NO crea esa aprobación. La política es propiedad de root, no del módulo. No se usa `sudo` desde Lifecycle ni se ejecuta ninguna ruta arbitraria suministrada por IPC. La presencia de un archivo o ELF no es una concesión de permiso.

### Compatibilidad con módulos sin privilegios

`require_privilege` es una munición opcional de Lifecycle: ausente o `"false"` conserva la ejecución normal; `"true"` solo corresponde a `boss.platform_operation`. El campo del manifiesto `platform_operations` también es opcional. Connect y Secrets no lo declaran, por lo que no pasan por sync/approve, no requieren root policy y no deben verse afectados por su ausencia. Si OS no suministra AuthoritySupply, Boss conserva su modo ordinario y no levanta el proveedor privilegiado; una solicitud privilegiada falla cerrada.

### Aprobación independiente de OS

El responsable del sistema certifica previamente el candidato con Esbirro/CUSTOM y lo copia a un directorio absoluto, root-owned y no escribible por usuarios sin privilegios. Solo luego invoca la interfaz OS administrativa, **fuera de Boss y del canal de módulo**:

```text
python3 -I /usr/lib/neebles/platform-operations/provider.py approve \
  --module <module-id> --candidate-dir <directorio-root-del-candidato> \
  --confirm-root-authority
```

La aprobación guarda únicamente hashes y queda bajo `/etc/neebles/platform-operations/approvals/`. Es una acción administrativa explícita, no un permiso concedido por instalar archivos ni por un socket. Toda modificación de identidad/contrato/ELF invalida la aprobación anterior. `audit-approval` y `revoke-approval` son acciones del mismo proveedor administrativo y no se exponen como endpoints de módulo. La ruta de candidato debe contener un subdirectorio final que coincida exactamente con el ID del módulo.

### Transacciones

- Install: registro después de publicar el checkout y MaterialBinding, antes del Lifecycle install; si falla, la transacción de instalación revierte.
- Update: sincronización tras publicar el nuevo checkout; un fallo posterior requiere restaurar política anterior antes de reactivar servicios.
- Uninstall: revocación después del punto de no retorno; el proveedor rechaza el módulo ausente incluso si la limpieza OS falla.
- Interrupción: journal persistente por módulo y bloqueo de operaciones mientras exista una transacción pendiente. Reconciliación bajo la identidad root de Boss.

Los módulos que no solicitan operaciones de plataforma no reciben privilegios. Los módulos nuevos nunca requieren recompilar Boss si ya existe la capacidad necesaria.

## Fronteras de propiedad

Esbirro certifica los ELF y mundos. CUSTOM conserva los materiales. NEEBLES OS suministra el proveedor/autoridad. Boss gobierna la autorización y la ejecución. Un nuevo módulo no puede ejecutar herramientas de host como fallback.

La publicación sigue gates distintos: código fuente, compilación Rust + Qt/Esbirro, certificación de artefactos, actualización en vivo y rollback real.
