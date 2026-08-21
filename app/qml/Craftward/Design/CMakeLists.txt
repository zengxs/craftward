set(craftward_design_singletons
    TailwindColors.qml
    Theme.qml
)

set_source_files_properties(
    ${craftward_design_singletons}
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE
)

qt_add_qml_module(CraftwardDesign
    STATIC
    NO_PLUGIN
    URI Craftward.Design
    QML_FILES
        ${craftward_design_singletons}
)

target_link_libraries(CraftwardDesign
    PRIVATE
        Qt6::Qml
        Qt6::Quick
)
