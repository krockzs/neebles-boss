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

BossController::BossController(QObject *parent)
    : QObject(parent)
{
    reload();
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
    m_normalNotifications = map.value(QStringLiteral("normal_notifications"), true).toBool();
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
    QSet<QString> installedNames;

    bool installedOk = false;
    const QVariant installedValue = parseJson(run({QStringLiteral("modules"), QStringLiteral("installed")}, false, 5000, &installedOk));
    if (installedOk && installedValue.canConvert<QVariantList>()) {
        const QVariantList installed = installedValue.toList();
        for (const QVariant &item : installed) {
            QVariantMap map = item.toMap();
            map.insert(QStringLiteral("installed"), true);
            installedNames.insert(map.value(QStringLiteral("name")).toString());
            combined.append(map);
        }
    }

    bool availableOk = false;
    const QVariant availableValue = parseJson(run({QStringLiteral("modules"), QStringLiteral("available")}, false, 10000, &availableOk));
    if (availableOk && availableValue.canConvert<QVariantList>()) {
        for (const QVariant &item : availableValue.toList()) {
            QVariantMap map = item.toMap();
            const QString name = map.value(QStringLiteral("name")).toString();
            if (!installedNames.contains(name)) {
                map.insert(QStringLiteral("installed"), false);
                map.insert(QStringLiteral("enabled"), false);
                combined.append(map);
            }
        }
    }

    m_modules = combined;
    emit modulesChanged();
}

void BossController::saveConfig(const QString &language,
                                bool trayEnabled,
                                bool launcherEnabled,
                                bool normalNotifications)
{
    setBusy(true);
    bool ok = true;
    bool current = false;

    run({QStringLiteral("config"), QStringLiteral("set"), QStringLiteral("language"), language}, false, 5000, &current);
    ok = ok && current;
    run({QStringLiteral("config"), QStringLiteral("set"), QStringLiteral("tray_enabled"), trayEnabled ? QStringLiteral("true") : QStringLiteral("false")}, false, 5000, &current);
    ok = ok && current;
    run({QStringLiteral("config"), QStringLiteral("set"), QStringLiteral("launcher_enabled"), launcherEnabled ? QStringLiteral("true") : QStringLiteral("false")}, false, 5000, &current);
    ok = ok && current;
    run({QStringLiteral("config"), QStringLiteral("set"), QStringLiteral("normal_notifications"), normalNotifications ? QStringLiteral("true") : QStringLiteral("false")}, false, 5000, &current);
    ok = ok && current;

    if (ok) {
        setStatusText(QStringLiteral("OK"));
        reload();
    }
    setBusy(false);
}

void BossController::runModuleOperation(const QString &operation, const QString &name, bool privileged)
{
    if (name.isEmpty())
        return;
    setBusy(true);
    bool ok = false;
    run({QStringLiteral("modules"), operation, name}, privileged, 600000, &ok);
    setStatusText(ok ? QStringLiteral("OK") : QStringLiteral("ERROR"));
    loadModules();
    setBusy(false);
}

void BossController::installModule(const QString &name)
{
    runModuleOperation(QStringLiteral("install"), name, true);
}

void BossController::updateModule(const QString &name)
{
    runModuleOperation(QStringLiteral("update"), name, true);
}

void BossController::uninstallModule(const QString &name)
{
    runModuleOperation(QStringLiteral("uninstall"), name, true);
}

void BossController::openModule(const QString &name)
{
    if (name.isEmpty())
        return;
    QProcess::startDetached(commandPath(), {name, QStringLiteral("open")});
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
