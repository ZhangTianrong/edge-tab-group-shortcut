const LOG_LEVELS = {
    debug: 10,
    info: 20,
    warn: 30,
    error: 40,
};

const ACTIVE_LOG_LEVEL = LOG_LEVELS.warn;
const LOG_PREFIX = "[TabGroupShortcut]";
const GROUP_COLLAPSE_SETTLE_MS = 250;
const HELP_PAGE_FILE = "help.html";
const HELP_PAGE_RATE_LIMIT_MS = 30_000;
const NATIVE_HOST_NAME = "com.tabgroup.shortcut";

let lastHelpPageOpenedAt = 0;

function log(level, message, ...details) {
    if (LOG_LEVELS[level] < ACTIVE_LOG_LEVEL) {
        return;
    }

    const method = level === "debug" ? "debug" : level;
    console[method](LOG_PREFIX, message, ...details);
}

function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function openHelpPage(reason = "manual", force = false) {
    const now = Date.now();
    if (!force && now - lastHelpPageOpenedAt < HELP_PAGE_RATE_LIMIT_MS) {
        return;
    }

    lastHelpPageOpenedAt = now;

    const url = new URL(chrome.runtime.getURL(HELP_PAGE_FILE));
    url.searchParams.set("reason", reason);

    try {
        await chrome.tabs.create({ url: url.toString() });
    } catch (error) {
        log("error", "Could not open the help page.", error);
    }
}

async function sendNativeMessage(message) {
    return new Promise((resolve, reject) => {
        chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, message, response => {
            const runtimeError = chrome.runtime.lastError;
            if (runtimeError) {
                reject(new Error(runtimeError.message));
                return;
            }

            resolve(response);
        });
    });
}

async function requestHoveredGroupIndex() {
    const response = await sendNativeMessage({
        type: "check_hover",
        data: {},
    });

    if (!response) {
        throw new Error("Native host returned no response.");
    }

    if (response.type === "error") {
        throw new Error(response.data?.message ?? "Hover check failed.");
    }

    const hoverIndex = response.data?.index;
    if (!Number.isInteger(hoverIndex) || hoverIndex < 0) {
        throw new Error(`Invalid hover index returned by native host: ${JSON.stringify(hoverIndex)}`);
    }

    return hoverIndex;
}

async function queryOrderedGroups(windowId, windowTabs) {
    const tabs = windowTabs ?? await chrome.tabs.query({ windowId });
    const groups = await chrome.tabGroups.query({ windowId });
    const groupPositions = new Map();

    tabs.forEach(tab => {
        if (tab.groupId === chrome.tabGroups.TAB_GROUP_ID_NONE) {
            return;
        }

        const previousPosition = groupPositions.get(tab.groupId);
        if (previousPosition === undefined || tab.index < previousPosition) {
            groupPositions.set(tab.groupId, tab.index);
        }
    });

    return groups
        .slice()
        .sort((left, right) => (groupPositions.get(left.id) ?? Infinity) - (groupPositions.get(right.id) ?? Infinity));
}

async function getCurrentWindowContext() {
    const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!activeTab) {
        return null;
    }

    const windowTabs = await chrome.tabs.query({ windowId: activeTab.windowId });
    const orderedGroups = await queryOrderedGroups(activeTab.windowId, windowTabs);

    return {
        activeTab,
        orderedGroups,
        windowTabs,
    };
}

function resolveHoveredGroupId(orderedGroups, hoverIndex) {
    if (hoverIndex === 0) {
        return null;
    }

    const group = orderedGroups[hoverIndex - 1];
    if (!group) {
        log(
            "warn",
            `Hover detector returned group index ${hoverIndex}, but only ${orderedGroups.length} tab groups were found.`
        );
        return null;
    }

    return group.id;
}

function pickNearestTabByIndex(tabs, pivotIndex) {
    if (tabs.length === 0) {
        return null;
    }

    return tabs.reduce((bestTab, tab) => {
        if (!bestTab) {
            return tab;
        }

        const bestDistance = Math.abs(bestTab.index - pivotIndex);
        const currentDistance = Math.abs(tab.index - pivotIndex);
        if (currentDistance !== bestDistance) {
            return currentDistance < bestDistance ? tab : bestTab;
        }

        return tab.index < bestTab.index ? tab : bestTab;
    }, null);
}

function findTabToActivateBeforeClose(windowTabs, activeTab, tabIdsToClose) {
    const closingTabIds = new Set(tabIdsToClose);
    if (!closingTabIds.has(activeTab.id)) {
        return null;
    }

    const survivingTabs = windowTabs.filter(tab => !closingTabIds.has(tab.id));
    if (survivingTabs.length === 0) {
        return null;
    }

    return pickNearestTabByIndex(survivingTabs, activeTab.index);
}

