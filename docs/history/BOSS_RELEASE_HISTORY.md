# N.E.E.B.L.E.S. Boss — Historico de releases y gates

> REGISTRO HISTORICO de desarrollo (no un manual de ejecucion para Boss 2.0.0). Se conserva el texto anterior de `README.md` sin reinterpretar los resultados originales. En las pruebas antiguas, un resultado GREEN de fuente no implica una aceptacion instalada de la version actual. Los SHAs/versions historicos no deben repinearse por copiar estos ejemplos.

---

# Evidencia histórica de cierre local (versiones anteriores)

Point 1 corrective work and the existing Point 2 source architecture were checked on the exact working tree before this commit.

These are **local source verification gates**, not Esbirro certification, not a published Boss release and not Fresh Live / installed-system acceptance.

Evidencia de cierre local registrada para una línea anterior (no representa las pruebas actuales de 1.0.42):

```text
git diff --check                                  GREEN
module_materialization focused suite             14 / 14 GREEN
Boss library suite                               120 / 120 GREEN
Boss main suite                                  647 / 647 GREEN
domesticacion executed tests                      13 GREEN
domestic-root certification test                   1 intentionally ignored
cargo check --locked                              GREEN
cargo build --release --locked --bins             GREEN
real certified perl-base SHA256 gate              GREEN
perl/perl5.40.1 staging hardlink identity         GREEN
perl/perl5.40.1 merged hardlink identity          GREEN
destination symlink-parent escape gate            GREEN
```

The real regression package is:

```text
perl-base_5.40.1-6+deb13u1_amd64.deb
SHA256 b795464137a0f4d443fc9284f4b93e883fb83883cb533adf300ac660807a352a
```

Its `usr/bin/perl5.40.1` entry is a TAR hardlink to `usr/bin/perl`. The controlled 1.0.28 materializer preserves that relationship after extraction and after layer merge.

Point 1 source closure now verifies:

- Essential + per-module delta material composition;
- MaterialBinding schema 2;
- authenticated `construction.json` stored inside the binding with SHA256;
- Preinstall Construction fetch from the exact pinned CUSTOM V2 revision;
- Construction subject validation before binding publication;
- productive Construction resolution from active MaterialBinding;
- authenticated ephemeral RuntimeLease;
- confined native TAR hardlink materialization inside RuntimeLease staging;
- preservation of hardlink identity across staging -> candidate/composed layer merge;
- destination-parent symlink confinement during preflight and publication;
- fail-closed rejection of unsupported special TAR entry types;
- concurrent independent RuntimeLeases;
- install/update/uninstall MaterialBinding transaction semantics;
- Governor-owned module identity propagation into prepared Lifecycle operations;
- workspace execution accepting exactly `munition.step` and rejecting module-supplied subject identity.

Point 2 source closure additionally verifies:

- Tray contracts declare a generic `construction_step`;
- Tray providers are not directly executed from the host path;
- Tray provider birth uses module-owned Construction;
- `modules.runtime` creates and retains the authenticated RuntimeLease;
- the declared world remains module/CUSTOM V2 truth, never Boss technology knowledge;
- session-local readonly resources are granted by `platform.desktop_session_interface`;
- Workspace re-authenticates session readonly grants before boundary composition;
- dynamic Construction environment injection is sealed to the `NEEBLES_*` namespace;
- host execution inputs such as `PATH`, `LD_PRELOAD`, `LD_LIBRARY_PATH` and `PYTHONPATH` cannot be injected through that overlay;
- Tray runtime ownership uses PID incarnation identity: `pid + start_time_ticks`;
- the Construction supervisor owns a dedicated process group;
- lifecycle stop signals only the authenticated governed group;
- loss or change of authenticated leader identity becomes UNKNOWN/UNSAFE and cannot authorize group signaling;
- Tray registration authenticates the real provider PID through kernel `SO_PEERCRED`;
- stale waiters cannot erase ownership for a newer Tray runtime.

**Point 1 corrective source closure and Point 2 source closure are locally GREEN.**

The Config/Features source closure additionally verifies:

