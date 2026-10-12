import QtQuick
import QtQuick.Layouts

Item {
    property var bossSettings: parent.bossSettings
    implicitWidth: bossSettings.launcherEnabled ? 20 : 0
    implicitHeight: 38
    visible: bossSettings.launcherEnabled
}

// NEEBLES 1.0.48: update boundary for prior-version surface detector.
