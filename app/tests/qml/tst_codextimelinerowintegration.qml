// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Components
import "../../qml/Craftward/Pages" as Pages
import "CodexTimelineTestFixtures.js" as Fixtures

Item {
    id: suite

    width: 720
    height: 480
    property var viewport
    property bool hasRunningEvidence: false
    property bool activityShimmerEnabled: false
    property bool forkEnabled: true
    property bool showForkActions: true
    property string lastForkedTurnId
    property var lastFileLocation: null
    property var lastOpenedImage: null

    Pages.CodexImagePreview {
        id: imagePreview
        onOpenImageRequested: image => suite.lastOpenedImage = image
    }

    ListModel {
        id: fakeTimelineModel

        property int revision: 0
        property var rows: []
        readonly property int totalRowCount: count

        function resetRows(nextRows) {
            rows = nextRows;
            clear();
            for (const row of nextRows) {
                append({
                    entryId: String(row.entryId)
                });
            }
            ++revision;
        }

        function entryIdAt(sourceRow) {
            const row = rows[sourceRow];
            return row ? row.entryId : "";
        }

        function valueAt(sourceRow, roleName) {
            const row = rows[sourceRow];
            // Match the fresh QVariantList returned by the C++ model on each read.
            if (row && roleName === "attachments" && row.attachments)
                return row.attachments.map(item => Object.assign({}, item));
            return row ? row[roleName] : undefined;
        }

        function indexOfEntryId(entryId) {
            const target = String(entryId);
            for (let row = 0; row < count; ++row) {
                if (entryIdAt(row) === target)
                    return row;
            }
            return -1;
        }
    }

    Component {
        id: rowComponent

        Pages.CodexTimelineRow {
            timelineModel: fakeTimelineModel
            imagePreviewHandler: imagePreview.handle
            onOpenImageRequested: image => suite.lastOpenedImage = image
            turnExpanded: false
            hasRunningEvidence: suite.hasRunningEvidence
            activityShimmerEnabled: suite.activityShimmerEnabled
            forkEnabled: suite.forkEnabled
            showForkActions: suite.showForkActions
            wallClockUnixMilliseconds: 0
            onForkRequested: turnId => suite.lastForkedTurnId = turnId
            onFileLocationRequested: (file, start, end) => suite.lastFileLocation = ({
                        file,
                        start,
                        end
                    })
        }
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 680
            height: 360
            timelineModel: fakeTimelineModel
            rowDelegate: rowComponent
            bottomContentInset: 0
            contentHorizontalInset: 20
            contentMaximumWidth: 640
            estimatedRowHeight: 120
        }
    }

    TestCase {
        name: "CodexTimelineRowIntegration"
        when: windowShown

        function messageRow(overrides) {
            return Object.assign({
                entryId: "message:turn-1:message-1",
                turnId: "turn-1",
                turnForkable: true,
                latestTurn: true,
                activityGroup: false,
                fromUser: false,
                finalAnswer: true,
                detailRow: false,
                firstDetailInTurn: false,
                detailCountInTurn: 0,
                standaloneActivity: false,
                text: "Copy this message",
                semanticBlock: true,
                firstBlockInEntry: true,
                lastBlockInEntry: true,
                blockText: "Rendered message",
                codeBlock: false,
                language: ""
            }, overrides ?? {});
        }

        function createViewport(rows) {
            destroyViewport();
            fakeTimelineModel.resetRows(rows);
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            tryVerify(() => suite.viewport.delegateForEntry(rows[0].entryId) !== null);
            const row = suite.viewport.delegateForEntry(rows[0].entryId);
            tryVerify(() => row.implicitHeight > 0);
            verify(waitForRendering(suite.viewport));
            return row;
        }

        function destroyViewport() {
            imagePreview.dismiss();
            if (!suite.viewport)
                return;
            suite.viewport.destroy();
            suite.viewport = null;
            wait(0);
        }

        function init() {
            ApplicationClipboard.reset();
            suite.hasRunningEvidence = false;
            suite.activityShimmerEnabled = false;
            suite.forkEnabled = true;
            suite.showForkActions = true;
            suite.lastForkedTurnId = "";
            suite.lastFileLocation = null;
            suite.lastOpenedImage = null;
        }

        function cleanup() {
            destroyViewport();
            fakeTimelineModel.resetRows([]);
        }

        function test_hoverRevealsOlderAnswerWithoutChangingRowHeight() {
            const row = createViewport([messageRow({
                    latestTurn: false
                })]);
            const actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(actions.available);
            tryVerify(() => row.implicitHeight > 24);
            tryCompare(actions, "opacity", 0);
            const retainedHeight = row.implicitHeight;

            mouseMove(row, 10, 8);

            tryCompare(actions, "opacity", 1);
            compare(row.implicitHeight, retainedHeight);
        }

        function test_annotationChipOwnsCollectionWithoutAnEmptyBubble() {
            const copyText = "[1] Selected text\n\nChange it";
            const annotated = messageRow({
                fromUser: true,
                finalAnswer: false,
                text: copyText,
                displayText: "",
                annotationCount: 2,
                semanticBlock: false
            });
            const row = createViewport([annotated]);
            const chip = findChild(row, "codexAnnotationChip");
            verify(chip.visible);
            compare(chip.text, "2 annotations");
            verify(!findChild(row, "codexUserMessageSurface").visible);
            const actions = findChild(row, "codexMessageActions");
            verify(actions.available);
            actions.copyRequested();
            compare(ApplicationClipboard.lastCopiedText, copyText);
            const height = row.implicitHeight;
            mouseMove(chip, chip.width / 2, chip.height / 2);
            compare(row.implicitHeight, height);
        }

        function test_annotationChipAppearsOnlyAboveFirstBodySegment() {
            const annotated = messageRow({
                fromUser: true,
                finalAnswer: false,
                displayText: "Body",
                annotationCount: 1
            });
            let row = createViewport([annotated]);
            const chip = findChild(row, "codexAnnotationChip");
            const surface = findChild(row, "codexUserMessageSurface");
            verify(chip.visible && surface.visible);
            verify(surface.mapToItem(row, 0, 0).y > chip.mapToItem(row, 0, chip.height).y);
            row = createViewport([Object.assign({}, annotated, {
                    firstBlockInEntry: false
                })]);
            verify(!findChild(row, "codexAnnotationChip").visible);
        }

        function test_fileOnlyMessageOpensFileAndCopiesWithoutAnEmptyBubble() {
            const copyText = "Pasted excerpt: /missing/pasted-text.txt:2-4";
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false,
                    text: copyText,
                    displayText: "",
                    semanticBlock: false,
                    attachments: [
                        {
                            label: "Pasted excerpt",
                            path: "/missing/pasted-text.txt",
                            image: false,
                            pasted: true,
                            startLine: 2,
                            endLine: 4
                        }
                    ]
                })]);
            const attachments = findChild(row, "codexMessageAttachments");
            verify(attachments.visible);
            verify(attachments.height > 0);
            verify(!findChild(row, "codexUserMessageSurface").visible);
            const card = findChild(attachments, "codexAttachmentCard");
            mouseClick(card);
            compare(suite.lastFileLocation.file, "/missing/pasted-text.txt");
            compare(suite.lastFileLocation.start, 2);
            compare(suite.lastFileLocation.end, 4);
            findChild(row, "codexMessageActions").copyRequested();
            compare(ApplicationClipboard.lastCopiedText, copyText);
        }

        function test_imagePreviewAndAnnotationsShareTheFirstSegment() {
            const input = messageRow({
                fromUser: true,
                finalAnswer: false,
                displayText: "Compare these",
                annotationCount: 1,
                attachments: [
                    {
                        label: "Design sketch",
                        path: "/work/design.png",
                        image: true,
                        url: Qt.resolvedUrl("fixtures/attachment-small.png")
                    }
                ]
            });
            let row = createViewport([input]);
            const attachments = findChild(row, "codexMessageAttachments");
            const thumbnail = findChild(attachments, "codexAttachmentThumbnail");
            tryCompare(thumbnail, "status", Image.Ready);
            tryCompare(findChild(attachments, "codexAttachmentCard"), "height", 90);
            tryCompare(attachments, "height", 90);
            row.prepareForLayout();
            verify(waitForPolish(suite.viewport));
            const chip = findChild(row, "codexAnnotationChip");
            const surface = findChild(row, "codexUserMessageSurface");
            verify(chip.visible && surface.visible);
            verify(chip.mapToItem(row, 0, 0).y > attachments.mapToItem(row, 0, attachments.height).y);
            verify(surface.mapToItem(row, 0, 0).y > chip.mapToItem(row, 0, chip.height).y);
            const height = row.implicitHeight;
            const card = findChild(attachments, "codexAttachmentCard");
            const preview = imagePreview;
            mouseMove(card, card.width / 2, card.height / 2);
            wait(550);
            verify(!preview.visible);
            mouseClick(card);
            tryCompare(preview, "opened", true);
            tryCompare(findChild(preview, "codexAttachmentFullImage"), "status", Image.Ready);
            compare(row.implicitHeight, height);
            keyClick(Qt.Key_Escape);
            tryCompare(preview, "visible", false);
            mouseClick(card);
            tryCompare(preview, "opened", true);
            mouseClick(preview.contentItem, 1, preview.height + 20);
            tryCompare(preview, "visible", false);

            row = createViewport([Object.assign({}, input, {
                    firstBlockInEntry: false
                })]);
            verify(!findChild(row, "codexMessageAttachments").visible);
            verify(!findChild(row, "codexAnnotationChip").visible);
        }

        function test_thumbnailFitsInsideAFixedSquare_data() {
            return [
                {
                    tag: "small",
                    width: 90,
                    height: 90 * 48 / 130
                },
                {
                    tag: "landscape",
                    width: 90,
                    height: 60
                },
                {
                    tag: "portrait",
                    width: 30,
                    height: 90
                },
                {
                    tag: "panorama",
                    width: 90,
                    height: 1.8
                }
            ];
        }

        function test_thumbnailFitsInsideAFixedSquare(data) {
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false,
                    displayText: "A short request",
                    attachments: [
                        {
                            label: data.tag + ".png",
                            path: "/test/" + data.tag + ".png",
                            image: true,
                            url: Qt.resolvedUrl("fixtures/attachment-" + data.tag + ".png")
                        }
                    ]
                })]);
            const card = findChild(row, "codexAttachmentCard");
            const thumbnail = findChild(card, "codexAttachmentThumbnail");
            compare(card.width, 90);
            compare(card.height, 90);
            tryCompare(thumbnail, "status", Image.Ready);
            tryVerify(() => Math.abs(thumbnail.width - data.width) < 1 && Math.abs(thumbnail.height - data.height) < 1);
            verify(Math.abs(thumbnail.x - (90 - thumbnail.width) / 2) < 1);
            verify(Math.abs(thumbnail.y - (90 - thumbnail.height) / 2) < 1);
            tryCompare(findChild(row, "codexMessageAttachments"), "height", card.height);
            verify(!findChild(card, "codexAttachmentFileLabel").visible);
            tryVerify(() => {
                // Sample the presented viewport, including ancestor layering.
                const image = grabImage(suite.viewport);
                const center = card.mapToItem(suite.viewport, card.width / 2, card.height / 2);
                const centerX = Math.floor(center.x * image.width / suite.viewport.width);
                const centerY = Math.floor(center.y * image.height / suite.viewport.height);
                return image.blue(centerX, centerY) > image.red(centerX, centerY) + 50;
            }, 5000, "The thumbnail must render the blue fixture, not just its frame.");
            const surface = findChild(row, "codexUserMessageSurface");
            verify(surface.width < 240);
            tryVerify(() => surface.mapToItem(row, 0, 0).y > card.mapToItem(row, 0, card.height).y);
        }

        function test_loadedImagesScrollWithoutRecreationOrChangingOrder() {
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false,
                    displayText: "",
                    semanticBlock: false,
                    attachments: ["landscape", "small", "portrait", "panorama"].map(tag => ({
                                label: tag,
                                path: "",
                                image: true,
                                url: Qt.resolvedUrl("fixtures/attachment-" + tag + ".png")
                            }))
                })]);
            const attachments = findChild(row, "codexMessageAttachments");
            const first = findChild(attachments, "codexAttachmentCard");
            const cards = first.parent.children.filter(item => item.objectName === "codexAttachmentCard");
            compare(cards.length, 4);
            for (const card of cards) {
                tryCompare(findChild(card, "codexAttachmentThumbnail"), "status", Image.Ready);
                compare(card.width, 90);
                compare(card.height, 90);
            }
            tryCompare(attachments, "height", 90);
            for (let index = 1; index < cards.length; ++index) {
                compare(cards[index].y, cards[0].y);
                compare(cards[index].x, cards[index - 1].x + 98);
            }
            attachments.width = 188;
            tryCompare(attachments, "height", 120);
            const strip = findChild(attachments, "codexAttachmentStrip");
            compare(strip.contentX, 0);
            compare(cards[0].y, cards[3].y);
            mouseClick(findChild(attachments, "attachmentScrollNext"));
            tryVerify(() => strip.contentX > 0);
            mouseClick(findChild(attachments, "attachmentScrollNext"));
            compare(strip.contentX, strip.contentWidth - strip.width);
            verify(!findChild(attachments, "attachmentScrollNext").enabled);
            const scrolledPosition = strip.contentX;
            ++fakeTimelineModel.revision;
            wait(0);
            compare(strip.contentX, scrolledPosition);
            compare(findChild(attachments, "codexAttachmentCard"), first);
            cards[3].forceActiveFocus();
            suite.Window.window.requestActivate();
            tryVerify(() => cards[3].activeFocus);
            keyClick(Qt.Key_Left);
            keyClick(Qt.Key_Left);
            keyClick(Qt.Key_Left);
            tryCompare(strip, "contentX", 0);
            compare(findChild(attachments, "codexAttachmentCard"), first);
            compare(cards[0].width, 90);
        }

        function test_missingImageKeepsItsCardAndReportsUnavailablePreview() {
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false,
                    displayText: "",
                    semanticBlock: false,
                    attachments: [
                        {
                            label: "Missing image",
                            path: "/missing/image.png",
                            url: "file:///missing/image.png",
                            image: true
                        }
                    ]
                })]);
            const attachments = findChild(row, "codexMessageAttachments");
            const thumbnail = findChild(attachments, "codexAttachmentThumbnail");
            tryCompare(thumbnail, "status", Image.Error);
            const card = findChild(attachments, "codexAttachmentCard");
            verify(card.visible);
            compare(card.width, 90);
            compare(card.height, 90);
            mouseClick(card);
            const preview = imagePreview;
            tryCompare(preview, "opened", true);
            tryCompare(findChild(preview, "codexAttachmentFullImage"), "status", Image.Error);
            preview.close();
        }

        function test_previewSwitchesImagesAndOnlyExplicitlyOpensATab() {
            const row = createViewport([messageRow({
                    fromUser: true,
                    displayText: "Compare",
                    attachments: ["landscape", "portrait"].map(tag => ({
                                image: true,
                                label: tag,
                                resourceId: "image:" + tag,
                                url: Qt.resolvedUrl("fixtures/attachment-" + tag + ".png")
                            }))
                })]);
            const first = findChild(row, "codexAttachmentCard");
            const cards = first.parent.children.filter(item => item.objectName === "codexAttachmentCard");
            mouseClick(first);
            tryCompare(imagePreview, "opened", true);
            mouseClick(first);
            tryCompare(imagePreview, "visible", false);
            mouseClick(first);
            tryCompare(imagePreview, "opened", true);
            compare(suite.lastOpenedImage, null);
            mouseClick(findChild(imagePreview, "imagePreviewNext"));
            compare(imagePreview.currentImage.resourceId, "image:portrait");
            compare(suite.lastOpenedImage, null);
            mouseClick(cards[0]);
            tryCompare(imagePreview, "visible", false);
            mouseClick(cards[1]);
            tryCompare(imagePreview, "opened", true);
            tryCompare(imagePreview, "currentIndex", 1);
            mouseClick(findChild(imagePreview, "imagePreviewOpenTab"));
            tryCompare(imagePreview, "visible", false);
            compare(suite.lastOpenedImage.resourceId, "image:portrait");
            mouseClick(first);
            tryCompare(imagePreview, "opened", true);
            first.parent.x += 1;
            tryCompare(imagePreview, "visible", false);
        }

        function test_previewStaysOpenAcrossUnrelatedTimelineUpdates_data() {
            return [
                {
                    tag: "single",
                    images: ["landscape"]
                },
                {
                    tag: "selected-second",
                    images: ["landscape", "portrait"]
                }
            ];
        }

        function test_previewStaysOpenAcrossUnrelatedTimelineUpdates(data) {
            const row = createViewport([messageRow({
                    fromUser: true,
                    displayText: "Compare",
                    attachments: data.images.map(tag => ({
                                image: true,
                                label: tag,
                                resourceId: "image:" + tag,
                                url: Qt.resolvedUrl("fixtures/attachment-" + tag + ".png")
                            }))
                })]);
            const card = findChild(row, "codexAttachmentCard");
            tryCompare(findChild(card, "codexAttachmentThumbnail"), "status", Image.Ready);
            row.prepareForLayout();
            verify(waitForPolish(suite.viewport));
            mouseClick(card);
            tryCompare(imagePreview, "opened", true);
            if (data.images.length > 1)
                mouseClick(findChild(imagePreview, "imagePreviewNext"));
            const expectedResource = "image:" + data.images[data.images.length - 1];
            compare(imagePreview.currentImage.resourceId, expectedResource);

            ++fakeTimelineModel.revision;
            verify(waitForPolish(suite.viewport));
            wait(50);

            verify(imagePreview.opened, "An unchanged attachment list must not dismiss the preview on a timeline revision.");
            compare(imagePreview.currentImage.resourceId, expectedResource);
            compare(findChild(row, "codexAttachmentCard"), card);
        }

        function test_changedAttachmentInvalidatesTheOpenPreview() {
            const input = messageRow({
                fromUser: true,
                displayText: "Compare",
                attachments: [
                    {
                        image: true,
                        label: "Image",
                        url: Qt.resolvedUrl("fixtures/attachment-landscape.png")
                    }
                ]
            });
            const row = createViewport([input]);
            const card = findChild(row, "codexAttachmentCard");
            tryCompare(findChild(card, "codexAttachmentThumbnail"), "status", Image.Ready);
            row.prepareForLayout();
            verify(waitForPolish(suite.viewport));
            mouseClick(card);
            tryCompare(imagePreview, "opened", true);

            input.attachments[0].url = Qt.resolvedUrl("fixtures/attachment-portrait.png");
            ++fakeTimelineModel.revision;

            tryCompare(imagePreview, "visible", false);
            const replacement = findChild(row, "codexAttachmentCard");
            verify(replacement !== card);
            tryCompare(findChild(replacement, "codexAttachmentThumbnail"), "status", Image.Ready);
        }

        function test_listContinuationUsesItsItemSpacing() {
            for (const spacing of [0, 10]) {
                const row = createViewport([messageRow({
                        lastBlockInEntry: false,
                        renderParts: [
                            {
                                kind: "text",
                                spacingAfter: spacing
                            }
                        ]
                    })]);
                compare(row.semanticBlockSpacing, spacing);
            }
            const last = createViewport([messageRow({
                    renderParts: [
                        {
                            kind: "text",
                            spacingAfter: 10
                        }
                    ]
                })]);
            compare(last.semanticBlockSpacing, 0);
        }

        function test_latestRunningEvidenceSuppressesActionsButNotOlderMessages() {
            suite.hasRunningEvidence = true;
            let row = createViewport([messageRow()]);
            let actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(!actions.available);
            verify(!actions.visible);

            row = createViewport([messageRow({
                    entryId: "message:turn-0:message-1",
                    turnId: "turn-0",
                    latestTurn: false
                })]);
            actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(actions.available);
        }

        function test_userCopyUsesClipboardGlueAndKeepsGeometryStable() {
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false
                })]);
            const copyButton = findChild(row, "codexMessageCopyButton");
            const actions = findChild(row, "codexMessageActions");
            verify(copyButton !== null);
            verify(actions !== null);
            const retainedHeight = row.implicitHeight;

            mouseClick(copyButton);

            compare(ApplicationClipboard.copyCount, 1);
            compare(ApplicationClipboard.lastCopiedText, "Copy this message");
            verify(actions.copied);
            compare(row.implicitHeight, retainedHeight);
            verify(findChild(row, "codexMessageForkButton") === null || !findChild(row, "codexMessageForkButton").visible);
        }

        function test_forkRequiresAFinalForkableAnswerAndEmitsItsTurnId() {
            const row = createViewport([messageRow()]);
            const forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(forkButton.visible);

            mouseClick(forkButton);

            compare(suite.lastForkedTurnId, "turn-1");
        }

        function test_forkHonorsEnablementVisibilityAndModelConstraints() {
            suite.forkEnabled = false;
            let row = createViewport([messageRow()]);
            let forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(forkButton.visible);
            verify(!forkButton.enabled);
            mouseClick(forkButton);
            compare(suite.lastForkedTurnId, "");

            suite.forkEnabled = true;
            suite.showForkActions = false;
            row = createViewport([messageRow()]);
            forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(!forkButton.visible);

            suite.showForkActions = true;
            row = createViewport([messageRow({
                    turnForkable: false
                })]);
            forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(!forkButton.visible);
        }

        function test_runningParentActivityShimmerDoesNotChangeGeometry() {
            const row = createViewport([Fixtures.standaloneActivityRow()]);
            const shimmer = findChild(row, "codexActivityShimmer");
            verify(shimmer !== null);
            verify(!shimmer.visible);
            const retainedHeight = row.implicitHeight;

            suite.activityShimmerEnabled = true;

            tryVerify(() => shimmer.visible);
            compare(row.implicitHeight, retainedHeight);

            suite.activityShimmerEnabled = false;

            tryVerify(() => !shimmer.visible);
            compare(row.implicitHeight, retainedHeight);
        }
    }
}
