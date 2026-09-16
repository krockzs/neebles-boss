#include "traysocketclient.h"

#include <LayerShellQt/Window>

#include <QCoreApplication>
#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QLocale>
#include <QLockFile>
#include <QStandardPaths>
#include <QMargins>
#include <QQuickWindow>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QSize>
#include <QTimer>

#include <cerrno>
#include <cstring>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static QString normalizeLocale(QString value)
{
    value = value.trimmed();

    const qsizetype dot = value.indexOf(QLatin1Char('.'));
    if (dot >= 0)
        value.truncate(dot);

    const qsizetype modifier = value.indexOf(QLatin1Char('@'));
    if (modifier >= 0)
        value.truncate(modifier);

    value.replace(QLatin1Char('-'), QLatin1Char('_'));

    const QStringList parts =
        value.split(
            QLatin1Char('_'),
            Qt::SkipEmptyParts
        );

    if (parts.isEmpty())
        return QString();

    if (parts.size() == 1)
        return parts.at(0).toLower();

    return parts.at(0).toLower()
        + QLatin1Char('_')
        + parts.at(1).toUpper();
}

static QString bossConfigPath(
    QString *error
)
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CONFIG"
        )
    ) {
        const QString value =
            qEnvironmentVariable(
                "NEEBLES_CONFIG"
            ).trimmed();

        if (value.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "NEEBLES_CONFIG is explicitly set but empty"
                    );
            }

            return {};
        }

        return value;
    }

    if (
        qEnvironmentVariableIsSet(
            "XDG_CONFIG_HOME"
        )
    ) {
        const QString value =
            qEnvironmentVariable(
                "XDG_CONFIG_HOME"
            ).trimmed();

        if (value.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "XDG_CONFIG_HOME is explicitly set but empty"
                    );
            }

            return {};
        }

        return QDir(value).filePath(
            QStringLiteral(
                "neebles/boss.json"
            )
        );
    }

    if (
        qEnvironmentVariableIsSet(
            "HOME"
        )
    ) {
        const QString value =
            qEnvironmentVariable(
                "HOME"
            ).trimmed();

        if (value.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "HOME is explicitly set but empty"
                    );
            }

            return {};
        }

        return QDir(value).filePath(
            QStringLiteral(
                ".config/neebles/boss.json"
            )
        );
    }

    if (error) {
        *error =
            QStringLiteral(
                "could not determine Boss configuration path"
            );
    }

    return {};
}

static QString resolveClientRoot(
    QString *error
)
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CLIENT_ROOT"
        )
    ) {
        const QString root =
            qEnvironmentVariable(
                "NEEBLES_CLIENT_ROOT"
            ).trimmed();

        if (root.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "NEEBLES_CLIENT_ROOT is explicitly set but empty"
                    );
            }

            return {};
        }

        const QString manifest =
            QDir(root).filePath(
                QStringLiteral(
                    "languages/manifest.json"
                )
            );

        if (!QFileInfo::exists(manifest)) {
            if (error) {
                *error =
                    QStringLiteral(
                        "NEEBLES_CLIENT_ROOT does not contain languages/manifest.json: "
                    ) + root;
            }

            return {};
        }

        return root;
    }

    const QStringList candidates = {
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral("..")
        ),

        QStringLiteral(
            "/opt/neebles/client"
        )
    };

    for (const QString &root : candidates) {
        const QString manifest =
            QDir(root).filePath(
                QStringLiteral(
                    "languages/manifest.json"
                )
            );

        if (QFileInfo::exists(manifest))
            return root;
    }

    if (error) {
        *error =
            QStringLiteral(
                "could not locate N.E.E.B.L.E.S. client language manifest"
            );
    }

    return {};
}

static QJsonObject loadLanguageManifest(
    const QString &root,
    QString *error
)
{
    const QString path =
        QDir(root).filePath(
            QStringLiteral(
                "languages/manifest.json"
            )
        );

    QFile file(path);

    if (!file.open(QIODevice::ReadOnly)) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not read language manifest: "
                ) + path;
        }

        return {};
    }

    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            file.readAll(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid language manifest: "
                ) + path;
        }

        return {};
    }

    const QJsonObject manifest =
        document.object();

    if (
        manifest.value(
            QStringLiteral("schema")
        ).toInt(-1) != 1
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "unsupported language manifest schema: "
                ) + path;
        }

        return {};
    }

    const QJsonValue languagesValue =
        manifest.value(
            QStringLiteral("languages")
        );

    if (
        !languagesValue.isArray()
        || languagesValue.toArray().isEmpty()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "language manifest must contain a non-empty languages array: "
                ) + path;
        }

        return {};
    }

    return manifest;
}

