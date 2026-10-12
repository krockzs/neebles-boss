# N.E.E.B.L.E.S. Boss v1.0.44

## Corrección en preparación

- Inicio minimizado: solicitar showMinimized() después de registrar /BossWindow en D-Bus, para crear una ventana minimizada real y recuperable.
- Mantiene correcciones 1.0.43 de Critical Update y de los interruptores Startup.
- Servicio de sesión conserva After=graphical-session.target: no se ha demostrado que sea necesaria una espera adicional específica de KWin o Plasma.

Validación pendiente: compilación certificada Esbirro Qt 6.8.2, Rust 1.98.1, pruebas instaladas en X11 y eventualmente Wayland. No afirmar resolución hasta aceptación real.
