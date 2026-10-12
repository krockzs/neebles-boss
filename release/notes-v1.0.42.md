# N.E.E.B.L.E.S. Boss v1.0.42

## Español (es_CL)

### 1. Critical Update sin reiniciar Plasma
Critical Update sustituye el reinicio completo de `plasma-plasmashell.service` por la renovación selectiva de Launcher, Spacer y Tray de NEEBLES. El objetivo es mantener el escritorio KDE Plasma activo y preservar las preferencias y la disposición de sus componentes.

### 2. Inicio automático y minimizado de Boss
Se incorporan dos opciones independientes en **Config**: iniciar Boss con la sesión de NEEBLES OS e iniciar su ventana minimizada. Las preferencias persisten en la configuración de Boss; abrirlo manualmente continúa mostrando la ventana normalmente. Los módulos administran su propio inicio, sin depender del arranque de Boss UI.

## English (en_US)

### 1. Critical Update without a Plasma restart
Critical Update replaces a full restart of `plasma-plasmashell.service` with a selective refresh of NEEBLES Launcher, Spacer, and Tray. The goal is to keep the KDE Plasma desktop running while retaining component preferences and layout.

### 2. Automatic and minimized Boss startup
Two independent **Config** options control whether Boss starts with the NEEBLES OS desktop session and whether its window starts minimized. Settings persist across sessions; manual launches still open the window normally. Modules manage their own startup independently of Boss UI.

## Estado de validación / Validation status
Los cambios están integrados en el código fuente; compilación, prueba funcional y publicación de 1.0.42 siguen pendientes.
Changes are integrated in source; the 1.0.42 build, functional verification, and publication are still pending.
