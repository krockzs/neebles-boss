#include "installercontroller.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QLocale>
#include <QRegularExpression>

InstallerController::InstallerController(
    const QStringList &arguments,
    const QVariantMap &strings,
    const QString &language,
    QObject *parent
)
    : QObject(parent),
      m_strings(strings),
      m_language(language)
{
    m_status = text(
        QStringLiteral(
            "installer.status.waiting_authorization"
        )
    );
    if (arguments.size() >= 6) {
        m_installScript = arguments.at(1);
        m_backendBinary = arguments.at(2);
        m_uiBinary = arguments.at(3);
        m_trayBinary = arguments.at(4);
        m_clientDataArchive = arguments.at(5);
    }

    m_process.setProcessChannelMode(QProcess::MergedChannels);

    connect(&m_process, &QProcess::readyReadStandardOutput,
            this, &InstallerController::readOutput);
    connect(&m_process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
            this, &InstallerController::processFinished);
    connect(&m_process, &QProcess::errorOccurred,
            this, &InstallerController::processError);
}

QString InstallerController::text(
    const QString &key
) const
{
    return m_strings
        .value(key, key)
        .toString();
}


QString InstallerController::authorizationPath() const
{
    const QString override =
        qEnvironmentVariable(
            "NEEBLES_AUTH_AGENT"
        );

    const QStringList candidates = {
        override,

        QStringLiteral(
            "/opt/neebles/client/auth/neebles-auth-agent"
        ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "neebles-auth-agent"
            )
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
            !candidate.isEmpty()
            && QFileInfo(candidate).isExecutable()
        ) {
            return QFileInfo(
                candidate
            ).absoluteFilePath();
        }
    }

    return {};
}


void InstallerController::startInstallation()
{
    if (m_running)
        return;

    if (m_installScript.isEmpty() || m_backendBinary.isEmpty() || m_uiBinary.isEmpty()
        || m_trayBinary.isEmpty() || m_clientDataArchive.isEmpty()) {
        setStatus(
            text(
                QStringLiteral(
                    "installer.status.payload_incomplete"
                )
            )
        );
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.expected_arguments"
                )
            )
        );
        m_finished = true;
        m_success = false;
        emit finishedChanged();
        return;
    }

    if (!QFileInfo::exists(m_installScript) ||
        !QFileInfo::exists(m_backendBinary) ||
        !QFileInfo::exists(m_uiBinary) ||
        !QFileInfo::exists(m_trayBinary) ||
        !QFileInfo::exists(m_clientDataArchive)) {
        setStatus(
            text(
                QStringLiteral(
                    "installer.status.payload_missing"
                )
            )
        );
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.payload_missing"
                )
            )
        );
        m_finished = true;
        m_success = false;
        emit finishedChanged();
        return;
    }

    m_running = true;
    m_finished = false;
    m_success = false;
    emit runningChanged();
    emit finishedChanged();

    setStatus(
        text(
            QStringLiteral(
                "installer.status.waiting_authorization"
            )
        )
    );
    setProgress(2);
    appendLog(
        text(
            QStringLiteral(
                "installer.log.requesting_privileges"
            )
        )
    );

    const QString authAgent =
        authorizationPath();

    if (authAgent.isEmpty()) {
        setStatus(
            text(
                QStringLiteral(
                    "installer.status.auth_agent_missing"
                )
            )
        );

        appendLog(
            text(
                QStringLiteral(
                    "installer.log.auth_agent_missing"
                )
            )
        );

        m_running = false;
        m_finished = true;
        m_success = false;

        emit runningChanged();
        emit finishedChanged();

        return;
    }

    QStringList args {
        QStringLiteral("--locale"),
        m_language,

        QStringLiteral("--operation"),
        QStringLiteral("install-boss"),

        QStringLiteral("--name"),
        QStringLiteral("N.E.E.B.L.E.S. Boss"),

        QStringLiteral("--running"),
        QStringLiteral("false"),

        QStringLiteral("--"),

        m_installScript,
        m_backendBinary,
        m_uiBinary,
        m_trayBinary,
        m_clientDataArchive,

        /*
         * install.sh receives the auth agent as
         * mandatory fifth payload and installs it
         * permanently with Boss.
         */
        authAgent
    };

    m_process.start(
        authAgent,
        args
    );
}

void InstallerController::readOutput()
{
    m_buffer.append(m_process.readAllStandardOutput());

    int newline = -1;
    while ((newline = m_buffer.indexOf('\n')) >= 0) {
        const QByteArray raw = m_buffer.left(newline);
        m_buffer.remove(0, newline + 1);
        consumeLine(QString::fromUtf8(raw).trimmed());
    }
}

void InstallerController::consumeLine(const QString &line)
{
    if (line.isEmpty())
        return;

    static const QRegularExpression progressRx(
        QStringLiteral("^NEEBLES_PROGRESS=(\\d{1,3})$")
    );

    static const QRegularExpression statusKeyRx(
        QStringLiteral("^NEEBLES_STATUS_KEY=([A-Za-z0-9_.-]+)$")
    );

    const auto progressMatch = progressRx.match(line);
    if (progressMatch.hasMatch()) {
        setProgress(progressMatch.captured(1).toInt());
        return;
    }

    const auto statusKeyMatch =
        statusKeyRx.match(line);

    if (statusKeyMatch.hasMatch()) {
        const QString key =
            statusKeyMatch.captured(1);

        const QString value =
            text(key);

        setStatus(value);
        appendLog(value);

        return;
    }

    appendLog(line);
}

