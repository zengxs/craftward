// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 640
    height: 480
    Component {
        id: factory
        Flickable {
            width: 400
            height: 360
            contentWidth: width
            contentHeight: 900
            flickableDirection: Flickable.VerticalFlick
            boundsBehavior: Flickable.StopAtBounds
            Pages.CodexMessageAttachments {
                objectName: "attachments"
                y: 80
                width: 188
                attachments: ["small", "portrait", "landscape", "panorama"].map(tag => ({
                            label: tag,
                            image: true,
                            url: Qt.resolvedUrl("fixtures/attachment-" + tag + ".png")
                        }))
            }
        }
    }
    TestCase {
        name: "CodexMessageAttachments"
        when: windowShown
        function test_horizontalInputDoesNotStealVerticalTimelineScrolling() {
            const outer = createTemporaryObject(factory, suite);
            const attachments = findChild(outer, "attachments");
            const strip = findChild(attachments, "codexAttachmentStrip");
            tryVerify(() => strip.contentWidth > strip.width);
            mouseWheel(strip, 50, 45, -120, 0);
            tryVerify(() => strip.contentX > 0);
            compare(outer.contentY, 0);
            const x = strip.contentX;
            mouseWheel(strip, 50, 45, 0, -120);
            tryVerify(() => outer.contentY > 0);
            compare(strip.contentX, x);
            outer.cancelFlick();
            outer.contentY = 0;
            strip.contentX = 0;
            mouseDrag(strip, 145, 45, -90, 0);
            tryVerify(() => strip.contentX > 0);
            compare(outer.contentY, 0);
            compare(attachments.height, 120);
        }
    }
}