static QString canonicalLanguage(
    const QJsonObject &manifest,
    const QString &requested
)
{
    const QString normalized =
        normalizeLocale(requested);

    if (normalized.isEmpty())
        return {};

    const QJsonArray languages =
        manifest.value(
            QStringLiteral("languages")
        ).toArray();

    for (const QJsonValue &value : languages) {
        const QString code =
            value.toObject()
                .value(
                    QStringLiteral("code")
                )
                .toString();

        if (
            normalizeLocale(code)
                == normalized
        ) {
            return code;
        }
    }

    return {};
}

static QString manifestDefaultLanguage(
    const QJsonObject &manifest,
    QString *error
)
{
    const QString declared =
        manifest.value(
            QStringLiteral("default")
        ).toString().trimmed();

    if (declared.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "language manifest does not declare a default language"
                );
        }

        return {};
    }

    const QString canonical =
        canonicalLanguage(
            manifest,
            declared
        );

    if (canonical.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "language manifest default is not declared in languages: "
                ) + declared;
        }

        return {};
    }

    return canonical;
}

static QString languageFromConfig(
    const QJsonObject &manifest,
    QString *error,
    bool *found
)
{
    if (found)
        *found = false;

    QString pathError;

    const QString path =
        bossConfigPath(
            &pathError
        );

    if (path.isEmpty()) {
        if (error)
            *error = pathError;

        return {};
    }

    const bool explicitConfig =
        qEnvironmentVariableIsSet(
            "NEEBLES_CONFIG"
        );

    QFile file(path);

    if (!file.exists()) {
        if (explicitConfig) {
            if (error) {
                *error =
                    QStringLiteral(
                        "explicit Boss config does not exist: "
                    ) + path;
            }

            return {};
        }

        return {};
    }

    if (!file.open(QIODevice::ReadOnly)) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not read Boss config: "
                ) + path;
        }

        return {};
    }

    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            file.readAll(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid Boss config: "
                ) + path;
        }

        return {};
    }

    const QJsonObject config =
        document.object();

    const QString requested =
        config.value(
            QStringLiteral("language")
        ).toString().trimmed();

    if (requested.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "Boss config does not declare language: "
                ) + path;
        }

        return {};
    }

    const QString canonical =
        canonicalLanguage(
            manifest,
            requested
        );

    if (canonical.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "Boss config declares unsupported language: "
                ) + requested;
        }

        return {};
    }

    if (found)
        *found = true;

    return canonical;
}

static QString bossSocketPath()
{
    const QString overridePath =
        qEnvironmentVariable(
            "NEEBLES_SOCKET"
        ).trimmed();

    if (!overridePath.isEmpty()) {
        return overridePath;
    }

    return QStringLiteral(
        "/run/neebles/neebles.sock"
    );
}

