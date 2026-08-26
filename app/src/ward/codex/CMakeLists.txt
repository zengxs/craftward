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
        codexthreadmodel.cpp
        codexthreadmodel.h
        codextimelinemodel.cpp
        codextimelinemodel.h
        codextimelinepagemodel.cpp
        codextimelinepagemodel.h
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

    qt_add_executable(CraftwardCodexTimelinePageModelTest
        codextimelinepagemodel.cpp
        codextimelinepagemodel.h
        tests/codextimelinepagemodeltest.cpp
    )
    target_include_directories(CraftwardCodexTimelinePageModelTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexTimelinePageModelTest
        PRIVATE
            Qt6::Gui
            Qt6::Qml
            Qt6::Test
    )
    add_test(
        NAME CraftwardCodexTimelinePageModelTest
        COMMAND CraftwardCodexTimelinePageModelTest
    )
endif()