- Surface schema 2 typed `require` declarations;
- strict `active` and RuntimeRegistry-backed `open` resolution;
- requirement preservation through Projection -> SurfaceContent -> JSON;
- persistent `surface-model` requirement evaluation;
- persistent `surface-action` execution keyed by owner + item_id + action;
- backend requirement revalidation before Lifecycle execution;
- module-scoped Surface translations;
- dynamic Config -> Features inventory materialization;
- install/update/rollback/uninstall/startup Feature reconciliation;
- Modules remaining administrative-only;
- bounded Launcher and Tray presentation lists;
- generic Lifecycle `boss.module_ipc` Commands execution;
- Test Module Notify button and Notify switch tutorial behavior;
- canonical switch state committed only after successful governed transition;
- Test Module Launcher and Tray Open projections.

**Config/Features source closure is locally GREEN.**

Release certification, Fresh Live acceptance and installed-system acceptance remain separate later gates.

## Boss 1.0.29 CAST29 local closure (histórico)

Fresh Live acceptance of published Boss 1.0.28 proved that package transport, the 91-package material world, MaterialBinding schema 2 and native RuntimeLease hardlink materialization were no longer the blocking frontier.

The next integration frontier exposed four independent defects:

- governed Tray provider birth was attempted from the desktop Tray Manager instead of the persistent Boss runtime authority;
- Boss Config Surface projection rejected C++ `QVariantList` values through JavaScript `Array.isArray()` checks;
- Launcher dynamic module content could be clipped below the fixed module-list viewport;
- Launcher `surfaceActionCommand()` contained a JavaScript automatic-semicolon-insertion return defect.

CAST29 closes those source defects generically:

- Tray Manager remains the desktop presentation/session coordinator and requests provider lifecycle from persistent Boss;
- persistent Boss owns governed Construction, RuntimeLease and provider process lifecycle;
- start, stop, reconcile and runtime-deactivate IPC boundaries validate installed module identity before touching runtime state;
- `runtime.env` carries the certified desktop UID/GID, runtime identity and session Tray socket into persistent Boss;
- Boss UI accepts QML array-like `QVariantList` / nested SurfaceContent values;
- Launcher height follows real `moduleList.contentHeight`;
- Launcher Surface action command construction returns the command value correctly;
- post-install enable receives a bounded 600-second execution window for legitimate first RuntimeLease composition.

Local CAST29 gates:

```text
git diff --check                                  GREEN
bash -n scripts/install.sh                        GREEN
Boss library suite                               120 / 120 GREEN
Boss main suite                                  650 / 650 GREEN
persistent runtime IPC confinement tests           3 / 3 GREEN
domesticacion executed tests                      13 GREEN
domestic-root certification test                   1 intentionally ignored
cargo check --locked                              GREEN
```

These remain source/local gates. Fresh Live and installed-system acceptance remain separate.
## Boss 1.0.30 — CAST30 source closure (histórico)

The CAST30 source gates close:

- B3A: persistent Boss module-Governor-action IPC and governed lifecycle results.
- B3B: canonical closed/opening/open state, atomic single-flight Open, generation/execution/PID ownership.
- B4: module-wide Launcher/Tray visibility and conservative 1.0.29 settings migration.
- B5: separate desktop-UID Notification Presenter, user-session authority, SO_PEERCRED,
  Present/Presented/Activate ownership handshake and authenticated return routing.

The recorded Gate 75 passed 665 Boss main tests, 120 library tests,
15 ownership tests, 3 Presenter tests and 13 domesticacion tests
(1 intentionally ignored). This does not imply installed-system acceptance.

## Boss 1.0.31 — CAST30 post-Fresh-Live repairs (registro histórico)

- Synchronize persistent runtime shutdown with RuntimeRegistry deregistration before disabling a module; preserve fail-closed behavior if cleanup fails.
- Retain diagnostic errors from the Boss UI operation transport rather than replacing them with a generic failure.
- Repair Tray Host selected-module QML scope and absolute icon URLs.
- Permit Installed -> Uninstall while a module runtime is running; Boss still owns lifecycle, dependency checks and the shutdown transaction.
- Consume Test Module v1.2.2 (commit `ee91d94b1026e7890379c656dbdee2896d00e32b`), including asynchronous `notification_closed` IPC handling.
- CUSTOM V2 source truth: `c0afe5baa6a58f50166ed12f17a1637a667b4d88`, with module-material manifest version 1.2.2 and Construction fetch/checkout bound to the same module commit.
- Local evidence: 805 Rust tests passed (one intentionally ignored), 12 Test Module Python tests passed, Boss UI/Tray Host stage builds passed with controlled Esbirro Qt 6.8.2. These results precede the v1.0.31 release build and are not Fresh Live acceptance.
- The published v1.0.30 release remains unchanged; full v1.0.31 CI and 11-asset verification precede trigger publication, then Fresh Live and installed-system acceptance.

