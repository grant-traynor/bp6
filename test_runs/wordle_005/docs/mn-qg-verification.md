# MN-QG-01, MN-QG-02, MN-QG-03, MN-PRIV-01 Verification Report

**Date**: 2026-03-13
**Task**: Execute MN-QG-01 through MN-QG-03 and MN-PRIV-01 verification procedures
**Test script**: `test-mnqg.js` (Node.js, no external dependencies)
**Verdict**: ALL PASS — no violations found

---

## MN-QG-01 — Word Validation

**Procedure**: Verify (a) a real word from the word list is accepted, (b) a 5-character non-word string is rejected, (c) wrong-length strings are rejected before submission.

**Evidence** (from `test-mnqg.js`, 15 assertions):

```
PASS  crane (in list) accepted
PASS  slate (in list) accepted
PASS  CRANE (uppercase) accepted via toLowerCase
PASS  zzzzz (not in list) rejected
PASS  xkqvw (not in list) rejected
PASS  empty string rejected
PASS  null rejected
PASS  four (4 chars) rejected
PASS  abcdef (6 chars) rejected
PASS  hi (2 chars) rejected
PASS  all words exactly 5 chars
PASS  all words lowercase
PASS  no duplicates in word list
PASS  word list length >= 365
INFO  word list length: 1211
```

**Code path** (`game.js:6–9`):
```javascript
function isValidGuess(word) {
  if (!word || word.length !== 5) return false;
  return WORDS.includes(word.toLowerCase());
}
```

The guard `!word` short-circuits for `null`/`undefined`/empty. Length check rejects non-5-char inputs. `toLowerCase()` normalises case before the list lookup.

**Shake animation** (from prior browser smoke test `smoke-test.js:16/16`): `shakeRow()` is invoked on both "not enough letters" and "not in word list" paths (`game.js:176, 184`). Confirmed via Playwright headless test.

**Verdict**: PASS

---

## MN-QG-02 — Epoch-Day Algorithm Correctness

**Procedure**: Verify the epoch-day index wraps cleanly (modulo word list length). Confirm no two dates within a 365-day window map to the same word index. Test epoch day 0, day 1, day = WORDS.length, and day −1 (pre-epoch).

**Evidence** (from `test-mnqg.js`, 7 assertions):

```
PASS  epoch day 0 returns WORDS[0]
PASS  epoch day 1 returns WORDS[1]
PASS  day -1 (2020-12-31) wraps to WORDS[length-1]
PASS  day = WORDS.length wraps back to WORDS[0]
PASS  no duplicate days in 365-day window (word list >= 365)
PASS  same date always returns same word (determinism)
PASS  adjacent dates return different words
```

**Code path** (`game.js:14–21`):
```javascript
function getDailyWord() {
  const epoch = new Date(2021, 0, 1); // fixed reference: 2021-01-01
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const dayIndex = Math.floor((today - epoch) / 86400000);
  // Double-modulo guard: ensures non-negative index for any dayIndex value
  return WORDS[((dayIndex % WORDS.length) + WORDS.length) % WORDS.length];
}
```

The double-modulo guard `((n % len) + len) % len` correctly handles negative `dayIndex` values (pre-epoch dates) that would otherwise produce negative array indices. With 1211 words and a 365-day window, no collision is possible (1211 > 365, all indices are unique within the window).

**Bounds analysis**:
- Minimum dayIndex (pre-epoch): wraps to a valid positive index
- Maximum dayIndex (far future, e.g., year 2050): dayIndex ≈ 10,585 → 10,585 % 1211 = 551 (valid, in-bounds)
- No `undefined` behaviour for any date input

**Verdict**: PASS

---

## MN-QG-03 — Exception Safety & Corrupted localStorage Handling

**Procedure**: Confirm no uncaught exceptions during a full play session; confirm that if localStorage is empty or malformed the game initialises cleanly to defaults rather than crashing.

**Evidence** (from `test-mnqg.js`, 18 assertions):

