# Must-Nots: Wordle Clone

## Overview

This document defines **14 hard constraints** across 7 categories that the Wordle Clone system must never violate. These are non-negotiable prohibitions — not preferences or best practices. Any implementation that violates a must-not is considered broken by definition.

**Authority**: Derived from `conops.md` (v1). Constraints marked `[DEFAULT: rationale]` were inferred from domain norms where the CONOPS was silent; all others are explicitly specified or directly implied by the CONOPS.

Implementation agents must treat this as a checklist: if the task being executed would violate a must-not, stop and escalate.

---

## 1. Security Must-Nots

**MN-SEC-01**

MUST NOT make any network requests at runtime (XHR, fetch, WebSocket, or any HTTP/S call).

Rationale: The system is defined as fully client-side with zero backend. Any outbound network call violates the architecture and introduces an attack surface that the design explicitly eliminates.

Verification: Open browser DevTools → Network tab → play a full game. Zero requests must appear beyond the initial file load.

---

**MN-SEC-02**

MUST NOT load external scripts, stylesheets, or resources from CDNs or third-party domains.

Rationale: External resource loading introduces supply-chain risk, Content Security Policy violations, and network dependency — all inconsistent with a zero-dependency, offline-capable design.

Verification: Inspect HTML source. All `<script>`, `<link>`, and `<img>` tags must reference only local or inline resources. No `https://` URLs in resource references.

---

**MN-SEC-03** [DEFAULT: standard browser security practice]

MUST NOT use `eval()`, `Function()` constructor, or `innerHTML` assignments with user-controlled input.

Rationale: These patterns create XSS vectors. Even though this app has no authentication or valuable data, injecting malicious scripts could harm users on shared or untrusted pages, and violates minimum-security obligations for any web application.

Verification: Search source for `eval(`, `new Function(`, and `innerHTML =`. Any assignment to `innerHTML` must use only hard-coded or system-generated strings, never user keyboard input.

---

## 2. Data Privacy Must-Nots

**MN-PRIV-01**

MUST NOT persist any user data to localStorage, sessionStorage, cookies, or IndexedDB.

Rationale: Explicitly specified in CONOPS §8 constraint 4: "No persistent state." Retaining user gameplay data without consent would violate the stated design contract and user expectations.

Verification: Open browser DevTools → Application tab → Storage section. After playing a complete game, all storage categories must be empty (no keys set by the application).

---

**MN-PRIV-02** [DEFAULT: no-tracking baseline for user-facing web apps]

MUST NOT embed analytics, telemetry, or tracking code (including pixel trackers, beacons, or fingerprinting).

Rationale: The CONOPS specifies no external integrations and no statistics tracking. Any telemetry collection would constitute unsolicited data collection from users who have not been informed or given consent.

Verification: Inspect source for analytics library imports or beacon calls (`navigator.sendBeacon`, image pixels with tracking URLs). None must be present.

---

**MN-PRIV-03** [DEFAULT: no-logging baseline for client-side apps]

MUST NOT log player guesses or gameplay data to any external endpoint or console in production builds.

Rationale: Player input (typed words) is user-generated data. Transmitting or logging it to any external system without disclosure violates user privacy expectations, even in a casual game context.

Verification: Review all `fetch`, `XMLHttpRequest`, and `console` calls. No player input (guess strings) may appear as arguments to any external call.

---

## 3. User Trust Must-Nots

**MN-TRUST-01**

MUST NOT consume an attempt when the player submits an invalid word (not in dictionary or fewer than 5 letters).

Rationale: Explicitly specified in CONOPS §5 error states. Consuming an attempt for an invalid guess would punish players for dictionary gaps or typos — a breach of the stated game contract.

Verification: Submit a non-dictionary word and a 3-letter word. The attempt counter must not increment and no row must be committed.

---

**MN-TRUST-02**

MUST NOT reveal the target word before the game is over (win or loss).

Rationale: Exposing the answer mid-game — in source, DOM attributes, console output, or network traffic — defeats the core game mechanic and breaks user trust.

Verification: (a) Play an in-progress game and inspect the DOM for the target word in any attribute or text node. (b) Check browser console for any logged target word. The answer must not appear until the game-over state is rendered.

