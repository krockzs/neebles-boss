import QtQuick
import QtQuick.Controls

ApplicationWindow {
    width: 760
    height: 460
    visible: true
    title: "N.E.E.B.L.E.S."

    Column {
        anchors.centerIn: parent
        spacing: 24
        width: parent.width * 0.72

        Label {
            text: "N.E.E.B.L.E.S."
            font.pixelSize: 34
            font.bold: true
            anchors.horizontalCenter: parent.horizontalCenter
        }

        Label {
            text: "Preparing your system..."
            font.pixelSize: 20
            anchors.horizontalCenter: parent.horizontalCenter
        }

        ProgressBar {
            from: 0
            to: 1
            value: 0
            width: parent.width
        }

        Label {
            text: "Boss 0.0.1 installer"
            anchors.horizontalCenter: parent.horizontalCenter
        }
    }
}
