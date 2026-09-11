# QML Tests

Run `task app:test:qml` from the repository root. The task builds
`CraftwardQmlTest`, which uses Qt Quick Test and registers the test-only
`Craftward.TestSupport.AnimationClock` type. Pass individual Qt Quick Test case
names to that executable to run a focused check. Keep `tests/qml/imports` on its
import path and set `QT_QPA_PLATFORM=offscreen`, as the task does. The runner
deliberately does not link application QML plugins.

Timing-sensitive tests enable `AnimationClock` after creating their viewport,
advance animation time explicitly, and wait for rendering after each step. This
keeps animation phases independent of runner scheduling while still checking
real presented frames. Disable the clock during cleanup so other tests retain
normal animation timing.

Observe each flick's velocity peak from velocity changes. Inject geometry changes
from the controlled motion phase, independently of frame observations. Capture
positions and bounds together when comparing layout phases, and keep separate
checks for final tail alignment and visible-row stability across settlement.
The stopped-frame baseline follows synchronous viewport settlement. Compare it
with the last presented moving frame and subsequent stopped frames; an internal
position that is corrected before presentation is not a displayed frame.
