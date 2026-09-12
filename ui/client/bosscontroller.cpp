#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QSet>
#include <QTimer>
#include <QVersionNumber>

BossController::BossController(QObject *parent)
    : QObject(parent)
{
    m_modulePollTimer = new QTimer(this);
    m_modulePollTimer->setInterval(1000);

    connect(
        m_modulePollTimer,
        &QTimer::timeout,
        this,
        &BossController::pollModuleRuntime
    );

    reload();
    m_modulePollTimer->start();
}


static bool applyLauncherPanelState(bool enabled)
{
    QString qdbus;

    if (QFileInfo::exists(QStringLiteral("/usr/bin/qdbus6")))
        qdbus = QStringLiteral("/usr/bin/qdbus6");
    else if (QFileInfo::exists(QStringLiteral("/usr/bin/qdbus")))
        qdbus = QStringLiteral("/usr/bin/qdbus");
    else
        return false;

    QString script = QStringLiteral(R"JS(
var enabled = %1;
var ps = panels();

/*
 * Always remove current N.E.E.B.L.E.S. panel instances first.
 */
for (var p = 0; p < ps.length; ++p) {
    var ws = ps[p].widgets();

    for (var i = ws.length - 1; i >= 0; --i) {
        if (
            ws[i].type == "org.neebles.launcher"
            || ws[i].type == "org.neebles.spacer"
        ) {
            ws[i].remove();
        }
    }
}

if (enabled) {
    var targetPanel = null;
    var kickoffX = 0;
    var kickoffWidth = 0;

    for (var p = 0; p < ps.length; ++p) {
        var ws = ps[p].widgets();

        for (var i = 0; i < ws.length; ++i) {
            if (
                ws[i].type == "org.kde.plasma.kickoff"
                || ws[i].type == "org.kde.plasma.kicker"
            ) {
                targetPanel = ps[p];
                kickoffX = ws[i].geometry.x;
                kickoffWidth = ws[i].geometry.width;
                break;
            }
        }

        if (targetPanel)
            break;
    }

    if (targetPanel) {
        var launcherX = kickoffX + kickoffWidth + 4;

        targetPanel.addWidget(
            "org.neebles.launcher",
            launcherX,
            16,
            38,
            38
        );

        targetPanel.addWidget(
            "org.neebles.spacer",
            launcherX + 38,
            16,
            20,
            38
        );
    }
}
)JS").arg(enabled ? QStringLiteral("true")
                  : QStringLiteral("false"));

    QProcess process;

    process.start(
        qdbus,
        {
            QStringLiteral("org.kde.plasmashell"),
            QStringLiteral("/PlasmaShell"),
            QStringLiteral(
                "org.kde.PlasmaShell.evaluateScript"
            ),
            script
        }
    );

    if (!process.waitForStarted(3000))
        return false;

    if (!process.waitForFinished(10000))
        return false;

    return process.exitStatus() == QProcess::NormalExit
        && process.exitCode() == 0;
}

QString BossController::commandPath() const
{
    const QString override = qEnvironmentVariable("NEEBLES_COMMAND");
    if (!override.isEmpty())
        return override;

    const QString installed = QStringLiteral("/usr/local/bin/neebles");
    if (QFileInfo::exists(installed))
        return installed;

    return QStringLiteral("neebles");
}

QByteArray BossController::run(const QStringList &arguments, bool privileged, int timeoutMs, bool *ok)
{
    QProcess process;
    process.setProcessChannelMode(QProcess::SeparateChannels);

    if (privileged) {
        QStringList elevated;
        elevated << commandPath();
        elevated << arguments;
        process.start(QStringLiteral("pkexec"), elevated);
    } else {
        process.start(commandPath(), arguments);
    }

    const bool started = process.waitForStarted(5000);
    const bool finished = started && process.waitForFinished(timeoutMs);
    const bool success = finished && process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0;

    if (ok)
        *ok = success;

    if (!success) {
        const QString error = QString::fromUtf8(process.readAllStandardError()).trimmed();
        if (!error.isEmpty())
            setStatusText(error);
    }

    return process.readAllStandardOutput();
}

