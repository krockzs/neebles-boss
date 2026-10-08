#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDebug>
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
     * remote update check.
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
        &BossController::pollUpdates
    );

    reload();

    m_modulePollTimer->start();
    m_updatePollTimer->start();
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
            "/opt/neebles/client/bin/neebles"
        );

    if (
        QFileInfo::exists(installed)
        && QFileInfo(installed).isExecutable()
    )
        return installed;

    return {};
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

QStringList BossController::runtimeAuthorityArguments() const
{
    const QString resolver =
        qEnvironmentVariable(
            "NEEBLES_RUNTIME_RESOLVER"
        ).trimmed();

    const QString manifest =
        qEnvironmentVariable(
            "NEEBLES_RUNTIME_MANIFEST"
        ).trimmed();

    const QFileInfo resolverInfo(resolver);
    const QFileInfo manifestInfo(manifest);

    if (
        resolver.isEmpty()
        || manifest.isEmpty()
        || !resolverInfo.isAbsolute()
        || !resolverInfo.isFile()
        || !resolverInfo.isExecutable()
        || !manifestInfo.isAbsolute()
        || !manifestInfo.isFile()
        || !manifestInfo.isReadable()
    ) {
        return {};
    }

    return {
        QStringLiteral("--runtime-resolver"),
        resolverInfo.absoluteFilePath(),
        QStringLiteral("--runtime-manifest"),
        manifestInfo.absoluteFilePath()
    };
}



QString BossController::authoritySupplyPath() const
{
    /*
     * Transport only.
     *
     * Qt does not load, authenticate, register, interpret or grant this
     * AuthoritySupply. The Boss child process owns all of those decisions.
     */
    return qEnvironmentVariable(
        "NEEBLES_AUTHORITY_SUPPLY"
    ).trimmed();
}


