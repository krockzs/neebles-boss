#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QFile>
#include <QDebug>
#include <QGuiApplication>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QProcess>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QTemporaryDir>
#include <QUrl>

#include "installercontroller.h"

static QString findBrandingAsset(
    const QString &name,
    const QString &payloadRoot = QString()
)
{
    /*
     * An extracted explicit payload is authoritative.
     * Never borrow branding from another installation/root.
     */
    if (!payloadRoot.isEmpty()) {
        const QString path =
            QDir(payloadRoot).filePath(
                QStringLiteral(
                    "assets/branding/"
                ) + name
            );

        if (QFileInfo::exists(path)) {
            return QFileInfo(path)
                .absoluteFilePath();
        }

        return {};
    }

    /*
     * An explicit client root is also authoritative.
     * Empty/invalid roots are handled by the strict
     * client-root/language contract later in startup.
     */
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CLIENT_ROOT"
        )
    ) {
        const QString clientRoot =
            qEnvironmentVariable(
                "NEEBLES_CLIENT_ROOT"
            ).trimmed();

        if (clientRoot.isEmpty())
            return {};

        const QString path =
            QDir(clientRoot).filePath(
                QStringLiteral(
                    "assets/branding/"
                ) + name
            );

        if (QFileInfo::exists(path)) {
            return QFileInfo(path)
                .absoluteFilePath();
        }

        return {};
    }

    /*
     * No authoritative root was supplied:
     * development/installed discovery is allowed.
     */
    const QStringList candidates = {
        QDir::current().filePath(
            QStringLiteral(
                "client/assets/branding/"
            ) + name
        ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../../client/assets/branding/"
            ) + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/"
        ) + name
    };

    for (const QString &candidate : candidates) {
        if (QFileInfo::exists(candidate)) {
            return QFileInfo(candidate)
                .absoluteFilePath();
        }
    }

    return {};
}

static QString extractClientData(
    const QStringList &arguments,
    QTemporaryDir &temporary,
    QString *error
)
{
    /*
     * No explicit payload argument:
     * this is allowed for development/local execution.
     */
    if (arguments.size() < 5)
        return {};

    /*
     * Once a payload was explicitly supplied it is authoritative.
     * Failure must never fall through to another client root.
     */
    const QString archive =
        arguments.at(4).trimmed();

    if (archive.isEmpty()) {
        if (error) {
            *error =
                QStringLiteral(
                    "client-data payload path is explicitly empty"
                );
        }

        return {};
    }

    if (!QFileInfo::exists(archive)) {
        if (error) {
            *error =
                QStringLiteral(
                    "client-data payload does not exist: "
                ) + archive;
        }

        return {};
    }

    if (!QFileInfo(archive).isFile()) {
        if (error) {
            *error =
                QStringLiteral(
                    "client-data payload is not a file: "
                ) + archive;
        }

        return {};
    }

    if (!temporary.isValid()) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not create temporary directory for client-data payload"
                );
        }

        return {};
    }

    QProcess tar;

    tar.start(
        QStringLiteral("tar"),
        {
            QStringLiteral("-xzf"),
            archive,
            QStringLiteral("-C"),
            temporary.path()
        }
    );

    if (!tar.waitForStarted(5000)) {
        if (error) {
            *error =
                QStringLiteral(
                    "could not start tar to extract client-data payload"
                );
        }

        return {};
    }

    if (!tar.waitForFinished(30000)) {
        tar.kill();
        tar.waitForFinished();

        if (error) {
            *error =
                QStringLiteral(
                    "timed out extracting client-data payload: "
                ) + archive;
        }

        return {};
    }

    if (
        tar.exitStatus() != QProcess::NormalExit
        || tar.exitCode() != 0
    ) {
        if (error) {
            const QString detail =
                QString::fromUtf8(
                    tar.readAllStandardError()
                ).trimmed();

            *error =
                QStringLiteral(
                    "could not extract client-data payload: "
                ) + archive;

            if (!detail.isEmpty()) {
                *error +=
                    QStringLiteral(": ")
                    + detail;
            }
        }

        return {};
    }

    const QString branding =
        QDir(temporary.path()).filePath(
            QStringLiteral("assets/branding")
        );

    if (!QFileInfo(branding).isDir()) {
        if (error) {
            *error =
                QStringLiteral(
                    "client-data payload does not contain assets/branding: "
                ) + archive;
        }

        return {};
    }

    return temporary.path();
}

