#include "traysocketclient.h"

#include <LayerShellQt/Window>

#include <QGuiApplication>
#include <QMargins>
#include <QQuickWindow>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QSize>
#include <QTimer>

int main(
    int argc,
    char *argv[]
)
{
    QGuiApplication app(
        argc,
        argv
    );

    QGuiApplication::setApplicationName(
        QStringLiteral(
            "N.E.E.B.L.E.S. Tray Host"
        )
    );

    QGuiApplication::setOrganizationName(
        QStringLiteral(
            "N.E.E.B.L.E.S."
        )
    );

    TraySocketClient trayClient;

    QQmlApplicationEngine engine;

    engine.rootContext()
        ->setContextProperty(
            QStringLiteral(
                "trayClient"
            ),
            &trayClient
        );

    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        [] {
            QCoreApplication::exit(-1);
        },
        Qt::QueuedConnection
    );

    engine.loadFromModule(
        QStringLiteral(
            "NEEBLES.TrayHost"
        ),
        QStringLiteral(
            "Main"
        )
    );

    const auto roots =
        engine.rootObjects();

    if (roots.isEmpty()) {
        return -1;
    }

    auto *window =
        qobject_cast<QQuickWindow *>(
            roots.first()
        );

    if (!window) {
        return -1;
    }

    auto *layerWindow =
        LayerShellQt::Window::get(
            window
        );

    if (!layerWindow) {
        return -1;
    }

    layerWindow->setScope(
        QStringLiteral(
            "neebles-tray-host"
        )
    );

    layerWindow->setLayer(
        LayerShellQt::Window::LayerOverlay
    );

    LayerShellQt::Window::Anchors anchors;

    anchors.setFlag(
        LayerShellQt::Window::AnchorBottom
    );

    anchors.setFlag(
        LayerShellQt::Window::AnchorRight
    );

    layerWindow->setAnchors(
        anchors
    );

    layerWindow->setMargins(
        QMargins(
            0,
            0,
            12,
            52
        )
    );

    layerWindow->setExclusiveZone(
        0
    );

    layerWindow->setKeyboardInteractivity(
        LayerShellQt::Window::
            KeyboardInteractivityOnDemand
    );

    auto syncDesiredSize = [
        window,
        layerWindow
    ] {
        layerWindow->setDesiredSize(
            QSize(
                window->width(),
                window->height()
            )
        );
    };

    syncDesiredSize();

    QObject::connect(
        window,
        &QQuickWindow::widthChanged,
        &app,
        syncDesiredSize
    );

    QObject::connect(
        window,
        &QQuickWindow::heightChanged,
        &app,
        syncDesiredSize
    );

    layerWindow->setCloseOnDismissed(
        false
    );

    QTimer::singleShot(
        0,
        &trayClient,
        &TraySocketClient::connectToManager
    );

    return app.exec();
}
