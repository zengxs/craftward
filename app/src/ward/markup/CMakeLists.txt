add_library(CraftwardMarkup STATIC
    markupdocumentmodel.cpp
    markupdocumentmodel.h
)

qt_add_protobuf(CraftwardMarkup
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${PROJECT_SOURCE_DIR}/../proto/ward/markup/v1/document.proto"
    PROTO_INCLUDES
        "${PROJECT_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardMarkup
    PUBLIC
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src>"
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/markup/v1>"
)

target_link_libraries(CraftwardMarkup
    PRIVATE
        CraftwardCoreFfiSupport
        Qt6::Concurrent
        Qt6::Protobuf
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardMarkupDocumentModelTest
        tests/markupdocumentmodeltest.cpp
    )
    target_link_libraries(CraftwardMarkupDocumentModelTest
        PRIVATE
            CraftwardMarkup
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardMarkupDocumentModelTest
        COMMAND CraftwardMarkupDocumentModelTest
    )
endif()
