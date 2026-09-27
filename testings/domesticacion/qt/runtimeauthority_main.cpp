#include "runtimeauthority.h"

#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>
#include <QTextStream>

static QString takeArgument(
    const QStringList &arguments,
    const QString &name,
    QString *error
)
{
    const int index =
        arguments.indexOf(
            name
        );

    if (
        index < 0
        || index + 1 >= arguments.size()
        || arguments.indexOf(
               name,
               index + 1
           ) >= 0
    ) {
        if (error) {
            *error =
                name
                + QStringLiteral(
                    " must be supplied exactly once"
                );
        }

        return {};
    }

    return arguments.at(
        index + 1
    );
}

int main(
    int argc,
    char *argv[]
)
{
    QCoreApplication app(
        argc,
        argv
    );

    QString error;

    const QStringList arguments =
        app.arguments();

    const QString resolver =
        takeArgument(
            arguments,
            QStringLiteral(
                "--resolver"
            ),
            &error
        );

    if (!error.isEmpty()) {
        QTextStream(stderr)
            << error
            << Qt::endl;

        return 2;
    }

    const QString manifest =
        takeArgument(
            arguments,
            QStringLiteral(
                "--manifest"
            ),
            &error
        );

    if (!error.isEmpty()) {
        QTextStream(stderr)
            << error
            << Qt::endl;

        return 2;
    }

    const QStringList worlds = {
        QStringLiteral(
            "boss.tar"
        ),
        QStringLiteral(
            "boss.systemctl"
        ),
        QStringLiteral(
            "boss.qdbus6"
        ),
        QStringLiteral(
            "boss.pkexec"
        )
    };

    QJsonObject resolved;

    for (const QString &world : worlds) {
        const QString target =
            NeeblesRuntimeAuthority::resolve(
                resolver,
                manifest,
                world,
                QStringLiteral(
                    "executable"
                ),
                &error
            );

        if (target.isEmpty()) {
            QTextStream(stderr)
                << world
                << QStringLiteral(
                    " :: "
                )
                << error
                << Qt::endl;

            return 1;
        }

        resolved.insert(
            world,
            target
        );
    }

    QJsonObject output;

    output.insert(
        QStringLiteral(
            "schema"
        ),
        QStringLiteral(
            "1"
        )
    );

    output.insert(
        QStringLiteral(
            "name"
        ),
        QStringLiteral(
            "neebles-qt-runtime-authority-certification"
        )
    );

    output.insert(
        QStringLiteral(
            "resolved"
        ),
        resolved
    );

    QTextStream(stdout)
        << QJsonDocument(
               output
           ).toJson(
               QJsonDocument::Compact
           )
        << Qt::endl;

    return 0;
}
