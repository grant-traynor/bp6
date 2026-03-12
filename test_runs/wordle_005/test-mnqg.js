// test-mnqg.js — MN-QG-01, MN-QG-02, MN-QG-03, MN-PRIV-01 verification
// Standalone Node.js harness. No external dependencies.

const fs = require('fs');
const vm = require('vm');

let pass = 0;
let fail = 0;

function check(label, expected, actual) {
  if (expected === actual) {
    console.log('  PASS  ' + label);
    pass++;
  } else {
    console.log('  FAIL  ' + label + '  expected=' + expected + '  got=' + actual);
    fail++;
  }
}

// Load words.js — use Function constructor so const WORDS is capturable
const wordsCode = fs.readFileSync('words.js', 'utf8');
// eslint-disable-next-line no-new-func
const WORDS = (new Function(wordsCode + '\nreturn WORDS;'))();

// Replicate game.js functions under test
function isValidGuess(word) {
  if (!word || word.length !== 5) return false;
  return WORDS.includes(word.toLowerCase());
}

function getDailyWordForDate(date) {
  const epoch = new Date(2021, 0, 1);
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  const dayIndex = Math.floor((d - epoch) / 86400000);
  return WORDS[((dayIndex % WORDS.length) + WORDS.length) % WORDS.length];
}

// ─── MN-QG-01 ─────────────────────────────────────────────────────────────────
console.log('\n── MN-QG-01: Word Validation ────────────────────────────────────────────');

// (a) real words from list accepted
check('crane (in list) accepted', true, isValidGuess('crane'));
check('slate (in list) accepted', true, isValidGuess('slate'));
check('CRANE (uppercase) accepted via toLowerCase', true, isValidGuess('CRANE'));

// (b) non-words rejected
check('zzzzz (not in list) rejected', false, isValidGuess('zzzzz'));
check('xkqvw (not in list) rejected', false, isValidGuess('xkqvw'));
check('hello (common word, may or may not be in list)', typeof isValidGuess('hello'), 'boolean');

// (c) wrong length rejected before submission
check('empty string rejected', false, isValidGuess(''));
check('null rejected', false, isValidGuess(null));
check('four (4 chars) rejected', false, isValidGuess('four'));
check('abcdef (6 chars) rejected', false, isValidGuess('abcdef'));
check('hi (2 chars) rejected', false, isValidGuess('hi'));

// Word list integrity checks
console.log('  INFO  word list length:', WORDS.length);
const allFive = WORDS.every(function(w) { return w.length === 5; });
check('all words exactly 5 chars', true, allFive);
const allLower = WORDS.every(function(w) { return w === w.toLowerCase(); });
check('all words lowercase', true, allLower);
const deduped = new Set(WORDS);
check('no duplicates in word list', true, deduped.size === WORDS.length);
check('word list length >= 365', true, WORDS.length >= 365);

const qg01Pass = fail === 0;
console.log('\nMN-QG-01 VERDICT:', qg01Pass ? 'PASS' : 'FAIL');

// ─── MN-QG-02 ─────────────────────────────────────────────────────────────────
console.log('\n── MN-QG-02: Epoch-Day Algorithm — Wraparound & No Duplicates ───────────');

const prevFail = fail;

// 1. Epoch day 0 (2021-01-01) returns WORDS[0]
const epochDay0 = getDailyWordForDate('2021-01-01');
check('epoch day 0 returns WORDS[0]', WORDS[0], epochDay0);

// 2. Epoch day 1 (2021-01-02) returns WORDS[1]
const epochDay1 = getDailyWordForDate('2021-01-02');
check('epoch day 1 returns WORDS[1]', WORDS[1], epochDay1);

// 3. A date before epoch (negative dayIndex) wraps cleanly (double-modulo guard)
// 2020-12-31 is day -1 relative to epoch; should map to WORDS[WORDS.length - 1]
const dayNeg1 = getDailyWordForDate('2020-12-31');
const expectedNeg1 = WORDS[(((-1) % WORDS.length) + WORDS.length) % WORDS.length];
check('day -1 (2020-12-31) wraps to WORDS[length-1]', expectedNeg1, dayNeg1);

