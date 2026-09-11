// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.TestSupport

Item {
    width: 400
    height: 400

    AnimationClock {
        id: clock
    }

    Flickable {
        id: viewport
        anchors.fill: parent
        contentHeight: 100000
        maximumFlickVelocity: 32000
        flickDeceleration: 16000
    }

    TestCase {
        name: "AnimationClock"
        when: windowShown

        function startControlledFlick() {
            viewport.contentY = 0;
            clock.enable();
            viewport.flick(0, -32000);
            wait(0);
            for (let frame = 0; viewport.contentY === 0 && frame < 4; ++frame) {
                verify(clock.advance(16));
                wait(0);
            }
            verify(viewport.contentY > 0);
        }

        function cleanup() {
            viewport.cancelFlick();
            clock.disable();
            wait(0);
        }

        function test_holdsAnimationTimeWhileTheEventLoopContinues() {
            startControlledFlick();
            const position = viewport.contentY;
            const velocity = viewport.verticalVelocity;

            wait(50);

            compare(viewport.contentY, position);
            compare(viewport.verticalVelocity, velocity);
            verify(clock.advance(16));
            verify(viewport.contentY > position);
        }

        function test_restoresAutomaticAnimationAfterDisable() {
            startControlledFlick();
            const position = viewport.contentY;

            clock.disable();

            tryVerify(() => viewport.contentY > position, 1000);
        }
    }
}
