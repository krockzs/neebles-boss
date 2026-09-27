#pragma once

#include <QString>

namespace NeeblesRuntimeAuthority
{

QString resolve(
    const QString &resolver,
    const QString &manifest,
    const QString &world,
    const QString &category,
    QString *error = nullptr
);

}