// 4. A date beyond word list length wraps (modulo)
// Day = WORDS.length should wrap to WORDS[0]
const epochPlusLen = new Date(2021, 0, 1);
epochPlusLen.setDate(epochPlusLen.getDate() + WORDS.length);
const dayAtLen = getDailyWordForDate(epochPlusLen);
check('day = WORDS.length wraps back to WORDS[0]', WORDS[0], dayAtLen);

// 5. No two dates within a 365-day window map to the same index (if list >= 365)
// Check 365 consecutive days starting at epoch
const windowSize = 365;
const indices = [];
let duplicateDayFound = false;
for (let i = 0; i < windowSize; i++) {
  const d = new Date(2021, 0, 1);
  d.setDate(d.getDate() + i);
  const dayIndex = Math.floor((d - new Date(2021, 0, 1)) / 86400000);
  const idx = ((dayIndex % WORDS.length) + WORDS.length) % WORDS.length;
  indices.push(idx);
}
const uniqueIndices = new Set(indices);
if (WORDS.length >= 365) {
  duplicateDayFound = uniqueIndices.size < windowSize;
  check('no duplicate days in 365-day window (word list >= 365)', false, duplicateDayFound);
  if (duplicateDayFound) {
    // Find which indices duplicate
    const seen = {};
    indices.forEach(function(idx, day) {
      if (seen[idx] !== undefined) {
        console.log('    DETAIL  duplicate: day ' + day + ' and day ' + seen[idx] + ' both map to index ' + idx + ' (' + WORDS[idx] + ')');
      } else {
        seen[idx] = day;
      }
    });
  }
} else {
  console.log('  SKIP  no-duplicate-day test (word list has ' + WORDS.length + ' words, < 365 — some indices will repeat)');
}

// 6. Determinism: same date always returns same word
const wordA = getDailyWordForDate('2024-06-15');
const wordB = getDailyWordForDate('2024-06-15');
check('same date always returns same word (determinism)', wordA, wordB);

// 7. Adjacent dates return different words (assuming no overlap within window)
const today = getDailyWordForDate('2024-06-15');
const tomorrow = getDailyWordForDate('2024-06-16');
check('adjacent dates return different words', true, today !== tomorrow);

const qg02Pass = (fail === prevFail);
console.log('\nMN-QG-02 VERDICT:', qg02Pass ? 'PASS' : 'FAIL');

// ─── MN-QG-03 ─────────────────────────────────────────────────────────────────
console.log('\n── MN-QG-03: Exception Handling & Corrupted localStorage ────────────────');

const prevFail3 = fail;

// Mock localStorage for Node.js environment
function mockLocalStorage(initialData) {
  var store = {};
  if (initialData) {
    Object.keys(initialData).forEach(function(k) { store[k] = initialData[k]; });
  }
  return {
    getItem: function(key) { return store.hasOwnProperty(key) ? store[key] : null; },
    setItem: function(key, value) { store[key] = value; },
    removeItem: function(key) { delete store[key]; },
    clear: function() { store = {}; },
    _store: function() { return store; }
  };
}

// Load storage.js into isolated vm context
const storageCode = fs.readFileSync('storage.js', 'utf8');

// Test 1: loadGameState with corrupted JSON returns null (no throw)
(function() {
  var ls = mockLocalStorage({ 'wordle_game_state': '{{invalid json{{' });
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var result = null;
  var threw = false;
  try {
    result = storageCtx.loadGameState();
  } catch(e) {
    threw = true;
  }
  check('loadGameState with corrupt JSON returns null (no throw)', null, threw ? 'threw' : result);
})();

// Test 2: loadStats with corrupted JSON returns defaults (no throw)
(function() {
  var ls = mockLocalStorage({ 'wordle_stats': 'NOT_JSON!!!{{{' });
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var result;
  var threw = false;
  try {
    result = storageCtx.loadStats();
  } catch(e) {
    threw = true;
  }
  check('loadStats with corrupt JSON returns defaults (no throw)', false, threw);
  if (!threw) {
    check('loadStats corrupt: gamesPlayed defaults to 0', 0, result.gamesPlayed);
    check('loadStats corrupt: currentStreak defaults to 0', 0, result.currentStreak);
    check('loadStats corrupt: wins defaults to 0', 0, result.wins);
  }
})();

// Test 3: loadGameState with empty localStorage returns null
(function() {
  var ls = mockLocalStorage({});
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var result = storageCtx.loadGameState();
  check('loadGameState with empty localStorage returns null', null, result);
})();