## Boss 1.0.32 — CAST30 Active/Inactive and Open cross-surface synchronization (registro histórico)

- Send committed module state changes from short-lived CLI callers to the persistent Boss event owner; the latter publishes `module.lifecycle` for UI/Launcher/Tray subscribers. The module state in `ui.disabled_modules` remains canonical and distinct from Settings presentation toggles.
- Handle exited Tray process-group leaders conservatively: accept an orphaned group as quiescent only when kernel `/proc` inspection certifies that every visible member is terminated; unknown identities or live members fail closed.
- Gates131/132: 810 Rust tests passed, 1 intentionally ignored; 12 Active relay/tray and 25 Boss Settings source contracts passed. The 32-case visibility matrix is a **source specification**, not Live acceptance.
- Global `tray.enabled`, `launcher.enabled`, per-module surface visibility, locale, normal notifications and telemetry must not be altered by module Active/Inactive changes; hidden surfaces must recover canonical state when restored.
- Registry remains Test Module 1.2.2 (`ee91d94b1026e7890379c656dbdee2896d00e32b`); CUSTOM V2 remains `c0afe5baa6a58f50166ed12f17a1637a667b4d88`; pinned CUSTOM Classic/Esbirro remains `20488f6818d5e227f043425a682115f614f4bf79`.
- Gate137 Open projection: Boss UI retains its administrative Open control, enabled only for declared `governor.open` when the module is active and runtime is closed. Launcher/Tray Open remains optional SurfaceContent; object-owned Open actions remain module-declared. `RuntimeRegistry` is the single-flight authority, keyed by module, for `closed`, `opening`, `open`.
- Gate137: 815 Rust tests passed, 0 failed, 1 intentionally ignored; new tests cover single-module concurrent Open requests from three synthetic callers, independence between different modules, closed/opening/open presentation, and hidden Launcher/Tray without changing canonical state. These are **source tests, not actual GUI clicks or Fresh Live**.
- v1.0.32 release, genuine Settings/visibility matrix and actual GUI end-to-end Active/Inactive + Open tests remain pending. No release may be accepted before the live triplet is verified.

## Política de release actual — 1.0.42

- **Versión estable precedente:** 1.0.41. Preservar su release y punto de restauración.
- **Versión candidata:** 1.0.42. No afirmar que está publicada o funcional antes de los resultados reales.
- El código y el README se mantienen en `neebles-boss`; `Downloads` se utiliza únicamente para ejecuciones e informes.
- **Esbirro / CUSTOM:** seguir `neebles-custom/portable/esbirro/README.md` y el workflow `release-1.0.42.yml`, que utiliza el laboratorio controlado Qt 6.8.2 y los materiales Rust/domésticos certificados; no sustituirlo por compilación Qt del host.
- Antes de activar el trigger: versionado consistente (`Cargo`, CMake, C++, QML, metadata), ambas traducciones, release notes `es_CL`/`en_US`, pruebas estáticas y estado Git controlado.
- Ejecutar el workflow de **compilación y certificación del candidato**. Comprobar su resultado y la integridad de los **11 artefactos** previstos. Una ejecución verde no implica publicación automática.
- Publicar en un paso separado exactamente los artefactos certificados; no recompilar el candidato para publicarlo.
- Pruebas posteriores sobre NEEBLES OS instalado: (1) Critical Update desde 1.0.41 y verificación del escritorio, Launcher, Spacer, Tray y modal; (2) habilitar inicio automático y minimizado, reiniciar sesión/equipo y comprobar apertura minimizada, persistencia y apertura manual visible.
- Si las pruebas fallan, la 1.0.41 sigue siendo la base de recuperación. Registrar el defecto concreto; no atribuir resultados a pruebas no ejecutadas.

### Alcance explícito de 1.0.42

1. Handoff de Critical Update sin `systemctl --user restart plasma-plasmashell.service`.
2. Configuración de autostart y arranque minimizado de **Boss UI**. El inicio autónomo de otros módulos queda para cada módulo, no para esta release.