QVariant BossController::parseJson(const QByteArray &data) const
{
    QJsonParseError error;
    const QJsonDocument document = QJsonDocument::fromJson(data, &error);
    if (error.error != QJsonParseError::NoError)
        return {};
    return document.toVariant();
}

QString BossController::text(const QString &key) const
{
    return m_strings.value(key, key).toString();
}

QUrl BossController::assetUrl(const QString &name) const
{
    const QString clientRoot = qEnvironmentVariable("NEEBLES_CLIENT_ROOT");
    const QStringList candidates = {
        clientRoot.isEmpty() ? QString() : QDir(clientRoot).filePath(QStringLiteral("assets/branding/") + name),
        QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../assets/branding/") + name),
        QDir::current().filePath(QStringLiteral("client/assets/branding/") + name),
        QStringLiteral("/opt/neebles/client/assets/branding/") + name
    };

    for (const QString &path : candidates) {
        if (!path.isEmpty() && QFileInfo::exists(path))
            return QUrl::fromLocalFile(QFileInfo(path).absoluteFilePath());
    }
    return {};
}

QUrl BossController::flagUrl(const QString &name) const
{
    const QString clientRoot =
        qEnvironmentVariable("NEEBLES_CLIENT_ROOT");

    const QStringList candidates = {
        clientRoot.isEmpty()
            ? QString()
            : QDir(clientRoot).filePath(
                  QStringLiteral("assets/flags/4x3/") + name
              ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral("../assets/flags/4x3/") + name
        ),

        QDir::current().filePath(
            QStringLiteral("client/assets/flags/4x3/") + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/flags/4x3/"
        ) + name
    };

    for (const QString &path : candidates) {
        if (
            !path.isEmpty()
            && QFileInfo::exists(path)
        )
            return QUrl::fromLocalFile(
                QFileInfo(path).absoluteFilePath()
            );
    }

    return {};
}

void BossController::reload()
{
    m_autoUpdateFailedVersions.clear();

    loadConfig();
    loadLanguages();
    loadTranslations();
    loadModules();
}

void BossController::loadConfig()
{
    bool ok = false;
    const QVariant value = parseJson(run({QStringLiteral("config"), QStringLiteral("show")}, false, 5000, &ok));
    if (!ok || !value.canConvert<QVariantMap>())
        return;

    const QVariantMap map = value.toMap();
    m_language = map.value(QStringLiteral("language"), QStringLiteral("en_US")).toString();
    m_trayEnabled = map.value(QStringLiteral("tray_enabled"), true).toBool();
    m_launcherEnabled = map.value(QStringLiteral("launcher_enabled"), true).toBool();
    m_normalNotifications =
        map.value(
            QStringLiteral("normal_notifications"),
            true
        ).toBool();

    m_updateNotifications =
        map.value(
            QStringLiteral(
                "module_update_notifications"
            )
        ).toMap();

    emit configChanged();
}

void BossController::loadLanguages()
{
    bool ok = false;
    const QVariant value = parseJson(run({QStringLiteral("i18n"), QStringLiteral("languages")}, false, 5000, &ok));
    if (!ok || !value.canConvert<QVariantMap>())
        return;
    m_languages = value.toMap().value(QStringLiteral("languages")).toList();
    emit languagesChanged();
}

void BossController::loadTranslations()
{
    bool ok = false;
    const QVariant value = parseJson(run({QStringLiteral("i18n"), QStringLiteral("dump")}, false, 5000, &ok));
    if (!ok || !value.canConvert<QVariantMap>())
        return;
    m_strings = value.toMap();
    ++m_translationsRevision;
    emit translationsChanged();
}

