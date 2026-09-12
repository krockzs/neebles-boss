#include "authagent.h"

#include <QDialogButtonBox>
#include <QFont>
#include <QFormLayout>
#include <QGraphicsDropShadowEffect>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QVBoxLayout>


static QString valueOrDash(
    const QString &value
)
{
    return value.isEmpty()
        ? QStringLiteral("—")
        : value;
}


NeeblesAuthDialog::NeeblesAuthDialog(
    const NeeblesAuthContext &context,
    const QHash<QString, QString> &strings,
    QWidget *parent
)
    : QDialog(parent),
      m_context(context),
      m_strings(strings)
{
    setWindowTitle(
        text(QStringLiteral("auth.title"))
    );

    setModal(true);

    setMinimumWidth(560);
    setMaximumWidth(680);

    setStyleSheet(
        QStringLiteral(R"CSS(
            QDialog {
                background-color: #09090D;
                color: #F4F4F5;
            }

            QLabel {
                color: #E4E4E7;
                background: transparent;
            }

            QLabel#title {
                color: #C084FC;
                font-size: 22px;
                font-weight: 700;
            }

            QLabel#operation {
                color: #22D3EE;
                font-size: 17px;
                font-weight: 700;
            }

            QLabel#detailKey {
                color: #A1A1AA;
                font-weight: 600;
            }

            QLabel#detailValue {
                color: #67E8F9;
                font-weight: 600;
            }

            QLabel#message {
                color: #E4E4E7;
                font-size: 14px;
            }

            QLabel#feedback {
                color: #FB7185;
                font-weight: 600;
            }

            QLineEdit {
                background-color: #0F0F16;
                color: #FFFFFF;

                border: 2px solid #FF3344;
                border-radius: 8px;

                padding: 10px 12px;

                selection-background-color: #7C3AED;
                selection-color: #FFFFFF;
            }

            QLineEdit:focus {
                border: 3px solid #FF1744;
                background-color: #13131D;
            }

            QPushButton {
                min-width: 110px;
                min-height: 36px;

                border-radius: 8px;
                border: 2px solid #7C3AED;

                background-color: #181020;
                color: #E9D5FF;

                font-weight: 700;
                padding: 4px 14px;
            }

            QPushButton:hover {
                border-color: #22D3EE;
                background-color: #251044;
                color: #CFFAFE;
            }

            QPushButton:pressed {
                border-color: #67E8F9;
                background-color: #32155A;
                color: #FFFFFF;
            }

            QPushButton#cancel {
                border-color: #52525B;
                background-color: #121217;
                color: #A1A1AA;
            }

            QPushButton#cancel:hover {
                border-color: #71717A;
                color: #E4E4E7;
            }

            QPushButton#authorize {
                border-color: #A855F7;
                background-color: #241036;
                color: #F3E8FF;
            }

            QPushButton#authorize:hover {
                border-color: #22D3EE;
                background-color: #351454;
                color: #CFFAFE;
            }

            QPushButton#authorize:pressed {
                border-color: #67E8F9;
                background-color: #4C1D70;
                color: #FFFFFF;
            }
        )CSS")
    );

    auto *root =
        new QVBoxLayout(this);

    root->setContentsMargins(
        28,
        26,
        28,
        24
    );

    root->setSpacing(16);

    auto *title =
        new QLabel(
            text(
                QStringLiteral(
                    "auth.title"
                )
            ),
            this
        );

    title->setObjectName(
        QStringLiteral("title")
    );

    root->addWidget(title);

    auto *operation =
        new QLabel(
            operationTitle(),
            this
        );

    operation->setObjectName(
        QStringLiteral("operation")
    );

    root->addWidget(operation);

    auto *form =
        new QFormLayout();

    form->setLabelAlignment(
        Qt::AlignLeft
    );

    form->setHorizontalSpacing(20);
    form->setVerticalSpacing(8);

    auto addDetail =
        [&](const QString &key,
            const QString &value) {

            auto *label =
                new QLabel(key, this);

            label->setObjectName(
                QStringLiteral(
                    "detailKey"
                )
            );

            auto *detail =
                new QLabel(value, this);

            detail->setObjectName(
                QStringLiteral(
                    "detailValue"
                )
            );

            form->addRow(
                label,
                detail
            );
        };

    addDetail(
        text(
            QStringLiteral(
                "auth.operation"
            )
        ),
        operationTitle()
    );

    if (!m_context.name.isEmpty()) {
        addDetail(
            text(
                QStringLiteral(
                    "auth.module"
                )
            ),
            m_context.name
        );
    }

    if (
        !m_context.fromVersion.isEmpty()
        || !m_context.toVersion.isEmpty()
    ) {
        addDetail(
            text(
                QStringLiteral(
                    "auth.version"
                )
            ),
            QStringLiteral("%1 → %2")
                .arg(
                    valueOrDash(
                        m_context.fromVersion
                    ),
                    valueOrDash(
                        m_context.toVersion
                    )
                )
        );
    }

    addDetail(
        text(
            QStringLiteral(
                "auth.status"
            )
        ),
        m_context.running
            ? text(
                QStringLiteral(
                    "auth.status.running"
                )
            )
            : text(
                QStringLiteral(
                    "auth.status.closed"
                )
            )
    );

    root->addLayout(form);

    auto *message =
        new QLabel(
            operationMessage(),
            this
        );

    message->setObjectName(
        QStringLiteral("message")
    );

    message->setWordWrap(true);

    root->addWidget(message);

    auto *passwordLabel =
        new QLabel(
            text(
                QStringLiteral(
                    "auth.password"
                )
            ),
            this
        );

    passwordLabel->setObjectName(
        QStringLiteral(
            "detailKey"
        )
    );

    root->addWidget(passwordLabel);

    m_password =
        new QLineEdit(this);

    m_password->setEchoMode(
        QLineEdit::Password
    );

    m_password->setClearButtonEnabled(
        false
    );

    auto *glow =
        new QGraphicsDropShadowEffect(
            m_password
        );

    glow->setBlurRadius(24);
    glow->setOffset(0, 0);
    glow->setColor(
        QColor(
            255,
            23,
            68,
            190
        )
    );

    m_password->setGraphicsEffect(glow);

    root->addWidget(m_password);

    m_feedback =
        new QLabel(this);

    m_feedback->setObjectName(
        QStringLiteral(
            "feedback"
        )
    );

    m_feedback->setWordWrap(true);
    m_feedback->hide();

    root->addWidget(m_feedback);

    auto *buttons =
        new QDialogButtonBox(this);

    auto *cancel =
        buttons->addButton(
            text(
                QStringLiteral(
                    "auth.cancel"
                )
            ),
            QDialogButtonBox::RejectRole
        );

    cancel->setObjectName(
        QStringLiteral("cancel")
    );

    auto *authorize =
        buttons->addButton(
            text(
                QStringLiteral(
                    "auth.authorize"
                )
            ),
            QDialogButtonBox::AcceptRole
        );

    authorize->setObjectName(
        QStringLiteral(
            "authorize"
        )
    );

    connect(
        buttons,
        &QDialogButtonBox::accepted,
        this,
        &QDialog::accept
    );

    connect(
        buttons,
        &QDialogButtonBox::rejected,
        this,
        &QDialog::reject
    );

    connect(
        m_password,
        &QLineEdit::returnPressed,
        this,
        &QDialog::accept
    );

    root->addWidget(buttons);

    m_password->setFocus();
}