---

# Evolución de versiones 1.0.36 a 1.0.42

Las siguientes notas proceden de los archivos de release versionados en este repositorio.
Se conservan como historial técnico; una nota de candidato no demuestra una prueba en máquina real.
La 1.0.41 es la referencia estable previa y la 1.0.42 es código fuente pendiente de compilación y prueba.

## Versión 1.0.36 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.36 — Critical Update handoff and release-notes candidate

## Scope

- Boss in-place upgrade: post-install handoff to close the authenticated old Qt UI, restart only the Plasma shell service and relaunch the updated UI.
- Persistent user-owned Critical Update receipt and localized one-time Boss release-notes modal after a verified version transition.
- Module update path: validate staged release notes and show localized notes only after verifying the installed version transition; no Boss or Plasma restart for ordinary module updates.
- Keep the existing CUSTOM classic / CUSTOM V2 authority separation and the controlled Esbirro Qt 6.8.2 DESTDIR certification path.
- Synchronize version 1.0.36 across Rust, Qt clients, Installer, Auth Agent, Tray Host, Launcher plugin and Plasma metadata.

## Verification status before release

- Rust tests: 896 passed, 2 ignored; release backend and static MUSL resolver compiled.
- Controlled Esbirro Qt 6.8.2: Boss UI, Installer, Auth Agent, Launcher plugin and Tray Host built and staged, with pre-domestication certification.
- Domestic Boss runtime transport: certified; local 1.0.36 release candidate materialized and passed `verify-release.py`.
- In-place installed Boss 1.0.35 → 1.0.36 Critical Update, one-time modal, restart ordering and real module update flow remain unverified.
- Qt QML import warnings and UTF-8 locale warnings occurred during controlled chroot build; runtime acceptance must assess them.

## Critical Update acceptance

- Keep installed Boss 1.0.35 unchanged until candidate, rollback and publication are approved.
- Verify installed target version, receipt provenance, authenticated UI shutdown, Plasma shell-only restart and new UI launch.
- Confirm es_CL/en_US Boss notes appear once after a true upgrade and not on a clean install or no-op.
- Test module 1.2.3 clean install (no notes), then 1.2.4 true upgrade (localized notes) with tray operation and no Boss/Plasma restart.

## Publication policy

The release workflow produces a candidate artifact; it does not automatically publish a GitHub Release. Do not create or modify `release/trigger-v1.0.36` before candidate preflight approval. Publishing is a separate, explicitly authorized step after candidate certification.

## Versión 1.0.37 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.37 — Post-update handoff and module release-notes correction

## Scope
- Install and validate the authenticated post-update handoff helper from packaged client-data.
- Maintain the existing verified upgrade receipt and handoff barrier, then restart only `plasma-plasmashell.service` and launch the new Boss UI.
- Display the one-time Boss release-notes modal after a genuine installed-version transition.
- Display a separate module update notes dialog only after a verified module upgrade with authenticated non-empty notes. Initial installs do not show notes.
- Preserve pinned CUSTOM Esbirro and runtime material.

## Acceptance pending
- Real Boss 1.0.36 → 1.0.37 Critical Update: installed version, receipt, handoff log, scoped Plasma restart and one-time notes modal.
- Real module 1.2.3 → 1.2.4 update: notes must be included in the module payload; verify the popup, with no Boss or Plasma restart.
- Do not claim runtime acceptance from build success alone.

## Publication
GitHub Actions produces only the candidate artifact. Publish a release only after candidate verification and approval.

## Versión 1.0.38 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.38 — Receipt permissions and desktop handoff

## Changes
- The privileged installer validates ownership and safe filesystem nodes before acting on the desktop user's configuration tree.
- The dedicated `~/.config/neebles` directory is automatically secured to private mode 0700, including from a legitimate user-owned mode 0775. No manual chmod or alteration of the parent `~/.config` is required.
- Failures in receipt generation include a specific diagnostic and explicitly report that handoff was skipped.
- A genuine Boss upgrade creates the one-time receipt, launches the installed handoff helper, restarts only `plasma-plasmashell.service`, reopens Boss, and offers localized release notes. Those runtime effects require real-device acceptance.
- Preserves module release-notes modal functionality from 1.0.37; an actual verified module update is required to accept it.

