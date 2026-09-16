#pragma once

#include "../shared/bosseventclient.h"

#include <QtQml/qqmlregistration.h>

class LauncherBossEvents
    : public BossEventClient
{
    Q_OBJECT
    QML_ELEMENT

public:
    explicit LauncherBossEvents(
        QObject *parent = nullptr
    )
        : BossEventClient(parent)
    {
    }
};