static QJsonObject bossRequest(
    const QString &target,
    const QString &action,
    const QJsonArray &args,
    const QString &caller,
    QString *error
)
{
    const QString socketPath =
        bossSocketPath();

    const QByteArray encodedPath =
        QFile::encodeName(
            socketPath
        );

    if (
        encodedPath.isEmpty()
        || encodedPath.size()
            >= static_cast<qsizetype>(
                sizeof(sockaddr_un::sun_path)
            )
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid N.E.E.B.L.E.S. Boss socket path: "
                ) + socketPath;
        }

        return {};
    }

    const int socketFd =
        ::socket(
            AF_UNIX,
            SOCK_STREAM,
            0
        );

    if (socketFd < 0) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not create Boss IPC socket: "
                )
                + QString::fromLocal8Bit(
                    std::strerror(errno)
                );
        }

        return {};
    }

    const auto closeSocket =
        [&socketFd]() {
            ::close(socketFd);
        };

    sockaddr_un address{};
    address.sun_family = AF_UNIX;

    std::memcpy(
        address.sun_path,
        encodedPath.constData(),
        static_cast<std::size_t>(
            encodedPath.size()
        )
    );

    address.sun_path[
        encodedPath.size()
    ] = '\0';

    if (
        ::connect(
            socketFd,
            reinterpret_cast<sockaddr *>(
                &address
            ),
            sizeof(address)
        ) != 0
    ) {
        const QString message =
            QStringLiteral(
                "could not connect to N.E.E.B.L.E.S. Boss socket "
            )
            + socketPath
            + QStringLiteral(": ")
            + QString::fromLocal8Bit(
                std::strerror(errno)
            );

        closeSocket();

        if (error)
            *error = message;

        return {};
    }

    const QJsonObject request{
        {
            QStringLiteral("target"),
            target
        },
        {
            QStringLiteral("action"),
            action
        },
        {
            QStringLiteral("args"),
            args
        },
        {
            QStringLiteral("context"),
            QJsonObject{
                {
                    QStringLiteral("caller"),
                    caller
                }
            }
        }
    };

    const QByteArray payload =
        QJsonDocument(request)
            .toJson(
                QJsonDocument::Compact
            );

    qsizetype written = 0;

    while (written < payload.size()) {
        const ssize_t result =
            ::write(
                socketFd,
                payload.constData()
                    + written,
                static_cast<std::size_t>(
                    payload.size()
                        - written
                )
            );

        if (result < 0) {
            if (errno == EINTR)
                continue;

            const QString message =
                QStringLiteral(
                    "could not write Boss IPC request: "
                )
                + QString::fromLocal8Bit(
                    std::strerror(errno)
                );

            closeSocket();

            if (error)
                *error = message;

            return {};
        }

        written += result;
    }

    /*
     * neebles.sock uses EOF as the request framing
     * boundary. Preserve the read side so Boss can
     * return ExecutionResponse on the same connection.
     */
    if (
        ::shutdown(
            socketFd,
            SHUT_WR
        ) != 0
    ) {
        const QString message =
            QStringLiteral(
                "could not finish Boss IPC request: "
            )
            + QString::fromLocal8Bit(
                std::strerror(errno)
            );

        closeSocket();

        if (error)
            *error = message;

        return {};
    }

    QByteArray raw;
    char buffer[4096];

    while (true) {
        const ssize_t count =
            ::read(
                socketFd,
                buffer,
                sizeof(buffer)
            );

        if (count == 0)
            break;

        if (count < 0) {
            if (errno == EINTR)
                continue;

            const QString message =
                QStringLiteral(
                    "could not read Boss IPC response: "
                )
                + QString::fromLocal8Bit(
                    std::strerror(errno)
                );

            closeSocket();

            if (error)
                *error = message;

            return {};
        }

        raw.append(
            buffer,
            static_cast<qsizetype>(
                count
            )
        );
    }

    closeSocket();

    QJsonParseError parseError;

    const QJsonDocument responseDocument =
        QJsonDocument::fromJson(
            raw.trimmed(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !responseDocument.isObject()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid ExecutionResponse from N.E.E.B.L.E.S. Boss: "
                )
                + parseError.errorString();
        }

        return {};
    }

    const QJsonObject response =
        responseDocument.object();

    if (
        !response.value(
            QStringLiteral("ok")
        ).toBool()
    ) {
        const QString message =
            response.value(
                QStringLiteral("error")
            )
            .toObject()
            .value(
                QStringLiteral("message")
            )
            .toString();

        if (error) {
            *error =
                message.isEmpty()
                ? QStringLiteral(
                    "N.E.E.B.L.E.S. Boss request failed"
                )
                : message;
        }

        return {};
    }

    return response;
}

static QString activeBossLanguage(
    const QJsonObject &manifest,
    QString *error
)
{
    QString requestError;

    const QJsonObject response =
        bossRequest(
            QStringLiteral("settings"),
            QStringLiteral("get"),
            QJsonArray{
                QStringLiteral("boss"),
                QStringLiteral("ui.language")
            },
            QStringLiteral("tray-host"),
            &requestError
        );

    if (response.isEmpty()) {
        if (error)
            *error = requestError;

        return {};
    }

    const QJsonValue result =
        response.value(
            QStringLiteral("result")
        );

    if (!result.isString()) {
        if (error) {
            *error =
                QStringLiteral(
                    "Boss ui.language result is not a String"
                );
        }

        return {};
    }

    const QString requested =
        result.toString();

    const QString canonical =
        canonicalLanguage(
            manifest,
            requested
        );

    if (canonical.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "Boss returned unsupported ui.language: "
                ) + requested;
        }

        return {};
    }

    return canonical;
}