QString NeeblesAuthDialog::text(
    const QString &key
) const
{
    return m_strings.value(
        key,
        key
    );
}


QString
NeeblesAuthDialog::operationTitle() const
{
    const QString operation =
        m_context.operation;

    if (
        operation
        == QStringLiteral(
            "install-module"
        )
    )
        return text(
            QStringLiteral(
                "auth.install_module"
            )
        );

    if (
        operation
        == QStringLiteral(
            "update-module"
        )
    )
        return text(
            QStringLiteral(
                "auth.update_module"
            )
        );

    if (
        operation
        == QStringLiteral(
            "uninstall-module"
        )
    )
        return text(
            QStringLiteral(
                "auth.uninstall_module"
            )
        );

    if (
        operation
        == QStringLiteral(
            "install-boss"
        )
    )
        return text(
            QStringLiteral(
                "auth.install_boss"
            )
        );

    if (
        operation
        == QStringLiteral(
            "update-boss"
        )
    )
        return text(
            QStringLiteral(
                "auth.update_boss"
            )
        );

    return text(
        QStringLiteral(
            "auth.title"
        )
    );
}


QString
NeeblesAuthDialog::operationMessage() const
{
    const QString operation =
        m_context.operation;

    if (
        operation
        == QStringLiteral(
            "install-module"
        )
    ) {
        return text(
            QStringLiteral(
                "auth.install_module_message"
            )
        )
            .arg(
                m_context.name,
                valueOrDash(
                    m_context.toVersion
                )
            );
    }

    if (
        operation
        == QStringLiteral(
            "update-module"
        )
    ) {
        if (m_context.running) {
            return text(
                QStringLiteral(
                    "auth.update_module_running_message"
                )
            )
                .arg(
                    m_context.name,
                    valueOrDash(
                        m_context.fromVersion
                    ),
                    valueOrDash(
                        m_context.toVersion
                    )
                );
        }

        return text(
            QStringLiteral(
                "auth.update_module_message"
            )
        )
            .arg(
                m_context.name,
                valueOrDash(
                    m_context.fromVersion
                ),
                valueOrDash(
                    m_context.toVersion
                )
            );
    }

    if (
        operation
        == QStringLiteral(
            "uninstall-module"
        )
    ) {
        return text(
            QStringLiteral(
                "auth.uninstall_module_message"
            )
        ).arg(
            m_context.name
        );
    }

    if (
        operation
        == QStringLiteral(
            "install-boss"
        )
    ) {
        return text(
            QStringLiteral(
                "auth.install_boss_message"
            )
        );
    }

    if (
        operation
        == QStringLiteral(
            "update-boss"
        )
    ) {
        return text(
            QStringLiteral(
                "auth.update_boss_message"
            )
        );
    }

    return text(
        QStringLiteral(
            "auth.generic_message"
        )
    );
}


