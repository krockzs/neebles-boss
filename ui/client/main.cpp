#include <QGuiApplication>
#include <QQmlApplicationEngine>

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("N.E.E.B.L.E.S."));
    app.setOrganizationName(QStringLiteral("N.E.E.B.L.E.S."));

    QQmlApplicationEngine engine;
    engine.loadFromModule("NeeblesUI", "Main");

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
