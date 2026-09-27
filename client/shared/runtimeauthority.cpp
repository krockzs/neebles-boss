#include "runtimeauthority.h"

#include "domesticprocess.h"

#include <QByteArray>
#include <QFileInfo>
#include <QProcess>
#include <QProcessEnvironment>
#include <QSet>
#include <QStringList>

namespace NeeblesRuntimeAuthority
{

QString resolve(
    const QString &resolver,
    const QString &manifest,
    const QString &world,
    const QString &category,
    QString *error
)
{
    if (error)
        error->clear();

    const QFileInfo resolverInfo(
        resolver
    );

    if (
        !resolverInfo.isAbsolute()
        || !resolverInfo.isFile()
        || !resolverInfo.isExecutable()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "runtime resolver is not an absolute executable file: "
                )
                + resolver;
        }

        return {};
    }

    QProcess process;

    QString domesticError;

    if (
        !NeeblesDomesticProcess::configure(
            &process,
            NeeblesDomesticProcess::EnvironmentClass::Pure,
            QProcessEnvironment(),
            QProcessEnvironment(),
            QSet<QString>(),
            &domesticError
        )
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not seal runtime resolver process: "
                )
                + domesticError;
        }

        return {};
    }

    process.setProcessChannelMode(
        QProcess::SeparateChannels
    );

    process.start(
        resolver,
        {
            QStringLiteral("--manifest"),
            manifest,
            QStringLiteral("--world"),
            world,
            QStringLiteral("--category"),
            category
        }
    );

    if (!process.waitForStarted(5000)) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not start runtime resolver: "
                )
                + process.errorString();
        }

        return {};
    }

    if (!process.waitForFinished(10000)) {
        process.kill();
        process.waitForFinished();

        if (error) {
            *error =
                QStringLiteral(
                    "runtime resolver timed out"
                );
        }

        return {};
    }

    const QByteArray standardError =
        process.readAllStandardError();

    if (
        process.exitStatus()
            != QProcess::NormalExit
        || process.exitCode() != 0
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "runtime resolver failed"
                );

            const QString detail =
                QString::fromUtf8(
                    standardError
                ).trimmed();

            if (!detail.isEmpty()) {
                *error +=
                    QStringLiteral(
                        ": "
                    )
                    + detail;
            }
        }

        return {};
    }

    QByteArray output =
        process.readAllStandardOutput();

    while (
        output.endsWith('\n')
        || output.endsWith('\r')
    ) {
        output.chop(1);
    }

    if (
        output.isEmpty()
        || output.contains('\n')
        || output.contains('\r')
        || output.contains('\0')
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "runtime resolver returned invalid stdout contract"
                );
        }

        return {};
    }

    return QString::fromUtf8(
        output
    );
}

}
