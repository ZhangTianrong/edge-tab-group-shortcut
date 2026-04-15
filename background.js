// Native messaging port
let port = null;

// Current hover state
let hoveredGroupId = null;

// Ordered list of tab groups (left to right)
let orderedGroups = [];

// Last update timestamp for group state
let lastGroupUpdate = 0;

const GROUP_COLLAPSE_SETTLE_MS = 250;

// Connect to native messaging host
function connectNativeHost() {
    console.log('Connecting to native host...');
    try {
        // Log extension ID for debugging
        console.log('Extension ID:', chrome.runtime.id);
        
        // Check if native messaging permission is granted
        chrome.permissions.contains({
            permissions: ['nativeMessaging']
        }, (hasPermission) => {
            console.log('Has nativeMessaging permission:', hasPermission);
        });
        
        port = chrome.runtime.connectNative('com.tabgroup.shortcut');
        console.log('Port created');
        
        // Log port details
        console.debug('Port details:', {
            name: port.name,
            connected: port.connected
        });
        
        port.onMessage.addListener((message) => {
            console.log('Received from native host:', message);
            if (message.type === 'hover_result') {
                handleHoverResult(message);
            } else if (message.type === 'error') {
                console.error('Native host error:', message.message);
            }
        });
        
        port.onDisconnect.addListener(() => {
            const error = chrome.runtime.lastError;
            console.error('Disconnected from native host.');
            console.error('Last error:', error);
            console.error('Chrome runtime last error:', chrome.runtime.lastError);
            
            // Get more details about the error
            if (error && error.message) {
                console.error('Error message:', error.message);
            }
            
            port = null;
            // Try to reconnect after a delay
            setTimeout(connectNativeHost, 5000);
        });

        // Initialize tab groups state
        updateTabGroups();
    } catch (error) {
        console.error('Error connecting to native host:', error);
        console.error('Stack trace:', error.stack);
    }
}

// Update ordered list of tab groups
async function updateTabGroups() {
    try {
        const [currentTab] = await chrome.tabs.query({ active: true, currentWindow: true });
        if (!currentTab) {
            console.debug('No active tab found');
            return;
        }
        
        // Get all tabs to determine group positions
        const tabs = await chrome.tabs.query({ windowId: currentTab.windowId });
        const groups = await chrome.tabGroups.query({ windowId: currentTab.windowId });
        
        // Create a map of group IDs to their leftmost tab's index
        const groupPositions = new Map();
        tabs.forEach(tab => {
            if (tab.groupId !== chrome.tabGroups.TAB_GROUP_ID_NONE) {
                if (!groupPositions.has(tab.groupId) || tab.index < groupPositions.get(tab.groupId)) {
                    groupPositions.set(tab.groupId, tab.index);
                }
            }
        });
        
        // Sort groups by their visual position (leftmost tab's index)
        orderedGroups = groups.sort((a, b) => {
            const posA = groupPositions.get(a.id) ?? Infinity;
            const posB = groupPositions.get(b.id) ?? Infinity;
            return posA - posB;
        });
        
        // Update timestamp
        lastGroupUpdate = Date.now();
        
        console.debug('Tab groups updated (sorted by position):', orderedGroups);
    } catch (error) {
        console.error('Error updating tab groups:', error);
    }
}

// Handle hover result from native host
function handleHoverResult(message) {
    const index = message.data.index;
    console.log('Received hover index:', index);
    
    // Convert 1-based index to 0-based for array access
    const arrayIndex = index - 1;
    
    // Update hovered group ID
    if (index === 0) {
        hoveredGroupId = null;
    } else if ( arrayIndex >= orderedGroups.length) {
        hoveredGroupId = null;
        console.error('Invalid group index:', index);
    }
    else {
        hoveredGroupId = orderedGroups[arrayIndex].id;
    }
    
    console.log('Updated hovered group ID:', hoveredGroupId);
}

// Check which group is being hovered
function checkHoveredGroup() {
    return new Promise((resolve, reject) => {
        if (!port) {
            reject(new Error('No native host connection available'));
            return;
        }

        // Set up timeout
        const timeout = setTimeout(() => {
            port.onMessage.removeListener(messageHandler);
            reject(new Error('Hover check timeout'));
        }, 1000);

        // Message handler
        const messageHandler = (message) => {
            clearTimeout(timeout);
            port.onMessage.removeListener(messageHandler);
            if (message.type === 'hover_result') {
                resolve(message);
            } else {
                reject(new Error(`Unexpected message type: ${message.type}`));
            }
        };

        // Add listener and send message
        port.onMessage.addListener(messageHandler);
        console.log('Requesting hover check from native host');
        try {
            port.postMessage({ type: 'check_hover', data: {} });
        } catch (error) {
            clearTimeout(timeout);
            port.onMessage.removeListener(messageHandler);
            reject(error);
        }
    });
}

