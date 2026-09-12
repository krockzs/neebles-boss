import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid

PlasmoidItem {
    Layout.minimumWidth: 20
    Layout.preferredWidth: 20
    Layout.maximumWidth: 20

    Layout.minimumHeight: 1
    Layout.preferredHeight: 38

    preferredRepresentation: fullRepresentation

    fullRepresentation: Item {
        implicitWidth: 20
        implicitHeight: 38
    }
}
