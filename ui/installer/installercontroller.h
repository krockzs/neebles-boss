#pragma once

#include <QObject>
#include <QProcess>
#include <QStringList>

class InstallerController final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(int progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)
    Q_PROPERTY(bool running READ running NOTIFY runningChanged)
    Q_PROPERTY(bool finished READ finished NOTIFY finishedChanged)
    Q_PROPERTY(bool success READ success NOTIFY finishedChanged)

public:
    explicit InstallerController(const QStringList &arguments, QObject *parent = nullptr);

    int progress() const { return m_progress; }
    QString status() const { return m_status; }
    bool running() const { return m_running; }
    bool finished() const { return m_finished; }
    bool success() const { return m_success; }

    Q_INVOKABLE void startInstallation();

signals:
    void progressChanged();
    void statusChanged();
    void runningChanged();
    void finishedChanged();
    void logLine(const QString &line);

private slots:
    void readOutput();
    void processFinished(int exitCode, QProcess::ExitStatus exitStatus);
    void processError(QProcess::ProcessError error);

private:
    void setProgress(int value);
    void setStatus(const QString &value);
    void appendLog(const QString &line);
    void integrateDesktop();
    void startTray();
    void installLauncherIntoPanel();
    void consumeLine(const QString &line);

    QString m_installScript;
    QString m_backendBinary;
    QString m_uiBinary;
    QString m_trayBinary;
    QString m_clientDataArchive;
    QProcess m_process;
    QByteArray m_buffer;
    int m_progress = 0;
    QString m_status = QStringLiteral("Waiting for administrator authorization...");
    bool m_running = false;
    bool m_finished = false;
    bool m_success = false;
};