async function activateSurvivingTabIfNeeded(windowTabs, activeTab, tabIdsToClose) {
    const nextTab = findTabToActivateBeforeClose(windowTabs, activeTab, tabIdsToClose);
    if (!nextTab) {
        return;
    }

    try {
        await chrome.tabs.update(nextTab.id, { active: true });
    } catch (error) {
        log("warn", `Could not activate survivor tab ${nextTab.id} before closing.`, error);
    }
}

async function waitForGroupToBeCollapsed(groupId, timeoutMs = 1000) {
    const deadline = Date.now() + timeoutMs;

    while (Date.now() < deadline) {
        try {
            const group = await chrome.tabGroups.get(groupId);
            if (group.collapsed) {
                return true;
            }
        } catch (error) {
            log("warn", `Could not read group ${groupId} while waiting for collapse.`, error);
            return false;
        }

        await delay(50);
    }

    return false;
}

async function collapseGroupBeforeClosing(groupId) {
    try {
        const group = await chrome.tabGroups.get(groupId);
        if (group.collapsed) {
            return;
        }

        await chrome.tabGroups.update(groupId, { collapsed: true });

        const collapseObserved = await waitForGroupToBeCollapsed(groupId);
        if (!collapseObserved) {
            log("warn", `Timed out waiting for group ${groupId} to report a collapsed state.`);
        }

        await delay(GROUP_COLLAPSE_SETTLE_MS);
    } catch (error) {
        log("warn", `Could not fully collapse group ${groupId} before closing tabs.`, error);
    }
}

async function closeTabs(tabIds) {
    if (tabIds.length === 0) {
        return;
    }

    await chrome.tabs.remove(tabIds);
}

async function closeGroupTabs(groupId, activeTab, windowTabs) {
    const groupTabsToClose = windowTabs.filter(tab => tab.groupId === groupId);
    if (groupTabsToClose.length === 0) {
        log("warn", `No tabs were found for group ${groupId}.`);
        return;
    }

    await activateSurvivingTabIfNeeded(
        windowTabs,
        activeTab,
        groupTabsToClose.map(tab => tab.id)
    );

    await collapseGroupBeforeClosing(groupId);
    await closeTabs(groupTabsToClose.map(tab => tab.id));
}

async function closeOtherTabs(exceptGroupId, activeTab, windowTabs) {
    const tabsToClose = windowTabs.filter(
        tab =>
            tab.groupId !== exceptGroupId &&
            tab.groupId !== chrome.tabGroups.TAB_GROUP_ID_NONE
    );

    if (tabsToClose.length === 0) {
        return;
    }

    await activateSurvivingTabIfNeeded(
        windowTabs,
        activeTab,
        tabsToClose.map(tab => tab.id)
    );

    const groupsToCollapse = Array.from(new Set(tabsToClose.map(tab => tab.groupId)));
    await Promise.all(groupsToCollapse.map(groupId => collapseGroupBeforeClosing(groupId)));

    await closeTabs(tabsToClose.map(tab => tab.id));
}

async function handleCommand(command) {
    try {
        const context = await getCurrentWindowContext();
        if (!context) {
            log("warn", "No active tab was found while handling a command.");
            return;
        }

        let hoverIndex;
        try {
            hoverIndex = await requestHoveredGroupIndex();
        } catch (error) {
            log("error", "The hover detector could not be reached.", error);
            await openHelpPage("native-host");
            return;
        }

        const hoveredGroupId = resolveHoveredGroupId(context.orderedGroups, hoverIndex);
        if (!hoveredGroupId) {
            return;
        }

        switch (command) {
            case "close-group-tabs":
                await closeGroupTabs(hoveredGroupId, context.activeTab, context.windowTabs);
                break;
            case "close-other-groups":
                await closeOtherTabs(hoveredGroupId, context.activeTab, context.windowTabs);
                break;
            default:
                log("warn", `Received an unknown command: ${command}`);
                break;
        }
    } catch (error) {
        log("error", `Command "${command}" failed unexpectedly.`, error);
    }
}

chrome.commands.onCommand.addListener(command => {
    handleCommand(command);
});

chrome.runtime.onInstalled.addListener(({ reason }) => {
    if (reason === "install") {
        openHelpPage("install", true);
    }
});

chrome.action.onClicked.addListener(() => {
    openHelpPage("manual", true);
});
