#include "bosscommandclient.h"

#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QProcessEnvironment>
#include <QStringList>

BossCommandClient::BossCommandClient(
    QObject *parent
)
    : QObject(parent)
{
    connect(
        &m_actionProcess,
        qOverload<int, QProcess::ExitStatus>(
            &QProcess::finished
        ),
        this,
        [this](
            int exitCode,
            QProcess::ExitStatus exitStatus
        ) {
            const QString stderrText =
                QString::fromUtf8(
                    m_actionProcess.readAllStandardError()
                ).trimmed();

            if (
                exitStatus != QProcess::NormalExit
                || exitCode != 0
            ) {
                setError(
                    stderrText.isEmpty()
                    ? QStringLiteral(
                        "Module action failed"
                    )
                    : stderrText
                );
            } else {
                setError(QString());
            }

            setBusy(false);
            refreshModules();
        }
    );
}

QString
BossCommandClient::commandPath() const
{
    const auto environment =
        QProcessEnvironment::systemEnvironment();

    const QString overrideCommand =
        environment.value(
            QStringLiteral(
                "NEEBLES_COMMAND"
            )
        ).trimmed();

    if (!overrideCommand.isEmpty()) {
        return overrideCommand;
    }

    const QString installedCommand =
        QStringLiteral(
            "/opt/neebles/client/bin/neebles"
        );

    if (
        QFileInfo::exists(installedCommand)
        && QFileInfo(installedCommand)
            .isExecutable()
    ) {
        return installedCommand;
    }

    return QStringLiteral("neebles");
}

QVariantList
BossCommandClient::installedModules() const
{
    return m_installedModules;
}

bool
BossCommandClient::busy() const
{
    return m_busy;
}

QString
BossCommandClient::error() const
{
    return m_error;
}

void
BossCommandClient::setBusy(bool value)
{
    if (m_busy == value) {
        return;
    }

    m_busy = value;
    emit busyChanged();
}

void
BossCommandClient::setError(
    const QString &value
)
{
    if (m_error == value) {
        return;
    }

    m_error = value;
    emit errorChanged();
}

bool
BossCommandClient::startBoss()
{
    return QProcess::startDetached(
        commandPath(),
        QStringList{
            QStringLiteral("start")
        }
    );
}

void
BossCommandClient::refreshModules()
{
    QProcess process;

    process.start(
        commandPath(),
        QStringList{
            QStringLiteral("modules"),
            QStringLiteral("installed")
        }
    );

    if (!process.waitForStarted(1500)) {
        setError(
            QStringLiteral(
                "Could not start module query"
            )
        );
        return;
    }

    if (!process.waitForFinished(3000)) {
        process.kill();
        process.waitForFinished();

        setError(
            QStringLiteral(
                "Module query timed out"
            )
        );
        return;
    }

    if (
        process.exitStatus()
            != QProcess::NormalExit
        || process.exitCode() != 0
    ) {
        const QString stderrText =
            QString::fromUtf8(
                process.readAllStandardError()
            ).trimmed();

        setError(
            stderrText.isEmpty()
            ? QStringLiteral(
                "Module query failed"
            )
            : stderrText
        );

        return;
    }

    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            process.readAllStandardOutput(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isArray()
    ) {
        setError(
            QStringLiteral(
                "Invalid installed module model"
            )
        );
        return;
    }

    QVariantList next;

    for (
        const auto &value :
        document.array()
    ) {
        next.append(
            value.toObject().toVariantMap()
        );
    }

    m_installedModules = next;

    setError(QString());

    emit installedModulesChanged();
}

bool
BossCommandClient::moduleAction(
    const QString &moduleName,
    const QString &action,
    const QString &objectId,
    const QString &transition
)
{
    if (m_busy) {
        return false;
    }

    const QString module =
        moduleName.trimmed();

    const QString governorAction =
        action.trimmed();

    if (
        module.isEmpty()
        || governorAction.isEmpty()
    ) {
        return false;
    }

    QStringList arguments{
        QStringLiteral("modules"),
        QStringLiteral("action"),
        module,
        governorAction
    };

    const QString object =
        objectId.trimmed();

    const QString lifecycleTransition =
        transition.trimmed();

    if (!object.isEmpty()) {
        arguments.append(
            QStringLiteral("--object-id")
        );
        arguments.append(object);
    }

    if (!lifecycleTransition.isEmpty()) {
        arguments.append(
            QStringLiteral("--transition")
        );
        arguments.append(
            lifecycleTransition
        );
    }

    setError(QString());
    setBusy(true);

    m_actionProcess.start(
        commandPath(),
        arguments
    );

    if (!m_actionProcess.waitForStarted(1000)) {
        setBusy(false);

        setError(
            QStringLiteral(
                "Could not start module action"
            )
        );

        return false;
    }

    return true;
}
