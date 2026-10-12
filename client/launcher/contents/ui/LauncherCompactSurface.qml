import QtQuick
import QtQuick.Layouts

Item {
    property var bossRoot: parent.bossRoot
    property var bossSettings: parent.bossSettings
        implicitWidth: bossSettings.launcherEnabled ? 38 : 0
        implicitHeight: 38
        visible: bossSettings.launcherEnabled

        Image {
            anchors.fill: parent
            source: "../images/neebles-boss-launcher-icon.png"
            fillMode: Image.PreserveAspectFit
            smooth: true
        }

        MouseArea {
            anchors.fill: parent
            onClicked: bossRoot.expanded = !bossRoot.expanded
        }
    }