void InstallerController::processFinished(int exitCode, QProcess::ExitStatus exitStatus)
{
    if (!m_buffer.isEmpty()) {
        consumeLine(QString::fromUtf8(m_buffer).trimmed());
        m_buffer.clear();
    }

    m_running = false;
    emit runningChanged();

    m_finished = true;
    m_success = (exitStatus == QProcess::NormalExit && exitCode == 0);

    if (m_success) {
        setProgress(100);
        setStatus(
            text(
                QStringLiteral(
                    "installer.status.completed"
                )
            )
        );
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.completed"
                )
            )
        );
        integrateDesktop();
    } else {
        setStatus(
            text(
                QStringLiteral(
                    "installer.status.cancelled_or_failed"
                )
            )
        );
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.exit_code"
                )
            ).arg(exitCode)
        );
    }

    emit finishedChanged();
}

void InstallerController::processError(QProcess::ProcessError error)
{
    Q_UNUSED(error)

    if (m_process.state() != QProcess::NotRunning)
        return;

    m_running = false;
    m_finished = true;
    m_success = false;
    emit runningChanged();
    emit finishedChanged();

    setStatus(
        text(
            QStringLiteral(
                "installer.status.start_failed"
            )
        )
    );
    appendLog(
        text(
            QStringLiteral(
                "installer.log.process_error"
            )
        ).arg(
            m_process.errorString()
        )
    );
}

void InstallerController::integrateDesktop()
{
    startTray();
    installLauncherIntoPanel();
}

void InstallerController::startTray()
{
    const QString trayPath =
        QStringLiteral(
            "/opt/neebles/client/tray-host/neebles-tray-host"
        );

    if (!QFileInfo::exists(trayPath)) {
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.tray_missing"
                )
            )
        );
        return;
    }

    const bool started =
        QProcess::startDetached(trayPath, {});

    appendLog(
        started
            ? text(
                  QStringLiteral(
                      "installer.log.tray_started"
                  )
              )
            : text(
                  QStringLiteral(
                      "installer.log.tray_start_failed"
                  )
              )
    );
}

void InstallerController::installLauncherIntoPanel()
{
    const QString packagePath =
        QStringLiteral(
            "/usr/share/plasma/plasmoids/org.neebles.launcher"
        );

    if (!QFileInfo::exists(
            packagePath
            + QStringLiteral("/metadata.json")
        )) {
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.launcher_missing"
                )
            )
        );
        return;
    }

    QString qdbus;

    if (
        QFileInfo::exists(
            QStringLiteral("/usr/bin/qdbus6")
        )
    )
        qdbus = QStringLiteral("/usr/bin/qdbus6");
    else if (
        QFileInfo::exists(
            QStringLiteral("/usr/bin/qdbus")
        )
    )
        qdbus = QStringLiteral("/usr/bin/qdbus");
    else {
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.qdbus_missing"
                )
            )
        );
        return;
    }

    const QString script =
        QStringLiteral(R"JS(
var ps = panels();

/*
 * Remove stale N.E.E.B.L.E.S. panel instances first.
 * This also normalizes upgrades from older layouts.
 */
for (var p = 0; p < ps.length; ++p) {
    var oldWidgets = ps[p].widgets();

    for (var i = oldWidgets.length - 1; i >= 0; --i) {
        if (
            oldWidgets[i].type == "org.neebles.launcher"
            || oldWidgets[i].type == "org.neebles.spacer"
        ) {
            oldWidgets[i].remove();
        }
    }
}

/*
 * Find the panel that owns the application launcher.
 */
var targetPanel = null;
var kickoffX = 0;
var kickoffWidth = 0;

for (var p = 0; p < ps.length; ++p) {
    var widgets = ps[p].widgets();

    for (var i = 0; i < widgets.length; ++i) {
        if (
            widgets[i].type == "org.kde.plasma.kickoff"
            || widgets[i].type == "org.kde.plasma.kicker"
        ) {
            targetPanel = ps[p];
            kickoffX = widgets[i].geometry.x;
            kickoffWidth = widgets[i].geometry.width;
            break;
        }
    }

    if (targetPanel)
        break;
}

if (targetPanel) {
    /*
     * Physical positioning is intentional:
     *
     * [ Kickoff ][ N.E.E.B.L.E.S. ][ 20 px spacer ][ rest ]
     */
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
)JS");

    QProcess process;

    process.start(
        qdbus,
        {
            QStringLiteral(
                "org.kde.plasmashell"
            ),
            QStringLiteral(
                "/PlasmaShell"
            ),
            QStringLiteral(
                "org.kde.PlasmaShell.evaluateScript"
            ),
            script
        }
    );

    if (
        !process.waitForStarted(3000)
        || !process.waitForFinished(10000)
        || process.exitStatus()
            != QProcess::NormalExit
        || process.exitCode() != 0
    ) {
        appendLog(
            text(
                QStringLiteral(
                    "installer.log.launcher_integration_failed"
                )
            )
        );
        return;
    }

    appendLog(
        text(
            QStringLiteral(
                "installer.log.launcher_added"
            )
        )
    );
}

void InstallerController::setProgress(int value)
{
    value = qBound(0, value, 100);
    if (m_progress == value)
        return;

    m_progress = value;
    emit progressChanged();
}

void InstallerController::setStatus(const QString &value)
{
    if (m_status == value)
        return;

    m_status = value;
    emit statusChanged();
}

void InstallerController::appendLog(const QString &line)
{
    emit logLine(line);
}