static QString normalizeLocale(QString value)
{
    value = value.trimmed();

    const qsizetype dot =
        value.indexOf(QLatin1Char('.'));

    if (dot >= 0)
        value.truncate(dot);

    const qsizetype modifier =
        value.indexOf(QLatin1Char('@'));

    if (modifier >= 0)
        value.truncate(modifier);

    value.replace(
        QLatin1Char('-'),
        QLatin1Char('_')
    );

    const QStringList parts =
        value.split(
            QLatin1Char('_'),
            Qt::SkipEmptyParts
        );

    if (parts.isEmpty())
        return {};

    if (parts.size() == 1)
        return parts.at(0).toLower();

    return parts.at(0).toLower()
        + QLatin1Char('_')
        + parts.at(1).toUpper();
}

static QString resolveInstallerClientRoot(
    const QString &payloadRoot,
    QString *error
)
{
    if (!payloadRoot.isEmpty())
        return payloadRoot;

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
        QDir::current().filePath(
            QStringLiteral("client")
        ),

        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral("../../client")
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

static QString resolveInstallerLanguage(
    const QJsonObject &manifest,
    QString *error
)
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_LANGUAGE"
        )
    ) {
        const QString requested =
            qEnvironmentVariable(
                "NEEBLES_LANGUAGE"
            ).trimmed();

        if (requested.isEmpty()) {
            if (error) {
                *error =
                    QStringLiteral(
                        "NEEBLES_LANGUAGE is explicitly set but empty"
                    );
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
                        "unsupported explicit N.E.E.B.L.E.S. language: "
                    ) + requested;
            }

            return {};
        }

        return canonical;
    }

    const QString systemLanguage =
        canonicalLanguage(
            manifest,
            QLocale::system().name()
        );

    if (!systemLanguage.isEmpty())
        return systemLanguage;

    return manifestDefaultLanguage(
        manifest,
        error
    );
}

static QVariantMap loadInstallerStrings(
    const QString &payloadRoot,
    QString *resolvedLanguage,
    QString *error
)
{
    const QString root =
        resolveInstallerClientRoot(
            payloadRoot,
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
        resolveInstallerLanguage(
            manifest,
            error
        );

    if (language.isEmpty())
        return {};

    const QJsonArray languages =
        manifest.value(
            QStringLiteral("languages")
        ).toArray();

    QString fileName;

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

    const QFileInfo languageFileInfo(fileName);

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

    if (resolvedLanguage)
        *resolvedLanguage = language;

    return document.object().toVariantMap();
}

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);

    app.setApplicationName(
        QStringLiteral("N.E.E.B.L.E.S. Installer")
    );

    app.setApplicationVersion(
        QStringLiteral("1.0.5")
    );

    app.setOrganizationName(
        QStringLiteral("N.E.E.B.L.E.S.")
    );

    QTemporaryDir payloadDirectory;
    QString payloadError;

    const QString payloadRoot =
        extractClientData(
            app.arguments(),
            payloadDirectory,
            &payloadError
        );

    if (!payloadError.isEmpty()) {
        qCritical().noquote()
            << payloadError;

        return 2;
    }

    const QString iconPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-installer-icon.png"
            ),
            payloadRoot
        );

    if (!iconPath.isEmpty())
        app.setWindowIcon(QIcon(iconPath));

    QString installerLanguage;
    QString languageError;

    const QVariantMap bossStrings =
        loadInstallerStrings(
            payloadRoot,
            &installerLanguage,
            &languageError
        );

    if (
        !languageError.isEmpty()
        || installerLanguage.isEmpty()
        || bossStrings.isEmpty()
    ) {
        const QString message =
            languageError.isEmpty()
            ? QStringLiteral(
                "N.E.E.B.L.E.S. Installer language contract failed"
            )
            : languageError;

        qCritical().noquote()
            << message;

        return 2;
    }

    InstallerController installer(
        app.arguments(),
        bossStrings,
        installerLanguage
    );

    QQmlApplicationEngine engine;

    engine.rootContext()->setContextProperty(
        QStringLiteral("installer"),
        &installer
    );

    engine.rootContext()->setContextProperty(
        QStringLiteral("bossStrings"),
        bossStrings
    );

    const QString wallPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-boss-wall-transparent.png"
            ),
            payloadRoot
        );

    QUrl brandingRootUrl;

    if (!wallPath.isEmpty()) {
        const QDir brandingDir =
            QFileInfo(wallPath).absoluteDir();

        brandingRootUrl =
            QUrl::fromLocalFile(
                brandingDir.absolutePath()
                + QDir::separator()
            );
    }

    engine.rootContext()->setContextProperty(
        QStringLiteral("brandingRootUrl"),
        brandingRootUrl
    );

    engine.loadFromModule(
        "NeeblesInstaller",
        "Main"
    );

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
