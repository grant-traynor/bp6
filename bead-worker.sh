#!/bin/bash

# bead-worker.sh: Autonomous Bead Execution Loop
# Automates the lifecycle of task decomposition and execution using Gemini CLI.

# Ensure we are in a git repository
if ! git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
  echo "Error: Not in a git repository."
  exit 1
fi

while true; do
  # 1. Select the first high priority Task from those in the ready state (P0 or P1)
  # We use 'bd ready --json' to find unblocked work, prioritizing P0 then P1
  TASK_ID=$(bd ready --json | jq -r '
    sort_by(.priority) | 
    [.[] | select(.priority <= 1)] | 
    .[0].id
  ')

  if [ "$TASK_ID" == "null" ] || [ -z "$TASK_ID" ]; then
    echo "✅ No more high-priority ready tasks. Task list clear."
    break
  fi

  # Get task details
  TASK_JSON=$(bd show "$TASK_ID" --json)
  TASK_TYPE=$(echo "$TASK_JSON" | jq -r '.[0].issue_type')
  TASK_TITLE=$(echo "$TASK_JSON" | jq -r '.[0].title')

  echo "--------------------------------------------------------------------------------"
  echo "🚀 Processing $TASK_ID ($TASK_TYPE): $TASK_TITLE"
  echo "--------------------------------------------------------------------------------"

  # 2. If EPIC or FEATURE, use spec mode to decompose
  if [ "$TASK_TYPE" == "epic" ] || [ "$TASK_TYPE" == "feature" ]; then
    echo "📋 Decomposing $TASK_TYPE..."
    echo "load and use the spec skill to decompose $TASK_ID into actionable 
    tasks. Ensure each task has clear acceptance criteria and verification instructions.
    Focus on breaking it down into items that can be implemented in single sittings.'" | gemini --allowed-tools=activate_skill --allowed-tools=run_shell_command --model gemini-3-pro-preview
    
    # If it was an Epic, we might have created Features. If it was a Feature, we created Tasks.
    # We continue the loop to pick up the newly created (and hopefully ready) child items.
    continue 
  fi

  # 4-7. Execute work via Gemini CLI in Exec mode
  # This command handles:
  # - Work implementation
  # - File size assessment (> 600 lines) and automatic decomposition
  # - Compilation checks
  # - Code inspection
  # - Conventional commits
  # - Bead closure
  
  echo  "load and use the exec skill to perform: exec $TASK_ID --instructions '
    Activate skill-exec. Perform the requested work for $TASK_ID.
    
    CRITICAL QUALITY CHECKS:
    1. FILE SIZE: If any file you modify or create exceeds 600 lines, you MUST decompose it into smaller, logical modules or components. After decomposition, verify that the logic is preserved and there are no regressions.
    2. COMPILATION: Confirm the code compiles cleanly (e.g., npm run build, tsc, or equivalent).
    3. INSPECTION: Perform a thorough code inspection to confirm ALL acceptance criteria for $TASK_ID are met.
    
    FINALIZATION:
    - Commit your changes using conventional commit messages referencing $TASK_ID.
    - Close the bead using: bd close $TASK_ID --reason "Completed implementation, verified ACs, and checked file bounds."
  '" | gemini --allowed-tools=all --model gemini-3-pro-preview 

  # Check if the task was successfully closed
  FINAL_STATUS=$(bd show "$TASK_ID" --json | jq -r '.[0].status')
  if [ "$FINAL_STATUS" != "closed" ]; then
    echo "⚠️  Task $TASK_ID was not closed by the agent. Halting for manual review."
    exit 1
  fi

  echo "✅ Successfully processed $TASK_ID."
  echo ""
done
