#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QRegularExpression>
#include <QSet>
#include <QTimer>
#include <QVersionNumber>

BossController::BossController(QObject *parent)
    : QObject(parent)
{
    /*
     * Runtime state is local and cheap.
     */
    m_modulePollTimer =
        new QTimer(this);

    m_modulePollTimer->setInterval(1000);

    connect(
        m_modulePollTimer,
        &QTimer::timeout,
        this,
        &BossController::pollModuleRuntime
    );

    /*
     * DEMO:
     * remote module update check every 30 seconds.
     *
     * Production contract:
     * 5 minutes = 300000 ms.
     */
    m_updatePollTimer =
        new QTimer(this);

    m_updatePollTimer->setInterval(300000);

    connect(
        m_updatePollTimer,
        &QTimer::timeout,
        this,
        &BossController::pollModuleUpdates
    );

    reload();

    m_modulePollTimer->start();
    m_updatePollTimer->start();
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

static bool applyTrayServiceState(
    const QString &systemctl,
    const QString &service,
    bool enabled
)
{
    QProcess process;

    process.start(
        systemctl,
        {
            QStringLiteral("--user"),

            enabled
                ? QStringLiteral("enable")
                : QStringLiteral("disable"),

            QStringLiteral("--now"),

            service
        }
    );

    if (
        !process.waitForStarted(
            3000
        )
    ) {
        return false;
    }

    if (
        !process.waitForFinished(
            10000
        )
    ) {
        process.kill();
        process.waitForFinished();

        return false;
    }

    return
        process.exitStatus()
            == QProcess::NormalExit
        && process.exitCode() == 0;
}

static bool applyTrayState(
    bool enabled
)
{
    QString systemctl;

    if (
        QFileInfo::exists(
            QStringLiteral(
                "/usr/bin/systemctl"
            )
        )
    ) {
        systemctl =
            QStringLiteral(
                "/usr/bin/systemctl"
            );
    } else {
        systemctl =
            QStringLiteral(
                "systemctl"
            );
    }

    const QString manager =
        QStringLiteral(
            "neebles-tray-manager.service"
        );

    const QString host =
        QStringLiteral(
            "neebles-tray-host.service"
        );

    /*
     * The Tray is one user-facing surface composed of two services.
     *
     * ON:
     *   Manager first, because it owns tray.sock.
     *   Qt Host second, because it consumes tray.sock.
     *
     * OFF:
     *   Qt Host first, so the visual consumer disappears cleanly.
     *   Manager second.
     */
    if (enabled) {
        if (
            !applyTrayServiceState(
                systemctl,
                manager,
                true
            )
        ) {
            return false;
        }

        if (
            !applyTrayServiceState(
                systemctl,
                host,
                true
            )
        ) {
            /*
             * Do not leave a half-enabled Tray if the visual Host
             * could not be activated.
             */
            applyTrayServiceState(
                systemctl,
                manager,
                false
            );

            return false;
        }

        return true;
    }

    const bool hostOk =
        applyTrayServiceState(
            systemctl,
            host,
            false
        );

    const bool managerOk =
        applyTrayServiceState(
            systemctl,
            manager,
            false
        );

    return hostOk && managerOk;
}


QString BossController::commandPath() const
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_COMMAND"
        )
    ) {
        const QString override =
            qEnvironmentVariable(
                "NEEBLES_COMMAND"
            ).trimmed();

        if (override.isEmpty())
            return {};

        return override;
    }

    const QString installed =
        QStringLiteral(
            "/usr/local/bin/neebles"
        );

    if (QFileInfo::exists(installed))
        return installed;

    return QStringLiteral("neebles");
}

