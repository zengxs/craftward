set(craftward_legal_catalog
    "${CMAKE_CURRENT_SOURCE_DIR}/legal/components.json"
)
set(craftward_mit_legal_text
    "${CMAKE_CURRENT_SOURCE_DIR}/legal/MIT.txt"
)
set(craftward_phosphor_legal_text
    "${CMAKE_CURRENT_SOURCE_DIR}/legal/PhosphorIcons.txt"
)
set(craftward_qt_legal_text
    "${CMAKE_CURRENT_SOURCE_DIR}/legal/Qt.txt"
)
set(craftward_gpl_text
    "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE.md"
)
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
    "${craftward_legal_catalog}"
    PROPERTIES QT_RESOURCE_ALIAS "components.json"
)
set_source_files_properties(
    "${craftward_mit_legal_text}"
    PROPERTIES QT_RESOURCE_ALIAS "MIT.txt"
)
set_source_files_properties(
    "${craftward_phosphor_legal_text}"
    PROPERTIES QT_RESOURCE_ALIAS "PhosphorIcons.txt"
)
set_source_files_properties(
    "${craftward_qt_legal_text}"
    PROPERTIES QT_RESOURCE_ALIAS "Qt.txt"
)
set_source_files_properties(
    "${craftward_gpl_text}"
    PROPERTIES QT_RESOURCE_ALIAS "GPL-3.0.txt"
)

qt_add_resources(Craftward "craftward_legal"
    PREFIX "/legal"
    FILES
        "${craftward_legal_catalog}"
        "${craftward_mit_legal_text}"
        "${craftward_phosphor_legal_text}"
        "${craftward_qt_legal_text}"
        "${craftward_gpl_text}"
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
