qt_add_executable(CraftwardQmlTest
    runner.cpp
    filetreefixture.h
    ../../src/components/projectfilesmodel.cpp
    ../../src/components/projectfilesmodel.h
    ../../src/components/popupanchortoggle.cpp
    ../../src/components/popupanchortoggle.h
    ../../src/components/popuppositioner.cpp
    ../../src/components/popuppositioner.h
    ../../src/ward/codex/codexthreadfiltermodel.cpp
    ../../src/ward/codex/codexthreadfiltermodel.h
)
target_include_directories(CraftwardQmlTest PRIVATE ../../src/components ../../src/ward/codex)
# Resolve the QML test imports at runtime without linking application plugins.
set_target_properties(CraftwardQmlTest PROPERTIES
    MACOSX_BUNDLE FALSE
    QT_QML_MODULE_NO_IMPORT_SCAN TRUE
)
target_link_libraries(CraftwardQmlTest PRIVATE Qt6::Qml Qt6::Quick Qt6::QuickTest)

qt_add_resources(CraftwardQmlTest "workbench_test_icons"
    PREFIX "/icons"
    BASE "${PROJECT_SOURCE_DIR}/assets/icons"
    FILES
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/arrow-up-to-line.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/cancel-01.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/chat-01.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/chat-feedback-01.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/chevron-right.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/file-02.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/image-02.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/image-not-found-01.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/folder-02.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/fold-vertical.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/more-horizontal-circle-02.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/scan-image.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/split.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/stop-watch.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/zoom-in.svg"
        "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/zoom-out.svg"
)