```
PASS  loadGameState with corrupt JSON returns null (no throw)
PASS  loadStats with corrupt JSON returns defaults (no throw)
PASS  loadStats corrupt: gamesPlayed defaults to 0
PASS  loadStats corrupt: currentStreak defaults to 0
PASS  loadStats corrupt: wins defaults to 0
PASS  loadGameState with empty localStorage returns null
PASS  loadStats with empty localStorage returns default object (not null)
PASS  loadStats empty: gamesPlayed = 0
PASS  loadStats empty: wins = 0
PASS  loadStats empty: currentStreak = 0
PASS  loadStats empty: maxStreak = 0
PASS  loadStats empty: guessDistribution is array
PASS  loadStats empty: guessDistribution length = 7
PASS  loadStats empty: all distribution slots are 0
PASS  saveGameState does not throw when localStorage throws (quota)
PASS  saveStats does not throw when localStorage throws (quota)
PASS  loadGameState rejects stale (yesterday) state, returns null
PASS  isValidGuess handles null/undefined/number/object/array without throwing
```

**Code paths verified**:
- `storage.js:47–60` — `loadGameState` wraps `JSON.parse` in `try/catch`; returns `null` on any error
- `storage.js:99–108` — `loadStats` wraps `JSON.parse` in `try/catch`; returns `_defaultStats()` on error
- `storage.js:31–37` — `saveGameState` wraps `localStorage.setItem` in `try/catch`; silently fails on QuotaExceededError
- `storage.js:117–123` — `saveStats` wraps `localStorage.setItem` in `try/catch`; silently fails on QuotaExceededError
- `storage.js:53–54` — stale state guard rejects any saved state not matching today's ISO date

**Browser session coverage** (from prior Playwright smoke test `smoke-test.js:16/16`, `test-keyboard-a11y.js:31/31`): Full play session — valid guess, invalid guess, win, lose, stats modal — produced zero console errors in all scenarios.

**Verdict**: PASS

---

## MN-PRIV-01 — localStorage Key Whitelist

**Procedure**: Enumerate all `localStorage.setItem` calls in application source. Confirm each key stores only tile state, guess strings, and numeric statistics. No PII, no user identifiers.

**Evidence** (from `test-mnqg.js`, 14 assertions):

```
INFO  localStorage.setItem calls found in application source:
    storage.js at pos 1613
    storage.js at pos 4344
PASS  exactly 2 localStorage.setItem calls in app source
INFO  STORAGE_KEY_GAME constant value: "wordle_game_state"
INFO  STORAGE_KEY_STATS constant value: "wordle_stats"
PASS  STORAGE_KEY_GAME value is in whitelist
PASS  STORAGE_KEY_STATS value is in whitelist
PASS  no literal string keys outside constants (all calls use constant references)
PASS  no PII-indicative keys in localStorage.setItem calls
PASS  STORAGE_KEY_GAME constant = "wordle_game_state"
PASS  STORAGE_KEY_STATS constant = "wordle_stats"
PASS  schema field "userId" absent from storage.js
PASS  schema field "user_id" absent from storage.js
PASS  schema field "deviceId" absent from storage.js
PASS  schema field "device_id" absent from storage.js
PASS  schema field "ipAddress" absent from storage.js
PASS  schema field "ip_address" absent from storage.js
PASS  schema field "email" absent from storage.js
PASS  schema field "fingerprint" absent from storage.js
```

**Whitelisted keys and schemas** (from `storage.js`):

| Key | Constant | Schema Fields | PII? |
|-----|----------|--------------|------|
| `wordle_game_state` | `STORAGE_KEY_GAME` | `date`, `guesses`, `tileStates`, `gameOver`, `won` | No |
| `wordle_stats` | `STORAGE_KEY_STATS` | `gamesPlayed`, `wins`, `currentStreak`, `maxStreak`, `guessDistribution` | No |

Both keys use named constants (no magic strings). Both values are structured game/stats data only. No usernames, IPs, device identifiers, session tokens, or behavioural analytics of any kind.

**Also confirmed by MN-ARCH-05 verification** (prior task, `docs/mn-arch-verification.md`): same two calls identified, same PII-free conclusion.

**Verdict**: PASS

---

## Summary

| Constraint | Procedure | Result | Assertions |
|------------|-----------|--------|------------|
| MN-QG-01 | Word validation (accept valid, reject non-word, reject wrong length) | **PASS** | 15/15 |
| MN-QG-02 | Epoch-day wraparound, no duplicates in 365-day window | **PASS** | 7/7 |
| MN-QG-03 | Exception safety, corrupted localStorage initialises to defaults | **PASS** | 18/18 |
| MN-PRIV-01 | localStorage key whitelist, no PII fields | **PASS** | 15/15 |

**Total**: 55 passed, 0 failed

**Overall**: All 4 constraints satisfied. No violations.
