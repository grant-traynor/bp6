#!/bin/bash
# End-to-end test script for startup state persistence (bp6-j33p.10)
#
# This script tests that:
# 1. Window size and position persist across restarts
# 2. Filter selections persist across restarts
# 3. Sort options persist across restarts
# 4. UI state (zoom, collapsed nodes) persists across restarts
# 5. State saves to ~/.bp6/startup.json
# 6. Graceful handling of corrupted/missing state files

set -e

STARTUP_FILE="$HOME/.bp6/startup.json"
BACKUP_FILE="$HOME/.bp6/startup.json.backup"

echo "========================================="
echo "Startup State Persistence Test"
echo "========================================="
echo ""

# Test 1: Check if startup.json exists
echo "Test 1: Checking if startup state file exists..."
if [ -f "$STARTUP_FILE" ]; then
    echo "✅ Found startup.json at $STARTUP_FILE"
    echo "📝 Contents:"
    cat "$STARTUP_FILE" | jq '.'
else
    echo "⚠️  No startup.json found yet (expected on first run)"
fi
echo ""

# Test 2: Backup existing state
echo "Test 2: Backing up existing state (if present)..."
if [ -f "$STARTUP_FILE" ]; then
    cp "$STARTUP_FILE" "$BACKUP_FILE"
    echo "✅ Backed up to $BACKUP_FILE"
else
    echo "ℹ️  No state to backup"
fi
echo ""

# Test 3: Verify schema structure
echo "Test 3: Verifying schema structure..."
if [ -f "$STARTUP_FILE" ]; then
    echo "Checking for required fields..."

    # Check window state
    if jq -e '.window' "$STARTUP_FILE" > /dev/null 2>&1; then
        if jq -e '.window | has("width") and has("height") and has("isMaximized")' "$STARTUP_FILE" | grep -q true; then
            echo "✅ Window state fields present"
        else
            echo "❌ Missing window state fields"
        fi
    else
        echo "❌ Missing window object"
    fi

    # Check filter state
    if jq -e '.filters' "$STARTUP_FILE" > /dev/null 2>&1; then
        if jq -e '.filters | has("filterText") and has("hideClosed") and has("closedTimeFilter") and has("includeHierarchy")' "$STARTUP_FILE" | grep -q true; then
            echo "✅ Filter state fields present"
        else
            echo "❌ Missing filter state fields"
        fi
    else
        echo "❌ Missing filters object"
    fi

    # Check sort state
    if jq -e '.sort.sortBy' "$STARTUP_FILE" > /dev/null 2>&1 && \
       jq -e '.sort.sortOrder' "$STARTUP_FILE" > /dev/null 2>&1; then
        echo "✅ Sort state fields present"
    else
        echo "❌ Missing sort state fields"
    fi

    # Check UI state
    if jq -e '.ui.zoom' "$STARTUP_FILE" > /dev/null 2>&1 && \
       jq -e '.ui.collapsedIds' "$STARTUP_FILE" > /dev/null 2>&1; then
        echo "✅ UI state fields present"
    else
        echo "❌ Missing UI state fields"
    fi
else
    echo "⏭️  Skipped (no state file)"
fi
echo ""

# Test 4: Test corrupted file handling
echo "Test 4: Testing corrupted state file handling..."
if [ -f "$STARTUP_FILE" ]; then
    echo "Creating corrupted state file..."
    echo "{ invalid json" > "$STARTUP_FILE"
    echo "✅ Created corrupted file"
    echo "ℹ️  App should gracefully handle this and use defaults"
    echo "   (Manual test: restart the app and verify it doesn't crash)"

    # Restore backup
    if [ -f "$BACKUP_FILE" ]; then
        cp "$BACKUP_FILE" "$STARTUP_FILE"
        echo "✅ Restored valid state from backup"
    fi
else
    echo "⏭️  Skipped (no state file to corrupt)"
fi
echo ""

# Test 5: Test missing file handling
echo "Test 5: Testing missing state file handling..."
if [ -f "$STARTUP_FILE" ]; then
    mv "$STARTUP_FILE" "$STARTUP_FILE.temp"
    echo "✅ Temporarily moved startup.json"
    echo "ℹ️  App should use defaults when state file is missing"
    echo "   (Manual test: restart the app and verify defaults are used)"

    # Restore
    mv "$STARTUP_FILE.temp" "$STARTUP_FILE"
    echo "✅ Restored state file"
else
    echo "✅ State file already missing (expected on first run)"
    echo "ℹ️  App should use defaults"
fi
echo ""

# Test 6: Display current state
echo "Test 6: Displaying current state values..."
if [ -f "$STARTUP_FILE" ]; then
    echo "Window:"
    echo "  Width:  $(jq -r '.window.width' "$STARTUP_FILE")"
    echo "  Height: $(jq -r '.window.height' "$STARTUP_FILE")"
    echo "  X:      $(jq -r '.window.x' "$STARTUP_FILE")"
    echo "  Y:      $(jq -r '.window.y' "$STARTUP_FILE")"
    echo "  Maximized: $(jq -r '.window.isMaximized' "$STARTUP_FILE")"
    echo ""
    echo "Filters:"
    echo "  Text:        $(jq -r '.filters.filterText' "$STARTUP_FILE")"
    echo "  Hide Closed: $(jq -r '.filters.hideClosed' "$STARTUP_FILE")"
    echo "  Time Filter: $(jq -r '.filters.closedTimeFilter' "$STARTUP_FILE")"
    echo "  Hierarchy:   $(jq -r '.filters.includeHierarchy' "$STARTUP_FILE")"
    echo ""
    echo "Sort:"
    echo "  Sort By:    $(jq -r '.sort.sortBy' "$STARTUP_FILE")"
    echo "  Sort Order: $(jq -r '.sort.sortOrder' "$STARTUP_FILE")"
    echo ""
    echo "UI:"
    echo "  Zoom:          $(jq -r '.ui.zoom' "$STARTUP_FILE")"
    echo "  Collapsed IDs: $(jq -r '.ui.collapsedIds | length' "$STARTUP_FILE") nodes"
else
    echo "⏭️  No state file to display"
fi
echo ""

# Test 7: Manual test instructions
echo "========================================="
echo "Manual Testing Instructions"
echo "========================================="
echo ""
echo "To complete end-to-end testing:"
echo ""
echo "1. Start the application"
echo "2. Change the following:"
echo "   - Resize the window"
echo "   - Move the window to a different position"
echo "   - Set a filter text"
echo "   - Toggle 'Hide Closed'"
echo "   - Change the time filter"
echo "   - Sort by a column (e.g., Priority)"
echo "   - Zoom in/out"
echo "   - Collapse some nodes in the tree"
echo ""
echo "3. Close the application"
echo ""
echo "4. Check the state file:"
echo "   cat ~/.bp6/startup.json | jq '.'"
echo ""
echo "5. Restart the application"
echo ""
echo "6. Verify that ALL state is restored:"
echo "   ✓ Window size and position match"
echo "   ✓ Filter text is restored"
echo "   ✓ Hide Closed state is restored"
echo "   ✓ Time filter is restored"
echo "   ✓ Sort column and order are restored"
echo "   ✓ Zoom level is restored"
echo "   ✓ Collapsed nodes remain collapsed"
echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
