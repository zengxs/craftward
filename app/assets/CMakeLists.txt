set(craftward_external_link_icon
    "${CMAKE_CURRENT_SOURCE_DIR}/icons/phosphor/arrow-square-out.svg"
)
set(craftward_more_actions_icon
    "${CMAKE_CURRENT_SOURCE_DIR}/icons/phosphor/dots-three-circle.svg"
)
set(craftward_dismiss_icon
    "${CMAKE_CURRENT_SOURCE_DIR}/icons/phosphor/x-circle.svg"
)

set_source_files_properties(
    "${craftward_external_link_icon}"
    PROPERTIES QT_RESOURCE_ALIAS "phosphor/arrow-square-out.svg"
)
set_source_files_properties(
    "${craftward_more_actions_icon}"
    PROPERTIES QT_RESOURCE_ALIAS "phosphor/dots-three-circle.svg"
)
set_source_files_properties(
    "${craftward_dismiss_icon}"
    PROPERTIES QT_RESOURCE_ALIAS "phosphor/x-circle.svg"
)

qt_add_resources(Craftward "craftward_icons"
    PREFIX "/icons"
    FILES
        "${craftward_dismiss_icon}"
        "${craftward_external_link_icon}"
        "${craftward_more_actions_icon}"
)
