#pragma once

#include <QObject>
#include <QSet>
#include <QVariantList>
#include <QVariantMap>
#include <QUrl>

class QTimer;

class BossController final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QString language READ language NOTIFY configChanged)
    Q_PROPERTY(bool trayEnabled READ trayEnabled NOTIFY configChanged)
    Q_PROPERTY(bool launcherEnabled READ launcherEnabled NOTIFY configChanged)
    Q_PROPERTY(bool normalNotifications READ normalNotifications NOTIFY configChanged)
    Q_PROPERTY(QVariantList languages READ languages NOTIFY languagesChanged)
    Q_PROPERTY(QVariantList modules READ modules NOTIFY modulesChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString statusText READ statusText NOTIFY statusTextChanged)
    Q_PROPERTY(int translationsRevision READ translationsRevision NOTIFY translationsChanged)

public:
    explicit BossController(QObject *parent = nullptr);

    QString language() const { return m_language; }
    bool trayEnabled() const { return m_trayEnabled; }
    bool launcherEnabled() const { return m_launcherEnabled; }
    bool normalNotifications() const { return m_normalNotifications; }
    QVariantList languages() const { return m_languages; }
    QVariantList modules() const { return m_modules; }
    bool busy() const { return m_busy; }
    QString statusText() const { return m_statusText; }
    int translationsRevision() const { return m_translationsRevision; }

    Q_INVOKABLE QString text(const QString &key) const;
    Q_INVOKABLE QUrl assetUrl(const QString &name) const;
    Q_INVOKABLE QUrl flagUrl(const QString &name) const;
    Q_INVOKABLE void reload();
    Q_INVOKABLE void saveConfig(const QString &language,
                                bool trayEnabled,
                                bool launcherEnabled,
                                bool normalNotifications);
    Q_INVOKABLE void installModule(const QString &name);
    Q_INVOKABLE void updateModule(const QString &name);
    Q_INVOKABLE void uninstallModule(const QString &name);
    Q_INVOKABLE void openModule(const QString &name);
    Q_INVOKABLE void setModuleEnabled(const QString &name, bool enabled);

signals:
    void configChanged();
    void languagesChanged();
    void modulesChanged();
    void busyChanged();
    void statusTextChanged();
    void translationsChanged();

private:
    QString commandPath() const;
    QString authorizationPath() const;
    QByteArray run(const QStringList &arguments, bool privileged, int timeoutMs, bool *ok = nullptr);
    QVariant parseJson(const QByteArray &data) const;
    void loadConfig();
    void loadLanguages();
    void loadTranslations();
    void loadModules();
    void pollModuleRuntime();
    void pollModuleUpdates();
    void applyModuleLifecycle();
    void runModuleOperation(const QString &operation, const QString &name, bool privileged);
    void setBusy(bool value);
    void setStatusText(const QString &value);

    QString m_language = QStringLiteral("en_US");
    bool m_trayEnabled = true;
    bool m_launcherEnabled = true;
    bool m_normalNotifications = true;
    QVariantList m_languages;
    QVariantList m_modules;
    QVariantMap m_strings;
    QVariantMap m_updateNotifications;
    QTimer *m_modulePollTimer = nullptr;
    QTimer *m_updatePollTimer = nullptr;
    int m_translationsRevision = 0;
    bool m_busy = false;
    QString m_statusText;
};
