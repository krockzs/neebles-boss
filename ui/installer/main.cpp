#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>

#include "installercontroller.h"

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("N.E.E.B.L.E.S. Installer"));
    app.setOrganizationName(QStringLiteral("N.E.E.B.L.E.S."));

    InstallerController installer(app.arguments());

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("installer"), &installer);
    engine.loadFromModule("NeeblesInstaller", "Main");

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