void BossController::loadModules()
{
    QVariantList combined;
    QVariantMap installedByName;
    QSet<QString> seen;

    /*
     * Read installed state first.
     */
    bool installedOk = false;

    const QVariant installedValue = parseJson(
        run(
            {
                QStringLiteral("modules"),
                QStringLiteral("installed")
            },
            false,
            5000,
            &installedOk
        )
    );

    if (
        installedOk
        && installedValue.canConvert<QVariantList>()
    ) {
        for (
            const QVariant &item :
            installedValue.toList()
        ) {
            QVariantMap map = item.toMap();

            const QString name =
                map.value(
                    QStringLiteral("name")
                ).toString();

            map.insert(
                QStringLiteral("installed"),
                true
            );

            installedByName.insert(name, map);
        }
    }

    /*
     * Merge remote registry information with installed state.
     */
    bool availableOk = false;

    const QVariant availableValue = parseJson(
        run(
            {
                QStringLiteral("modules"),
                QStringLiteral("available")
            },
            false,
            10000,
            &availableOk
        )
    );

    if (
        availableOk
        && availableValue.canConvert<QVariantList>()
    ) {
        for (
            const QVariant &item :
            availableValue.toList()
        ) {
            const QVariantMap remote = item.toMap();

            const QString name =
                remote.value(
                    QStringLiteral("name")
                ).toString();

            const QString remoteVersionString =
                remote.value(
                    QStringLiteral("version")
                ).toString();

            QVariantMap map;

            if (installedByName.contains(name)) {
                map =
                    installedByName.value(name)
                    .toMap();

                const QString installedVersionString =
                    map.value(
                        QStringLiteral("version")
                    ).toString();

                map.insert(
                    QStringLiteral("installed"),
                    true
                );

                map.insert(
                    QStringLiteral("installed_version"),
                    installedVersionString
                );

                map.insert(
                    QStringLiteral("remote_version"),
                    remoteVersionString
                );

                map.insert(
                    QStringLiteral("repo"),
                    remote.value(
                        QStringLiteral("repo")
                    )
                );

                /*
                 * Installed icon has priority because it belongs
                 * to the exact installed module version.
                 */
                if (
                    map.value(
                        QStringLiteral("icon")
                    ).toString().isEmpty()
                ) {
                    map.insert(
                        QStringLiteral("icon"),
                        remote.value(
                            QStringLiteral("icon")
                        )
                    );
                }

                const QVersionNumber installedVersion =
                    QVersionNumber::fromString(
                        installedVersionString
                    );

                const QVersionNumber remoteVersion =
                    QVersionNumber::fromString(
                        remoteVersionString
                    );

                const bool updateAvailable =
                    !installedVersion.isNull()
                    && !remoteVersion.isNull()
                    && QVersionNumber::compare(
                        installedVersion,
                        remoteVersion
                    ) < 0;

                map.insert(
                    QStringLiteral("update_available"),
                    updateAvailable
                );
            } else {
                map = remote;

                map.insert(
                    QStringLiteral("installed"),
                    false
                );

                map.insert(
                    QStringLiteral("enabled"),
                    false
                );

                map.insert(
                    QStringLiteral("installed_version"),
                    QString()
                );

                map.insert(
                    QStringLiteral("remote_version"),
                    remoteVersionString
                );

                map.insert(
                    QStringLiteral("update_available"),
                    false
                );
            }

            combined.append(map);
            seen.insert(name);
        }
    }

    /*
     * Keep locally installed modules visible even if somebody
     * removes them from the remote registry.
     */
    for (
        auto it = installedByName.constBegin();
        it != installedByName.constEnd();
        ++it
    ) {
        if (seen.contains(it.key()))
            continue;

        QVariantMap map = it.value().toMap();

        const QString installedVersionString =
            map.value(
                QStringLiteral("version")
            ).toString();

        map.insert(
            QStringLiteral("installed"),
            true
        );

        map.insert(
            QStringLiteral("installed_version"),
            installedVersionString
        );

        map.insert(
            QStringLiteral("remote_version"),
            QString()
        );

        map.insert(
            QStringLiteral("update_available"),
            false
        );

        combined.append(map);
    }

    m_modules = combined;
    emit modulesChanged();

    applyModuleLifecycle();
}

