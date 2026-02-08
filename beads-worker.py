#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# beads-worker.py: Autonomous Bead Execution Loop (Python Version)
# Automates the lifecycle of task decomposition and execution using Gemini CLI.

import json
import os
import subprocess
import sys
import shutil

# Configuration
AGENT = "gemini"
AGENT_MODEL = "gemini-3-pro-preview"

# --- Helper Functions ---

def run_shell_command(cmd, input_text=None, capture_output=True):
    """Runs a shell command and returns stdout, stderr, and return code."""
    try:
        process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE if input_text else None,
            stdout=subprocess.PIPE if capture_output else None,
            stderr=subprocess.PIPE if capture_output else None,
            text=True,
            shell=False 
        )
        stdout, stderr = process.communicate(input=input_text)
        return {
            "stdout": stdout.strip() if stdout else "",
            "stderr": stderr.strip() if stderr else "",
            "returncode": process.returncode
        }
    except Exception as e:
        print(f"Error running command {' '.join(cmd)}: {e}")
        return None

def call_gemini(prompt, allowed_tools=None):
    """Calls the Gemini CLI with the given prompt and allowed tools."""
    cmd = [AGENT, '--yolo', '--model', AGENT_MODEL]
    
    if allowed_tools:
        if "all" in allowed_tools:
            cmd.append('--allowed-tools=all')
        else:
            for tool in allowed_tools:
                cmd.append(f'--allowed-tools={tool}')
    
    # We want to stream the output to the console so the user sees progress,
    # but we don't necessarily need to capture it for the script's logic 
    # (since the agent acts via tools). 
    # However, the original script piped to gemini. 
    # Let's just run it and let it inherit stdout/stderr so the user interacts/sees it.
    
    print(f"\n>>> Calling Gemini ({AGENT_MODEL}) with tools: {allowed_tools}")
    
    try:
        # Using subprocess.run to wait for completion
        # We pass the prompt via stdin
        process = subprocess.run(
            cmd,
            input=prompt,
            text=True,
            check=False # We handle return code manually if needed
        )
        return process.returncode == 0
    except Exception as e:
        print(f"Failed to call {AGENT}: {e}")
        return False

def check_git_repo():
    result = run_shell_command(["git", "rev-parse", "--is-inside-work-tree"])
    if not result or result["returncode"] != 0:
        print("Error: Not in a git repository.")
        sys.exit(1)

def get_ready_tasks():
    """Retrieves ready tasks using 'bd ready --json'."""
    if not shutil.which("bd"):
        print("Error: 'bd' command not found.")
        sys.exit(1)

    result = run_shell_command(["bd", "ready", "--json"])
    if not result or result["returncode"] != 0:
        print("Error fetching tasks: " + result["stderr"])
        return []
    
    try:
        tasks = json.loads(result["stdout"])
        return tasks
    except json.JSONDecodeError:
        print("Error parsing JSON from 'bd ready'. Output was:", result["stdout"])
        return []

def get_task_details(task_id):
    result = run_shell_command(["bd", "show", task_id, "--json"])
    if not result or result["returncode"] != 0:
        print(f"Error fetching details for {task_id}: {result['stderr']}")
        return None
    
    try:
        # bd show returns a list with one item
        data = json.loads(result["stdout"])
        return data[0] if data else None
    except json.JSONDecodeError:
        return None

# --- Main Logic ---

def main():
    check_git_repo()

    while True:
        # 1. Select the first high priority Task from those in the ready state (P0 or P1)
        tasks = get_ready_tasks()
        
        # Filter and Sort
        # equivalent to: sort_by(.priority) | [.[] | select(.priority <= 1)]
        high_priority_tasks = [t for t in tasks if t.get('priority', 999) <= 1]
        high_priority_tasks.sort(key=lambda x: x.get('priority', 999))
        
        if not high_priority_tasks:
            print("✅ No more high-priority ready tasks. Task list clear.")
            break
        
        current_task = high_priority_tasks[0]
        task_id = current_task.get('id')
        
        if not task_id:
            print("Found a task without an ID, skipping...")
            continue

        # Get fresh details (though 'bd ready' usually has them, 'bd show' is safer for latest state)
        task_details = get_task_details(task_id)
        if not task_details:
            print(f"Could not load details for {task_id}. Skipping.")
            continue

        task_type = task_details.get('issue_type')
        task_title = task_details.get('title')

        print("-" * 80)
        print(f"🚀 Processing {task_id} ({task_type}): {task_title}")
        print("-" * 80)

        # 2. If EPIC or FEATURE, use spec mode to decompose
        if task_type in ["epic", "feature"]:
            print(f"📋 Decomposing {task_type}...")
            prompt = f"""
load and use the spec skill to decompose {task_id} into actionable 
tasks. Ensure each task has clear acceptance criteria and verification instructions.
Focus on breaking it down into items that can be implemented in single sittings.
"""
            success = call_gemini(prompt, allowed_tools=["all"])
            
            # Continue loop to pick up new children
            continue

        # 3. Execute work via Gemini CLI in Exec mode
        # (Using specific instructions from the original script)
        prompt = f"""
load and use the exec skill to perform: exec {task_id} --instructions '
    Activate skill-exec. Perform the requested work for {task_id}.
    
    CRITICAL QUALITY CHECKS:
    1. FILE SIZE: If any file you modify or create exceeds 600 lines, you MUST decompose it into smaller, logical modules or components. After decomposition, verify that the logic is preserved and there are no regressions.
    2. COMPILATION: Confirm the code compiles cleanly (e.g., npm run build, tsc, or equivalent).
    3. INSPECTION: Perform a thorough code inspection to confirm ALL acceptance criteria for {task_id} are met.
    
    FINALIZATION:
    - Commit your changes using conventional commit messages referencing {task_id}.
    - Close the bead using: bd close {task_id} --reason "Completed implementation, verified ACs, and checked file bounds."
'
"""
        call_gemini(prompt, allowed_tools=["all"])

        # Check if the task was successfully closed
        # We re-fetch details to check status
        final_task_details = get_task_details(task_id)
        if not final_task_details:
             print(f"⚠️  Could not verify status of {task_id}.")
             sys.exit(1)

        final_status = final_task_details.get('status')
        if final_status != "closed":
            print(f"⚠️  Task {task_id} was not closed by the agent. Halting for manual review.")
            sys.exit(1)

        print(f"✅ Successfully processed {task_id}.\n")

if __name__ == "__main__":
    main()
