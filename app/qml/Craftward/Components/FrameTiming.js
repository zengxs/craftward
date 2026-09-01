.pragma library

function refreshRate(targetWindow) {
    const screen = targetWindow ? targetWindow.screen : null;
    const rate = screen ? Number(screen.refreshRate) : Number.NaN;
    return Number.isFinite(rate) && rate > 0 ? rate : 60;
}

function budgetMilliseconds(targetWindow) {
    return 1000 / refreshRate(targetWindow);
}