QString BossController::authorizationPath() const
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_AUTH_AGENT"
        )
    ) {
        const QString override =
            qEnvironmentVariable(
                "NEEBLES_AUTH_AGENT"
            ).trimmed();

        if (
            override.isEmpty()
            || !QFileInfo(override).isExecutable()
        ) {
            return {};
        }

        return QFileInfo(
            override
        ).absoluteFilePath();
    }

    const QStringList candidates = {
        QStringLiteral(
            "/opt/neebles/client/auth/neebles-auth-agent"
        ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../auth/neebles-auth-agent"
            )
        ),

        QDir::current().filePath(
            QStringLiteral(
                "ui/auth-agent/build/neebles-auth-agent"
            )
        )
    };

    for (const QString &candidate : candidates) {
        if (
            QFileInfo(candidate).isExecutable()
        ) {
            return QFileInfo(
                candidate
            ).absoluteFilePath();
        }
    }

    return {};
}


QByteArray BossController::run(
    const QStringList &arguments,
    bool privileged,
    int timeoutMs,
    bool *ok
)
{
    QProcess process;

    process.setProcessChannelMode(
        QProcess::SeparateChannels
    );

    const QString bossCommand =
        commandPath();

    if (bossCommand.isEmpty()) {
        if (ok)
            *ok = false;

        setStatusText(
            QStringLiteral(
                "NEEBLES_COMMAND is explicitly set but empty"
            )
        );

        return {};
    }

    if (privileged) {
        const QString authAgent =
            authorizationPath();

        if (authAgent.isEmpty()) {
            if (ok)
                *ok = false;

            setStatusText(
                text(
                    QStringLiteral(
                        "auth.agent_missing"
                    )
                )
            );

            return {};
        }

        QString operation =
            QStringLiteral("generic");

        QString name;
        QString fromVersion;
        QString toVersion;

        bool running = false;

        if (
            arguments.size() >= 2
            && arguments.at(0)
                == QStringLiteral("modules")
        ) {
            const QString action =
                arguments.at(1);

            if (
                action
                == QStringLiteral("install")
            ) {
                operation =
                    QStringLiteral(
                        "install-module"
                    );
            } else if (
                action
                == QStringLiteral("update")
            ) {
                operation =
                    QStringLiteral(
                        "update-module"
                    );
            } else if (
                action
                == QStringLiteral("uninstall")
            ) {
                operation =
                    QStringLiteral(
                        "uninstall-module"
                    );
            }

            if (arguments.size() >= 3)
                name = arguments.at(2);

            for (
                const QVariant &item :
                m_modules
            ) {
                const QVariantMap module =
                    item.toMap();

                if (
                    module.value(
                        QStringLiteral("name")
                    ).toString()
                    != name
                )
                    continue;

                fromVersion =
                    module.value(
                        QStringLiteral(
                            "installed_version"
                        ),
                        module.value(
                            QStringLiteral(
                                "version"
                            )
                        )
                    ).toString();

                toVersion =
                    module.value(
                        QStringLiteral(
                            "remote_version"
                        )
                    ).toString();

                if (toVersion.isEmpty()) {
                    toVersion =
                        module.value(
                            QStringLiteral(
                                "version"
                            )
                        ).toString();
                }

                running =
                    module.value(
                        QStringLiteral(
                            "running"
                        ),
                        false
                    ).toBool();

                break;
            }
        }

        QStringList elevated;

        elevated
            << QStringLiteral("--locale")
            << m_language

            << QStringLiteral("--operation")
            << operation;

        if (!name.isEmpty()) {
            elevated
                << QStringLiteral("--name")
                << name;
        }

        if (!fromVersion.isEmpty()) {
            elevated
                << QStringLiteral("--from")
                << fromVersion;
        }

        if (!toVersion.isEmpty()) {
            elevated
                << QStringLiteral("--to")
                << toVersion;
        }

        elevated
            << QStringLiteral("--running")
            << (
                running
                ? QStringLiteral("true")
                : QStringLiteral("false")
            );

        elevated
            << QStringLiteral("--")
            << bossCommand;

        elevated << arguments;

        process.start(
            authAgent,
            elevated
        );
    } else {
        process.start(
            bossCommand,
            arguments
        );
    }

    const bool started =
        process.waitForStarted(5000);

    const bool finished =
        started
        && process.waitForFinished(
            timeoutMs
        );

    const bool success =
        finished
        && process.exitStatus()
            == QProcess::NormalExit
        && process.exitCode() == 0;

    if (ok)
        *ok = success;

    if (!success) {
        const QString error =
            QString::fromUtf8(
                process.readAllStandardError()
            ).trimmed();

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
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CLIENT_ROOT"
        )
    ) {
        const QString clientRoot =
            qEnvironmentVariable(
                "NEEBLES_CLIENT_ROOT"
            ).trimmed();

        if (clientRoot.isEmpty())
            return {};

        const QString path =
            QDir(clientRoot).filePath(
                QStringLiteral(
                    "assets/branding/"
                ) + name
            );

        if (QFileInfo::exists(path)) {
            return QUrl::fromLocalFile(
                QFileInfo(path)
                    .absoluteFilePath()
            );
        }

        return {};
    }

    const QStringList candidates = {
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../assets/branding/"
            ) + name
        ),

        QDir::current().filePath(
            QStringLiteral(
                "client/assets/branding/"
            ) + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/"
        ) + name
    };

    for (const QString &path : candidates) {
        if (QFileInfo::exists(path)) {
            return QUrl::fromLocalFile(
                QFileInfo(path)
                    .absoluteFilePath()
            );
        }
    }

    return {};
}

