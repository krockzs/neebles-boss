#include "../../../client/shared/domesticprocess.h"

#include <QCoreApplication>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QProcess>
#include <QProcessEnvironment>
#include <QSet>
#include <QString>
#include <QStringList>

struct Outcome {
    QString status;
    QProcessEnvironment environment;
    bool hasEnvironment = false;
};

static QString requiredString(
    const QJsonObject &object,
    const QString &key
)
{
    const QJsonValue value =
        object.value(key);

    if (!value.isString())
        return {};

    return value.toString();
}

static bool assertOutcome(
    const Outcome &outcome,
    const QJsonObject &testCase,
    QString *error
)
{
    const QString expected =
        requiredString(
            testCase,
            QStringLiteral("expect")
        );

    if (
        expected.isEmpty()
        || outcome.status != expected
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "status mismatch"
                );
        }

        return false;
    }

    const QJsonValue assertionValue =
        testCase.value(
            QStringLiteral("assert")
        );

    if (!assertionValue.isObject())
        return true;

    if (!outcome.hasEnvironment) {
        if (error) {
            *error =
                QStringLiteral(
                    "assertion requires environment outcome"
                );
        }

        return false;
    }

    const QJsonObject assertion =
        assertionValue.toObject();

    const QJsonObject contains =
        assertion
            .value(
                QStringLiteral("contains")
            )
            .toObject();

    for (
        auto it = contains.constBegin();
        it != contains.constEnd();
        ++it
    ) {
        if (
            !it.value().isString()
            || outcome.environment.value(
                it.key()
            ) != it.value().toString()
        ) {
            if (error) {
                *error =
                    QStringLiteral(
                        "contains assertion failed for "
                    )
                    + it.key();
            }

            return false;
        }
    }

    const QJsonArray absent =
        assertion
            .value(
                QStringLiteral("absent")
            )
            .toArray();

    for (
        const QJsonValue &value
        : absent
    ) {
        if (
            !value.isString()
            || outcome.environment.contains(
                value.toString()
            )
        ) {
            if (error) {
                *error =
                    QStringLiteral(
                        "absent assertion failed"
                    );
            }

            return false;
        }
    }

    if (
        assertion.contains(
            QStringLiteral("size")
        )
    ) {
        const int expectedSize =
            assertion
                .value(
                    QStringLiteral("size")
                )
                .toInt(-1);

        if (
            expectedSize < 0
            || outcome.environment.keys().size()
                != expectedSize
        ) {
            if (error) {
                *error =
                    QStringLiteral(
                        "size assertion failed"
                    );
            }

            return false;
        }
    }

    return true;
}

static NeeblesDomesticProcess::EnvironmentClass
environmentClass(
    const QString &name,
    bool *ok
)
{
    if (
        name
        == QStringLiteral("pure")
    ) {
        *ok = true;

        return
            NeeblesDomesticProcess::
                EnvironmentClass::Pure;
    }

    if (
        name
        == QStringLiteral("session")
    ) {
        *ok = true;

        return
            NeeblesDomesticProcess::
                EnvironmentClass::Session;
    }

    if (
        name
        == QStringLiteral(
            "system_interface"
        )
    ) {
        *ok = true;

        return
            NeeblesDomesticProcess::
                EnvironmentClass::
                    SystemInterface;
    }

    *ok = false;

    return
        NeeblesDomesticProcess::
            EnvironmentClass::Pure;
}

