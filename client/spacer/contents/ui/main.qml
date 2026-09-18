import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import NEEBLES.BossEvents 1.0

PlasmoidItem {
    id: root

    LauncherBossEvents {
        id: bossEvents

        Component.onCompleted:
            connectToBoss()
    }

    Layout.minimumWidth:
        bossEvents.launcherEnabled ? 20 : 0

    Layout.preferredWidth:
        bossEvents.launcherEnabled ? 20 : 0

    Layout.maximumWidth:
        bossEvents.launcherEnabled ? 20 : 0

    Layout.minimumHeight: 1
    Layout.preferredHeight: 38

    preferredRepresentation: fullRepresentation

    fullRepresentation: Item {
        implicitWidth:
            bossEvents.launcherEnabled ? 20 : 0

        implicitHeight: 38

        visible:
            bossEvents.launcherEnabled
    }
}
