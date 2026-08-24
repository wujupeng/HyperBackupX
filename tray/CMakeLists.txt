cmake_minimum_required(VERSION 3.21)
project(hbx-tray VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

find_package(Qt6 6.6 REQUIRED COMPONENTS Quick Widgets)

qt_add_executable(hbx-tray
    src/main.cpp
    src/TrayIPC.cpp
    src/TrayModel.cpp
)

qt_add_qml_module(hbx-tray
    URI HyperBackupX.Tray
    VERSION 1.0
    QML_FILES
        qml/main.qml
        qml/StatusView.qml
        qml/JobListView.qml
)

target_link_libraries(hbx-tray PRIVATE
    Qt6::Quick
    Qt6::Widgets
)

set_target_properties(hbx-tray PROPERTIES
    WIN32_EXECUTABLE ON
)