## Acceptance (not implied by candidate build)
- Real 1.0.37 → 1.0.38 upgrade on Ryzen and VivoProBook: verified receipt, handoff log, Plasma service restart, new Boss UI and localized one-time modal.
- Real verified module update with authenticated release notes: separate modal without restarting Boss or Plasma.

GitHub Actions generates an audited candidate only. Publishing is a separate explicitly authorized step.

## Versión 1.0.39 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.39 — Isolated desktop upgrade handoff

- Launch post-upgrade handoff through a separate transient `systemd --user` service, outside the retiring Boss UI cgroup.
- Fail closed if the helper is not executing in its expected dedicated service.
- Preserve authenticated upgrade receipt, verified old-UI D-Bus identity and pidfd termination, Plasma-only restart and automatic Boss relaunch.
- Add durable transition logging for diagnosing real-device behavior.
- Preserve the existing Boss and module release-notes dialog functionality.

The CI candidate alone cannot certify the on-device behavior. Validate a real 1.0.38 → 1.0.39 upgrade before marking desktop handoff accepted.

## Versión 1.0.40 — sin notas históricas disponibles

No existe el archivo de notas de esta versión en el repositorio local. Este README no reconstruye ni atribuye cambios sin una fuente verificable.

## Versión 1.0.41 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.41 — Recovery baseline

- Rebuild from the exact functional source baseline of v1.0.39 (commit 4a35114).
- Restore the isolated desktop upgrade handoff and existing CAST30 packaging behavior.
- Exclude v1.0.40 B8-B42 changes.
- Update product, installer, UI, launcher, tray, handoff and release version identities to 1.0.41.
- Candidate build and installed-system acceptance are pending independent verification.

## Versión 1.0.42 — notas de la versión

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

---

---

# Fresh Live acceptance boundary

Source certification is not final integrated-system certification.

The first acceptance phase uses **one Test Module only**.

```text
Fresh Live
    -> Boss install
    -> Test Module install
    -> Preinstall authenticates Essential + module delta
    -> permanent package pools are correct
    -> active MaterialBinding matches installed module truth
    -> Modules remains administrative-only
    -> Config -> Features materializes Notify button + Notify switch
    -> Launcher exposes Open
    -> Tray exposes Open Surface
    -> Notify button `active` requirement resolves correctly
    -> Notify switch is disabled before runtime registration
    -> Open enters Lifecycle
    -> construction.step selects open-runtime
    -> modules.runtime creates independent RuntimeLease
    -> Essential + Test Module delta compose in the lease rootfs
    -> certified hardlink/material semantics survive RuntimeLease composition
    -> desktop identity/session authority
    -> persistent runtime retains RuntimeLease for real process lifetime
    -> Module IPC registration
    -> Notify switch becomes enabled only while `open` is satisfied
    -> UI opens
    -> Notify button reaches Boss-governed desktop notification
    -> Notify switch notification transition succeeds
    -> canonical switch state changes only after successful transition
    -> runtime close makes `open` requirement fail closed
    -> reopen restores `open` requirement
    -> Tray provider enters through tray-provider Construction
    -> Tray provider registers through kernel peer identity
    -> settings and switches communicate Boss <-> module
    -> state changes are reflected through canonical surfaces
    -> disable / enable works
    -> uninstall removes Feature inventory and runtime/module state
    -> reinstall rematerializes valid Feature inventory
    -> reinstall succeeds
```

After the complete Fresh Live sequence is GREEN:

```text
Calamares install
    -> boot installed N.E.E.B.L.E.S. OS
    -> repeat the complete single-module acceptance sequence
```

Only after the single-module Live + installed-system cycle is GREEN will the reference Test Module be cloned into another repository under a **different module identity**, for example `test-module-2`.

The complete acceptance battery will then be repeated with both modules simultaneously to prove:

```text
two installed module identities
    -> both project independently in Launcher and Tray
    -> both can Open
    -> independent MaterialBindings
    -> independent RuntimeLeases
    -> shared Essential material
    -> no mutable shared rootfs
    -> isolated settings and switches
    -> uninstall/reinstall of one does not disturb the other
    -> repeat after Calamares installation
```

The obsolete phrase **real 47-DEB materialization** is not current architecture.

Current certified material truth:

```text
Essential layer          59 DEBs
Test Module delta        32 DEBs
runtime composition      Essential + module delta
ownership                MaterialBinding + RuntimeLease
```

