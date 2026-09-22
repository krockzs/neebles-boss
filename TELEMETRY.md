# N.E.E.B.L.E.S. Telemetry / External

## Estado actual

Telemetry / External es la frontera opcional de diagnostico de N.E.E.B.L.E.S.

Actualmente la telemetria informa errores producidos por Boss o por modulos cuando el usuario la activa desde la UI.

La autoridad persistente es:

```text
telemetry.enabled
```

## Telemetry desactivada

Cuando telemetry.enabled es false, Boss retorna antes de construir package y message.

No se construye un ExternalEnvelope de produccion.
No se envia nada a external.sock.

Si Boss no puede leer la configuracion, Telemetry se considera desactivada.

Esto permite que N.E.E.B.L.E.S. funcione sin servidor remoto ni transporte remoto implementado.

## Telemetry activada

Cuando telemetry.enabled es true, los productores actuales pueden generar eventos External.

Actualmente Boss y Module IPC producen:

```text
type = error
```

Esto describe los productores actuales, no limita el contrato External a error.

## ExternalEnvelope

El contrato permanece generico:

```json
{
  "type": "<String>",
  "endpoint": "<String>",
  "activate": true,
  "package": {},
  "message": {}
}
```

type sigue siendo un String abierto.
endpoint sigue siendo un String abierto.

Las categorias historicas stage0 e incompatibility ya no forman parte del producto.

Eliminar esas categorias no impide que en el futuro aparezcan nuevos tipos de telemetria.

## Error

Para el tipo error, message contiene:

```json
{
  "code": "error_code",
  "message": "Human-readable diagnostic"
}
```

Boss puede producir errores desde ExecutionResponse.
Module IPC puede producir errores espontaneos informados por un runtime.

## external.sock

La frontera local es:

```text
/run/neebles/external.sock
```

External utiliza Unix SOCK_SEQPACKET.
Cada envelope viaja como un paquete independiente.
El receptor aplica un limite maximo de tamano.
El socket utiliza activacion mediante systemd.

El receptor valida las credenciales Unix del productor.
Actualmente acepta root y el desktop user de N.E.E.B.L.E.S.

La autorizacion local no reemplaza telemetry.enabled.

## Transporte remoto

Actualmente no existe transporte remoto implementado.
No existe servidor remoto definido por este contrato.
No existe endpoint remoto definitivo definido por este contrato.

external.sock es solamente la frontera local preparada para conectar un transporte futuro.

## Invariantes actuales

Telemetry apagada: no construir, no enviar.
Telemetry activada: errores actuales pueden entrar a external.sock.
type permanece abierto como String.
endpoint permanece abierto como String.
El transporte remoto queda para una fase futura.