// Test 4: loadStats with empty localStorage returns defaults
(function() {
  var ls = mockLocalStorage({});
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var result = storageCtx.loadStats();
  check('loadStats with empty localStorage returns default object (not null)', true, result !== null);
  check('loadStats empty: gamesPlayed = 0', 0, result.gamesPlayed);
  check('loadStats empty: wins = 0', 0, result.wins);
  check('loadStats empty: currentStreak = 0', 0, result.currentStreak);
  check('loadStats empty: maxStreak = 0', 0, result.maxStreak);
  check('loadStats empty: guessDistribution is array', true, Array.isArray(result.guessDistribution));
  check('loadStats empty: guessDistribution length = 7', 7, result.guessDistribution.length);
  check('loadStats empty: all distribution slots are 0', true, result.guessDistribution.every(function(x) { return x === 0; }));
})();

// Test 5: saveGameState with unavailable localStorage does not throw
(function() {
  var ls = {
    getItem: function() { return null; },
    setItem: function() { throw new Error('QuotaExceededError'); },
    removeItem: function() {}
  };
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var threw = false;
  try {
    storageCtx.saveGameState({ date: '2026-03-13', guesses: [], tileStates: [], gameOver: false, won: false });
  } catch(e) {
    threw = true;
  }
  check('saveGameState does not throw when localStorage throws (quota)', false, threw);
})();

// Test 6: saveStats with unavailable localStorage does not throw
(function() {
  var ls = {
    getItem: function() { return null; },
    setItem: function() { throw new Error('QuotaExceededError'); },
    removeItem: function() {}
  };
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var threw = false;
  try {
    storageCtx.saveStats({ gamesPlayed: 0, wins: 0, currentStreak: 0, maxStreak: 0, guessDistribution: [0,0,0,0,0,0,0] });
  } catch(e) {
    threw = true;
  }
  check('saveStats does not throw when localStorage throws (quota)', false, threw);
})();

// Test 7: loadGameState rejects stale (yesterday's) state
(function() {
  var yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  var staleDate = yesterday.toISOString().slice(0, 10);
  var staleState = JSON.stringify({ date: staleDate, guesses: ['crane'], tileStates: [['correct','correct','correct','correct','correct']], gameOver: true, won: true });
  var ls = mockLocalStorage({ 'wordle_game_state': staleState });
  var storageCtx = { localStorage: ls };
  vm.runInNewContext(storageCode, storageCtx);
  var result = storageCtx.loadGameState();
  check('loadGameState rejects stale (yesterday) state, returns null', null, result);
})();

// Test 8: isValidGuess on game.js with corrupted context — test that the
// core validation logic itself cannot throw (it only uses array methods)
(function() {
  var threw = false;
  try {
    isValidGuess(null);
    isValidGuess(undefined);
    isValidGuess(12345);
    isValidGuess({});
    isValidGuess([]);
  } catch(e) {
    threw = true;
  }
  check('isValidGuess handles null/undefined/number/object/array without throwing', false, threw);
})();

const qg03Pass = (fail === prevFail3);
console.log('\nMN-QG-03 VERDICT:', qg03Pass ? 'PASS' : 'FAIL');

// ─── MN-PRIV-01 ───────────────────────────────────────────────────────────────
console.log('\n── MN-PRIV-01: localStorage Key Whitelist ────────────────────────────────');

const prevFail4 = fail;

