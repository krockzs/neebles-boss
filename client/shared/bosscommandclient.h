#pragma once

#include <QObject>

class BossCommandClient final : public QObject
{
    Q_OBJECT

public:
    explicit BossCommandClient(QObject *parent = nullptr);

    Q_INVOKABLE bool startBoss();
};
