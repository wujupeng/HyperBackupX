import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import HyperBackupX.Tray 1.0

ListView {
    id: jobListView
    model: TrayModel
    clip: true
    spacing: 4

    delegate: Rectangle {
        width: jobListView.width
        height: 64
        radius: 6
        color: "#313244"

        RowLayout {
            anchors.fill: parent
            anchors.margins: 8
            spacing: 8

            Rectangle {
                width: 4
                Layout.fillHeight: true
                color: {
                    if (model.status === "completed") return "#a6e3a1"
                    if (model.status === "running") return "#f9e2af"
                    if (model.status === "failed") return "#f38ba8"
                    return "#6c7086"
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Text {
                    text: TrayModel.sanitizeText(model.name)
                    font.pixelSize: 14
                    font.bold: true
                    color: "#cdd6f4"
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: {
                        var parts = []
                        if (model.fileCount > 0)
                            parts.push(model.fileCount + " files")
                        if (model.bytesProcessed > 0) {
                            var mb = (model.bytesProcessed / 1048576).toFixed(1)
                            parts.push(mb + " MB")
                        }
                        if (model.startTime)
                            parts.push(model.startTime)
                        return parts.join(" · ")
                    }
                    font.pixelSize: 11
                    color: "#6c7086"
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            Text {
                text: model.status
                font.pixelSize: 12
                font.bold: true
                color: {
                    if (model.status === "completed") return "#a6e3a1"
                    if (model.status === "running") return "#f9e2af"
                    if (model.status === "failed") return "#f38ba8"
                    if (model.status === "pending") return "#89b4fa"
                    return "#6c7086"
                }
            }

            Button {
                text: model.status === "running" ? "Pause" : "Resume"
                visible: model.status === "running" || model.status === "paused"
                onClicked: {
                    if (model.status === "running")
                        TrayIPC.pauseTask(model.jobId)
                    else
                        TrayIPC.resumeTask(model.jobId)
                }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        text: "No jobs"
        color: "#6c7086"
        font.pixelSize: 14
        visible: jobListView.count === 0
    }
}
