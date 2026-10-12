# N.E.E.B.L.E.S. Boss v1.0.43

## Correcciones respecto de 1.0.42 (pendiente de certificación en equipo real)

- Critical Update: evita la destrucción/recreación de plasmoides Launcher/Spacer activos mediante evaluateScript. Las instancias y geometría de Plasma quedan preservadas. El nuevo QML se recarga al iniciar una nueva sesión de Plasma, sin reiniciar Plasma durante el update.
- Startup: al desactivar inicio automático fuerza `startup.minimized=false`; imposibilita activar minimizado sin autostart y bloquea el switch desactivado en UI.
- UI: una invocación manual `neebles start` intenta recuperar y mostrar la ventana de una instancia previamente oculta mediante D-Bus.

Importante: esta versión debe compilarse con Esbirro Qt 6.8.2 y probarse en NEEBLES OS. No afirmar correcciones verificadas en runtime hasta probarlas.