static QVariantMap loadBossStrings(
    QString *error
)
{
    const QString root =
        resolveClientRoot(
            error
        );

    if (root.isEmpty())
        return {};

    const QJsonObject manifest =
        loadLanguageManifest(
            root,
            error
        );

    if (manifest.isEmpty())
        return {};

    const QString language =
        activeBossLanguage(
            manifest,
            error
        );

    if (language.isEmpty())
        return {};

    QString fileName;

    const QJsonArray languages =
        manifest.value(
            QStringLiteral("languages")
        ).toArray();

    for (const QJsonValue &value : languages) {
        const QJsonObject entry =
            value.toObject();

        if (
            entry.value(
                QStringLiteral("code")
            ).toString()
            == language
        ) {
            fileName =
                entry.value(
                    QStringLiteral("file")
                ).toString().trimmed();

            break;
        }
    }

    if (fileName.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "language entry does not declare a file: "
                ) + language;
        }

        return {};
    }

    const QFileInfo languageFileInfo(
        fileName
    );

    if (
        languageFileInfo.isAbsolute()
        || fileName == QStringLiteral("..")
        || fileName.startsWith(
            QStringLiteral("../")
        )
        || fileName.contains(
            QStringLiteral("/../")
        )
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "language entry declares an unsafe file path: "
                ) + fileName;
        }

        return {};
    }

    const QString path =
        QDir(root).filePath(
            QStringLiteral("languages/")
            + fileName
        );

    QFile file(path);

    if (!file.open(QIODevice::ReadOnly)) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not read language file: "
                ) + path;
        }

        return {};
    }

    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            file.readAll(),
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        if (error) {
            *error =
                QStringLiteral(
                    "invalid language file: "
                ) + path;
        }

        return {};
    }

    return document.object().toVariantMap();
}

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

    const QString runtimeDir =
        QStandardPaths::writableLocation(
            QStandardPaths::RuntimeLocation
        );

    if (runtimeDir.isEmpty()) {
        qCritical()
            << "N.E.E.B.L.E.S. Tray Host:"
            << "runtime directory is unavailable";
        return 1;
    }

    QLockFile instanceLock(
        QDir(runtimeDir).filePath(
            QStringLiteral(
                "neebles-tray-host.lock"
            )
        )
    );

    instanceLock.setStaleLockTime(0);

    if (!instanceLock.tryLock()) {
        switch (instanceLock.error()) {
        case QLockFile::LockFailedError:
            qInfo()
                << "N.E.E.B.L.E.S. Tray Host:"
                << "another instance is already running";
            return 0;

        case QLockFile::PermissionError:
            qCritical()
                << "N.E.E.B.L.E.S. Tray Host:"
                << "could not acquire instance lock: permission denied";
            return 1;

        case QLockFile::UnknownError:
        default:
            qCritical()
                << "N.E.E.B.L.E.S. Tray Host:"
                << "could not acquire instance lock: unknown error";
            return 1;
        }
    }

    TraySocketClient trayClient;

    QString languageError;

    const QVariantMap bossStrings =
        loadBossStrings(
            &languageError
        );

    if (
        !languageError.isEmpty()
        || bossStrings.isEmpty()
    ) {
        const QString message =
            languageError.isEmpty()
            ? QStringLiteral(
                "N.E.E.B.L.E.S. Tray Host language contract failed"
            )
            : languageError;

        qCritical().noquote()
            << message;

        return 2;
    }

    QQmlApplicationEngine engine;

    engine.rootContext()
        ->setContextProperty(
            QStringLiteral(
                "trayClient"
            ),
            &trayClient
        );

    engine.rootContext()
        ->setContextProperty(
            QStringLiteral(
                "bossStrings"
            ),
            bossStrings
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

    layerWindow->setCloseOnDismissed(
        false
    );

    window->show();

    QTimer::singleShot(
        0,
        &trayClient,
        &TraySocketClient::connectToManager
    );

    return app.exec();
}
