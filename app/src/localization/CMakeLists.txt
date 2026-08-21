qt_add_qml_module(CraftwardLocalization
    STATIC
    URI Craftward.Localization
    SOURCES
        localizationcontroller.cpp
        localizationcontroller.h
)

target_include_directories(CraftwardLocalization
    PUBLIC
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src>"
)

target_link_libraries(CraftwardLocalization
    PRIVATE
        Qt6::Qml
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardLocalizationControllerTest
        tests/localizationcontrollertest.cpp
    )
    target_include_directories(CraftwardLocalizationControllerTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardLocalizationControllerTest
        PRIVATE
            CraftwardLocalization
            Qt6::Qml
            Qt6::Test
    )
    add_test(
        NAME CraftwardLocalizationControllerTest
        COMMAND CraftwardLocalizationControllerTest
    )
endif()