QUrl BossController::flagUrl(const QString &name) const
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CLIENT_ROOT"
        )
    ) {
        const QString clientRoot =
            qEnvironmentVariable(
                "NEEBLES_CLIENT_ROOT"
            ).trimmed();

        if (clientRoot.isEmpty())
            return {};

        const QString path =
            QDir(clientRoot).filePath(
                QStringLiteral(
                    "assets/flags/4x3/"
                ) + name
            );

        if (QFileInfo::exists(path)) {
            return QUrl::fromLocalFile(
                QFileInfo(path)
                    .absoluteFilePath()
            );
        }

        return {};
    }

    const QStringList candidates = {
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../assets/flags/4x3/"
            ) + name
        ),

        QDir::current().filePath(
            QStringLiteral(
                "client/assets/flags/4x3/"
            ) + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/flags/4x3/"
        ) + name
    };

    for (const QString &path : candidates) {
        if (QFileInfo::exists(path)) {
            return QUrl::fromLocalFile(
                QFileInfo(path)
                    .absoluteFilePath()
            );
        }
    }

    return {};
}

void BossController::reload()
{
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

    const QVariantMap map =
        value.toMap();

    const QStringList requiredKeys = {
        QStringLiteral("language"),
        QStringLiteral("tray_enabled"),
        QStringLiteral("launcher_enabled"),
        QStringLiteral("normal_notifications"),
        QStringLiteral("telemetry_enabled")
    };

    for (const QString &key : requiredKeys) {
        if (!map.contains(key)) {
            setStatusText(
                QStringLiteral(
                    "Boss config is missing required key: "
                ) + key
            );

            return;
        }
    }

    const QString language =
        map.value(
            QStringLiteral("language")
        ).toString().trimmed();

    if (language.isEmpty()) {
        setStatusText(
            QStringLiteral(
                "Boss config declares an empty language"
            )
        );

        return;
    }

    m_language = language;

    m_trayEnabled =
        map.value(
            QStringLiteral("tray_enabled")
        ).toBool();

    m_launcherEnabled =
        map.value(
            QStringLiteral("launcher_enabled")
        ).toBool();

    m_normalNotifications =
        map.value(
            QStringLiteral("normal_notifications")
        ).toBool();

    m_telemetryEnabled =
        map.value(
            QStringLiteral("telemetry_enabled")
        ).toBool();


    m_hiddenTrayModules =
        map.value(
            QStringLiteral("hidden_tray_modules")
        ).toList();

    m_hiddenLauncherModules =
        map.value(
            QStringLiteral("hidden_launcher_modules")
        ).toList();

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

void BossController::pollModuleUpdates()
{
    if (m_busy)
        return;

    loadModules();
}


void BossController::applyModuleLifecycle()
{
    for (
        const QVariant &item :
        m_modules
    ) {
        const QVariantMap module =
            item.toMap();

        if (
            !module.value(
                QStringLiteral(
                    "installed"
                )
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
                QStringLiteral(
                    "running"
                ),
                false
            ).toBool();

        /*
         * One notification for each
         * module + available version.
         */
        if (
            !m_normalNotifications
            || remoteVersion.isEmpty()
            || m_updateNotifications
                .value(name)
                .toString()
                == remoteVersion
        )
            continue;

        const QString key =
            running
            ? QStringLiteral(
                "modules.update_notification_running"
            )
            : QStringLiteral(
                "modules.update_notification"
            );

        const QString message =
            text(key)
                .arg(
                    name,
                    remoteVersion
                );

        bool notificationOk = false;

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

        if (!notificationOk)
            continue;

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
            true,
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
void BossController::saveConfig(const QString &language,
                                bool trayEnabled,
                                bool launcherEnabled,
                                bool normalNotifications)
{
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
            text(
                QStringLiteral("notifications.disabled")
            )
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
            text(
                QStringLiteral("notifications.enabled")
            )
        }, false, 5000, &notificationOk);
    }

    if (ok) {
        /*
         * Launcher is a real Plasma panel integration:
         * OFF removes launcher + spacer.
         * ON recreates both beside Kickoff.
         */
        if (launcherEnabled != launcherWasEnabled) {
            if (!applyLauncherPanelState(launcherEnabled))
                setStatusText(
                    text(
                        QStringLiteral(
                            "launcher.update_failed"
                        )
                    )
                );
        }

        reload();

        if (statusText().isEmpty()
            || statusText() == text(
                QStringLiteral("common.ok")
            )) {
            setStatusText(
                text(
                    QStringLiteral("common.ok")
                )
            );
        }
    }

    setBusy(false);
}


void BossController::setModuleOperationField(
    const QString &name,
    const QString &key,
    const QVariant &value
)
{
    QVariantMap state =
        m_moduleOperations
            .value(name)
            .toMap();

    state.insert(key, value);

    m_moduleOperations.insert(
        name,
        state
    );

    emit moduleOperationsChanged();
}


void BossController::appendModuleOperationLog(
    const QString &name,
    const QString &line
)
{
    if (line.trimmed().isEmpty())
        return;

    QVariantMap state =
        m_moduleOperations
            .value(name)
            .toMap();

    QString log =
        state.value(
            QStringLiteral("log")
        ).toString();

    if (!log.isEmpty())
        log.append(QLatin1Char('\n'));

    log.append(line.trimmed());

    state.insert(
        QStringLiteral("log"),
        log
    );

    m_moduleOperations.insert(
        name,
        state
    );

    emit moduleOperationsChanged();
}


void BossController::consumeModuleProcessOutput()
{
    if (!m_moduleOperationProcess)
        return;

    QString chunk =
        QString::fromUtf8(
            m_moduleOperationProcess
                ->readAllStandardOutput()
        );

    /*
     * apt, git, curl and similar tools may update
     * a line using CR instead of LF.
     */
    chunk.replace(
        QLatin1Char('\r'),
        QLatin1Char('\n')
    );

    m_moduleOutputPending += chunk;

    const QRegularExpression progressExpression(
        QStringLiteral(
            R"((\d{1,3})%)"
        )
    );

    while (true) {
        const qsizetype separator =
            m_moduleOutputPending.indexOf(
                QLatin1Char('\n')
            );

        if (separator < 0)
            break;

        const QString line =
            m_moduleOutputPending
                .left(separator)
                .trimmed();

        m_moduleOutputPending.remove(
            0,
            separator + 1
        );

        if (line.isEmpty())
            continue;

        appendModuleOperationLog(
            m_activeModuleName,
            line
        );

        QRegularExpressionMatchIterator matches =
            progressExpression.globalMatch(line);

        int progress = -1;

        while (matches.hasNext()) {
            const QRegularExpressionMatch match =
                matches.next();

            bool ok = false;

            const int value =
                match.captured(1).toInt(&ok);

            if (
                ok
                && value >= 0
                && value <= 100
            ) {
                progress = value;
            }
        }

        if (progress >= 0) {
            setModuleOperationField(
                m_activeModuleName,
                QStringLiteral("progress"),
                progress
            );
        }
    }
}


void BossController::startModuleProcess(
    const QString &operation,
    const QString &name,
    bool privileged,
    const QStringList &extraArguments
)
{
    if (
        name.isEmpty()
        || m_moduleOperationProcess
    ) {
        return;
    }

    setBusy(true);

    m_activeModuleName = name;
    m_activeModuleOperation = operation;
    m_moduleOutputPending.clear();

    QVariantMap state;

    state.insert(
        QStringLiteral("started"),
        true
    );

    state.insert(
        QStringLiteral("running"),
        true
    );

    state.insert(
        QStringLiteral("operation"),
        operation
    );

    /*
     * -1 means that the underlying command has not
     * supplied a real percentage yet.
     */
    state.insert(
        QStringLiteral("progress"),
        -1
    );

    state.insert(
        QStringLiteral("log"),
        QString()
    );

    state.insert(
        QStringLiteral("success"),
        false
    );

    m_moduleOperations.insert(
        name,
        state
    );

    emit moduleOperationsChanged();

    appendModuleOperationLog(
        name,
        text(
            QStringLiteral(
                "modules.operation.header"
            )
        ).arg(
            operation,
            name
        )
    );

    QStringList commandArguments = {
        QStringLiteral("modules"),
        operation,
        name
    };

    commandArguments.append(
        extraArguments
    );

    QString program;
    QStringList arguments;

    if (privileged) {
        const QString authAgent =
            authorizationPath();

        if (authAgent.isEmpty()) {
            appendModuleOperationLog(
                name,
                text(
                    QStringLiteral(
                        "auth.agent_missing"
                    )
                )
            );

            setModuleOperationField(
                name,
                QStringLiteral("running"),
                false
            );

            setBusy(false);
            return;
        }

        program = authAgent;

        QString fromVersion;
        QString toVersion;
        bool running = false;

        for (
            const QVariant &item :
            m_modules
        ) {
            const QVariantMap module =
                item.toMap();

            if (
                module.value(
                    QStringLiteral("name")
                ).toString()
                != name
            ) {
                continue;
            }

            fromVersion =
                module.value(
                    QStringLiteral(
                        "installed_version"
                    ),
                    module.value(
                        QStringLiteral("version")
                    )
                ).toString();

            toVersion =
                module.value(
                    QStringLiteral(
                        "remote_version"
                    )
                ).toString();

            if (toVersion.isEmpty()) {
                toVersion =
                    module.value(
                        QStringLiteral("version")
                    ).toString();
            }

            running =
                module.value(
                    QStringLiteral("running"),
                    false
                ).toBool();

            break;
        }

        QString authOperation =
            QStringLiteral("generic");

        if (operation == QStringLiteral("install"))
            authOperation =
                QStringLiteral("install-module");
        else if (
            operation
            == QStringLiteral("update")
        )
            authOperation =
                QStringLiteral("update-module");
        else if (
            operation
            == QStringLiteral("uninstall")
        )
            authOperation =
                QStringLiteral("uninstall-module");

        arguments
            << QStringLiteral("--locale")
            << m_language
            << QStringLiteral("--operation")
            << authOperation
            << QStringLiteral("--name")
            << name;

        if (!fromVersion.isEmpty()) {
            arguments
                << QStringLiteral("--from")
                << fromVersion;
        }

        if (!toVersion.isEmpty()) {
            arguments
                << QStringLiteral("--to")
                << toVersion;
        }

        arguments
            << QStringLiteral("--running")
            << (
                running
                ? QStringLiteral("true")
                : QStringLiteral("false")
            )
            << QStringLiteral("--")
            << commandPath();

        arguments.append(
            commandArguments
        );
    } else {
        program =
            commandPath();

        arguments =
            commandArguments;
    }

    QProcess *process =
        new QProcess(this);

    m_moduleOperationProcess =
        process;

    /*
     * Merge stderr/stdout because install tools
     * commonly print useful progress to stderr.
     */
    process->setProcessChannelMode(
        QProcess::MergedChannels
    );

    connect(
        process,
        &QProcess::readyReadStandardOutput,
        this,
        &BossController::consumeModuleProcessOutput
    );

    connect(
        process,
        &QProcess::errorOccurred,
        this,
        [this, name](
            QProcess::ProcessError error
        ) {
            appendModuleOperationLog(
                name,
                text(
                    QStringLiteral(
                        "modules.operation.process_error"
                    )
                ).arg(
                    static_cast<int>(error)
                )
            );
        }
    );

    connect(
        process,
        qOverload<
            int,
            QProcess::ExitStatus
        >(&QProcess::finished),
        this,
        [this, process, name, operation](
            int exitCode,
            QProcess::ExitStatus exitStatus
        ) {
            consumeModuleProcessOutput();

            if (
                !m_moduleOutputPending
                    .trimmed()
                    .isEmpty()
            ) {
                appendModuleOperationLog(
                    name,
                    m_moduleOutputPending
                );

                m_moduleOutputPending.clear();
            }

            bool success =
                exitStatus
                    == QProcess::NormalExit
                && exitCode == 0;

            /*
             * Existing install contract:
             * successful install is enabled automatically.
             */
            if (
                success
                && operation
                    == QStringLiteral(
                        "install"
                    )
            ) {
                appendModuleOperationLog(
                    name,
                    text(
                        QStringLiteral(
                            "modules.operation.enabling"
                        )
                    )
                );

                bool enabled = false;

                run(
                    {
                        QStringLiteral("modules"),
                        QStringLiteral("enable"),
                        name
                    },
                    true,
                    5000,
                    &enabled
                );

                success =
                    success && enabled;

                appendModuleOperationLog(
                    name,
                    enabled
                    ? text(
                        QStringLiteral(
                            "modules.operation.enabled"
                        )
                    )
                    : text(
                        QStringLiteral(
                            "modules.operation.enable_failed"
                        )
                    )
                );
            }

            setModuleOperationField(
                name,
                QStringLiteral("running"),
                false
            );

            setModuleOperationField(
                name,
                QStringLiteral("success"),
                success
            );

            if (success) {
                setModuleOperationField(
                    name,
                    QStringLiteral("progress"),
                    100
                );

                appendModuleOperationLog(
                    name,
                    text(
                        QStringLiteral(
                            "modules.operation.completed"
                        )
                    )
                );
            } else {
                appendModuleOperationLog(
                    name,
                    text(
                        QStringLiteral(
                            "modules.operation.failed"
                        )
                    )
                );
            }

            setStatusText(
                success
                ? text(
                    QStringLiteral("common.ok")
                )
                : text(
                    QStringLiteral("common.error")
                )
            );

            m_moduleOperationProcess =
                nullptr;

            m_activeModuleName.clear();
            m_activeModuleOperation.clear();

            process->deleteLater();

            loadModules();

            setBusy(false);
        }
    );

    process->start(
        program,
        arguments
    );
}


void BossController::runModuleOperation(
    const QString &operation,
    const QString &name,
    bool privileged
)
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

    setStatusText(
        ok
        ? text(
            QStringLiteral("common.ok")
        )
        : text(
            QStringLiteral("common.error")
        )
    );

    loadModules();

    setBusy(false);
}
void BossController::saveConfigValue(
    const QString &key,
    const QString &value
)
{
    static const QSet<QString> allowed = {
        QStringLiteral("language"),
        QStringLiteral("tray_enabled"),
        QStringLiteral("launcher_enabled"),
        QStringLiteral("normal_notifications"),
        QStringLiteral("telemetry_enabled")
    };

    if (
        !allowed.contains(key)
        || m_busy
    ) {
        return;
    }

    const QString normalized =
        value.trimmed();

    const bool trayWasEnabled =
        m_trayEnabled;

    const bool launcherWasEnabled =
        m_launcherEnabled;

    const bool notificationsWereEnabled =
        m_normalNotifications;

    const bool telemetryWasEnabled =
        m_telemetryEnabled;

    const bool disablingNotifications =
        key
            == QStringLiteral(
                "normal_notifications"
            )
        && notificationsWereEnabled
        && normalized
            == QStringLiteral("false");

    /*
     * OFF:
     * avisar antes de desactivar notificaciones normales.
     */
    if (disablingNotifications) {
        bool notificationOk = false;

        run(
            {
                QStringLiteral("notify"),
                QStringLiteral("info"),
                QStringLiteral(
                    "N.E.E.B.L.E.S."
                ),
                text(
                    QStringLiteral(
                        "notifications.disabled"
                    )
                )
            },
            false,
            5000,
            &notificationOk
        );
    }

    setBusy(true);

    bool ok = false;

    /*
     * Escritura administrativa:
     * pasa por el auth-agent.
     */
    run(
        {
            QStringLiteral("config"),
            QStringLiteral("set"),
            key,
            normalized
        },
        true,
        5000,
        &ok
    );

    if (!ok) {
        /*
         * Si se cancela o falla autorización,
         * recuperamos el estado real.
         */
        reload();
        setBusy(false);
        return;
    }

    /*
     * Tray Manager:
     *
     * El setting persistido es la intención del usuario.
     * El servicio real debe reflejar inmediatamente
     * ese mismo estado.
     *
     * ON:
     *   enable + start
     *
     * OFF:
     *   disable + stop
     */
    if (
        key
            == QStringLiteral(
                "tray_enabled"
            )
    ) {
        const bool enabled =
            normalized
                == QStringLiteral("true");

        if (
            enabled
            != trayWasEnabled
        ) {
            if (
                !applyTrayState(
                    enabled
                )
            ) {
                setStatusText(
                    text(
                        QStringLiteral(
                            "common.error"
                        )
                    )
                );
            }
        }
    }


    /*
     * Launcher:
     * mantener sincronizado estado lógico y panel Plasma.
     */
    if (
        key
            == QStringLiteral(
                "launcher_enabled"
            )
    ) {
        const bool enabled =
            normalized
                == QStringLiteral("true");

        if (
            enabled
            != launcherWasEnabled
        ) {
            if (
                !applyLauncherPanelState(
                    enabled
                )
            ) {
                setStatusText(
                    text(
                        QStringLiteral(
                            "launcher.update_failed"
                        )
                    )
                );
            }
        }
    }

    /*
     * ON:
     * primero persistir, luego avisar.
     */
    if (
        key
            == QStringLiteral(
                "normal_notifications"
            )
        && !notificationsWereEnabled
        && normalized
            == QStringLiteral("true")
    ) {
        bool notificationOk = false;

        run(
            {
                QStringLiteral("notify"),
                QStringLiteral("success"),
                QStringLiteral(
                    "N.E.E.B.L.E.S."
                ),
                text(
                    QStringLiteral(
                        "notifications.enabled"
                    )
                )
            },
            false,
            5000,
            &notificationOk
        );
    }

    /*
     * Telemetry:
     * notificar sólo después de una escritura persistente
     * correcta y únicamente cuando el estado cambió.
     */
    if (
        key
            == QStringLiteral(
                "telemetry_enabled"
            )
    ) {
        const bool telemetryEnabled =
            normalized
                == QStringLiteral("true");

        if (
            telemetryEnabled
            != telemetryWasEnabled
        ) {
            bool notificationOk = false;

            run(
                {
                    QStringLiteral("notify"),

                    telemetryEnabled
                        ? QStringLiteral("success")
                        : QStringLiteral("info"),

                    QStringLiteral(
                        "N.E.E.B.L.E.S."
                    ),

                    text(
                        telemetryEnabled
                            ? QStringLiteral(
                                "telemetry.activated"
                            )
                            : QStringLiteral(
                                "telemetry.deactivated"
                            )
                    )
                },
                false,
                5000,
                &notificationOk
            );
        }
    }

    reload();

    if (
        statusText().isEmpty()
        || statusText()
            == text(
                QStringLiteral(
                    "common.ok"
                )
            )
    ) {
        setStatusText(
            text(
                QStringLiteral(
                    "common.ok"
                )
            )
        );
    }

    setBusy(false);
}