static Outcome processEnvironmentCase(
    const QJsonObject &select
)
{
    bool classOk = false;

    const QString className =
        requiredString(
            select,
            QStringLiteral("class")
        );

    const auto processClass =
        environmentClass(
            className,
            &classOk
        );

    if (!classOk)
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };

    const QString domesticMode =
        requiredString(
            select,
            QStringLiteral("domestic")
        );

    const QString sessionMode =
        requiredString(
            select,
            QStringLiteral("session")
        );

    const QString allowlistMode =
        requiredString(
            select,
            QStringLiteral("allowlist")
        );

    const QString application =
        requiredString(
            select,
            QStringLiteral("application")
        );

    QProcessEnvironment domestic;

    if (
        domesticMode
        == QStringLiteral("explicit")
    ) {
        if (
            className
            == QStringLiteral(
                "system_interface"
            )
        ) {
            domestic.insert(
                QStringLiteral(
                    "XDG_RUNTIME_DIR"
                ),
                QStringLiteral(
                    "/run/user/1000"
                )
            );

            domestic.insert(
                QStringLiteral(
                    "DBUS_SESSION_BUS_ADDRESS"
                ),
                QStringLiteral(
                    "unix:path=/run/user/1000/bus"
                )
            );
        } else {
            domestic.insert(
                QStringLiteral(
                    "NEEBLES_TEST"
                ),
                QStringLiteral("inside")
            );
        }
    } else if (
        domesticMode
        == QStringLiteral("private_path")
    ) {
        domestic.insert(
            QStringLiteral("PATH"),
            QStringLiteral(
                "/opt/neebles/private/bin"
            )
        );
    } else if (
        domesticMode
        != QStringLiteral("empty")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcessEnvironment session;

    if (
        sessionMode
        == QStringLiteral("allowed")
    ) {
        session.insert(
            QStringLiteral("DISPLAY"),
            QStringLiteral(":0")
        );

        session.insert(
            QStringLiteral(
                "NEEBLES_TEST"
            ),
            QStringLiteral("outside")
        );
    } else if (
        sessionMode
        == QStringLiteral("hostile")
    ) {
        session.insert(
            QStringLiteral("PATH"),
            QStringLiteral("/host/bin")
        );

        session.insert(
            QStringLiteral("LD_PRELOAD"),
            QStringLiteral(
                "/tmp/evil.so"
            )
        );

        session.insert(
            QStringLiteral("DISPLAY"),
            QStringLiteral(":666")
        );
    } else if (
        sessionMode
        == QStringLiteral(
            "graphical_bridge"
        )
    ) {
        const QJsonObject fixture{
            {
                QStringLiteral("DISPLAY"),
                QStringLiteral(":0")
            },
            {
                QStringLiteral(
                    "WAYLAND_DISPLAY"
                ),
                QStringLiteral(
                    "wayland-test"
                )
            },
            {
                QStringLiteral("XAUTHORITY"),
                QStringLiteral(
                    "/run/user/test/xauthority"
                )
            },
            {
                QStringLiteral(
                    "XDG_RUNTIME_DIR"
                ),
                QStringLiteral(
                    "/run/user/test"
                )
            },
            {
                QStringLiteral(
                    "DBUS_SESSION_BUS_ADDRESS"
                ),
                QStringLiteral(
                    "unix:path=/run/user/test/bus"
                )
            },
            {
                QStringLiteral("PATH"),
                QStringLiteral("/host/bin")
            },
            {
                QStringLiteral("LD_PRELOAD"),
                QStringLiteral(
                    "/host/evil.so"
                )
            },
            {
                QStringLiteral(
                    "XDG_CURRENT_DESKTOP"
                ),
                QStringLiteral(
                    "HOST_DESKTOP"
                )
            },
            {
                QStringLiteral(
                    "DESKTOP_SESSION"
                ),
                QStringLiteral(
                    "host-session"
                )
            }
        };

        for (
            auto it = fixture.constBegin();
            it != fixture.constEnd();
            ++it
        ) {
            session.insert(
                it.key(),
                it.value().toString()
            );
        }
    } else if (
        sessionMode
        != QStringLiteral("empty")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QSet<QString> allowed;

    if (
        allowlistMode
        == QStringLiteral("exact")
    ) {
        allowed.insert(
            QStringLiteral("DISPLAY")
        );
    } else if (
        allowlistMode
        == QStringLiteral("forbidden")
    ) {
        allowed.insert(
            QStringLiteral("PATH")
        );
    } else if (
        allowlistMode
        == QStringLiteral("override")
    ) {
        allowed.insert(
            QStringLiteral(
                "NEEBLES_TEST"
            )
        );
    } else if (
        allowlistMode
        == QStringLiteral(
            "graphical_bridge"
        )
    ) {
        for (
            const QString &key
            : QStringList{
                QStringLiteral("DISPLAY"),
                QStringLiteral(
                    "WAYLAND_DISPLAY"
                ),
                QStringLiteral("XAUTHORITY"),
                QStringLiteral(
                    "XDG_RUNTIME_DIR"
                ),
                QStringLiteral(
                    "DBUS_SESSION_BUS_ADDRESS"
                )
            }
        ) {
            allowed.insert(key);
        }
    } else if (
        allowlistMode
        != QStringLiteral("none")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcessEnvironment sealed;
    QString error;

    if (
        application
        == QStringLiteral("map")
    ) {
        const bool ok =
            NeeblesDomesticProcess::
                buildEnvironment(
                    processClass,
                    domestic,
                    session,
                    allowed,
                    &sealed,
                    &error
                );

        if (!ok) {
            return {
                QStringLiteral("error"),
                {},
                false
            };
        }

        return {
            QStringLiteral("pass"),
            sealed,
            true
        };
    }

    if (
        application
        == QStringLiteral("command")
    ) {
        QProcess process;

        const bool ok =
            NeeblesDomesticProcess::
                configure(
                    &process,
                    processClass,
                    domestic,
                    session,
                    allowed,
                    &error
                );

        if (!ok) {
            return {
                QStringLiteral("error"),
                {},
                false
            };
        }

        return {
            QStringLiteral("pass"),
            process.processEnvironment(),
            true
        };
    }

    return {
        QStringLiteral("runner_error"),
        {},
        false
    };
}

static Outcome rawParserCase(
    const QJsonObject &select
)
{
    if (
        requiredString(
            select,
            QStringLiteral("input_shape")
        )
        != QStringLiteral("mixed")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    const QString raw =
        QStringLiteral(
            "PATH=/host/bin\n"
            "DISPLAY=:0\n"
            "LD_PRELOAD=/tmp/evil.so\n"
            "BROKEN\n"
        );

    const QProcessEnvironment parsed =
        NeeblesDomesticProcess::
            parseEnvironmentLines(raw);

    const QString policy =
        requiredString(
            select,
            QStringLiteral("policy")
        );

    if (
        policy
        == QStringLiteral("parse_only")
    ) {
        return {
            QStringLiteral("pass"),
            parsed,
            true
        };
    }

    if (
        policy
        == QStringLiteral("seal_exact")
    ) {
        QProcessEnvironment sealed;
        QString error;

        const bool ok =
            NeeblesDomesticProcess::
                buildEnvironment(
                    NeeblesDomesticProcess::
                        EnvironmentClass::Session,
                    QProcessEnvironment(),
                    parsed,
                    QSet<QString>{
                        QStringLiteral("DISPLAY")
                    },
                    &sealed,
                    &error
                );

        if (!ok) {
            return {
                QStringLiteral("error"),
                {},
                false
            };
        }

        return {
            QStringLiteral("pass"),
            sealed,
            true
        };
    }

    return {
        QStringLiteral("runner_error"),
        {},
        false
    };
}

static Outcome commandSealingCase(
    const QJsonObject &select
)
{
    if (
        requiredString(
            select,
            QStringLiteral("preseed")
        )
        != QStringLiteral("hostile")
        || requiredString(
            select,
            QStringLiteral("sealed")
        )
        != QStringLiteral("explicit")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcess process;

    QProcessEnvironment hostile;

    hostile.insert(
        QStringLiteral("PATH"),
        QStringLiteral("/host/bin")
    );

    hostile.insert(
        QStringLiteral("LD_PRELOAD"),
        QStringLiteral("/host/evil.so")
    );

    hostile.insert(
        QStringLiteral(
            "UNRELATED_HOST_VALUE"
        ),
        QStringLiteral("must-die")
    );

    process.setProcessEnvironment(
        hostile
    );

    QProcessEnvironment sealed;

    sealed.insert(
        QStringLiteral(
            "NEEBLES_MODULE"
        ),
        QStringLiteral("test-module")
    );

    sealed.insert(
        QStringLiteral("DISPLAY"),
        QStringLiteral(":0")
    );

    process.setProcessEnvironment(
        sealed
    );

    return {
        QStringLiteral("pass"),
        process.processEnvironment(),
        true
    };
}

static Outcome sessionPolicyCase(
    const QJsonObject &select
)
{
    const QString request =
        requiredString(
            select,
            QStringLiteral("request")
        );

    const QString source =
        requiredString(
            select,
            QStringLiteral("source")
        );

    const QString collision =
        requiredString(
            select,
            QStringLiteral(
                "domestic_collision"
            )
        );

    QProcessEnvironment domestic;
    QProcessEnvironment session;
    QSet<QString> allowed;

    if (
        collision
        == QStringLiteral("yes")
    ) {
        domestic.insert(
            QStringLiteral(
                "NEEBLES_CONFIG"
            ),
            QStringLiteral(
                "/domestic/config"
            )
        );
    } else if (
        collision
        != QStringLiteral("no")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    if (
        request
        == QStringLiteral("exact")
    ) {
        allowed.insert(
            QStringLiteral("DISPLAY")
        );

        if (
            source
            == QStringLiteral("present")
        ) {
            session.insert(
                QStringLiteral("DISPLAY"),
                QStringLiteral(":0")
            );
        }
    } else if (
        request
        == QStringLiteral("forbidden")
    ) {
        allowed.insert(
            QStringLiteral("PATH")
        );

        if (
            source
            == QStringLiteral("present")
        ) {
            session.insert(
                QStringLiteral("PATH"),
                QStringLiteral("/host/bin")
            );
        }
    } else if (
        request
        == QStringLiteral("override")
    ) {
        allowed.insert(
            QStringLiteral(
                "NEEBLES_CONFIG"
            )
        );

        if (
            source
            == QStringLiteral("present")
        ) {
            session.insert(
                QStringLiteral(
                    "NEEBLES_CONFIG"
                ),
                QStringLiteral(
                    "/host/config"
                )
            );
        }
    } else if (
        request
        != QStringLiteral("absent")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcessEnvironment sealed;
    QString error;

    const bool ok =
        NeeblesDomesticProcess::
            buildEnvironment(
                NeeblesDomesticProcess::
                    EnvironmentClass::Session,
                domestic,
                session,
                allowed,
                &sealed,
                &error
            );

    if (!ok) {
        return {
            QStringLiteral("error"),
            {},
            false
        };
    }

    return {
        QStringLiteral("pass"),
        sealed,
        true
    };
}

static Outcome selectionCase(
    const QJsonObject &select
)
{
    if (
        requiredString(
            select,
            QStringLiteral("selection")
        )
        != QStringLiteral("exact")
        || requiredString(
            select,
            QStringLiteral("source")
        )
        != QStringLiteral("neighbors")
    ) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcessEnvironment session;

    const QJsonObject fixture{
        {
            QStringLiteral("DISPLAY"),
            QStringLiteral(":0")
        },
        {
            QStringLiteral(
                "XDG_SESSION_TYPE"
            ),
            QStringLiteral("wayland")
        },
        {
            QStringLiteral(
                "XDG_CURRENT_DESKTOP"
            ),
            QStringLiteral("KDE")
        },
        {
            QStringLiteral(
                "QT_SCALE_FACTOR"
            ),
            QStringLiteral("2")
        },
        {
            QStringLiteral(
                "KDE_FULL_SESSION"
            ),
            QStringLiteral("true")
        }
    };

    for (
        auto it = fixture.constBegin();
        it != fixture.constEnd();
        ++it
    ) {
        session.insert(
            it.key(),
            it.value().toString()
        );
    }

    QProcessEnvironment sealed;
    QString error;

    const bool ok =
        NeeblesDomesticProcess::
            buildEnvironment(
                NeeblesDomesticProcess::
                    EnvironmentClass::Session,
                QProcessEnvironment(),
                session,
                QSet<QString>{
                    QStringLiteral("DISPLAY"),
                    QStringLiteral(
                        "XDG_SESSION_TYPE"
                    )
                },
                &sealed,
                &error
            );

    if (!ok) {
        return {
            QStringLiteral("error"),
            {},
            false
        };
    }

    return {
        QStringLiteral("pass"),
        sealed,
        true
    };
}

static Outcome forbiddenCase(
    const QJsonObject &select
)
{
    const QString key =
        requiredString(
            select,
            QStringLiteral("key")
        );

    if (key.isEmpty()) {
        return {
            QStringLiteral("runner_error"),
            {},
            false
        };
    }

    QProcessEnvironment session;

    session.insert(
        key,
        QStringLiteral(
            "host-controlled"
        )
    );

    QProcessEnvironment sealed;
    QString error;

    const bool ok =
        NeeblesDomesticProcess::
            buildEnvironment(
                NeeblesDomesticProcess::
                    EnvironmentClass::Session,
                QProcessEnvironment(),
                session,
                QSet<QString>{key},
                &sealed,
                &error
            );

    if (!ok) {
        return {
            QStringLiteral("error"),
            {},
            false
        };
    }

    return {
        QStringLiteral("pass"),
        sealed,
        true
    };
}

static bool executeMatrix(
    const QString &path,
    const QString &expectedCategory,
    int *executed,
    QString *error
)
{
    QFile file(path);

    if (
        !file.open(
            QIODevice::ReadOnly
        )
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "cannot open tester "
                )
                + path;
        }

        return false;
    }

    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            file.readAll(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid tester JSON "
                )
                + path;
        }

        return false;
    }

    const QJsonObject definition =
        document.object();

    if (
        requiredString(
            definition,
            QStringLiteral("schema")
        )
            != QStringLiteral("3")
        || requiredString(
            definition,
            QStringLiteral("category")
        )
            != expectedCategory
        || requiredString(
            definition,
            QStringLiteral("tester")
        )
            != QStringLiteral("matrix")
        || definition.value(
            QStringLiteral(
                "technology_specific"
            )
        ).toBool(true)
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "tester identity mismatch"
                );
        }

        return false;
    }

    const QJsonArray spells =
        definition
            .value(
                QStringLiteral("spells")
            )
            .toArray();

    for (
        const QJsonValue &spellValue
        : spells
    ) {
        const QJsonObject spell =
            spellValue.toObject();

        const QString spellName =
            requiredString(
                spell,
                QStringLiteral("name")
            );

        const QString dispatchSpellName =
            spellName.startsWith(
                QStringLiteral("boss.")
            )
            ? spellName.mid(5)
            : spellName;

        if (
            dispatchSpellName
            == QStringLiteral("physical")
        ) {
            continue;
        }

        const QJsonArray cases =
            spell
                .value(
                    QStringLiteral("cases")
                )
                .toArray();

        for (
            const QJsonValue &caseValue
            : cases
        ) {
            const QJsonObject testCase =
                caseValue.toObject();

            const QString id =
                requiredString(
                    testCase,
                    QStringLiteral("id")
                );

            const QJsonObject select =
                testCase
                    .value(
                        QStringLiteral("select")
                    )
                    .toObject();

            Outcome outcome;

            if (
                dispatchSpellName
                == QStringLiteral(
                    "process_environment"
                )
            ) {
                outcome =
                    processEnvironmentCase(
                        select
                    );
            } else if (
                dispatchSpellName
                == QStringLiteral(
                    "raw_environment_parser"
                )
            ) {
                outcome =
                    rawParserCase(
                        select
                    );
            } else if (
                dispatchSpellName
                == QStringLiteral(
                    "command_sealing"
                )
            ) {
                outcome =
                    commandSealingCase(
                        select
                    );
            } else if (
                dispatchSpellName
                == QStringLiteral(
                    "session_policy"
                )
            ) {
                outcome =
                    sessionPolicyCase(
                        select
                    );
            } else if (
                dispatchSpellName
                == QStringLiteral(
                    "selection_semantics"
                )
            ) {
                outcome =
                    selectionCase(
                        select
                    );
            } else if (
                dispatchSpellName
                == QStringLiteral(
                    "forbidden_catalog"
                )
            ) {
                outcome =
                    forbiddenCase(
                        select
                    );
            } else {
                if (error) {
                    *error =
                        QStringLiteral(
                            "Qt matrix runner does not know generic spell "
                        )
                        + spellName;
                }

                return false;
            }

            QString assertionError;

            if (
                !assertOutcome(
                    outcome,
                    testCase,
                    &assertionError
                )
            ) {
                if (error) {
                    *error =
                        expectedCategory
                        + QLatin1Char('/')
                        + spellName
                        + QLatin1Char('/')
                        + id
                        + QStringLiteral(
                            " :: "
                        )
                        + assertionError;
                }

                return false;
            }

            ++(*executed);
        }
    }

    return true;
}

int main(
    int argc,
    char **argv
)
{
    QCoreApplication app(
        argc,
        argv
    );

    const QStringList arguments =
        app.arguments();

    if (arguments.size() != 3)
        return 2;

    int executed = 0;
    QString error;

    if (
        !executeMatrix(
            arguments.at(1),
            QStringLiteral("environment"),
            &executed,
            &error
        )
    ) {
        qCritical().noquote()
            << error;

        return 3;
    }

    if (
        !executeMatrix(
            arguments.at(2),
            QStringLiteral(
                "allowed_session_inputs"
            ),
            &executed,
            &error
        )
    ) {
        qCritical().noquote()
            << error;

        return 4;
    }

    if (executed <= 0)
        return 5;

    qInfo()
        << "QT DOMESTIC MATRIX CASES ::"
        << executed;

    return 0;
}