function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
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
        console.log(`Activated survivor tab ${nextTab.id} before closing active tab ${activeTab.id}`);
    } catch (error) {
        console.warn(`Could not activate survivor tab ${nextTab.id} before close:`, error);
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
            console.warn(`Could not read group ${groupId} while waiting for collapse:`, error);
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
            console.log(`Group ${groupId} already collapsed before closing`);
            return;
        }

        await chrome.tabGroups.update(groupId, { collapsed: true });

        const collapseObserved = await waitForGroupToBeCollapsed(groupId);
        if (!collapseObserved) {
            console.warn(`Timed out waiting for group ${groupId} to report collapsed`);
        }

        // Chromium resolves the update before the UI collapse animation fully settles.
        await delay(GROUP_COLLAPSE_SETTLE_MS);
        console.log(`Group ${groupId} collapsed and settled before closing`);
    } catch (collapseError) {
        console.warn(`Could not fully collapse group ${groupId}, continuing with removal:`, collapseError);
    }
}

// Close tabs in a specific group
async function closeGroupTabs(groupId) {
    try {
        console.log(`Closing tabs in group ${groupId}`);

        const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
        const windowTabs = activeTab
            ? await chrome.tabs.query({ windowId: activeTab.windowId })
            : [];
        const groupTabsToClose = windowTabs.filter(tab => tab.groupId === groupId);

        if (activeTab) {
            await activateSurvivingTabIfNeeded(
                windowTabs,
                activeTab,
                groupTabsToClose.map(tab => tab.id)
            );
        }
        
        // Collapse the group first to avoid animation lag
        await collapseGroupBeforeClosing(groupId);
        
        // Get all tabs in the group
        const tabs = await chrome.tabs.query({ groupId });
        console.log(`Found ${tabs.length} tabs in group`);
        
        const tabIds = tabs.map(tab => tab.id);
        await chrome.tabs.remove(tabIds);
        console.log('Successfully closed group tabs');
    } catch (error) {
        console.error('Error closing group tabs:', error);
        throw error;
    }
}

// Close all tabs except those in a specific group
async function closeOtherTabs(exceptGroupId) {
    try {
        console.log(`Closing all tabs except group ${exceptGroupId}`);
        
        // Get all tabs in the current window
        const [currentTab] = await chrome.tabs.query({ active: true, currentWindow: true });
        if (!currentTab) {
            console.warn('No active tab found while closing other tabs');
            return;
        }

        const tabs = await chrome.tabs.query({ windowId: currentTab.windowId });
        
        // Filter tabs not in the excepted group
        const tabsToClose = tabs.filter(tab => 
            tab.groupId !== exceptGroupId && 
            tab.groupId !== chrome.tabGroups.TAB_GROUP_ID_NONE
        );

        await activateSurvivingTabIfNeeded(
            tabs,
            currentTab,
            tabsToClose.map(tab => tab.id)
        );
        
        // Collapse all groups that will have tabs removed to avoid animation lag
        const groupsToCollapse = new Set(tabsToClose.map(tab => tab.groupId));
        await Promise.all(
            Array.from(groupsToCollapse, groupId => collapseGroupBeforeClosing(groupId))
        );
        
        const tabIds = tabsToClose.map(tab => tab.id);
        await chrome.tabs.remove(tabIds);
        console.log('Successfully closed other tabs');
    } catch (error) {
        console.error('Error closing other tabs:', error);
        throw error;
    }
}

// Listen for commands
chrome.commands.onCommand.addListener(async (command) => {
    console.log(`Received command: ${command}`);
    
    try {
        // Update groups state to ensure it's fresh
        await updateTabGroups();

        // Check which group is being hovered
        const start = Date.now();
        const result = await checkHoveredGroup();
        const end = Date.now();
        console.log(`Hover check execution time: ${end - start} ms`);
        
        // Only proceed if we have a hovered group
        if (!hoveredGroupId) {
            console.log('No group hovered, ignoring command');
            return;
        }
        
        switch (command) {
            case 'close-group-tabs':
                await closeGroupTabs(hoveredGroupId);
                break;
            case 'close-other-groups':
                await closeOtherTabs(hoveredGroupId);
                break;
        }
    } catch (error) {
        console.error('Error handling command:', error);
    }
});

// Listen for tab group changes
chrome.tabGroups.onCreated.addListener((group) => {
    console.debug('Tab group created:', group);
    updateTabGroups();
});

chrome.tabGroups.onUpdated.addListener((group) => {
    console.debug('Tab group updated:', group);
    updateTabGroups();
});

chrome.tabGroups.onRemoved.addListener((group) => {
    console.debug('Tab group removed:', group);
    updateTabGroups();
});

// Initialize native messaging connection
console.log('TabGroup Keyboard Shortcuts extension started');
connectNativeHost();
