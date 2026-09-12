#pragma once

#include <QDialog>
#include <QHash>
#include <QString>

#include <PolkitQt1/Agent/Listener>
#include <PolkitQt1/Agent/Session>
#include <PolkitQt1/Details>
#include <PolkitQt1/Identity>

class QLabel;
class QLineEdit;

struct NeeblesAuthContext
{
    QString locale;
    QString operation;
    QString name;
    QString fromVersion;
    QString toVersion;
    bool running = false;
};

class NeeblesAuthDialog final : public QDialog
{
    Q_OBJECT

public:
    explicit NeeblesAuthDialog(
        const NeeblesAuthContext &context,
        const QHash<QString, QString> &strings,
        QWidget *parent = nullptr
    );

    QString response() const;

    void setRequest(
        const QString &request,
        bool echo
    );

    void showAuthenticationError(
        const QString &message
    );

    void showAuthenticationInfo(
        const QString &message
    );

private:
    QString text(const QString &key) const;
    QString operationTitle() const;
    QString operationMessage() const;

    NeeblesAuthContext m_context;
    QHash<QString, QString> m_strings;

    QLabel *m_feedback = nullptr;
    QLineEdit *m_password = nullptr;
};


class NeeblesAuthListener final
    : public PolkitQt1::Agent::Listener
{
    Q_OBJECT

public:
    explicit NeeblesAuthListener(
        const NeeblesAuthContext &context,
        const QHash<QString, QString> &strings,
        QObject *parent = nullptr
    );

public slots:
    void initiateAuthentication(
        const QString &actionId,
        const QString &message,
        const QString &iconName,
        const PolkitQt1::Details &details,
        const QString &cookie,
        const PolkitQt1::Identity::List &identities,
        PolkitQt1::Agent::AsyncResult *result
    ) override;

    bool initiateAuthenticationFinish() override;

    void cancelAuthentication() override;

private:
    NeeblesAuthContext m_context;
    QHash<QString, QString> m_strings;

    PolkitQt1::Agent::Session *m_session = nullptr;
    NeeblesAuthDialog *m_dialog = nullptr;

    bool m_lastAuthorization = false;
    bool m_cancelled = false;
};