void BossController::pollModuleRuntime()
{
    bool ok = false;

    const QVariant value = parseJson(
        run(
            {
                QStringLiteral("modules"),
                QStringLiteral("installed")
            },
            false,
            5000,
            &ok
        )
    );

    if (
        !ok
        || !value.canConvert<QVariantList>()
    )
        return;

    QVariantMap runningByName;

    for (
        const QVariant &item :
        value.toList()
    ) {
        const QVariantMap module =
            item.toMap();

        runningByName.insert(
            module.value(
                QStringLiteral("name")
            ).toString(),
            module.value(
                QStringLiteral("running"),
                false
            ).toBool()
        );
    }

    QVariantList updated = m_modules;
    bool changed = false;

    for (int i = 0; i < updated.size(); ++i) {
        QVariantMap module =
            updated.at(i).toMap();

        if (
            !module.value(
                QStringLiteral("installed")
            ).toBool()
        )
            continue;

        const QString name =
            module.value(
                QStringLiteral("name")
            ).toString();

        const bool running =
            runningByName.value(
                name,
                false
            ).toBool();

        if (
            module.value(
                QStringLiteral("running"),
                false
            ).toBool()
            != running
        ) {
            module.insert(
                QStringLiteral("running"),
                running
            );

            updated[i] = module;
            changed = true;
        }
    }

    if (changed) {
        m_modules = updated;
        emit modulesChanged();
    }

    applyModuleLifecycle();
}

void BossController::applyModuleLifecycle()
{
    for (const QVariant &item : m_modules) {
        const QVariantMap module =
            item.toMap();

        if (
            !module.value(
                QStringLiteral("installed")
            ).toBool()
            || !module.value(
                QStringLiteral(
                    "update_available"
                )
            ).toBool()
        )
            continue;

        const QString name =
            module.value(
                QStringLiteral("name")
            ).toString();

        const QString remoteVersion =
            module.value(
                QStringLiteral(
                    "remote_version"
                )
            ).toString();

        const bool running =
            module.value(
                QStringLiteral("running"),
                false
            ).toBool();

        /*
         * Notify only once for each available
         * module/version combination.
         */
        if (
            m_normalNotifications
            && !remoteVersion.isEmpty()
            && m_updateNotifications
                .value(name)
                .toString()
                != remoteVersion
        ) {
            bool notificationOk = false;

            const QString message =
                running
                ? QStringLiteral(
                    "Hay una actualización para %1. "
                    "Guarda tu trabajo. "
                    "La actualización comenzará "
                    "automáticamente cuando cierres "
                    "la aplicación."
                ).arg(name)
                : QStringLiteral(
                    "Hay una actualización para %1. "
                    "Se instalará automáticamente."
                ).arg(name);

            run(
                {
                    QStringLiteral("notify"),
                    running
                        ? QStringLiteral("warning")
                        : QStringLiteral("info"),
                    QStringLiteral(
                        "N.E.E.B.L.E.S."
                    ),
                    message
                },
                false,
                5000,
                &notificationOk
            );

            if (notificationOk) {
                bool markOk = false;

                run(
                    {
                        QStringLiteral("config"),
                        QStringLiteral(
                            "module-update-notified"
                        ),
                        name,
                        remoteVersion
                    },
                    false,
                    5000,
                    &markOk
                );

                if (markOk) {
                    m_updateNotifications.insert(
                        name,
                        remoteVersion
                    );
                }
            }
        }

        /*
         * While running:
         * - Open is blocked by UI.
         * - update remains pending.
         * - Boss waits.
         *
         * As soon as running becomes false,
         * automatic update is scheduled.
         */
        if (running)
            continue;

        /*
         * Do not loop forever if pkexec/update
         * was cancelled or failed.
         *
         * Global refresh clears this suppression
         * and allows an explicit retry.
         */
        if (
            m_autoUpdateFailedVersions
                .value(name)
                .toString()
                == remoteVersion
        )
            continue;

        scheduleAutomaticUpdate(name);
    }
}

void BossController::scheduleAutomaticUpdate(
    const QString &name
)
{
    if (
        name.isEmpty()
        || m_autoUpdatesInFlight.contains(name)
    )
        return;

    m_autoUpdatesInFlight.insert(name);

    QTimer::singleShot(
        0,
        this,
        [this, name]() {
            performAutomaticUpdate(name);
        }
    );
}

