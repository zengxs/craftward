qt_add_qml_module(CraftwardCodex
    STATIC
    URI Craftward.Codex
    SOURCES
        codexattachmentinput.cpp
        codexattachmentinput.h
        codexconversationcontroller.cpp
        codexconversationcontroller_events.cpp
        codexconversationcontroller.h
        codexhistorycontroller.cpp
        codexhistorycontroller_events.cpp
        codexhistorycontroller.h
        codexinteractionmodel.cpp
        codexinteractionmodel.h
        codexmodelcatalogmodel.cpp
        codexmodelcatalogmodel.h
        persistentstringroleindex.h
        codexthreadmodel.cpp
        codexthreadmodel.h
        codextimelinemodel.cpp
        codextimelinemodel.h
        codextimelinepresentationmodel.cpp
        codextimelinepresentationmodel.h
        codextimelineviewportmodel.cpp
        codextimelineviewportmodel.h
)

qt_add_protobuf(CraftwardCodex
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${PROJECT_SOURCE_DIR}/../proto/ward/codex/v1/history.proto"
    PROTO_INCLUDES
        "${PROJECT_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardCodex
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/codex/v1>"
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src"
)

target_link_libraries(CraftwardCodex
    PRIVATE
        CraftwardCoreFfiSupport
        CraftwardMarkup
        Qt6::Gui
        Qt6::Protobuf
        Qt6::Qml
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardCodexHistoryControllerTest
        tests/codexhistorycontrollertest.cpp
    )
    target_include_directories(CraftwardCodexHistoryControllerTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexHistoryControllerTest
        PRIVATE
            CraftwardCodex
            Qt6::Gui
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardCodexHistoryControllerTest
        COMMAND CraftwardCodexHistoryControllerTest
    )

    qt_add_executable(CraftwardCodexTimelinePresentationModelTest
        codextimelinepresentationmodel.cpp
        codextimelinepresentationmodel.h
        tests/codextimelinepresentationmodeltest.cpp
    )
    target_include_directories(CraftwardCodexTimelinePresentationModelTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexTimelinePresentationModelTest
        PRIVATE
            Qt6::Gui
            Qt6::Qml
            Qt6::Test
    )
    add_test(
        NAME CraftwardCodexTimelinePresentationModelTest
        COMMAND CraftwardCodexTimelinePresentationModelTest
    )

    qt_add_executable(CraftwardCodexTimelineViewportIntegrationTest
        codextimelinepresentationmodel.cpp
        codextimelinepresentationmodel.h
        tests/codextimelineviewportintegrationtest.cpp
    )
    target_compile_definitions(CraftwardCodexTimelineViewportIntegrationTest
        PRIVATE
            CRAFTWARD_TEST_QML_IMPORT_PATH="${PROJECT_SOURCE_DIR}/tests/qml/imports"
            CRAFTWARD_TIMELINE_VIEWPORT_HARNESS="${CMAKE_CURRENT_SOURCE_DIR}/tests/data/timelineviewportintegration.qml"
    )
    target_include_directories(CraftwardCodexTimelineViewportIntegrationTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexTimelineViewportIntegrationTest
        PRIVATE
            Qt6::Gui
            Qt6::Qml
            Qt6::Quick
            Qt6::Test
    )
    add_test(
        NAME CraftwardCodexTimelineViewportIntegrationTest
        COMMAND CraftwardCodexTimelineViewportIntegrationTest
    )

    qt_add_executable(CraftwardCodexTimelineViewportModelTest
        codextimelineviewportmodel.cpp
        codextimelineviewportmodel.h
        tests/codextimelineviewportmodeltest.cpp
    )
    target_include_directories(CraftwardCodexTimelineViewportModelTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexTimelineViewportModelTest
        PRIVATE
            CraftwardMarkup
            Qt6::Gui
            Qt6::Qml
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardCodexTimelineViewportModelTest
        COMMAND CraftwardCodexTimelineViewportModelTest
    )
endif()
