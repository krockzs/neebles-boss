import QtQuick
import QtQuick.Controls as QQC2

QQC2.Switch {
        id: control

        implicitWidth: 42
        implicitHeight: 24

        hoverEnabled: true

        indicator: Rectangle {
            anchors.centerIn: parent

            width: 36
            height: 18
            radius: 9

            color:
                control.checked
                ? "#5B21B6"
                : "#27272A"

            border.width:
                control.hovered
                ? 2
                : 1

            border.color:
                control.checked
                ? "#A855F7"
                : control.hovered
                  ? "#67E8F9"
                  : "#3F3F46"

            Rectangle {
                width: 14
                height: 14
                radius: 7

                anchors.verticalCenter:
                    parent.verticalCenter

                x:
                    control.checked
                    ? parent.width - width - 2
                    : 2

                color:
                    control.checked
                    ? "#F5F3FF"
                    : "#A1A1AA"
            }
        }

        contentItem: Item {
        }
}