QByteArray BossController::run(
    const QStringList &arguments,
    bool privileged,
    int timeoutMs,
    bool *ok,
    QString *diagnostic
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

    const QString authoritySupply =
        authoritySupplyPath();

    if (authoritySupply.isEmpty()) {
        if (ok)
            *ok = false;

        setStatusText(
            QStringLiteral(
                "N.E.E.B.L.E.S. AuthoritySupply transport is unavailable"
            )
        );

        return {};
    }

    QStringList bossArguments = {
        QStringLiteral("--authority-supply"),
        authoritySupply
    };

    bossArguments.append(arguments);

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

        if (
            arguments.size() >= 2
            && arguments.at(0)
                == QStringLiteral("--request-json")
        ) {
            const QJsonDocument document =
                QJsonDocument::fromJson(
                    arguments.at(1).toUtf8()
                );

            if (document.isObject()) {
                const QJsonObject request =
                    document.object();

                const QString target =
                    request.value(
                        QStringLiteral("target")
                    ).toString();

                const QString action =
                    request.value(
                        QStringLiteral("action")
                    ).toString();

                if (
                    target
                        == QStringLiteral("boss")
                    && action
                        == QStringLiteral("update-execute")
                ) {
                    operation =
                        QStringLiteral("update-boss");

                    name =
                        QStringLiteral(
                            "N.E.E.B.L.E.S. Boss"
                        );

                    fromVersion =
                        m_bossInstalledVersion;

                    toVersion =
                        m_bossRemoteVersion;
                }
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

        const QStringList runtimeAuthority =
            runtimeAuthorityArguments();

        if (runtimeAuthority.isEmpty()) {
            if (ok)
                *ok = false;

            setStatusText(
                QStringLiteral(
                    "N.E.E.B.L.E.S. runtime authority transport is unavailable"
                )
            );

            return {};
        }

        elevated.append(runtimeAuthority);

        elevated
            << QStringLiteral("--")
            << bossCommand;

        elevated << bossArguments;

        process.start(
            authAgent,
            elevated
        );
    } else {
        process.start(
            bossCommand,
            bossArguments
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

    if (diagnostic)
        diagnostic->clear();

    if (!success) {
        QString error =
            QString::fromUtf8(
                process.readAllStandardError()
            ).trimmed();

        if (error.isEmpty()) {
            if (!started) {
                error = process.errorString();
            } else if (!finished) {
                error = QStringLiteral("Boss command timed out");
            } else {
                error = QStringLiteral("Boss command exited with code %1")
                    .arg(process.exitCode());
            }
        }

        if (diagnostic)
            *diagnostic = error;

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
    loadBossUpdateStatus();
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

void BossController::loadBossUpdateStatus()
{
    const QString request =
        QStringLiteral(
            "{\"target\":\"boss\","
            "\"action\":\"update-status\","
            "\"args\":[],"
            "\"context\":{\"caller\":\"boss-ui\"}}"
        );

    bool ok = false;

    const QVariant responseValue =
        parseJson(
            run(
                {
                    QStringLiteral(
                        "--request-json"
                    ),
                    request
                },
                false,
                10000,
                &ok
            )
        );

    if (
        !ok
        || !responseValue.canConvert<QVariantMap>()
    ) {
        return;
    }

    const QVariantMap response =
        responseValue.toMap();

    if (
        !response.value(
            QStringLiteral("ok")
        ).toBool()
    ) {
        return;
    }

    const QVariant resultValue =
        response.value(
            QStringLiteral("result")
        );

    if (
        !resultValue.canConvert<QVariantMap>()
    ) {
        return;
    }

    const QVariantMap result =
        resultValue.toMap();

    const QString installedVersion =
        result.value(
            QStringLiteral(
                "installed_version"
            )
        ).toString().trimmed();

    const QString remoteVersion =
        result.value(
            QStringLiteral(
                "remote_version"
            )
        ).toString().trimmed();

    if (
        installedVersion.isEmpty()
        || remoteVersion.isEmpty()
    ) {
        return;
    }

    const bool updateAvailable =
        result.value(
            QStringLiteral(
                "update_available"
            )
        ).toBool();

    if (
        m_bossInstalledVersion
            == installedVersion
        && m_bossRemoteVersion
            == remoteVersion
        && m_bossUpdateAvailable
            == updateAvailable
    ) {
        return;
    }

    m_bossInstalledVersion =
        installedVersion;

    m_bossRemoteVersion =
        remoteVersion;

    m_bossUpdateAvailable =
        updateAvailable;

    emit bossUpdateChanged();
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
                QStringLiteral("boss"),
                QStringLiteral("surface-model")
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
                QStringLiteral("boss"),
                QStringLiteral("surface-model")
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

    QVariantMap presentationByName;

    for (
        const QVariant &item :
        value.toList()
    ) {
        const QVariantMap module =
            item.toMap();

        const QString name =
            module.value(
                QStringLiteral("name")
            ).toString();

        if (!name.isEmpty()) {
            presentationByName.insert(
                name,
                module
            );
        }
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

        const QVariantMap presentation =
            presentationByName
                .value(name)
                .toMap();

        if (presentation.isEmpty())
            continue;

        bool moduleChanged = false;

        const bool running =
            presentation.value(
                QStringLiteral("running"),
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

            moduleChanged = true;
        }

        const QString runtimeState =
            presentation.value(
                QStringLiteral("runtime_state"),
                QStringLiteral("closed")
            ).toString();

        if (
            module.value(
                QStringLiteral("runtime_state")
            ).toString()
            != runtimeState
        ) {
            module.insert(
                QStringLiteral("runtime_state"),
                runtimeState
            );

            moduleChanged = true;
        }

        const bool openAvailable =
            presentation.value(
                QStringLiteral("open_available"),
                false
            ).toBool();

        if (
            module.value(
                QStringLiteral("open_available"),
                false
            ).toBool()
            != openAvailable
        ) {
            module.insert(
                QStringLiteral("open_available"),
                openAvailable
            );

            moduleChanged = true;
        }
        const bool enabled =
            presentation.value(
                QStringLiteral("enabled"),
                false
            ).toBool();

        if (
            module.value(
                QStringLiteral("enabled"),
                false
            ).toBool()
            != enabled
        ) {
            module.insert(
                QStringLiteral("enabled"),
                enabled
            );

            moduleChanged = true;
        }

        const QVariant surfaceContent =
            presentation.value(
                QStringLiteral("surface_content")
            );

        if (
            module.value(
                QStringLiteral("surface_content")
            )
            != surfaceContent
        ) {
            module.insert(
                QStringLiteral("surface_content"),
                surfaceContent
            );

            moduleChanged = true;
        }

        if (moduleChanged) {
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
void BossController::pollUpdates()
{
    if (m_busy)
        return;

    loadModules();
    loadBossUpdateStatus();
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
void BossController::installBossUpdate()
{
    if (
        m_busy
        || !m_bossUpdateAvailable
        || m_bossInstalledVersion.isEmpty()
        || m_bossRemoteVersion.isEmpty()
    ) {
        return;
    }

    const QJsonObject requestObject {
        {
            QStringLiteral("target"),
            QStringLiteral("boss")
        },
        {
            QStringLiteral("action"),
            QStringLiteral("update-execute")
        },
        {
            QStringLiteral("args"),
            QJsonArray()
        },
        {
            QStringLiteral("context"),
            QJsonObject {
                {
                    QStringLiteral("caller"),
                    QStringLiteral("boss-ui")
                }
            }
        }
    };

    const QString request =
        QString::fromUtf8(
            QJsonDocument(
                requestObject
            ).toJson(
                QJsonDocument::Compact
            )
        );

    setBusy(true);

    bool ok = false;

    run(
        {
            QStringLiteral("--request-json"),
            request
        },
        true,
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

    if (!ok) {
        loadBossUpdateStatus();
    }

    setBusy(false);
}


void BossController::saveConfig(const QString &language,
                                bool trayEnabled,
                                bool launcherEnabled,
                                bool normalNotifications)
{
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
         * Launcher visibility is driven live by settings.boss.
         *
         * The Plasma launcher and spacer remain installed and alive.
         * ON/OFF must never destroy or recreate panel widgets.
         */
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


void BossController::setTransactionOperationField(
    const QString &key,
    const QVariant &value
)
{
    m_transactionOperation.insert(
        key,
        value
    );

    emit transactionOperationChanged();
}


void BossController::appendTransactionOperationLog(
    const QString &line
)
{
    if (line.trimmed().isEmpty())
        return;

    QString log =
        m_transactionOperation.value(
            QStringLiteral("log")
        ).toString();

    if (!log.isEmpty())
        log.append(QLatin1Char('\n'));

    log.append(
        line.trimmed()
    );

    m_transactionOperation.insert(
        QStringLiteral("log"),
        log
    );

    emit transactionOperationChanged();
}


bool BossController::consumeLifecycleProcessEvent(
    const QString &line
)
{
    static const QString prefix =
        QStringLiteral(
            "NEEBLES_LIFECYCLE\t"
        );

    if (!line.startsWith(prefix))
        return false;

    /*
     * Reserved machine lines never enter the human
     * transaction transcript, even when malformed.
     *
     * Observation is auxiliary and must never make
     * the underlying module transaction fail.
     */
    const QByteArray payload =
        line.mid(
            prefix.size()
        ).toUtf8();

    QJsonParseError error;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            payload,
            &error
        );

    if (
        error.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        return true;
    }

    const QVariantMap event =
        document
            .object()
            .toVariantMap();

    if (
        event.value(
            QStringLiteral("type")
        ).toString()
        != QStringLiteral(
            "lifecycle.communication"
        )
    ) {
        return true;
    }

    const QVariantMap communication =
        event.value(
            QStringLiteral("communication")
        ).toMap();

    if (communication.isEmpty())
        return true;

    /*
     * LifecycleCommunication is canonical semantic
     * execution observation.
     *
     * BossController stores a projection only.
     */
    m_transactionOperation.insert(
        QStringLiteral("lifecycle"),
        communication
    );

    m_transactionOperation.insert(
        QStringLiteral("lifecycleModule"),
        event.value(
            QStringLiteral("module")
        )
    );

    m_transactionOperation.insert(
        QStringLiteral("lifecycleAction"),
        event.value(
            QStringLiteral("action")
        )
    );

    const auto project =
        [this, &communication](
            const char *source,
            const char *destination
        ) {
            const QString value =
                communication.value(
                    QString::fromLatin1(
                        source
                    )
                ).toString();

            if (!value.isEmpty()) {
                m_transactionOperation.insert(
                    QString::fromLatin1(
                        destination
                    ),
                    value
                );
            }
        };

    project(
        "state.execution.id",
        "lifecycleExecutionId"
    );

    project(
        "state.transition.id",
        "lifecycleTransitionId"
    );

    project(
        "state.object.id",
        "lifecycleObjectId"
    );

    project(
        "state.operation.id",
        "lifecycleOperationId"
    );

    project(
        "state.phase",
        "lifecyclePhase"
    );

    bool totalOk = false;
    bool completedOk = false;

    const int total =
        communication.value(
            QStringLiteral(
                "progress.total"
            )
        ).toString().toInt(
            &totalOk
        );

    const int completed =
        communication.value(
            QStringLiteral(
                "progress.completed"
            )
        ).toString().toInt(
            &completedOk
        );

    if (
        totalOk
        && completedOk
        && total > 0
        && completed >= 0
    ) {
        const int bounded =
            qBound(
                0,
                completed,
                total
            );

        const int progress =
            qRound(
                (
                    static_cast<double>(
                        bounded
                    )
                    / static_cast<double>(
                        total
                    )
                )
                * 100.0
            );

        m_transactionOperation.insert(
            QStringLiteral("progress"),
            progress
        );
    }

    emit transactionOperationChanged();

    return true;
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

        if (
            consumeLifecycleProcessEvent(
                line
            )
        ) {
            continue;
        }

        /*
         * Everything not belonging to the reserved
         * machine protocol remains human transcript.
         */
        appendTransactionOperationLog(
            line
        );
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

    state.insert(
        QStringLiteral("id"),
        QString::number(
            ++m_transactionSequence
        )
    );

    state.insert(
        QStringLiteral("module"),
        name
    );

    m_transactionOperation =
        state;

    emit transactionOperationChanged();

    appendTransactionOperationLog(
        text(
            QStringLiteral(
                "modules.operation.header"
            )
        ).arg(
            operation,
            name
        )
    );

    QStringList commandArguments;

    if (
        operation
            == QStringLiteral("surface-action")
    ) {
        commandArguments = {
            QStringLiteral("boss"),
            QStringLiteral("surface-action"),
            name
        };
    } else if (
        operation
            == QStringLiteral("open")
    ) {
        commandArguments = {
            QStringLiteral("boss"),
            QStringLiteral("module-governor-action"),
            name,
            QStringLiteral("open"),
            QString(),
            QString(),
            QStringLiteral("false")
        };
    } else {
        commandArguments = {
            QStringLiteral("modules"),
            operation,
            name,
            QStringLiteral(
                "--lifecycle-events"
            )
        };
    }

    commandArguments.append(
        extraArguments
    );

    const QString authoritySupply =
        authoritySupplyPath();

    if (authoritySupply.isEmpty()) {
        appendTransactionOperationLog(
            QStringLiteral(
                "N.E.E.B.L.E.S. AuthoritySupply transport is unavailable"
            )
        );

        setTransactionOperationField(
            QStringLiteral("running"),
            false
        );

        setBusy(false);
        return;
    }

    QStringList bossArguments = {
        QStringLiteral("--authority-supply"),
        authoritySupply
    };

    bossArguments.append(
        commandArguments
    );

    QString program;
    QStringList arguments;

    if (privileged) {
        const QString authAgent =
            authorizationPath();

        if (authAgent.isEmpty()) {
            appendTransactionOperationLog(
                text(
                    QStringLiteral(
                        "auth.agent_missing"
                    )
                )
            );

            setTransactionOperationField(
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
            );

        const QStringList runtimeAuthority =
            runtimeAuthorityArguments();

        if (runtimeAuthority.isEmpty()) {
            appendTransactionOperationLog(
                QStringLiteral(
                    "N.E.E.B.L.E.S. runtime authority transport is unavailable"
                )
            );

            setTransactionOperationField(
                QStringLiteral("running"),
                false
            );

            setBusy(false);
            return;
        }

        arguments.append(runtimeAuthority);

        arguments
            << QStringLiteral("--")
            << commandPath();

        arguments.append(
            bossArguments
        );
    } else {
        program =
            commandPath();

        arguments =
            bossArguments;
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
            appendTransactionOperationLog(
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
                appendTransactionOperationLog(
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
                appendTransactionOperationLog(
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
                    false,
                    600000,
                    &enabled
                );

                success =
                    success && enabled;

                appendTransactionOperationLog(
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

            setTransactionOperationField(
                QStringLiteral("running"),
                false
            );

            setTransactionOperationField(
                QStringLiteral("success"),
                success
            );

            if (success) {
                setTransactionOperationField(
                    QStringLiteral("progress"),
                    100
                );

                appendTransactionOperationLog(
                    text(
                        QStringLiteral(
                            "modules.operation.completed"
                        )
                    )
                );
            } else {
                appendTransactionOperationLog(
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
    QString diagnostic;

    run(
        {
            QStringLiteral("modules"),
            operation,
            name
        },
        privileged,
        600000,
        &ok,
        &diagnostic
    );

    loadModules();

    if (ok) {
        setStatusText(text(QStringLiteral("common.ok")));
    } else {
        const QString details = diagnostic.isEmpty()
            ? QStringLiteral("command failed without stderr")
            : diagnostic;

        const QString message =
            QStringLiteral("N.E.E.B.L.E.S.: modules %1 %2 failed: %3")
                .arg(operation)
                .arg(name)
                .arg(details);

        // Preserve the exact failing transaction in the session journal.
        qWarning().noquote() << message;
        setStatusText(message);
    }

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
     * Boss-owned settings write.
     *
     * UI transports intent only and does not add an independent
     * privilege policy.
     */
    run(
        {
            QStringLiteral("config"),
            QStringLiteral("set"),
            key,
            normalized
        },
        false,
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
     * Tray visibility is driven live by settings.boss.
     *
     * Manager, Qt Host and SNI Host remain installed and running.
     * tray_enabled controls only the user-facing root surface.
     */

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


void BossController::requestSurfaceAction(
    const QString &name,
    const QString &itemId,
    const QString &action
)
{
    const QString normalizedName =
        name.trimmed();

    const QString normalizedItemId =
        itemId.trimmed();

    const QString normalizedAction =
        action.trimmed();

    if (
        normalizedName.isEmpty()
        || normalizedItemId.isEmpty()
        || normalizedAction.isEmpty()
        || m_busy
    ) {
        return;
    }

    /*
     * Surface execution transports declarative identity only.
     *
     * The persistent Boss re-resolves object/transition and
     * validates require immediately before Lifecycle execution.
     */
    startModuleProcess(
        QStringLiteral("surface-action"),
        normalizedName,
        false,
        {
            normalizedItemId,
            normalizedAction
        }
    );
}

void BossController::openModule(
    const QString &name
)
{
    const QString normalizedName =
        name.trimmed();

    if (
        normalizedName.isEmpty()
        || m_busy
    ) {
        return;
    }

    startModuleProcess(
        QStringLiteral("open"),
        normalizedName,
        false
    );
}

QVariantMap BossController::dependencyPreflight(
    const QString &action,
    const QString &name
)
{
    QVariantMap failure;

    failure.insert(
        QStringLiteral("action"),
        action
    );

    failure.insert(
        QStringLiteral("module"),
        name
    );

    failure.insert(
        QStringLiteral("allowed"),
        false
    );

    failure.insert(
        QStringLiteral("affected"),
        QVariantList()
    );

    failure.insert(
        QStringLiteral("blockers"),
        QVariantList()
    );

    if (
        action.isEmpty()
        || name.isEmpty()
    ) {
        return failure;
    }

    bool ok = false;

    const QByteArray output =
        run(
            {
                QStringLiteral("modules"),
                QStringLiteral("preflight"),
                action,
                name
            },
            false,
            5000,
            &ok
        );

    if (!ok)
        return failure;

    const QVariant parsed =
        parseJson(output);

    if (
        !parsed.canConvert<QVariantMap>()
    ) {
        return failure;
    }

    const QVariantMap result =
        parsed.toMap();

    if (
        !result.contains(
            QStringLiteral("action")
        )
        || !result.contains(
            QStringLiteral("module")
        )
        || !result.contains(
            QStringLiteral("allowed")
        )
        || !result.contains(
            QStringLiteral("affected")
        )
        || !result.contains(
            QStringLiteral("blockers")
        )
    ) {
        return failure;
    }

    return result;
}


void BossController::setModuleEnabled(const QString &name, bool enabled)
{
    runModuleOperation(
        enabled
            ? QStringLiteral("enable")
            : QStringLiteral("disable"),
        name,
        false
    );
}

void BossController::setSurfaceModuleVisibility(
    const QString &surface,
    const QString &module,
    bool visible
)
{
    bool ok = false;

    run(
        {
            QStringLiteral("config"),
            QStringLiteral("surface-module-visibility"),
            surface,
            module,
            visible
                ? QStringLiteral("true")
                : QStringLiteral("false")
        },
        false,
        5000,
        &ok
    );

    if (!ok)
        return;

    /*
     * modules installed carries canonical SurfaceContent,
     * including effective presentation visibility.
     */
    loadModules();
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
