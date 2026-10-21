# N.E.E.B.L.E.S. Boss v2.0.3 — candidato

## Actualización de servicios persistentes de módulos

- El Update de módulos incorpora las transiciones declarativas `Keep`, `Replace`, `Add` y `Retire` para servicios `systemd --user` pertenecientes al módulo.
- `Keep` conserva la definición de unidad y reinicia el proceso que estaba activo para cargar la versión actualizada.
- `Replace` actualiza la definición de unidad gobernada y reanuda el servicio según su estado anterior.
- `Add` instala/habilita/inicia el servicio declarado por la versión nueva.
- `Retire` detiene/deshabilita/retira las unidades que ya no están declaradas.
- El Update no elimina los datos privados del módulo, incluidas bóvedas o Master Keys conservadas fuera del árbol instalable.

## Alcance y certificación

- Se conservan Registry schema 3, `depends_install`, IPC de variables y demás capacidades de Boss 2.0.2.
- El ciclo de Update de servicios no está certificado físicamente hasta probar una actualización real en Ryzen.
- La compilación de código y pruebas seleccionadas son gates independientes de release y aceptación Live.
- El modal de Release Notes mantiene el botón Reboot y la advertencia de guardar el trabajo.
- Esta nota describe el candidato. No afirma que la release ya esté publicada.