int BossController::moduleLocalState(
    const QString &name
)
{
    if (name.isEmpty())
        return -1;

    QJsonObject request;

    request.insert(
        QStringLiteral("target"),
        QStringLiteral("settings")
    );

    request.insert(
        QStringLiteral("action"),
        QStringLiteral("has-state")
    );

    QJsonArray args;

    args.append(name);

    request.insert(
        QStringLiteral("args"),
        args
    );

    QJsonObject context;

    context.insert(
        QStringLiteral("caller"),
        QStringLiteral("ui")
    );

    request.insert(
        QStringLiteral("context"),
        context
    );

    const QString rawRequest =
        QString::fromUtf8(
            QJsonDocument(
                request
            ).toJson(
                QJsonDocument::Compact
            )
        );

    bool ok = false;

    const QByteArray output =
        run(
            {
                QStringLiteral(
                    "--request-json"
                ),
                rawRequest
            },
            false,
            5000,
            &ok
        );

    if (!ok)
        return -1;

    const QVariantMap response =
        parseJson(
            output
        ).toMap();

    if (
        !response.value(
            QStringLiteral("ok")
        ).toBool()
        || !response.contains(
            QStringLiteral("result")
        )
    ) {
        return -1;
    }

    return response.value(
        QStringLiteral("result")
    ).toBool()
        ? 1
        : 0;
}