---

**MN-TRUST-03**

MUST NOT change the target word mid-game (after the first guess has been submitted).

Rationale: Changing the target word after play has begun is deceptive and makes the game unwinnable by design. This violates the implicit contract of any fair game.

Verification: Log the target word at game start (or inspect state), submit two guesses, then verify the target word is identical.

---

## 4. Architecture Must-Nots

**MN-ARCH-01**

MUST NOT use any JavaScript frameworks, libraries, or build tools (React, Vue, Angular, jQuery, Webpack, Vite, etc.).

Rationale: Explicitly specified in CONOPS §8 constraint 1: "Vanilla JavaScript — no frameworks or build tools." Introducing dependencies violates the zero-dependency design and breaks the offline/file-load capability.

Verification: Inspect `package.json` — it must not exist or must have no runtime dependencies. Inspect HTML for framework script imports. None permitted.

---

**MN-ARCH-02**

MUST NOT require a build step to produce a runnable application.

Rationale: Directly implied by CONOPS §8 constraint 3 (single HTML file or minimal file set) and §2 objective 1 (playable in any browser with zero dependencies). If the game cannot be opened by double-clicking the HTML file, the architecture constraint is violated.

Verification: Take the output files, open `index.html` directly in a browser via `file://` protocol (no local server). The game must be fully playable.

---

**MN-ARCH-03**

MUST NOT introduce a backend server, API endpoint, or server-side logic of any kind.

Rationale: Explicitly specified in CONOPS §8 (backend server is out of scope) and §4 (fully client-side). Any server component violates the architecture and adds operational burden the project has explicitly rejected.

Verification: The deliverable must consist solely of static files. No `server.js`, `app.py`, `Dockerfile`, or equivalent must be required to operate the game.

---

## 5. Scope Must-Nots

**MN-SCOPE-01**

MUST NOT implement user accounts, authentication, or any login mechanism.

Rationale: Explicitly out of scope in CONOPS §9 item 1. Implementing auth adds complexity, security obligations, and data handling requirements that are entirely outside the project's stated purpose.

Verification: Inspect HTML and JS. No login forms, auth tokens, session management, or user identity concepts must exist.

---

**MN-SCOPE-02**

MUST NOT implement a daily-word mode, synchronized word schedules, or shared target words across sessions.

Rationale: Explicitly out of scope in CONOPS §9 item 3. Daily synchronization requires either a backend or device clock-based coordination, both of which contradict the stateless, random-word design.

Verification: Reload the page multiple times. Each load must select a new random word (verify by winning or losing, then reloading and playing again). No date-based word selection logic must exist.

---

## 6. Quality Gate Must-Nots

**MN-QG-01**

MUST NOT ship with any JavaScript error that prevents game start or gameplay completion in Chrome, Firefox, Safari, or Edge.

Rationale: CONOPS §7 specifies compatibility across all four major browsers. An unhandled JS exception that breaks the game loop is a functional defect, not an acceptable edge case.

Verification: Open browser console in each target browser. Load the page and complete a full game (win and lose path). Zero uncaught errors must appear.

---

**MN-QG-02** [DEFAULT: correctness gate for game-logic implementations]

MUST NOT ship with incorrect color-coding logic (e.g., double-letter handling errors, green overriding yellow incorrectly).

Rationale: Color feedback is the core mechanic. Incorrect feedback makes the game unplayable and misleading. Standard Wordle rules for duplicate letters must be implemented correctly: greens are resolved first, then yellows are allocated to remaining unmatched letters.

Verification: Test the following cases: (a) guess with a repeated letter where only one instance is in the word — only one tile should be yellow/green; (b) guess where a letter appears in the correct position and also elsewhere — the correct-position tile is green, the other is gray.

---

## 7. Open Questions

None. The CONOPS declares scope intentionally minimal with no open questions. All must-nots derived from CONOPS constraints are marked as specified; inferred defaults are marked `[DEFAULT: rationale]` above.

---

*Generated by Must-Not Analyst — Guardrails stage. Authority: conops.md. Date: 2026-03-12.*
