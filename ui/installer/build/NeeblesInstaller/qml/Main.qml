import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 820
    height: detailsVisible ? 620 : 420
    minimumWidth: 760
    minimumHeight: 400
    visible: true
    title: "N.E.E.B.L.E.S."
    color: "#09090B"

    property bool detailsVisible: false

    Component.onCompleted: installer.startInstallation()

    Connections {
        target: installer
        function onLogLine(line) {
            logArea.text += (logArea.text.length > 0 ? "\n" : "") + line
            logArea.cursorPosition = logArea.length
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#09090B"

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 34
            spacing: 18

            RowLayout {
                Layout.fillWidth: true

                ColumnLayout {
                    spacing: 2

                    Label {
                        text: "N.E.E.B.L.E.S."
                        color: "#F5F5F5"
                        font.pixelSize: 32
                        font.bold: true
                    }

                    Label {
                        text: "Boss 0.0.1"
                        color: "#A78BFA"
                        font.pixelSize: 14
                    }
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    width: 14
                    height: 14
                    radius: 7
                    color: installer.finished
                           ? (installer.success ? "#22D3EE" : "#EF4444")
                           : "#7C3AED"
                }
            }

            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: "#27272A"
            }

            Label {
                Layout.fillWidth: true
                text: installer.status
                color: "#F5F5F5"
                font.pixelSize: 18
                wrapMode: Text.WordWrap
            }

            ProgressBar {
                id: progress
                Layout.fillWidth: true
                from: 0
                to: 100
                value: installer.progress

                background: Rectangle {
                    implicitHeight: 12
                    radius: 6
                    color: "#18181B"
                }

                contentItem: Item {
                    implicitHeight: 12

                    Rectangle {
                        width: progress.visualPosition * parent.width
                        height: parent.height
                        radius: 6
                        color: "#7C3AED"
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    text: installer.progress + "%"
                    color: "#A1A1AA"
                    font.pixelSize: 13
                }

                Item { Layout.fillWidth: true }

                Button {
                    text: window.detailsVisible ? "Hide details" : "Show details"
                    onClicked: window.detailsVisible = !window.detailsVisible

                    background: Rectangle {
                        radius: 6
                        color: parent.down ? "#5B21B6" : "#27272A"
                        border.color: "#7C3AED"
                    }

                    contentItem: Text {
                        text: parent.text
                        color: "#F5F5F5"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: window.detailsVisible
                color: "#050507"
                radius: 6
                border.color: "#27272A"
                clip: true

                ScrollView {
                    anchors.fill: parent
                    anchors.margins: 10

                    TextArea {
                        id: logArea
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.WrapAnywhere
                        color: "#D4D4D8"
                        selectionColor: "#7C3AED"
                        selectedTextColor: "#FFFFFF"
                        font.family: "monospace"
                        font.pixelSize: 12
                        background: null
                        text: ""
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: installer.finished
                text: installer.success
                      ? "N.E.E.B.L.E.S. is ready."
                      : "Installation did not complete. Review the details above."
                color: installer.success ? "#22D3EE" : "#F87171"
                font.pixelSize: 14
            }
        }
    }
}