void BossController::performAutomaticUpdate(
    const QString &name
)
{
    QString remoteVersion;

    for (const QVariant &item : m_modules) {
        const QVariantMap module =
            item.toMap();

        if (
            module.value(
                QStringLiteral("name")
            ).toString()
            == name
        ) {
            remoteVersion =
                module.value(
                    QStringLiteral(
                        "remote_version"
                    )
                ).toString();

            if (
                module.value(
                    QStringLiteral("running"),
                    false
                ).toBool()
            ) {
                m_autoUpdatesInFlight
                    .remove(name);
                return;
            }

            break;
        }
    }

    setBusy(true);

    bool ok = false;

    run(
        {
            QStringLiteral("modules"),
            QStringLiteral("update"),
            name
        },
        true,
        600000,
        &ok
    );

    m_autoUpdatesInFlight.remove(name);

    if (!ok) {
        if (!remoteVersion.isEmpty()) {
            m_autoUpdateFailedVersions.insert(
                name,
                remoteVersion
            );
        }

        setStatusText(
            QStringLiteral(
                "Update failed: %1"
            ).arg(name)
        );

        setBusy(false);
        return;
    }

    setStatusText(
        QStringLiteral("OK")
    );

    setBusy(false);

    /*
     * Re-read local manifest and remote registry.
     * If versions match, update_available disappears
     * and Open becomes available again.
     */
    loadModules();
}

void BossController::saveConfig(const QString &language,
                                bool trayEnabled,
                                bool launcherEnabled,
                                bool normalNotifications)
{
    const bool trayWasEnabled = m_trayEnabled;
    const bool launcherWasEnabled = m_launcherEnabled;
    const bool notificationsWereEnabled = m_normalNotifications;

    setBusy(true);

    bool ok = true;
    bool current = false;

    run({
        QStringLiteral("config"),
        QStringLiteral("set"),
        QStringLiteral("language"),
        language
    }, false, 5000, &current);
    ok = ok && current;

    run({
        QStringLiteral("config"),
        QStringLiteral("set"),
        QStringLiteral("tray_enabled"),
        trayEnabled
            ? QStringLiteral("true")
            : QStringLiteral("false")
    }, false, 5000, &current);
    ok = ok && current;

    run({
        QStringLiteral("config"),
        QStringLiteral("set"),
        QStringLiteral("launcher_enabled"),
        launcherEnabled
            ? QStringLiteral("true")
            : QStringLiteral("false")
    }, false, 5000, &current);
    ok = ok && current;

    /*
     * OFF: notify before disabling normal notifications.
     */
    if (!normalNotifications && notificationsWereEnabled) {
        bool notificationOk = false;

        run({
            QStringLiteral("notify"),
            QStringLiteral("info"),
            QStringLiteral("N.E.E.B.L.E.S."),
            QStringLiteral("Notificaciones desactivadas")
        }, false, 5000, &notificationOk);
    }

    run({
        QStringLiteral("config"),
        QStringLiteral("set"),
        QStringLiteral("normal_notifications"),
        normalNotifications
            ? QStringLiteral("true")
            : QStringLiteral("false")
    }, false, 5000, &current);
    ok = ok && current;

    /*
     * ON: enable first, then notify.
     */
    if (
        ok
        && normalNotifications
        && !notificationsWereEnabled
    ) {
        bool notificationOk = false;

        run({
            QStringLiteral("notify"),
            QStringLiteral("success"),
            QStringLiteral("N.E.E.B.L.E.S."),
            QStringLiteral("Notificaciones activadas")
        }, false, 5000, &notificationOk);
    }

    if (ok) {
        /*
         * Tray OFF is handled by the tray process itself when it
         * observes tray_enabled=false.
         *
         * On the OFF -> ON transition, Boss starts it again.
         */
        if (trayEnabled && !trayWasEnabled) {
            const QString trayPath =
                QStringLiteral(
                    "/opt/neebles/client/tray/neebles-tray"
                );

            if (QFileInfo::exists(trayPath))
                QProcess::startDetached(trayPath, {});
        }

        /*
         * Launcher is a real Plasma panel integration:
         * OFF removes launcher + spacer.
         * ON recreates both beside Kickoff.
         */
        if (launcherEnabled != launcherWasEnabled) {
            if (!applyLauncherPanelState(launcherEnabled))
                setStatusText(
                    QStringLiteral(
                        "Could not update Plasma launcher state"
                    )
                );
        }

        reload();

        if (statusText().isEmpty()
            || statusText() == QStringLiteral("OK")) {
            setStatusText(QStringLiteral("OK"));
        }
    }

    setBusy(false);
}

