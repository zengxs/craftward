import QtQuick
import QtQuick.Controls

Button {
    id: control

    highlighted: true
    palette.buttonText: {
        if (!control.enabled)
            return systemPalette.disabled.buttonText;
        if (control.highlighted && control.Window.active)
            return systemPalette.active.brightText;
        return control.Window.active ? systemPalette.active.buttonText : systemPalette.inactive.buttonText;
    }

    Palette {
        id: systemPalette
    }
}
