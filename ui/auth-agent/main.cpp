#include "authagent.h"

#include <QApplication>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QTimer>

#include <PolkitQt1/Subject>


static QHash<QString, QString>
loadStrings(QString locale)
{
    if (locale.startsWith(
            QStringLiteral("es"),
            Qt::CaseInsensitive
        )) {
        locale =
            QStringLiteral("es_CL");
    } else {
        locale =
            QStringLiteral("en_US");
    }

    const QString path =
        QStringLiteral(
            ":/languages/%1.json"
        ).arg(locale);

    QFile file(path);

    if (!file.open(QIODevice::ReadOnly)) {
        if (
            locale
            != QStringLiteral("en_US")
        ) {
            return loadStrings(
                QStringLiteral("en_US")
            );
        }

        return {};
    }

    const QJsonDocument document =
        QJsonDocument::fromJson(
            file.readAll()
        );

    QHash<QString, QString> result;

    const QJsonObject object =
        document.object();

    for (
        auto it = object.constBegin();
        it != object.constEnd();
        ++it
    ) {
        result.insert(
            it.key(),
            it.value().toString()
        );
    }

    return result;
}


static QString bossIconPath()
{
    const QString clientRoot =
        qEnvironmentVariable(
            "NEEBLES_CLIENT_ROOT"
        );

    const QStringList candidates = {
        clientRoot.isEmpty()
            ? QString()
            : QDir(clientRoot).filePath(
                  QStringLiteral(
                      "assets/branding/neebles-boss-launcher-icon.png"
                  )
              ),

        QDir::current().filePath(
            QStringLiteral(
                "client/assets/branding/neebles-boss-launcher-icon.png"
            )
        ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../../../client/assets/branding/neebles-boss-launcher-icon.png"
            )
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/neebles-boss-launcher-icon.png"
        ),

        QStringLiteral(
            "/usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png"
        )
    };

    for (const QString &candidate : candidates) {
        if (
            !candidate.isEmpty()
            && QFileInfo::exists(candidate)
        ) {
            return QFileInfo(
                candidate
            ).absoluteFilePath();
        }
    }

    return {};
}


static bool parseBool(
    const QString &value
)
{
    return value.compare(
               QStringLiteral("true"),
               Qt::CaseInsensitive
           ) == 0
        || value == QStringLiteral("1")
        || value.compare(
               QStringLiteral("yes"),
               Qt::CaseInsensitive
           ) == 0;
}


int main(
    int argc,
    char *argv[]
)
{
    QApplication app(argc, argv);

    /*
     * The authentication dialog may close before pkexec
     * finishes. The agent must stay alive until the
     * privileged process exits.
     */
    app.setQuitOnLastWindowClosed(false);

    app.setApplicationName(
        QStringLiteral(
            "neebles-auth-agent"
        )
    );

    const QString iconPath =
        bossIconPath();

    if (!iconPath.isEmpty()) {
        app.setWindowIcon(
            QIcon(iconPath)
        );
    }

    const QStringList args =
        app.arguments();

    NeeblesAuthContext context;

    QStringList command;

    bool commandSection = false;

    for (
        int i = 1;
        i < args.size();
        ++i
    ) {
        const QString value =
            args.at(i);

        if (commandSection) {
            command.append(value);
            continue;
        }

        if (value == QStringLiteral("--")) {
            commandSection = true;
            continue;
        }

        auto takeValue =
            [&](QString &target) -> bool {
                if (i + 1 >= args.size())
                    return false;

                target = args.at(++i);
                return true;
            };

        if (
            value
            == QStringLiteral("--locale")
        ) {
            if (!takeValue(context.locale))
                return 2;
        } else if (
            value
            == QStringLiteral("--operation")
        ) {
            if (!takeValue(context.operation))
                return 2;
        } else if (
            value
            == QStringLiteral("--name")
        ) {
            if (!takeValue(context.name))
                return 2;
        } else if (
            value
            == QStringLiteral("--from")
        ) {
            if (!takeValue(context.fromVersion))
                return 2;
        } else if (
            value
            == QStringLiteral("--to")
        ) {
            if (!takeValue(context.toVersion))
                return 2;
        } else if (
            value
            == QStringLiteral("--running")
        ) {
            QString running;

            if (!takeValue(running))
                return 2;

            context.running =
                parseBool(running);
        }
    }

    if (command.isEmpty())
        return 2;

    if (context.locale.isEmpty())
        context.locale =
            QStringLiteral("en_US");

    const auto strings =
        loadStrings(context.locale);

    NeeblesAuthListener listener(
        context,
        strings
    );

    PolkitQt1::UnixProcessSubject subject(
        QCoreApplication::applicationPid()
    );

    if (
        !listener.registerListener(
            subject,
            QStringLiteral(
                "/org/neebles/PolicyKit1/AuthenticationAgent"
            )
        )
    ) {
        return 3;
    }

    QProcess privilegedProcess;

    privilegedProcess.setProcessChannelMode(
        QProcess::ForwardedChannels
    );

    QObject::connect(
        &privilegedProcess,
        qOverload<
            int,
            QProcess::ExitStatus
        >(&QProcess::finished),
        &app,
        [&](int exitCode,
            QProcess::ExitStatus status) {

            if (
                status
                != QProcess::NormalExit
            ) {
                app.exit(1);
                return;
            }

            /*
             * pkexec returns non-zero when authorization
             * is cancelled or denied.
             *
             * Propagate that exact result to Boss.
             */
            if (exitCode != 0) {
                app.exit(exitCode);
                return;
            }

            app.exit(0);
        }
    );

    QObject::connect(
        &privilegedProcess,
        &QProcess::errorOccurred,
        &app,
        [&](QProcess::ProcessError) {
            if (
                privilegedProcess.state()
                == QProcess::NotRunning
            )
                app.exit(1);
        }
    );

    QTimer::singleShot(
        0,
        &app,
        [&]() {
            QStringList pkexecArgs;

            pkexecArgs
                << QStringLiteral(
                       "--disable-internal-agent"
                   );

            pkexecArgs
                << command.first();

            pkexecArgs
                << command.mid(1);

            privilegedProcess.start(
                QStringLiteral("pkexec"),
                pkexecArgs
            );
        }
    );

    return app.exec();
}