void BossController::installModule(
    const QString &name
)
{
    startModuleProcess(
        QStringLiteral("install"),
        name,
        true
    );
}


void BossController::updateModule(
    const QString &name
)
{
    startModuleProcess(
        QStringLiteral("update"),
        name,
        true,
        {
            QStringLiteral(
                "--close-running"
            )
        }
    );
}


void BossController::uninstallModule(
    const QString &name,
    bool removeSettings
)
{
    QStringList extraArguments;

    if (removeSettings) {
        extraArguments
            << QStringLiteral(
                "--remove-settings"
            );
    }

    startModuleProcess(
        QStringLiteral("uninstall"),
        name,
        true,
        extraArguments
    );
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

        const QString launcherAction =
            module.value(
                QStringLiteral(
                    "launcher_action"
                )
            ).toString();

        if (launcherAction.isEmpty())
            return;

        qint64 processId = 0;

        const bool started =
            QProcess::startDetached(
                commandPath(),
                {
                    name,
                    launcherAction
                },
                QString(),
                &processId
            );

        if (!started) {
            setStatusText(
                text(
                    QStringLiteral(
                        "modules.open_failed"
                    )
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
    runModuleOperation(
        enabled
            ? QStringLiteral("enable")
            : QStringLiteral("disable"),
        name,
        true
    );
}

void BossController::setModuleVisibility(
    const QString &surface,
    const QString &name,
    bool visible
)
{
    bool ok = false;

    run(
        {
            QStringLiteral("config"),
            QStringLiteral("module-visibility"),
            surface,
            name,
            visible
                ? QStringLiteral("true")
                : QStringLiteral("false")
        },
        true,
        5000,
        &ok
    );

    if (!ok)
        return;

    loadConfig();
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
