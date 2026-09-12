#include <QAction>
#include <QApplication>
#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMenu>
#include <QProcess>
#include <QSystemTrayIcon>
#include <QTimer>
#include <QVariantMap>

static QString neeblesCommand()
{
    const QString override = qEnvironmentVariable("NEEBLES_COMMAND");
    if (!override.isEmpty())
        return override;
    if (QFileInfo::exists(QStringLiteral("/usr/local/bin/neebles")))
        return QStringLiteral("/usr/local/bin/neebles");
    return QStringLiteral("neebles");
}

static QByteArray run(const QStringList &arguments)
{
    QProcess process;
    process.start(neeblesCommand(), arguments);
    if (!process.waitForStarted(3000) || !process.waitForFinished(7000))
        return {};
    if (process.exitCode() != 0)
        return {};
    return process.readAllStandardOutput();
}

static QJsonObject config()
{
    return QJsonDocument::fromJson(run({QStringLiteral("config"), QStringLiteral("show")})).object();
}

static QJsonObject strings()
{
    return QJsonDocument::fromJson(run({QStringLiteral("i18n"), QStringLiteral("dump")})).object();
}

static QString trKey(const QJsonObject &strings, const QString &key, const QString &fallback)
{
    return strings.value(key).toString(fallback);
}

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);
    app.setApplicationName(QStringLiteral("N.E.E.B.L.E.S. Tray"));

    QSystemTrayIcon tray;

    const QString clientRoot =
        qEnvironmentVariable("NEEBLES_CLIENT_ROOT");

    const QStringList iconCandidates = {
        clientRoot.isEmpty()
            ? QString()
            : clientRoot
                + QStringLiteral(
                    "/assets/branding/neebles-boss-launcher-icon.png"
                ),

        QCoreApplication::applicationDirPath()
            + QStringLiteral(
                "/../../assets/branding/neebles-boss-launcher-icon.png"
            ),

        QDir::currentPath()
            + QStringLiteral(
                "/client/assets/branding/neebles-boss-launcher-icon.png"
            ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/neebles-boss-launcher-icon.png"
        )
    };

    QIcon trayIcon;

    for (const QString &path : iconCandidates) {
        if (!path.isEmpty() && QFileInfo::exists(path)) {
            trayIcon = QIcon(path);
            break;
        }
    }

    if (trayIcon.isNull())
        trayIcon = QIcon::fromTheme(
            QStringLiteral("applications-system")
        );

    tray.setIcon(trayIcon);
    tray.setToolTip(QStringLiteral("N.E.E.B.L.E.S."));

    QMenu menu;
    tray.setContextMenu(&menu);

    QObject::connect(&menu, &QMenu::aboutToShow, [&]() {
        menu.clear();
        const QJsonObject text = strings();

        QAction *bossAction = menu.addAction(trKey(text, QStringLiteral("tray.open_boss"), QStringLiteral("Open N.E.E.B.L.E.S. Boss")));
        QObject::connect(bossAction, &QAction::triggered, []() {
            QProcess::startDetached(neeblesCommand(), {QStringLiteral("start")});
        });
        menu.addSeparator();

        const QJsonArray modules = QJsonDocument::fromJson(
            run({QStringLiteral("modules"), QStringLiteral("installed")})).array();

        if (modules.isEmpty()) {
            QAction *empty = menu.addAction(trKey(text, QStringLiteral("modules.empty"), QStringLiteral("No modules are installed.")));
            empty->setEnabled(false);
        }

        for (const QJsonValue &value : modules) {
            const QJsonObject module = value.toObject();
            const QString name = module.value(QStringLiteral("name")).toString();
            const bool enabled = module.value(QStringLiteral("enabled")).toBool(true);
            QMenu *moduleMenu = menu.addMenu(name);

            QAction *open = moduleMenu->addAction(trKey(text, QStringLiteral("common.open"), QStringLiteral("Open")));
            open->setEnabled(enabled);
            QObject::connect(open, &QAction::triggered, [name]() {
                QProcess::startDetached(neeblesCommand(), {name, QStringLiteral("open")});
            });

            QAction *toggle = moduleMenu->addAction(enabled
                ? trKey(text, QStringLiteral("common.disable"), QStringLiteral("Disable"))
                : trKey(text, QStringLiteral("common.enable"), QStringLiteral("Enable")));
            QObject::connect(toggle, &QAction::triggered, [name, enabled]() {
                QProcess::startDetached(neeblesCommand(), {
                    QStringLiteral("modules"),
                    enabled ? QStringLiteral("disable") : QStringLiteral("enable"),
                    name
                });
            });
        }

        menu.addSeparator();
        QAction *quit = menu.addAction(trKey(text, QStringLiteral("tray.quit"), QStringLiteral("Quit tray")));
        QObject::connect(quit, &QAction::triggered, &app, &QApplication::quit);
    });

    QTimer visibilityTimer;
    visibilityTimer.setInterval(2000);
    QObject::connect(&visibilityTimer, &QTimer::timeout, [&]() {
        const bool enabled = config().value(QStringLiteral("tray_enabled")).toBool(true);
        if (enabled && !tray.isVisible())
            tray.show();
        else if (!enabled && tray.isVisible())
            tray.hide();
    });
    visibilityTimer.start();

    if (config().value(QStringLiteral("tray_enabled")).toBool(true))
        tray.show();

    return app.exec();
}
