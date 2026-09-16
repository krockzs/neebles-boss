#include "bosscommandclient.h"

#include <QFileInfo>
#include <QProcess>
#include <QProcessEnvironment>
#include <QString>
#include <QStringList>

BossCommandClient::BossCommandClient(
    QObject *parent
)
    : QObject(parent)
{
}

bool BossCommandClient::startBoss()
{
    const auto environment =
        QProcessEnvironment::systemEnvironment();

    QString command =
        environment.value(
            QStringLiteral("NEEBLES_COMMAND")
        ).trimmed();

    if (command.isEmpty()) {
        const QString installedCommand =
            QStringLiteral(
                "/usr/local/bin/neebles"
            );

        if (
            QFileInfo::exists(installedCommand)
            && QFileInfo(installedCommand).isExecutable()
        ) {
            command = installedCommand;
        } else {
            command =
                QStringLiteral("neebles");
        }
    }

    return QProcess::startDetached(
        command,
        QStringList{
            QStringLiteral("start")
        }
    );
}