void BossController::runModuleOperation(const QString &operation, const QString &name, bool privileged)
{
    if (name.isEmpty())
        return;
    setBusy(true);
    bool ok = false;
    run(
        {
            QStringLiteral("modules"),
            operation,
            name
        },
        privileged,
        600000,
        &ok
    );

    if (
        operation == QStringLiteral("update")
        && !ok
    ) {
        for (const QVariant &item : m_modules) {
            const QVariantMap module =
                item.toMap();

            if (
                module.value(
                    QStringLiteral("name")
                ).toString()
                == name
            ) {
                const QString version =
                    module.value(
                        QStringLiteral(
                            "remote_version"
                        )
                    ).toString();

                if (!version.isEmpty()) {
                    m_autoUpdateFailedVersions
                        .insert(
                            name,
                            version
                        );
                }

                break;
            }
        }
    } else if (
        operation
        == QStringLiteral("update")
    ) {
        m_autoUpdateFailedVersions
            .remove(name);
    }

    setStatusText(
        ok
        ? QStringLiteral("OK")
        : QStringLiteral("ERROR")
    );

    loadModules();
    setBusy(false);
}

void BossController::installModule(
    const QString &name
)
{
    if (name.isEmpty())
        return;

    setBusy(true);

    bool installed = false;

    run(
        {
            QStringLiteral("modules"),
            QStringLiteral("install"),
            name
        },
        true,
        600000,
        &installed
    );

    bool enabled = false;

    if (installed) {
        run(
            {
                QStringLiteral("modules"),
                QStringLiteral("enable"),
                name
            },
            false,
            5000,
            &enabled
        );
    }

    const bool ok =
        installed && enabled;

    setStatusText(
        ok
        ? QStringLiteral("OK")
        : QStringLiteral("ERROR")
    );

    loadModules();
    setBusy(false);
}

void BossController::updateModule(
    const QString &name
)
{
    m_autoUpdateFailedVersions.remove(name);

    runModuleOperation(
        QStringLiteral("update"),
        name,
        true
    );
}

void BossController::uninstallModule(const QString &name)
{
    runModuleOperation(QStringLiteral("uninstall"), name, true);
}

void BossController::openModule(
    const QString &name
)
{
    if (name.isEmpty())
        return;

    for (int i = 0; i < m_modules.size(); ++i) {
        QVariantMap module =
            m_modules.at(i).toMap();

        if (
            module.value(
                QStringLiteral("name")
            ).toString()
            != name
        )
            continue;

        if (
            !module.value(
                QStringLiteral("installed")
            ).toBool()
            || !module.value(
                QStringLiteral("enabled")
            ).toBool()
            || module.value(
                QStringLiteral("running"),
                false
            ).toBool()
            || module.value(
                QStringLiteral(
                    "update_available"
                ),
                false
            ).toBool()
        )
            return;

        qint64 processId = 0;

        const bool started =
            QProcess::startDetached(
                commandPath(),
                {
                    name,
                    QStringLiteral("open")
                },
                QString(),
                &processId
            );

        if (!started) {
            setStatusText(
                QStringLiteral(
                    "Could not open module: %1"
                ).arg(name)
            );
            return;
        }

        /*
         * Hide Open immediately.
         * The backend runtime marker becomes
         * authoritative on the next poll.
         */
        module.insert(
            QStringLiteral("running"),
            true
        );

        m_modules[i] = module;
        emit modulesChanged();

        QTimer::singleShot(
            250,
            this,
            &BossController::pollModuleRuntime
        );

        return;
    }
}

void BossController::setModuleEnabled(const QString &name, bool enabled)
{
    runModuleOperation(enabled ? QStringLiteral("enable") : QStringLiteral("disable"), name, false);
}

void BossController::setBusy(bool value)
{
    if (m_busy == value)
        return;
    m_busy = value;
    emit busyChanged();
}

void BossController::setStatusText(const QString &value)
{
    if (m_statusText == value)
        return;
    m_statusText = value;
    emit statusTextChanged();
}
