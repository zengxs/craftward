qt_add_executable(CraftwardQmlTest
    runner.cpp
    ../../src/components/popuppositioner.cpp
    ../../src/components/popuppositioner.h
)
target_include_directories(CraftwardQmlTest PRIVATE ../../src/components)
# Resolve the QML test imports at runtime without linking application plugins.
set_target_properties(CraftwardQmlTest PROPERTIES
    MACOSX_BUNDLE FALSE
    QT_QML_MODULE_NO_IMPORT_SCAN TRUE
)
target_link_libraries(CraftwardQmlTest PRIVATE Qt6::Qml Qt6::Quick Qt6::QuickTest)
