#pragma once

#include "../shared/bosseventclient.h"

#include <QtQml/qqmlregistration.h>
#include <QCryptographicHash>
#include <QFile>
#include <QTimer>
#include <QDebug>
#include <QByteArray>
#include <QStringList>

class LauncherBossEvents
    : public BossEventClient
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int surfaceRevision READ surfaceRevision NOTIFY surfaceRevisionChanged)

public:
    explicit LauncherBossEvents(
        QObject *parent = nullptr
    )
        : BossEventClient(parent)
    {
        // Updates replace files on disk. The applet QObject and its position
        // remain unchanged while Loader recreates only its visual children.
        m_paths = {
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.launcher/contents/ui/main.qml"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.spacer/contents/ui/main.qml"),
                        QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.launcher/contents/ui/NeeblesSwitch.qml"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.launcher/contents/ui/LauncherCompactSurface.qml"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.launcher/contents/ui/LauncherFullSurface.qml"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.spacer/contents/ui/SpacerSurface.qml"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.launcher/metadata.json"),
            QStringLiteral("/usr/share/plasma/plasmoids/org.neebles.spacer/metadata.json")
        };
        m_digest = surfaceDigest();
        auto *poll = new QTimer(this);
        poll->setInterval(2500);
        connect(poll, &QTimer::timeout, this, [this]() {
            QByteArray next = surfaceDigest();
            if (next.isEmpty() || next == m_digest)
                return;
            // Only accept stable, fully written payloads across two polls.
            if (next != m_candidate) {
                m_candidate = next;
                return;
            }
            m_digest = next;
            m_candidate.clear();
            ++m_surfaceRevision;
            qInfo("NEEBLES_SURFACE_RELOAD revision=%d", m_surfaceRevision);
            emit surfaceRevisionChanged();
        });
        poll->start();
    }

    int surfaceRevision() const { return m_surfaceRevision; }

signals:
    void surfaceRevisionChanged();

private:
    QByteArray surfaceDigest() const
    {
        QCryptographicHash digest(QCryptographicHash::Sha256);
        for (const QString &path : m_paths) {
            QFile file(path);
            // Every surface must exist before permitting a reload.
            if (!file.open(QIODevice::ReadOnly))
                return {};
            digest.addData(file.readAll());
        }
        return digest.result();
    }

    QStringList m_paths;
    QByteArray m_digest;
    QByteArray m_candidate;
    int m_surfaceRevision = 0;
};
