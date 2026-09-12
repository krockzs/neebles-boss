#include "installercontroller.h"

#include <QCoreApplication>
#include <QFileInfo>
#include <QRegularExpression>

InstallerController::InstallerController(const QStringList &arguments, QObject *parent)
    : QObject(parent)
{
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

void InstallerController::startInstallation()
{
    if (m_running)
        return;

    if (m_installScript.isEmpty() || m_backendBinary.isEmpty() || m_uiBinary.isEmpty()
        || m_trayBinary.isEmpty() || m_clientDataArchive.isEmpty()) {
        setStatus(QStringLiteral("Installer payload is incomplete."));
        appendLog(QStringLiteral(
            "ERROR: expected installer arguments: <install.sh> <backend> <ui> <tray> <client-data.tar.gz>"
        ));
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
        setStatus(QStringLiteral("Installer payload files are missing."));
        appendLog(QStringLiteral("ERROR: one or more installer payload files do not exist."));
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

    setStatus(QStringLiteral("Waiting for administrator authorization..."));
    setProgress(2);
    appendLog(QStringLiteral("Requesting administrator privileges through Polkit..."));

    const QStringList args {
        m_installScript,
        m_backendBinary,
        m_uiBinary,
        m_trayBinary,
        m_clientDataArchive
    };

    m_process.start(QStringLiteral("pkexec"), args);
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

    static const QRegularExpression progressRx(QStringLiteral("^NEEBLES_PROGRESS=(\\d{1,3})$"));
    static const QRegularExpression statusRx(QStringLiteral("^NEEBLES_STATUS=(.+)$"));

    const auto progressMatch = progressRx.match(line);
    if (progressMatch.hasMatch()) {
        setProgress(progressMatch.captured(1).toInt());
        return;
    }

    const auto statusMatch = statusRx.match(line);
    if (statusMatch.hasMatch()) {
        setStatus(statusMatch.captured(1));
        appendLog(statusMatch.captured(1));
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
        setStatus(QStringLiteral("N.E.E.B.L.E.S. installation completed."));
        appendLog(QStringLiteral("Installation completed successfully."));
        integrateDesktop();
    } else {
        setStatus(QStringLiteral("Installation was cancelled or failed."));
        appendLog(QStringLiteral("Installer exited with code %1.").arg(exitCode));
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

    setStatus(QStringLiteral("Could not start the privileged installer."));
    appendLog(QStringLiteral("ERROR: %1").arg(m_process.errorString()));
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
            "/opt/neebles/client/tray/neebles-tray"
        );

    if (!QFileInfo::exists(trayPath)) {
        appendLog(
            QStringLiteral(
                "WARNING: tray binary not found."
            )
        );
        return;
    }

    const bool started =
        QProcess::startDetached(trayPath, {});

    appendLog(
        started
            ? QStringLiteral(
                  "N.E.E.B.L.E.S. tray started."
              )
            : QStringLiteral(
                  "WARNING: could not start N.E.E.B.L.E.S. tray."
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
            QStringLiteral(
                "WARNING: Plasma launcher package not found."
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
            QStringLiteral(
                "WARNING: qdbus was not found; launcher was installed but could not be added to the panel."
            )
        );
        return;
    }

    const QString script =
        QStringLiteral(R"JS(
var found = false;
var ps = panels();

for (var i = 0; i < ps.length; ++i) {
    var ws = ps[i].widgets();

    for (var j = 0; j < ws.length; ++j) {
        if (ws[j].type == "org.neebles.launcher") {
            found = true;
            break;
        }
    }

    if (found)
        break;
}

if (!found && ps.length > 0) {
    var targetPanel = ps[0];

    for (var p = 0; p < ps.length; ++p) {
        var widgets = ps[p].widgets();

        for (var w = 0; w < widgets.length; ++w) {
            if (
                widgets[w].type == "org.kde.plasma.kickoff"
                || widgets[w].type == "org.kde.plasma.kicker"
            ) {
                targetPanel = ps[p];
                p = ps.length;
                break;
            }
        }
    }

    targetPanel.addWidget(
        "org.neebles.launcher"
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
            QStringLiteral(
                "WARNING: launcher package installed, but Plasma panel integration failed."
            )
        );
        return;
    }

    appendLog(
        QStringLiteral(
            "N.E.E.B.L.E.S. launcher added to Plasma."
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