QString
NeeblesAuthDialog::response() const
{
    return m_password->text();
}


void NeeblesAuthDialog::setRequest(
    const QString &request,
    bool echo
)
{
    Q_UNUSED(request)

    m_password->clear();

    m_password->setEchoMode(
        echo
            ? QLineEdit::Normal
            : QLineEdit::Password
    );

    m_feedback->clear();
    m_feedback->hide();

    m_password->setFocus();
}


void
NeeblesAuthDialog::showAuthenticationError(
    const QString &message
)
{
    m_feedback->setText(
        message.isEmpty()
            ? text(
                QStringLiteral(
                    "auth.incorrect_password"
                )
            )
            : message
    );

    m_feedback->show();

    m_password->clear();
    m_password->setFocus();
}


void
NeeblesAuthDialog::showAuthenticationInfo(
    const QString &message
)
{
    if (message.isEmpty())
        return;

    m_feedback->setText(message);
    m_feedback->show();
}


NeeblesAuthListener::NeeblesAuthListener(
    const NeeblesAuthContext &context,
    const QHash<QString, QString> &strings,
    QObject *parent
)
    : PolkitQt1::Agent::Listener(parent),
      m_context(context),
      m_strings(strings)
{
}


void
NeeblesAuthListener::initiateAuthentication(
    const QString &actionId,
    const QString &message,
    const QString &iconName,
    const PolkitQt1::Details &details,
    const QString &cookie,
    const PolkitQt1::Identity::List &identities,
    PolkitQt1::Agent::AsyncResult *result
)
{
    Q_UNUSED(actionId)
    Q_UNUSED(message)
    Q_UNUSED(iconName)
    Q_UNUSED(details)

    m_lastAuthorization = false;
    m_cancelled = false;

    if (identities.isEmpty()) {
        result->setCompleted();
        return;
    }

    if (m_session) {
        m_session->cancel();
        m_session->deleteLater();
        m_session = nullptr;
    }

    m_session =
        new PolkitQt1::Agent::Session(
            identities.first(),
            cookie,
            result,
            this
        );

    connect(
        m_session,
        &PolkitQt1::Agent::Session::request,
        this,
        [this](
            const QString &request,
            bool echo
        ) {
            if (m_dialog) {
                m_dialog->close();
                m_dialog->deleteLater();
            }

            m_dialog =
                new NeeblesAuthDialog(
                    m_context,
                    m_strings
                );

            m_dialog->setRequest(
                request,
                echo
            );

            const int response =
                m_dialog->exec();

            if (!m_session)
                return;

            if (
                response
                == QDialog::Accepted
            ) {
                m_session->setResponse(
                    m_dialog->response()
                );
            } else {
                m_cancelled = true;
                m_session->cancel();
            }
        }
    );

    connect(
        m_session,
        &PolkitQt1::Agent::Session::showError,
        this,
        [this](const QString &text) {
            if (m_dialog) {
                m_dialog
                    ->showAuthenticationError(
                        text
                    );
            }
        }
    );

    connect(
        m_session,
        &PolkitQt1::Agent::Session::showInfo,
        this,
        [this](const QString &text) {
            if (m_dialog) {
                m_dialog
                    ->showAuthenticationInfo(
                        text
                    );
            }
        }
    );

    connect(
        m_session,
        &PolkitQt1::Agent::Session::completed,
        this,
        [this](bool gainedAuthorization) {
            m_lastAuthorization =
                gainedAuthorization;

            if (m_session) {
                if (m_session->result())
                    m_session
                        ->result()
                        ->setCompleted();

                m_session->deleteLater();
                m_session = nullptr;
            }

            if (m_dialog) {
                m_dialog->close();
                m_dialog->deleteLater();
                m_dialog = nullptr;
            }
        }
    );

    m_session->initiate();
}


bool
NeeblesAuthListener::initiateAuthenticationFinish()
{
    return m_lastAuthorization;
}


void
NeeblesAuthListener::cancelAuthentication()
{
    m_cancelled = true;
    m_lastAuthorization = false;

    if (m_session)
        m_session->cancel();

    if (m_dialog)
        m_dialog->reject();
}