Only those later integrated gates permit an installed-system production-ready claim.

---

---

## NEEBLES Boss 1.0.43 — Correcciones pendientes de prueba


- Critical Update: evita la destrucción/recreación de plasmoides Launcher/Spacer activos mediante evaluateScript. Las instancias y geometría de Plasma quedan preservadas. El nuevo QML se recarga al iniciar una nueva sesión de Plasma, sin reiniciar Plasma durante el update.
- Startup: al desactivar inicio automático fuerza `startup.minimized=false`; imposibilita activar minimizado sin autostart y bloquea el switch desactivado en UI.
- UI: una invocación manual `neebles start` intenta recuperar y mostrar la ventana de una instancia previamente oculta mediante D-Bus.

Importante: esta versión debe compilarse con Esbirro Qt 6.8.2 y probarse en NEEBLES OS. No afirmar correcciones verificadas en runtime hasta probarlas.

## Boss 1.0.44 — Inicio minimizado (candidato)

- Inicio minimizado: solicitar showMinimized() después de registrar /BossWindow en D-Bus, para crear una ventana minimizada real y recuperable.
- Mantiene correcciones 1.0.43 de Critical Update y de los interruptores Startup.
- Servicio de sesión conserva After=graphical-session.target: no se ha demostrado que sea necesaria una espera adicional específica de KWin o Plasma.

Validación pendiente: compilación certificada Esbirro Qt 6.8.2, Rust 1.98.1, pruebas instaladas en X11 y eventualmente Wayland. No afirmar resolución hasta aceptación real.

## Boss 1.0.45 — Critical Update cgroup handoff

La identidad de la unidad transitoria aislada se deriva de TARGET; deja de depender de la versión anterior fija. Compilación Esbirro y aceptación instalada pendientes.

## Boss 1.0.46 — Launcher/Spacer surfaces and Tray

Las superficies gráficas se regeneran con Loader tras detectar cambios estables en los archivos instalados, sin eliminar applets. Se verifica mediante la próxima actualización 1.0.46 → 1.0.47.


## Boss 1.0.49 — Release Notes compartidas y actualización segura de módulos

- Una única presentación localizada de Release Notes para Boss y módulos, siempre con advertencia previa y botón de reinicio. El reinicio se solicita al cerrar el diálogo; no se añade un segundo modal.
- Mantiene intactos la confirmación del recibo de Boss y el requisito de transición verificada de versión de módulo.
- Corrige identidad de actualización de módulo (`transactionOperation.module`), y la tarjeta muestra Critical Update con la versión remota sin reemplazar el updater transaccional.
- `critical-update/manifest.json` de módulos se valida ANTES de desactivar la versión anterior: `[]`/ausencia legada permitidos, instrucciones sin ejecutor rechazadas. Nightmare de Boss permanece intacto.
- B5/R3 local Rust: 714/714 pruebas del binario; 171/171 pruebas activas de biblioteca (una ignorada); build Rust release GREEN con toolchain 1.98.1.
- **No certificar por estos tests:** Qt 6.8.2/Esbirro, DESTDIR, 11 assets, release publicada y aceptación instalada siguen pendientes. Test Module 1.2.3 permanece para el salto real a 1.2.4.


## Boss 1.0.50 — actualización de versión y handoff

- Critical Update solo desde una versión instalada inferior a la versión candidata, sin origen fijo.
- Igual versión no genera transición de actualización; los downgrades se rechazan.
- Instalador, recibo y handoff usan el destino 1.0.50 y conservan sus verificaciones.
- Nightmare, motor Rust de Critical Update, módulos y Esbirro/Qt 6.8.2 sin cambios.
- Pendiente: prueba real de actualización en Ryzen.


## Boss 1.0.51 — Launcher Open y presentación de transacciones

- El Launcher vincula su botón Open con la disponibilidad gobernada desde una lectura nueva del Surface Model (`closed`, `opening`, `open`), sin alterar contratos ni Governor.
- El módulo puede abrirse desde Boss UI, Launcher o Tray; la apertura única permanece defendida por RuntimeRegistry.
- La acción Open de Boss UI no muestra Module Transaction; Install, Update, Uninstall y Critical Update mantienen sus transacciones.
- Requiere certificación del pipeline Esbirro/Qt y validación visual real antes de declararse estable.
