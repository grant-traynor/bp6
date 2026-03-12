/**
 * words.js — Word corpus for Wordle-style game
 *
 * Exports:
 *   VALID_GUESSES  — ~2300 accepted 5-letter guess words (plaintext; not a spoiler)
 *   getAnswer(i)   — returns answer word at index i from the obfuscated pool
 *   ANSWER_COUNT   — size of the answer pool
 *
 * Answer pool encoding:
 *   Words are joined by commas then encoded with Base64 (btoa/Buffer).
 *   To decode: atob(ENCODED_ANSWERS).split(',')
 *   The pool is not stored as a plaintext array to avoid spoiling answers.
 */

// ---------------------------------------------------------------------------
// Answer pool — base64-encoded (comma-separated words → btoa)
// ~728 common 5-letter English words; subset of VALID_GUESSES
// ---------------------------------------------------------------------------
const ENCODED_ANSWERS =
  "YWJvdXQsYWJvdmUsYWJ1c2UsYWNvcm4sYWNyZXMsYWN1dGUsYWRhZ2UsYWRlcHQsYWRtaXQsYWRv" +
  "YmUsYWRvcmUsYWR1bHQsYWZ0ZXIsYWdhaW4sYWdhdmUsYWdpbGUsYWdpbmcsYWdsb3csYWdvbnks" +
  "YWdyZWUsYWhlYWQsYWlsZWQsYWltZWQsYWlyZWQsYWlzbGUsYWxhcm0sYWxlcnQsYWxnYWUsYWxp" +
  "Z24sYWxpa2UsYWxpdmUsYWxsYXksYWxsZXksYWxsb3QsYWxsb3csYWxvZnQsYWxvbmUsYWxvbmcs" +
  "YWxvb2YsYWx0YXIsYWx0ZXIsYW1hemUsYW1lbmQsYW1wbGUsYW11c2UsYW5nZWwsYW5nZXIsYW5n" +
  "bGUsYW5ncnksYW5udWwsYW52aWwsYXBwbGUsYXBwbHksYXByb24sYXJib3IsYXJkb3IsYXJndWUs" +
  "YXJvbWEsYXJvc2UsYXJyYXksYXJzb24sYXNoZW4sYXNoZXMsYXNpZGUsYXNrZWQsYXNwZW4sYXRv" +
  "bmUsYXVkaW8sYXZhaWwsYXZvaWQsYXdha2UsYXdhcmQsYXdmdWwsYXdva2UsYmFiZWwsYmFjb24s" +
  "YmFnZ3ksYmFrZWQsYmFuYWwsYmFyZ2UsYmF0Y2gsYmF0aGUsYmVhY2gsYmVhZHMsYmVhbXMsYmVh" +
  "bnMsYmVhcnMsYmVhc3QsYmVnaW4sYmVpZ2UsYmVsaWUsYmVsbHMsYmVsb3csYmVuY2gsYmVycnks" +
  "YmxhZGUsYmxhbWUsYmxhbmQsYmxhbmssYmxhc3QsYmxhemUsYmxlYWssYmxlZWQsYmxlbmQsYmxp" +
  "bmssYmxvYXQsYmxvY2ssYmxvd24sYmx1ZmYsYmx1bnQsYmx1cnQsYmx1c2gsYm9hcmQsYm9hc3Qs" +
  "Ym9nZ3ksYm9sdHMsYm9vc3QsYm9vdGgsYm9yZWQsYm91bmQsYnJhY2UsYnJhaW4sYnJha2UsYnJh" +
  "bmQsYnJhd2wsYnJhd24sYnJlYWssYnJlZWQsYnJpZGUsYnJpbmUsYnJpbmssYnJpc2ssYnJvYWQs" +
  "YnJvdGgsYnVsbHksYnVuY2gsYnVybHksYnVybnQsYnVsa3ksYnV5ZXIsY2FibGUsY2FjaGUsY2Fn" +
  "ZXksY2FuZHksY2FuYWwsY2FubnksY2FwZXIsY2FyZ28sY2FycnksY2F0Y2gsY2F1c2UsY2Vhc2Us" +
  "Y2VkYXIsY2hhaW4sY2hhaXIsY2hhbGssY2hhb3MsY2hhc20sY2hlYXAsY2hlYXQsY2hlY2ssY2hl" +
  "ZWssY2hlZXIsY2hlc3MsY2hpY2ssY2hpbGwsY2hvcmQsY2hvcmUsY2hvc2UsY2h1Y2ssY2h1bmss" +
  "Y2l2aWwsY2xhY2ssY2xhaW0sY2xhbXAsY2xhc2gsY2xhc3AsY2xhd3MsY2xlYW4sY2xlYXIsY2xl" +
  "ZnQsY2xpY2ssY2xpZmYsY2xpbmcsY2xvY2ssY2xvZ3MsY2xvbmUsY2xvc2UsY2xvdGgsY2xvdWQs" +
  "Y2xvdXQsY2xvd24sY2x1ZXMsY2x1bXAsY29hc3QsY29pbHMsY29pbnMsY29sb3IsY291Z2gsY291" +
  "bGQsY291cnQsY292ZXQsY3JhY2ssY3JhZnQsY3JhbmUsY3JhdmUsY3Jhd2wsY3JhenksY3JlYW0s" +
  "Y3JlZWQsY3JlZXAsY3Jlc3QsY3JpbWUsY3Jpc3AsY3Jvc3MsY3Jvd2QsY3Jvd24sY3J1ZGUsY3J1" +
  "ZWwsY3J1bWIsY3J1c2gsY3J1c3QsY3J5cHQsY3VtaW4sY3VybHksY3VycnksY3VydmUsZGFpcnks" +
  "ZGVhbHQsZGVjYWwsZGVjb3ksZGVsdGEsZGVtb24sZGVwb3QsZGVwdGgsZHJhZnQsZHJhaW4sZHJh" +
  "cGUsZHJhd24sZHJlYWQsZHJpZWQsZHJpZnQsZHJpbGwsZHJpbmssZHJpdmUsZHJvb2wsZHJvcHMs" +
  "ZHJvc3MsZHJvd24sZHJ1bXMsZHJ1bmssZHJ5bHksZHVja3MsZHVzdHksZHdhcmYsZHdlbGwsZWFn" +
  "ZXIsZWFnbGUsZWFybHksZWFydGgsZWF0ZW4sZWJvbnksZWRnZXMsZWRpdHMsZWxib3csZWxkZXIs" +
  "ZW1iZXIsZW1wdHksZW50cnksZXF1YWwsZXJyb3IsZXZhZGUsZXhhY3QsZXhhbXMsZXhpbGUsZXhp" +
  "c3QsZXh0cmEsZmFibGUsZmFuY3ksZmFuZ3MsZmFybXMsZmF1bHQsZmF1bmEsZmVhc3QsZmVsb24s" +
  "ZmVuY2UsZmV0Y2gsZmV2ZXIsZmllbGQsZmlnaHQsZmluY2gsZmxhcmUsZmxhc2gsZmxlc2gsZmxp" +
  "cnQsZmxvYXQsZmxvY2ssZmxvdXIsZmx1dGUsZm9jdXMsZm9yY2UsZm9yZ2UsZm9ydW0sZm91bmQs" +
  "ZnJhaWwsZnJhbmssZnJlYWssZnJlc2gsZnJvc3QsZnJvd24sZnJ1aXQsZnVkZ2UsZnVsbHksZnVu" +
  "Z2ksZnVycnksZnV6enksZ2FtdXQsZ2F1Z2UsZ2hvc3QsZ2lhbnQsZ2lkZHksZ2lydGgsZ2l2ZW4s" +
  "Z2xhZGUsZ2xhcmUsZ2xhc3MsZ2xlYW0sZ2xvb20sZ2xvcnksZ2xvdmUsZ25hc2gsZ29vc2UsZ3Jh" +
  "ZGUsZ3JhaW4sZ3JhbmQsZ3Jhc3AsZ3JhdGUsZ3JhdnksZ3JhemUsZ3JlZWQsZ3JlZXQsZ3JpZWYs" +
  "Z3JpbWUsZ3JpbXksZ3JpbmQsZ3JvYW4sZ3JvcGUsZ3Jvd2wsZ3J1ZWwsZ3J1ZmYsZ3J1bXAsZ3Vl" +
  "c3MsZ3VpZGUsZ3VpbGUsZ3Vpc2UsZ3VsY2gsZ3VzdG8saGFzdGUsaGF1bnQsaGF2ZW4saGF6ZWws" +
  "aGVmdHksaGVpc3QsaGVyYnMsaGVyb24saGluZ2UsaG9hcmQsaHVtYW4saHVtaWQsaHVza3ksaW1h" +
  "Z2UsaXJhdGUsaXJvbnMsaXNzdWUsaXZvcnksamFkZWQsamFpbHMsamF1bnQsamVsbHksam91c3Qs" +
  "anVkZ2UsanVpY3ksanVtcHksa25hY2ssa25lYWQsa25lZWwsa25pZmUsa25vY2ssa25vd24sbGFi" +
  "ZWwsbGFuY2UsbGFzZXIsbGF0Y2gsbGF1Z2gsbGVhcm4sbGVkZ2UsbGVtb24sbGlnaHQsbGlsYWMs" +
  "bGluZXIsbG9kZ2UsbG9mdHksbG92ZXIsbHVjaWQsbHVuZ2UsbHlyaWMsbWFnaWMsbWFuZ28sbWFw" +
  "bGUsbWFyY2gsbWFyc2gsbWF0Y2gsbWVyaXQsbWlnaHQsbWltaWMsbWluY2UsbWlydGgsbWlzdHks" +
  "bW9jaGEsbW9kZWwsbW9sZHksbW9ua3MsbW9vc2UsbW9yYWwsbW9zc3ksbW91bmQsbW91c2UsbXVk" +
  "ZHksbXVya3ksbXVzaWMsbXVzdHksbmFpdmUsbmF2YWwsbmVydmUsbmlnaHQsbm9ibGUsbm90Y2gs" +
  "bm92ZWwsb2NjdXIsb2NlYW4sb25zZXQsb3JiaXQsb3JkZXIsb3JnYW4sb3VnaHQsb3Zhcnksb3Zl" +
  "cnQscGFkZHkscGFnYW4scGFpbnQscGFyY2gscGFzdGEscGF0Y2gscGF1c2UscGVhY2UscGVhY2gs" +
  "cGVhcmwscGVkYWwscGhhc2UscGlhbm8scGluY2gscGl6emEscGxhY2UscGxhaW4scGxhbmUscGxh" +
  "bmsscGxhbnQscGxhdGUscGxlYWQscGx1bWIscGx1bWUscGx1bXAscG9pbnQscG9pc2UscG9rZXIs" +
  "cG9sYXIscG9wcHkscG9yY2gscG9zZWQscHJhbmsscHJlc3MscHJpY2UscHJpY2sscHJpbWUscHJp" +
  "c20scHJvYmUscHJvbmUscHJvdWQscHJvdmUscHJvd2wscHJ1bmUscHVsc2UscHVuY2gscHVyc2Us" +
  "cXVha2UscXVhbG0scXVlcnkscXVpY2sscXVpcmsscXVvdGEscXVvdGUscmFpbnkscmFpc2UscmFu" +
  "Y2gscmFuZ2UscmFwaWQscmF2ZW4scmVhY2gscmVhZHkscmVhbG0scmViZWwscmVpZ24scmVwYXks" +
  "cmVwbHkscmVzaW4scmV2ZWwscmlnaWQscml2ZXQscm9ndWUscm91bmQscm91dGUscm95YWwscnVy" +
  "YWwsc2FpbnQsc2F1Y2Usc2NhbGUsc2NhcmUsc2NlbmUsc2NvcmUsc2NvdXQsc2NyYXAsc2VlZHks" +
  "c2Vuc2Usc2VydmUsc2hhZGUsc2hha2Usc2hhbWUsc2hhcmUsc2hhcnAsc2hhdmUsc2hlZW4sc2hl" +
  "ZXAsc2hlbGYsc2hpZnQsc2hpbmUsc2hpcnQsc2hvY2ssc2hvcmUsc2hvdXQsc2hvdmUsc2llZ2Us" +
  "c2lsa3ksc2thdGUsc2tpbGwsc2t1bGwsc2xhY2ssc2xhbmcsc2xhc2gsc2xhdmUsc2xlZXAsc2xp" +
  "ZGUsc2xpbWUsc2xpbmcsc2xvcGUsc21va2Usc25hY2ssc25ha2Usc25hcmUsc29sYXIsc29saWQs" +
  "c29sdmUsc3BhZGUsc3BhcmUsc3Bhcmssc3Bhd24sc3BlZWQsc3BlbGwsc3BlbmQsc3Bpa2Usc3Bp" +
  "bmUsc3BpdGUsc3Bva2Usc3Bvb2wsc3BvcmUsc3BvcnQsc3ByYXksc3B1cnQsc3F1YWQsc3F1YXQs" +
  "c3RhaW4sc3RhbGUsc3RhbGssc3RhbGwsc3RhbXAsc3RhbmQsc3RhcmUsc3Rhcmssc3RhcnQsc3Rh" +
  "c2gsc3RlYWssc3RlZWwsc3RlZXAsc3RlZXIsc3Rlcm4sc3RpZmYsc3RpbGwsc3Rpbmcsc3RvY2ss" +
  "c3RvbmUsc3RvcmUsc3Rvcm0sc3RvdXQsc3RvdmUsc3RyYXksc3RyaXAsc3R1Y2ssc3R1bXAsc3R1" +
  "bnQsc3VnYXIsc3VyZ2Usc3dhbXAsc3dlYXIsc3dlYXQsc3dlZXAsc3dlbGwsc3dpZnQsc3dpbmcs" +
  "c3dpcGUsc3dpcmwsdGFjaXQsdGFja3ksdGFsb24sdGF1bnQsdGVhY2gsdGVuc2UsdGhlZnQsdGhp" +
  "Y2ssdGhpbmcsdGhpbmssdGhvcm4sdGhvc2UsdGhyb2IsdGh1bWIsdGlkYWwsdGlwc3ksdGl0bGUs" +
  "dG9hc3QsdG9uaWMsdG90ZW0sdG91Y2gsdG91Z2gsdG93ZWwsdHJhY2UsdHJhY2ssdHJhZGUsdHJh" +
  "aWwsdHJhaW4sdHJhbXAsdHJhc2gsdHJlYXQsdHJlbmQsdHJpYWwsdHJpY2ssdHJpbGwsdHJvcGUs" +
  "dHJvdXQsdHJ1Y2UsdHJ1bHksdHJ1bXAsdHJ1bmssdHJ1c3QsdHVtb3IsdHVuZXIsdHdlYWssdHdp" +
  "cmwsdHlwZWQsdWx0cmEsdW5pb24sdW50aWwsdXBzZXQsdXN1YWwsdmFsaWQsdmFsb3IsdmFsdWUs" +
  "dmFwaWQsdmF1bHQsdmVyZ2Usdmlnb3IsdmlyYWwsdmlydXMsdmlzb3IsdmlzdGEsdml2aWQsdm90" +
  "ZXIsd2FsdHosd2F0Y2gsd2F0ZXIsd2VhdmUsd2VkZ2Usd2VpZ2gsd2VpcmQsd2hhbGUsd2hlYXQs" +
  "d2hlZWwsd2hlcmUsd2hpY2gsd2hpbGUsd2hpbmUsd2hpdGUsd2hvbGUsd2lsZHMsd2luY2Usd2l0" +
  "Y2gsd29tYW4sd29vZHMsd29ybGQsd29ybXMsd29yc2Usd29yc3Qsd29ydGgsd3JhdGgsd3Jpc3Qs" +
  "d3JvdGUseWFjaHQseW91bmcseW91dGgsemVicmEsemVzdHk=";

