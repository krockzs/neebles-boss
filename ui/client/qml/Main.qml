import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    width: 900
    height: 560
    visible: true
    title: "N.E.E.B.L.E.S."
    color: "#09090B"

    Rectangle {
        anchors.fill: parent
        color: "#09090B"

        ColumnLayout {
            anchors.centerIn: parent
            spacing: 12

            Label {
                text: "N.E.E.B.L.E.S."
                color: "#F5F5F5"
                font.pixelSize: 38
                font.bold: true
                Layout.alignment: Qt.AlignHCenter
            }

            Label {
                text: "Boss 0.0.1"
                color: "#A78BFA"
                font.pixelSize: 16
                Layout.alignment: Qt.AlignHCenter
            }

            Rectangle {
                Layout.preferredWidth: 360
                Layout.preferredHeight: 1
                color: "#27272A"
            }

            Label {
                text: "N.E.E.B.L.E.S. is ready."
                color: "#22D3EE"
                font.pixelSize: 18
                Layout.alignment: Qt.AlignHCenter
            }
        }
    }
}