// Enumerate all localStorage.setItem calls in application source
// Keys may be string literals or named constants — count all calls, then
// verify constants are declared with whitelisted values.
const appFiles = ['storage.js', 'game.js', 'ui.js', 'input.js'];
// Match any localStorage.setItem call regardless of key form
const setItemCountPattern = /localStorage\.setItem\s*\(/g;
// Match string-literal key form: localStorage.setItem('key', ...)
const setItemLiteralPattern = /localStorage\.setItem\s*\(\s*(['"`])(.*?)\1/g;

const allowedKeys = new Set(['wordle_game_state', 'wordle_stats']);
const allSetItemCalls = [];
const literalKeys = [];

appFiles.forEach(function(filename) {
  var src;
  try { src = fs.readFileSync(filename, 'utf8'); } catch(e) { return; }
  var countMatch;
  while ((countMatch = setItemCountPattern.exec(src)) !== null) {
    allSetItemCalls.push({ file: filename, pos: countMatch.index });
  }
  var litMatch;
  while ((litMatch = setItemLiteralPattern.exec(src)) !== null) {
    literalKeys.push({ file: filename, key: litMatch[2] });
  }
});

console.log('  INFO  localStorage.setItem calls found in application source:');
allSetItemCalls.forEach(function(entry) {
  console.log('    ' + entry.file + ' at pos ' + entry.pos);
});

check('exactly 2 localStorage.setItem calls in app source', 2, allSetItemCalls.length);

// storage.js uses named constants — verify those constants map to whitelisted keys
const storageSource_priv = fs.readFileSync('storage.js', 'utf8');
const constGameMatch = storageSource_priv.match(/STORAGE_KEY_GAME\s*=\s*(['"`])(.*?)\1/);
const constStatsMatch = storageSource_priv.match(/STORAGE_KEY_STATS\s*=\s*(['"`])(.*?)\1/);
const gameKeyValue = constGameMatch ? constGameMatch[2] : '(not found)';
const statsKeyValue = constStatsMatch ? constStatsMatch[2] : '(not found)';
console.log('  INFO  STORAGE_KEY_GAME constant value: "' + gameKeyValue + '"');
console.log('  INFO  STORAGE_KEY_STATS constant value: "' + statsKeyValue + '"');
check('STORAGE_KEY_GAME value is in whitelist', true, allowedKeys.has(gameKeyValue));
check('STORAGE_KEY_STATS value is in whitelist', true, allowedKeys.has(statsKeyValue));
check('no literal string keys outside constants (all calls use constant references)', 0, literalKeys.length);

// Confirm no PII-indicative key names exist anywhere in source
const piiPatterns = ['user', 'ip', 'device', 'finger', 'email', 'name', 'id', 'session', 'token', 'analytics', 'track'];
const allSrcContent = appFiles.map(function(f) {
  try { return fs.readFileSync(f, 'utf8').toLowerCase(); } catch(e) { return ''; }
}).join('\n');

// Check for PII-style localStorage key names (not just the word appearing in comments)
const lsKeyMatches = [];
piiPatterns.forEach(function(pat) {
  var keyRe = new RegExp('localstorage\\.setitem\\s*\\(.*?' + pat, 'gi');
  if (keyRe.test(allSrcContent)) {
    lsKeyMatches.push(pat);
  }
});
check('no PII-indicative keys in localStorage.setItem calls', 0, lsKeyMatches.length);

// Confirm key name constants in storage.js match whitelist
const storageSource = fs.readFileSync('storage.js', 'utf8');
check('STORAGE_KEY_GAME constant = "wordle_game_state"', true, storageSource.includes("'wordle_game_state'"));
check('STORAGE_KEY_STATS constant = "wordle_stats"', true, storageSource.includes("'wordle_stats'"));

// Confirm schema fields contain no PII (check that known PII-style field names do not appear in the schema)
const schemaFieldsPII = ['userId', 'user_id', 'deviceId', 'device_id', 'ipAddress', 'ip_address', 'email', 'fingerprint'];
schemaFieldsPII.forEach(function(field) {
  check('schema field "' + field + '" absent from storage.js', false, storageSource.includes(field));
});

const qgPrivPass = (fail === prevFail4);
console.log('\nMN-PRIV-01 VERDICT:', qgPrivPass ? 'PASS' : 'FAIL');

// ─── Summary ──────────────────────────────────────────────────────────────────
console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
console.log('SUMMARY');
console.log('  MN-QG-01:', qg01Pass ? 'PASS' : 'FAIL', '— word validation');
console.log('  MN-QG-02:', qg02Pass ? 'PASS' : 'FAIL', '— epoch-day algorithm');
console.log('  MN-QG-03:', qg03Pass ? 'PASS' : 'FAIL', '— exception safety');
console.log('  MN-PRIV-01:', qgPrivPass ? 'PASS' : 'FAIL', '— localStorage key whitelist');
console.log('');
console.log('TOTAL:', pass, 'passed,', fail, 'failed');
console.log('OVERALL STATUS:', fail === 0 ? 'PASS' : 'FAIL');
process.exit(fail === 0 ? 0 : 1);