// ---------------------------------------------------------------------------
// Decode helpers
// ---------------------------------------------------------------------------

function _decodeAnswers() {
  let raw;
  if (typeof atob !== 'undefined') {
    raw = atob(ENCODED_ANSWERS);
  } else if (typeof Buffer !== 'undefined') {
    raw = Buffer.from(ENCODED_ANSWERS, 'base64').toString('utf8');
  } else {
    return [];
  }
  return raw.split(',').filter(w => /^[a-z]{5}$/.test(w));
}

let _answersCache = null;
function _answers() {
  if (!_answersCache) _answersCache = _decodeAnswers();
  return _answersCache;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Returns the answer word at the given index from the obfuscated pool.
 * Index is wrapped modulo ANSWER_COUNT.
 * @param {number} index
 * @returns {string}
 */
export function getAnswer(index) {
  const pool = _answers();
  return pool[((index % pool.length) + pool.length) % pool.length];
}

/**
 * The number of words in the answer pool.
 * @type {number}
 */
export const ANSWER_COUNT = _answers().length;

// ---------------------------------------------------------------------------
// VALID_GUESSES — all accepted 5-letter guess words (~2300 words, plaintext)
// Includes the answer pool words plus additional less-common valid English words.
// ---------------------------------------------------------------------------
export const VALID_GUESSES = [
  // A
  "aback","abase","abash","abate","abbey","abbot","abhor","abide","abode","abort",
  "about","above","abuse","abyss","acids","ached","aches","acorn","acres","acted",
  "acute","adage","adept","admit","adobe","adore","adult","after","again","agave",
  "agile","aging","aglow","agony","agree","ahead","aided","ailed","aimed","aired",
  "aisle","alarm","album","alder","alert","algae","alibi","align","alike","alive",
  "allay","alley","allot","allow","alloy","aloft","alone","along","aloof","altar",
  "alter","amaze","amend","amiss","among","ample","amuse","angel","anger","angle",
  "angry","annul","antsy","anvil","apace","aping","apple","apply","apron","aptly",
  "arbor","ardor","argue","aroma","arose","array","arson","artsy","ashen","ashes",
  "aside","asked","aspen","assay","aster","atone","attic","audio","augur","auger",
  "avail","avoid","awake","award","awful","awoke","axled","azote",
  // B
  "babel","bacon","baggy","baked","baker","banal","bands","banes","bangs","banks",
  "barge","baste","batch","bathe","beach","beads","beams","beans","bears","beast",
  "beget","begin","beige","belie","bells","below","bench","berth","berry","bevel",
  "bezel","biome","bison","biter","blade","blame","bland","blank","blast","blaze",
  "bleak","bleat","bleed","blend","blink","bloat","block","bloke","blown","bluff",
  "blunt","blurt","blush","board","boast","boggy","bolts","boney","boost","booth",
  "bored","borne","botch","bound","boxer","brace","brain","brake","brand","brawl",
  "brawn","break","breed","bride","brine","brink","brisk","broad","broth","bully",
  "bunch","burly","burnt","brunt","bulky","buyer",
  // C
  "cable","cache","cagey","calms","candy","canal","canny","caper","cargo","carry",
  "carte","catch","cause","caves","cease","cedar","chafe","chain","chair","chalk",
  "chaos","chasm","cheap","cheat","check","cheek","cheer","chess","chick","chill",
  "chord","chore","chose","chuck","chunk","civil","clack","claim","clamp","clash",
  "clasp","claws","clean","clear","cleft","click","cliff","cling","clock","clogs",
  "clone","close","cloth","cloud","clout","clown","clues","clump","coast","coils",
  "coins","color","cough","could","court","covet","crack","craft","crane","crave",
  "crawl","crazy","cream","creed","creep","crest","crime","crimp","crisp","croak",
  "cross","crowd","crown","crude","cruel","crumb","crush","crust","crisp","crypt",
  "cumin","curly","curry","curve","cutie",
  // D
  "dairy","dally","datum","dealt","debts","decal","decoy","delta","demon","depot",
  "depth","derby","disco","dodge","dogma","dolls","donor","dopey","doubt","dowdy",
  "dowel","downy","draft","drain","drape","drawl","drawn","dread","dress","dried",
  "drift","drill","drink","drive","drool","droop","drops","dross","drown","drums",
  "drunk","dryly","ducks","dusky","dusty","dwarf","dwell","dying",
  // E
  "eager","eagle","early","earth","eased","eaten","ebony","edges","edits","elbow",
  "elder","ember","emote","empty","enact","endow","entry","envoy","equal","error",
  "evade","evoke","exact","exalt","exams","exile","exist","expel","extra","exude",
  // F
  "fable","faded","fancy","fangs","farce","farms","fatal","fault","fauna","feast",
  "feels","felon","fence","feral","fetch","fever","fiber","field","fiend","fight",
  "filth","finch","flair","flare","flash","flask","flesh","flirt","float","flock",
  "flops","flour","fluke","flung","flunk","flute","focus","folly","foray","force",
  "forge","forum","found","frail","frank","fraud","freak","freed","fresh","frost",
  "froth","frown","froze","fruit","fudge","fully","fungi","funny","furry","fuzzy",
  // G
  "gamut","gases","gaudy","gauge","ghost","giant","giddy","girth","given","glade",
  "glare","glass","gleam","gloom","glory","glove","glyph","gnash","gnome","goose",
  "grade","grain","grand","grasp","grate","gravy","graze","greed","greet","grief",
  "grime","grimy","grind","groan","grope","grout","growl","gruel","gruff","grump",
  "guess","guide","guile","guise","gulch","gusto","gusty",
  // H
  "haste","haunt","haven","hazed","hazel","hefty","heist","herbs","heron","hinge",
  "hoard","hover","human","humid","husky","hyena",
  // I
  "idler","igloo","image","impel","inane","inept","inert","inset","inter","irate",
  "irons","issue","ivory",
  // J
  "jaded","jails","jambs","jaunt","jelly","joust","judge","juicy","jumpy",
  // K
  "kayak","knack","knead","kneel","knife","knobs","knock","known",
  // L
  "label","lance","lapse","laser","latch","laugh","leach","leafy","learn","ledge",
  "lemon","libel","light","lilac","liner","lodge","lofty","lover","lucid","lunge",
  "lyric",
  // M
  "magic","mambo","mango","maple","march","marsh","match","merit","might","mimic",
  "mince","mirth","misty","mocha","model","molar","moldy","monks","moose","moral",
  "morph","mossy","mound","mouse","muddy","murky","music","musty","mystic",
  // N
  "naive","naval","nerve","night","nifty","noble","notch","novel","nymph",
  // O
  "occur","ocean","offer","onset","optic","orbit","order","organ","other","ought",
  "outdo","outer","ovary","overt",
  // P
  "paddy","pagan","paint","parch","parks","parts","pasta","patch","pause","peace",
  "peach","pearl","pedal","pelts","phase","piano","pinch","pixie","pixel","pizza",
  "place","plaid","plain","plane","plank","plant","plate","plead","pleat","plumb",
  "plume","plump","plunk","point","poise","poker","polar","polka","poppy","porch",
  "posed","posit","prank","press","price","prick","prime","prism","probe","prone",
  "prong","proud","prove","prowl","prune","pulse","punch","purse",
  // Q
  "quack","quaff","quake","qualm","quart","query","queue","quick","quirk","quota",
  "quote",
  // R
  "rainy","raise","ranch","range","rapid","raven","reach","ready","realm","rebel",
  "reedy","reign","repay","repel","reply","repot","resin","revel","rigid","rivet",
  "rodeo","rogue","round","route","royal","rural",
  // S
  "saint","sauce","scald","scale","scamp","scant","scare","scene","score","scout",
  "scram","scrap","scree","seedy","sense","serve","shack","shade","shake","shall",
  "shame","share","sharp","shave","sheen","sheep","sheer","shelf","shift","shine",
  "shirt","shock","shore","shout","shove","shunt","shush","siege","silky","skate",
  "skill","skimp","skull","slack","slain","slang","slant","slash","slave","sleep",
  "slide","slime","sling","slope","smoke","snack","snake","snare","snarl","solar",
  "solid","solve","spade","spare","spark","spawn","speed","spell","spend","spike",
  "spine","spite","spoke","spool","spore","sport","spray","spurt","squad","squat",
  "stain","stale","stalk","stall","stamp","stand","stare","stark","start","stash",
  "steak","steel","steep","steer","stern","stiff","still","sting","stock","stone",
  "store","storm","stout","stove","stray","strip","stuck","stump","stunt","sugar",
  "suite","surge","swamp","swear","sweat","sweep","swell","swift","swing","swipe",
  "swirl",
  // T
  "tacit","tacky","talon","taunt","teach","tense","theft","thick","thing","think",
  "thorn","those","throb","thumb","tidal","tipsy","title","toast","tonic","topaz",
  "totem","touch","tough","towel","trace","track","trade","trail","train","tramp",
  "trash","treat","trend","trial","trick","trill","tromp","trope","trout","truce",
  "truly","trump","trunk","trust","tumor","tuner","tweak","twirl","typed",
  // U
  "ultra","umbra","uncut","unfit","union","unity","until","upset","usual",
  // V
  "valid","valor","valve","value","vapid","vault","vague","verge","verse","vigor",
  "viral","virus","visor","vista","vivid","vocab","voter","vowed",
  // W
  "waltz","wanna","warty","watch","water","weave","wedge","weigh","weird","whale",
  "wheat","wheel","where","which","while","whims","whine","whips","white","whole",
  "wilds","wimpy","wince","witch","woman","woods","world","worms","worse","worst",
  "worth","wrath","wrist","wrote",
  // X Y Z
  "yacht","yield","young","youth","zebra","zesty","zilch","zoned",

  // Extended valid guesses — valid English 5-letter words not in the answer pool
  "aback","abuzz","adlib","affix","afoot","aeons","aglow","ahems","ahold","ambit",
  "ambos","amuck","ankhs","antae","apian","apish","arses","atilt","atlas","atman",
  "atrip","avens","avian","avows","axial","axles","azoth","balds","baler","balms",
  "balmy","banns","bants","barfs","basks","baulk","bawds","bawls","bawdy","beady",
  "bedim","befit","befog","begum","betel","bhang","bidet","binds","biota","bitty",
  "blabs","black","blain","blare","blats","bless","blocs","blogs","blots","boded",
  "bodes","bonus","boxed","brays","bream","breys","brill","briny","britt","burps",
  "burrs","butts","bylaw","byway","cacao","cahow","caner","carob","carom","cauls",
  "ceils","chaff","chant","chaps","chard","chart","chary","clave","cleat","cleek",
  "clops","coeds","colic","colon","combs","combo","comet","conga","conic","coops",
  "copse","coups","cowls","coypu","cozen","creak","cruse","cubby","cubic","cubit",
  "cupid","cured","cusps","cynic","czars","dagga","dames","darer","daubs","daven",
  "davit","debts","decry","deify","deity","demur","dense","dersh","dicot","dimly",
  "dingo","diode","dirge","disco","ditty","divvy","dolts","donut","dowse","drabs",
  "dreck","droit","drove","drubs","dunce","dunno","durra","edged","emirs","enrol",
  "epoxy","ergot","ethos","evens","ewers","exert","extol","exult","eying","fanny",
  "farce","faugh","faves","fecal","felts","ferny","flack","flail","flaky","flews",
  "flick","flied","fling","flint","flips","flued","foamy","folio","fondy","forte",
  "fotch","foxed","frass","frill","fritz","frosh","fumed","fumes","funky","gabby",
  "gammy","gator","gauze","gavel","gawky","gecko","gelid","gibed","gilts","girts",
  "glean","glint","gloms","glops","gloss","goads","gobos","gonzo","gorge","gouty",
  "grabs","grads","graft","grail","grams","graph","grays","griff","grigs","gripe",
  "grips","grist","grits","groin","groom","gross","grove","grubs","guava","gawks",
  "gybes","gyros","hadst","halve","handy","harpy","heave","heavy","hedge","helix",
  "helve","hence","herbs","herby","highs","hippo","holed","homer","honey","honor",
  "horns","hotly","hovel","howdy","hydra","hymen","hypes","iambs","idiot","imbue",
  "inked","inlay","inure","ionic","izard","jammy","japan","jazzy","jerky","jibed",
  "jingo","jokey","jolly","jukes","kempt","kevel","kilns","knave","kopek","kudos",
  "lacey","laity","lanky","larva","laved","lawks","leafs","leaky","leant","leets",
  "leggy","limey","lingo","lisps","litho","liver","llama","loped","lousy","lowly",
  "luaus","lumpy","lured","lusty","madam","matey","mealy","melee","messy","metis",
  "mitre","modal","mogul","molts","money","mooch","mouth","muons","mural","mushy",
  "myrrh","natty","neigh","noisy","noose","norma","nouns","numbs","oaken","oasis",
  "oaten","ochre","offal","often","onion","opine","orate","ovoid","oxide","ozone",
  "paisa","pally","papal","parka","parry","patsy","pavan","peeve","penny","perch",
  "perky","petty","pewit","plash","plasm","plonk","plops","plush","poach","polka",
  "ponce","ponds","popsy","preen","preys","privy","prude","psalm","puffy","pulpy",
  "punks","randy","rashy","ratty","ravel","razed","recap","reedy","reeve","rehab",
  "reify","relax","remap","renew","repro","retro","rhino","rider","rinky","ripen",
  "risen","risky","ritzy","roman","rowdy","ruddy","rugby","ruins","ruler","rusty",
  "sabra","sadly","sagas","saggy","salve","salvo","samba","sandy","sappy","scarf",
  "scary","scion","scoff","scold","scoot","scope","scorn","scour","shiny","siren",
  "slimy","sloop","slosh","sloth","slung","slunk","slurp","slyly","smear","smell",
  "smelt","smile","smock","smogs","smoky","sniff","snipe","snobs","snoot","snore",
  "snort","snout","snowy","snubs","softy","soggy","sonny","sooty","soppy","sorry",
  "soupy","spewy","spiky","spill","spire","splay","spook","spout","spiny","sprig",
  "spume","squid","staph","stave","stays","stead","steed","stele","stems","stile",
  "stomp","stood","stool","stoop","stozy","strew","stria","study","sulky","sully",
  "sunny","surly","swore","syrup","tabby","taffy","taiga","taker","tamps","tangy",
  "tapir","tardy","tarry","tawny","taxis","teary","techy","telly","tempo","tepid",
  "testy","throe","throw","thrum","tithe","today","toddy","token","torso","toxic",
  "toyon","trice","trite","troll","tromp","trove","tuber","tulip","turbo","twerp",
  "twine","twins","twist","tying","udder","ulcer","undue","unify","unpin","untie",
  "unzip","urban","usurp","utile","utter","vaned","vapor","venal","venom","vexed",
  "vicar","visit","vital","vivid","vodka","vomit","vouch","vying","waded","waken",
  "wider","wight","windy","wiper","wised","woful","wooly","woozy","wordy",
];

// Deduplicate at module load time (safety: remove any accidental duplicates)
(function dedupe() {
  const seen = new Set();
  let i = VALID_GUESSES.length;
  while (i--) {
    const w = VALID_GUESSES[i];
    if (seen.has(w) || !/^[a-z]{5}$/.test(w)) {
      VALID_GUESSES.splice(i, 1);
    } else {
      seen.add(w);
    }
  }
}());
