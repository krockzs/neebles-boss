#pragma once

#include <QProcess>
#include <QProcessEnvironment>
#include <QSet>
#include <QString>

namespace NeeblesDomesticProcess {

enum class EnvironmentClass {
    Pure,
    Session,
    SystemInterface
};

inline QProcessEnvironment parseEnvironmentLines(
    const QString &raw
)
{
    QProcessEnvironment result;

    const QStringList lines =
        raw.split(
            QLatin1Char('\n')
        );

    for (
        const QString &line
        : lines
    ) {
        const qsizetype separator =
            line.indexOf(
                QLatin1Char('=')
            );

        if (separator < 0)
            continue;

        const QString key =
            line.left(
                separator
            ).trimmed();

        if (key.isEmpty())
            continue;

        result.insert(
            key,
            line.mid(
                separator + 1
            )
        );
    }

    return result;
}

inline bool forbiddenSessionInput(
    const QString &key
)
{
    static const QSet<QString> forbidden{
        QStringLiteral("PATH"),
        QStringLiteral("LD_PRELOAD"),
        QStringLiteral("LD_LIBRARY_PATH"),
        QStringLiteral("PYTHONPATH"),
        QStringLiteral("PYTHONHOME"),
        QStringLiteral("QT_PLUGIN_PATH"),
        QStringLiteral("QT_QPA_PLATFORM_PLUGIN_PATH"),
        QStringLiteral("QML_IMPORT_PATH"),
        QStringLiteral("QML2_IMPORT_PATH"),
        QStringLiteral("GI_TYPELIB_PATH"),
        QStringLiteral("GIO_EXTRA_MODULES")
    };

    return forbidden.contains(
        key
    );
}

inline bool buildEnvironment(
    EnvironmentClass environmentClass,
    const QProcessEnvironment &domestic,
    const QProcessEnvironment &session,
    const QSet<QString> &allowedSessionInputs,
    QProcessEnvironment *result,
    QString *error = nullptr
)
{
    if (!result) {
        if (error) {
            *error =
                QStringLiteral(
                    "result environment is null"
                );
        }

        return false;
    }

    if (
        environmentClass
            != EnvironmentClass::Session
        && !allowedSessionInputs.isEmpty()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "non-session process cannot receive allowed session inputs"
                );
        }

        return false;
    }

    QProcessEnvironment sealed;

    const QStringList domesticKeys =
        domestic.keys();

    for (
        const QString &key
        : domesticKeys
    ) {
        if (key.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "domestic environment contains empty key"
                    );
            }

            return false;
        }

        sealed.insert(
            key,
            domestic.value(key)
        );
    }

    if (
        environmentClass
        == EnvironmentClass::Session
    ) {
        for (
            const QString &key
            : allowedSessionInputs
        ) {
            if (key.isEmpty()) {
                if (error) {
                    *error =
                        QStringLiteral(
                            "allowed session input contains empty key"
                        );
                }

                return false;
            }

            if (
                forbiddenSessionInput(key)
            ) {
                if (error) {
                    *error =
                        QStringLiteral(
                            "forbidden session input: "
                        )
                        + key;
                }

                return false;
            }

            if (
                domestic.contains(key)
            ) {
                if (error) {
                    *error =
                        QStringLiteral(
                            "session input cannot override domestic environment: "
                        )
                        + key;
                }

                return false;
            }

            if (
                session.contains(key)
            ) {
                sealed.insert(
                    key,
                    session.value(key)
                );
            }
        }
    }

    *result = sealed;

    return true;
}

inline bool configure(
    QProcess *process,
    EnvironmentClass environmentClass,
    const QProcessEnvironment &domestic,
    const QProcessEnvironment &session,
    const QSet<QString> &allowedSessionInputs,
    QString *error = nullptr
)
{
    if (!process) {
        if (error) {
            *error =
                QStringLiteral(
                    "process is null"
                );
        }

        return false;
    }

    QProcessEnvironment sealed;

    if (
        !buildEnvironment(
            environmentClass,
            domestic,
            session,
            allowedSessionInputs,
            &sealed,
            error
        )
    ) {
        return false;
    }

    process->setProcessEnvironment(
        sealed
    );

    return true;
}

}
