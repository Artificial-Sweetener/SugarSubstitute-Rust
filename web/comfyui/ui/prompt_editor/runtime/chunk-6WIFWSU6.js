// frontend/sugarprompt/core/src/required.ts
function requireValue(value, owner) {
  if (value === void 0 || value === null)
    throw new Error(`Missing required SugarPrompt value in ${owner}.`);
  return value;
}

// frontend/sugarprompt/core/src/prompt/unicode-decimal-data.ts
var decimalZeros = [
  48,
  1632,
  1776,
  1984,
  2406,
  2534,
  2662,
  2790,
  2918,
  3046,
  3174,
  3302,
  3430,
  3558,
  3664,
  3792,
  3872,
  4160,
  4240,
  6112,
  6160,
  6470,
  6608,
  6784,
  6800,
  6992,
  7088,
  7232,
  7248,
  42528,
  43216,
  43264,
  43472,
  43504,
  43600,
  44016,
  65296,
  66720,
  68912,
  69734,
  69872,
  69942,
  70096,
  70384,
  70736,
  70864,
  71248,
  71360,
  71472,
  71904,
  72016,
  72784,
  73040,
  73120,
  73552,
  92768,
  92864,
  93008,
  120782,
  120792,
  120802,
  120812,
  120822,
  123200,
  123632,
  124144,
  125264,
  130032
];

// frontend/sugarprompt/core/src/prompt/weights.ts
function asciiDigits(text) {
  let result = "";
  for (const character of text) {
    if ("+-.".includes(character)) {
      result += character;
      continue;
    }
    const point = requireValue(character.codePointAt(0), "weights");
    const zero = decimalZeros.find((start) => point >= start && point < start + 10);
    if (zero === void 0) return null;
    result += String(point - zero);
  }
  return result;
}
function parseWeight(text, allowPlus) {
  const ascii = asciiDigits(text);
  if (ascii === null || !(allowPlus ? /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u : /^-?(?:\d+(?:\.\d*)?|\.\d+)$/u).test(ascii))
    return null;
  const sign = ascii.startsWith("-") ? "-" : "";
  const unsigned = ascii.replace(/^[+-]/u, "");
  const [integer = "", fraction = ""] = unsigned.split(".");
  const coefficient = (integer + fraction).replace(/^0+/u, "") || "0";
  const adjusted = coefficient.length - 1 - fraction.length;
  if (adjusted < -6)
    return sign + String(coefficient[0]) + (coefficient.length > 1 ? "." + coefficient.slice(1) : "") + "E" + String(adjusted);
  return sign + (integer.replace(/^0+/u, "") || "0") + (fraction ? "." + fraction : "");
}
function weightField(source, from, to, lora) {
  if (lora) while (from < to && /[ \t]/u.test(requireValue(source[from], "weights"))) from++;
  const value = parseWeight(source.slice(from, to), lora);
  return value === null ? null : { range: { from, to }, value };
}

// frontend/sugarprompt/core/src/prompt/lora-fields.ts
function loraWeightFields(source, separator, bodyEnd) {
  const fieldEnd = (start) => {
    const colon = source.indexOf(":", start);
    return colon < 0 || colon >= bodyEnd ? bodyEnd : colon;
  };
  const firstEnd = fieldEnd(separator + 1), first = weightField(source, separator + 1, firstEnd, true);
  if (!first) return null;
  let second = null, blockWeightsRange = null;
  if (firstEnd < bodyEnd) {
    const secondFrom = firstEnd + 1, secondEnd = fieldEnd(secondFrom);
    second = weightField(source, secondFrom, secondEnd, true);
    if (!second) blockWeightsRange = { from: secondFrom, to: bodyEnd };
    else if (secondEnd < bodyEnd) blockWeightsRange = { from: secondEnd + 1, to: bodyEnd };
  }
  return { first, second, blockWeightsRange };
}

// frontend/sugarprompt/core/src/prompt/lora.ts
function loraSpan(source, from, close) {
  if (!source.startsWith("lora:", from + 1)) return null;
  const nameFrom = from + 6, nameTo = source.indexOf(":", nameFrom);
  if (nameTo <= nameFrom || nameTo >= close || /[<>:]/u.test(source.slice(nameFrom, nameTo)))
    return null;
  const fields = loraWeightFields(source, nameTo, close);
  if (!fields) return null;
  return {
    outerRange: { from, to: close + 1 },
    nameRange: { from: nameFrom, to: nameTo },
    firstWeightRange: fields.first.range,
    firstWeight: fields.first.value,
    secondWeightRange: fields.second?.range ?? null,
    secondWeight: fields.second?.value ?? null,
    blockWeightsRange: fields.blockWeightsRange,
    depth: 0
  };
}

// frontend/sugarprompt/core/src/prompt/regions.ts
function regionSeparatorAt(source, from) {
  const token = regionTokenAt(source, from);
  if (from > 0 && !/[\r\n]/u.test(requireValue(source[from - 1], "regions")) || !token || token.tokenRange.to < source.length && !/[\r\n]/u.test(requireValue(source[token.tokenRange.to], "regions")))
    return null;
  const to = token.tokenRange.to;
  return {
    ...token,
    lineRange: {
      from,
      to: to + (source.startsWith("\r\n", to) ? 2 : to < source.length ? 1 : 0)
    }
  };
}
function regionTokenAt(source, from) {
  if (from < 0 || !source.startsWith("[SEP", from)) return null;
  let end = from + 4, nameRange = null, name = null;
  if (source[end] === "|") {
    const nameFrom = ++end;
    while (end < source.length && !/[\]\r\n]/u.test(requireValue(source[end], "regions"))) end++;
    if (end === nameFrom || source[end] !== "]") return null;
    nameRange = { from: nameFrom, to: end };
    name = source.slice(nameFrom, end);
  } else if (source[end] !== "]") return null;
  const to = end + 1;
  return {
    tokenRange: { from, to },
    nameRange,
    name
  };
}
function regionTokensInRange(source, from, to) {
  const tokens = [];
  let cursor = Math.max(0, from);
  while (cursor < Math.min(source.length, to)) {
    const start = source.indexOf("[SEP", cursor);
    if (start < 0 || start >= to) break;
    const token = regionTokenAt(source, start);
    if (token && token.tokenRange.to <= to) {
      tokens.push(token);
      cursor = token.tokenRange.to;
    } else cursor = start + 4;
  }
  return tokens;
}
function regionStructure(sourceLength, separators) {
  let from = 0;
  const partitions = separators.map((separator, index) => {
    const partition = {
      index,
      sourceRange: { from, to: separator.lineRange.from },
      isGlobal: index === 0
    };
    from = separator.lineRange.to;
    return partition;
  });
  partitions.push({
    index: partitions.length,
    sourceRange: { from, to: sourceLength },
    isGlobal: partitions.length === 0
  });
  return { separators, partitions };
}

// frontend/sugarprompt/core/src/prompt/unicode-casefold-data.ts
var foldRanges = [
  [65, 90, 1, 32],
  [192, 214, 1, 32],
  [216, 222, 1, 32],
  [256, 302, 2, 1],
  [306, 310, 2, 1],
  [313, 327, 2, 1],
  [330, 374, 2, 1],
  [377, 381, 2, 1],
  [416, 420, 2, 1],
  [459, 475, 2, 1],
  [478, 494, 2, 1],
  [504, 542, 2, 1],
  [546, 562, 2, 1],
  [582, 590, 2, 1],
  [904, 906, 1, 37],
  [913, 929, 1, 32],
  [931, 939, 1, 32],
  [984, 1006, 2, 1],
  [1021, 1023, 1, -130],
  [1024, 1039, 1, 80],
  [1040, 1071, 1, 32],
  [1120, 1152, 2, 1],
  [1162, 1214, 2, 1],
  [1217, 1229, 2, 1],
  [1232, 1326, 2, 1],
  [1329, 1366, 1, 48],
  [4256, 4293, 1, 7264],
  [5112, 5117, 1, -8],
  [7312, 7354, 1, -3008],
  [7357, 7359, 1, -3008],
  [7680, 7828, 2, 1],
  [7840, 7934, 2, 1],
  [7944, 7951, 1, -8],
  [7960, 7965, 1, -8],
  [7976, 7983, 1, -8],
  [7992, 7999, 1, -8],
  [8008, 8013, 1, -8],
  [8025, 8031, 2, -8],
  [8040, 8047, 1, -8],
  [8136, 8139, 1, -86],
  [8544, 8559, 1, 16],
  [9398, 9423, 1, 26],
  [11264, 11311, 1, 48],
  [11367, 11371, 2, 1],
  [11392, 11490, 2, 1],
  [42560, 42604, 2, 1],
  [42624, 42650, 2, 1],
  [42786, 42798, 2, 1],
  [42802, 42862, 2, 1],
  [42878, 42886, 2, 1],
  [42902, 42920, 2, 1],
  [42932, 42946, 2, 1],
  [43888, 43967, 1, -38864],
  [65313, 65338, 1, 32],
  [66560, 66599, 1, 40],
  [66736, 66771, 1, 40],
  [66928, 66938, 1, 39],
  [66940, 66954, 1, 39],
  [66956, 66962, 1, 39],
  [68736, 68786, 1, 64],
  [71840, 71871, 1, 32],
  [93760, 93791, 1, 32],
  [125184, 125217, 1, 34]
];
var foldMappings = /* @__PURE__ */ new Map([
  [181, "\u03BC"],
  [223, "ss"],
  [304, "i\u0307"],
  [329, "\u02BCn"],
  [376, "\xFF"],
  [383, "s"],
  [385, "\u0253"],
  [386, "\u0183"],
  [388, "\u0185"],
  [390, "\u0254"],
  [391, "\u0188"],
  [393, "\u0256"],
  [394, "\u0257"],
  [395, "\u018C"],
  [398, "\u01DD"],
  [399, "\u0259"],
  [400, "\u025B"],
  [401, "\u0192"],
  [403, "\u0260"],
  [404, "\u0263"],
  [406, "\u0269"],
  [407, "\u0268"],
  [408, "\u0199"],
  [412, "\u026F"],
  [413, "\u0272"],
  [415, "\u0275"],
  [422, "\u0280"],
  [423, "\u01A8"],
  [425, "\u0283"],
  [428, "\u01AD"],
  [430, "\u0288"],
  [431, "\u01B0"],
  [433, "\u028A"],
  [434, "\u028B"],
  [435, "\u01B4"],
  [437, "\u01B6"],
  [439, "\u0292"],
  [440, "\u01B9"],
  [444, "\u01BD"],
  [452, "\u01C6"],
  [453, "\u01C6"],
  [455, "\u01C9"],
  [456, "\u01C9"],
  [458, "\u01CC"],
  [496, "j\u030C"],
  [497, "\u01F3"],
  [498, "\u01F3"],
  [500, "\u01F5"],
  [502, "\u0195"],
  [503, "\u01BF"],
  [544, "\u019E"],
  [570, "\u2C65"],
  [571, "\u023C"],
  [573, "\u019A"],
  [574, "\u2C66"],
  [577, "\u0242"],
  [579, "\u0180"],
  [580, "\u0289"],
  [581, "\u028C"],
  [837, "\u03B9"],
  [880, "\u0371"],
  [882, "\u0373"],
  [886, "\u0377"],
  [895, "\u03F3"],
  [902, "\u03AC"],
  [908, "\u03CC"],
  [910, "\u03CD"],
  [911, "\u03CE"],
  [912, "\u03B9\u0308\u0301"],
  [944, "\u03C5\u0308\u0301"],
  [962, "\u03C3"],
  [975, "\u03D7"],
  [976, "\u03B2"],
  [977, "\u03B8"],
  [981, "\u03C6"],
  [982, "\u03C0"],
  [1008, "\u03BA"],
  [1009, "\u03C1"],
  [1012, "\u03B8"],
  [1013, "\u03B5"],
  [1015, "\u03F8"],
  [1017, "\u03F2"],
  [1018, "\u03FB"],
  [1216, "\u04CF"],
  [1415, "\u0565\u0582"],
  [4295, "\u2D27"],
  [4301, "\u2D2D"],
  [7296, "\u0432"],
  [7297, "\u0434"],
  [7298, "\u043E"],
  [7299, "\u0441"],
  [7300, "\u0442"],
  [7301, "\u0442"],
  [7302, "\u044A"],
  [7303, "\u0463"],
  [7304, "\uA64B"],
  [7830, "h\u0331"],
  [7831, "t\u0308"],
  [7832, "w\u030A"],
  [7833, "y\u030A"],
  [7834, "a\u02BE"],
  [7835, "\u1E61"],
  [7838, "ss"],
  [8016, "\u03C5\u0313"],
  [8018, "\u03C5\u0313\u0300"],
  [8020, "\u03C5\u0313\u0301"],
  [8022, "\u03C5\u0313\u0342"],
  [8064, "\u1F00\u03B9"],
  [8065, "\u1F01\u03B9"],
  [8066, "\u1F02\u03B9"],
  [8067, "\u1F03\u03B9"],
  [8068, "\u1F04\u03B9"],
  [8069, "\u1F05\u03B9"],
  [8070, "\u1F06\u03B9"],
  [8071, "\u1F07\u03B9"],
  [8072, "\u1F00\u03B9"],
  [8073, "\u1F01\u03B9"],
  [8074, "\u1F02\u03B9"],
  [8075, "\u1F03\u03B9"],
  [8076, "\u1F04\u03B9"],
  [8077, "\u1F05\u03B9"],
  [8078, "\u1F06\u03B9"],
  [8079, "\u1F07\u03B9"],
  [8080, "\u1F20\u03B9"],
  [8081, "\u1F21\u03B9"],
  [8082, "\u1F22\u03B9"],
  [8083, "\u1F23\u03B9"],
  [8084, "\u1F24\u03B9"],
  [8085, "\u1F25\u03B9"],
  [8086, "\u1F26\u03B9"],
  [8087, "\u1F27\u03B9"],
  [8088, "\u1F20\u03B9"],
  [8089, "\u1F21\u03B9"],
  [8090, "\u1F22\u03B9"],
  [8091, "\u1F23\u03B9"],
  [8092, "\u1F24\u03B9"],
  [8093, "\u1F25\u03B9"],
  [8094, "\u1F26\u03B9"],
  [8095, "\u1F27\u03B9"],
  [8096, "\u1F60\u03B9"],
  [8097, "\u1F61\u03B9"],
  [8098, "\u1F62\u03B9"],
  [8099, "\u1F63\u03B9"],
  [8100, "\u1F64\u03B9"],
  [8101, "\u1F65\u03B9"],
  [8102, "\u1F66\u03B9"],
  [8103, "\u1F67\u03B9"],
  [8104, "\u1F60\u03B9"],
  [8105, "\u1F61\u03B9"],
  [8106, "\u1F62\u03B9"],
  [8107, "\u1F63\u03B9"],
  [8108, "\u1F64\u03B9"],
  [8109, "\u1F65\u03B9"],
  [8110, "\u1F66\u03B9"],
  [8111, "\u1F67\u03B9"],
  [8114, "\u1F70\u03B9"],
  [8115, "\u03B1\u03B9"],
  [8116, "\u03AC\u03B9"],
  [8118, "\u03B1\u0342"],
  [8119, "\u03B1\u0342\u03B9"],
  [8120, "\u1FB0"],
  [8121, "\u1FB1"],
  [8122, "\u1F70"],
  [8123, "\u1F71"],
  [8124, "\u03B1\u03B9"],
  [8126, "\u03B9"],
  [8130, "\u1F74\u03B9"],
  [8131, "\u03B7\u03B9"],
  [8132, "\u03AE\u03B9"],
  [8134, "\u03B7\u0342"],
  [8135, "\u03B7\u0342\u03B9"],
  [8140, "\u03B7\u03B9"],
  [8146, "\u03B9\u0308\u0300"],
  [8147, "\u03B9\u0308\u0301"],
  [8150, "\u03B9\u0342"],
  [8151, "\u03B9\u0308\u0342"],
  [8152, "\u1FD0"],
  [8153, "\u1FD1"],
  [8154, "\u1F76"],
  [8155, "\u1F77"],
  [8162, "\u03C5\u0308\u0300"],
  [8163, "\u03C5\u0308\u0301"],
  [8164, "\u03C1\u0313"],
  [8166, "\u03C5\u0342"],
  [8167, "\u03C5\u0308\u0342"],
  [8168, "\u1FE0"],
  [8169, "\u1FE1"],
  [8170, "\u1F7A"],
  [8171, "\u1F7B"],
  [8172, "\u1FE5"],
  [8178, "\u1F7C\u03B9"],
  [8179, "\u03C9\u03B9"],
  [8180, "\u03CE\u03B9"],
  [8182, "\u03C9\u0342"],
  [8183, "\u03C9\u0342\u03B9"],
  [8184, "\u1F78"],
  [8185, "\u1F79"],
  [8186, "\u1F7C"],
  [8187, "\u1F7D"],
  [8188, "\u03C9\u03B9"],
  [8486, "\u03C9"],
  [8490, "k"],
  [8491, "\xE5"],
  [8498, "\u214E"],
  [8579, "\u2184"],
  [11360, "\u2C61"],
  [11362, "\u026B"],
  [11363, "\u1D7D"],
  [11364, "\u027D"],
  [11373, "\u0251"],
  [11374, "\u0271"],
  [11375, "\u0250"],
  [11376, "\u0252"],
  [11378, "\u2C73"],
  [11381, "\u2C76"],
  [11390, "\u023F"],
  [11391, "\u0240"],
  [11499, "\u2CEC"],
  [11501, "\u2CEE"],
  [11506, "\u2CF3"],
  [42873, "\uA77A"],
  [42875, "\uA77C"],
  [42877, "\u1D79"],
  [42891, "\uA78C"],
  [42893, "\u0265"],
  [42896, "\uA791"],
  [42898, "\uA793"],
  [42922, "\u0266"],
  [42923, "\u025C"],
  [42924, "\u0261"],
  [42925, "\u026C"],
  [42926, "\u026A"],
  [42928, "\u029E"],
  [42929, "\u0287"],
  [42930, "\u029D"],
  [42931, "\uAB53"],
  [42948, "\uA794"],
  [42949, "\u0282"],
  [42950, "\u1D8E"],
  [42951, "\uA7C8"],
  [42953, "\uA7CA"],
  [42960, "\uA7D1"],
  [42966, "\uA7D7"],
  [42968, "\uA7D9"],
  [42997, "\uA7F6"],
  [64256, "ff"],
  [64257, "fi"],
  [64258, "fl"],
  [64259, "ffi"],
  [64260, "ffl"],
  [64261, "st"],
  [64262, "st"],
  [64275, "\u0574\u0576"],
  [64276, "\u0574\u0565"],
  [64277, "\u0574\u056B"],
  [64278, "\u057E\u0576"],
  [64279, "\u0574\u056D"],
  [66964, "\u{105BB}"],
  [66965, "\u{105BC}"]
]);

// frontend/sugarprompt/core/src/prompt/unicode-alphanumeric-data.ts
var alphanumericIntervals = `30-39 41-5a 61-7a aa-aa b2-b3 b5-b5 b9-ba bc-be c0-d6 d8-f6 f8-2c1 2c6-2d1 2e0-2e4 2ec-2ec
2ee-2ee 370-374 376-377 37a-37d 37f-37f 386-386 388-38a 38c-38c 38e-3a1 3a3-3f5 3f7-481
48a-52f 531-556 559-559 560-588 5d0-5ea 5ef-5f2 620-64a 660-669 66e-66f 671-6d3 6d5-6d5
6e5-6e6 6ee-6fc 6ff-6ff 710-710 712-72f 74d-7a5 7b1-7b1 7c0-7ea 7f4-7f5 7fa-7fa 800-815
81a-81a 824-824 828-828 840-858 860-86a 870-887 889-88e 8a0-8c9 904-939 93d-93d 950-950
958-961 966-96f 971-980 985-98c 98f-990 993-9a8 9aa-9b0 9b2-9b2 9b6-9b9 9bd-9bd 9ce-9ce
9dc-9dd 9df-9e1 9e6-9f1 9f4-9f9 9fc-9fc a05-a0a a0f-a10 a13-a28 a2a-a30 a32-a33 a35-a36
a38-a39 a59-a5c a5e-a5e a66-a6f a72-a74 a85-a8d a8f-a91 a93-aa8 aaa-ab0 ab2-ab3 ab5-ab9
abd-abd ad0-ad0 ae0-ae1 ae6-aef af9-af9 b05-b0c b0f-b10 b13-b28 b2a-b30 b32-b33 b35-b39
b3d-b3d b5c-b5d b5f-b61 b66-b6f b71-b77 b83-b83 b85-b8a b8e-b90 b92-b95 b99-b9a b9c-b9c
b9e-b9f ba3-ba4 ba8-baa bae-bb9 bd0-bd0 be6-bf2 c05-c0c c0e-c10 c12-c28 c2a-c39 c3d-c3d
c58-c5a c5d-c5d c60-c61 c66-c6f c78-c7e c80-c80 c85-c8c c8e-c90 c92-ca8 caa-cb3 cb5-cb9
cbd-cbd cdd-cde ce0-ce1 ce6-cef cf1-cf2 d04-d0c d0e-d10 d12-d3a d3d-d3d d4e-d4e d54-d56
d58-d61 d66-d78 d7a-d7f d85-d96 d9a-db1 db3-dbb dbd-dbd dc0-dc6 de6-def e01-e30 e32-e33
e40-e46 e50-e59 e81-e82 e84-e84 e86-e8a e8c-ea3 ea5-ea5 ea7-eb0 eb2-eb3 ebd-ebd ec0-ec4
ec6-ec6 ed0-ed9 edc-edf f00-f00 f20-f33 f40-f47 f49-f6c f88-f8c 1000-102a 103f-1049
1050-1055 105a-105d 1061-1061 1065-1066 106e-1070 1075-1081 108e-108e 1090-1099 10a0-10c5
10c7-10c7 10cd-10cd 10d0-10fa 10fc-1248 124a-124d 1250-1256 1258-1258 125a-125d 1260-1288
128a-128d 1290-12b0 12b2-12b5 12b8-12be 12c0-12c0 12c2-12c5 12c8-12d6 12d8-1310 1312-1315
1318-135a 1369-137c 1380-138f 13a0-13f5 13f8-13fd 1401-166c 166f-167f 1681-169a 16a0-16ea
16ee-16f8 1700-1711 171f-1731 1740-1751 1760-176c 176e-1770 1780-17b3 17d7-17d7 17dc-17dc
17e0-17e9 17f0-17f9 1810-1819 1820-1878 1880-1884 1887-18a8 18aa-18aa 18b0-18f5 1900-191e
1946-196d 1970-1974 1980-19ab 19b0-19c9 19d0-19da 1a00-1a16 1a20-1a54 1a80-1a89 1a90-1a99
1aa7-1aa7 1b05-1b33 1b45-1b4c 1b50-1b59 1b83-1ba0 1bae-1be5 1c00-1c23 1c40-1c49 1c4d-1c7d
1c80-1c88 1c90-1cba 1cbd-1cbf 1ce9-1cec 1cee-1cf3 1cf5-1cf6 1cfa-1cfa 1d00-1dbf 1e00-1f15
1f18-1f1d 1f20-1f45 1f48-1f4d 1f50-1f57 1f59-1f59 1f5b-1f5b 1f5d-1f5d 1f5f-1f7d 1f80-1fb4
1fb6-1fbc 1fbe-1fbe 1fc2-1fc4 1fc6-1fcc 1fd0-1fd3 1fd6-1fdb 1fe0-1fec 1ff2-1ff4 1ff6-1ffc
2070-2071 2074-2079 207f-2089 2090-209c 2102-2102 2107-2107 210a-2113 2115-2115 2119-211d
2124-2124 2126-2126 2128-2128 212a-212d 212f-2139 213c-213f 2145-2149 214e-214e 2150-2189
2460-249b 24ea-24ff 2776-2793 2c00-2ce4 2ceb-2cee 2cf2-2cf3 2cfd-2cfd 2d00-2d25 2d27-2d27
2d2d-2d2d 2d30-2d67 2d6f-2d6f 2d80-2d96 2da0-2da6 2da8-2dae 2db0-2db6 2db8-2dbe 2dc0-2dc6
2dc8-2dce 2dd0-2dd6 2dd8-2dde 2e2f-2e2f 3005-3007 3021-3029 3031-3035 3038-303c 3041-3096
309d-309f 30a1-30fa 30fc-30ff 3105-312f 3131-318e 3192-3195 31a0-31bf 31f0-31ff 3220-3229
3248-324f 3251-325f 3280-3289 32b1-32bf 3400-4dbf 4e00-a48c a4d0-a4fd a500-a60c a610-a62b
a640-a66e a67f-a69d a6a0-a6ef a717-a71f a722-a788 a78b-a7ca a7d0-a7d1 a7d3-a7d3 a7d5-a7d9
a7f2-a801 a803-a805 a807-a80a a80c-a822 a830-a835 a840-a873 a882-a8b3 a8d0-a8d9 a8f2-a8f7
a8fb-a8fb a8fd-a8fe a900-a925 a930-a946 a960-a97c a984-a9b2 a9cf-a9d9 a9e0-a9e4 a9e6-a9fe
aa00-aa28 aa40-aa42 aa44-aa4b aa50-aa59 aa60-aa76 aa7a-aa7a aa7e-aaaf aab1-aab1 aab5-aab6
aab9-aabd aac0-aac0 aac2-aac2 aadb-aadd aae0-aaea aaf2-aaf4 ab01-ab06 ab09-ab0e ab11-ab16
ab20-ab26 ab28-ab2e ab30-ab5a ab5c-ab69 ab70-abe2 abf0-abf9 ac00-d7a3 d7b0-d7c6 d7cb-d7fb
f900-fa6d fa70-fad9 fb00-fb06 fb13-fb17 fb1d-fb1d fb1f-fb28 fb2a-fb36 fb38-fb3c fb3e-fb3e
fb40-fb41 fb43-fb44 fb46-fbb1 fbd3-fd3d fd50-fd8f fd92-fdc7 fdf0-fdfb fe70-fe74 fe76-fefc
ff10-ff19 ff21-ff3a ff41-ff5a ff66-ffbe ffc2-ffc7 ffca-ffcf ffd2-ffd7 ffda-ffdc
10000-1000b 1000d-10026 10028-1003a 1003c-1003d 1003f-1004d 10050-1005d 10080-100fa
10107-10133 10140-10178 1018a-1018b 10280-1029c 102a0-102d0 102e1-102fb 10300-10323
1032d-1034a 10350-10375 10380-1039d 103a0-103c3 103c8-103cf 103d1-103d5 10400-1049d
104a0-104a9 104b0-104d3 104d8-104fb 10500-10527 10530-10563 10570-1057a 1057c-1058a
1058c-10592 10594-10595 10597-105a1 105a3-105b1 105b3-105b9 105bb-105bc 10600-10736
10740-10755 10760-10767 10780-10785 10787-107b0 107b2-107ba 10800-10805 10808-10808
1080a-10835 10837-10838 1083c-1083c 1083f-10855 10858-10876 10879-1089e 108a7-108af
108e0-108f2 108f4-108f5 108fb-1091b 10920-10939 10980-109b7 109bc-109cf 109d2-10a00
10a10-10a13 10a15-10a17 10a19-10a35 10a40-10a48 10a60-10a7e 10a80-10a9f 10ac0-10ac7
10ac9-10ae4 10aeb-10aef 10b00-10b35 10b40-10b55 10b58-10b72 10b78-10b91 10ba9-10baf
10c00-10c48 10c80-10cb2 10cc0-10cf2 10cfa-10d23 10d30-10d39 10e60-10e7e 10e80-10ea9
10eb0-10eb1 10f00-10f27 10f30-10f45 10f51-10f54 10f70-10f81 10fb0-10fcb 10fe0-10ff6
11003-11037 11052-1106f 11071-11072 11075-11075 11083-110af 110d0-110e8 110f0-110f9
11103-11126 11136-1113f 11144-11144 11147-11147 11150-11172 11176-11176 11183-111b2
111c1-111c4 111d0-111da 111dc-111dc 111e1-111f4 11200-11211 11213-1122b 1123f-11240
11280-11286 11288-11288 1128a-1128d 1128f-1129d 1129f-112a8 112b0-112de 112f0-112f9
11305-1130c 1130f-11310 11313-11328 1132a-11330 11332-11333 11335-11339 1133d-1133d
11350-11350 1135d-11361 11400-11434 11447-1144a 11450-11459 1145f-11461 11480-114af
114c4-114c5 114c7-114c7 114d0-114d9 11580-115ae 115d8-115db 11600-1162f 11644-11644
11650-11659 11680-116aa 116b8-116b8 116c0-116c9 11700-1171a 11730-1173b 11740-11746
11800-1182b 118a0-118f2 118ff-11906 11909-11909 1190c-11913 11915-11916 11918-1192f
1193f-1193f 11941-11941 11950-11959 119a0-119a7 119aa-119d0 119e1-119e1 119e3-119e3
11a00-11a00 11a0b-11a32 11a3a-11a3a 11a50-11a50 11a5c-11a89 11a9d-11a9d 11ab0-11af8
11c00-11c08 11c0a-11c2e 11c40-11c40 11c50-11c6c 11c72-11c8f 11d00-11d06 11d08-11d09
11d0b-11d30 11d46-11d46 11d50-11d59 11d60-11d65 11d67-11d68 11d6a-11d89 11d98-11d98
11da0-11da9 11ee0-11ef2 11f02-11f02 11f04-11f10 11f12-11f33 11f50-11f59 11fb0-11fb0
11fc0-11fd4 12000-12399 12400-1246e 12480-12543 12f90-12ff0 13000-1342f 13441-13446
14400-14646 16800-16a38 16a40-16a5e 16a60-16a69 16a70-16abe 16ac0-16ac9 16ad0-16aed
16b00-16b2f 16b40-16b43 16b50-16b59 16b5b-16b61 16b63-16b77 16b7d-16b8f 16e40-16e96
16f00-16f4a 16f50-16f50 16f93-16f9f 16fe0-16fe1 16fe3-16fe3 17000-187f7 18800-18cd5
18d00-18d08 1aff0-1aff3 1aff5-1affb 1affd-1affe 1b000-1b122 1b132-1b132 1b150-1b152
1b155-1b155 1b164-1b167 1b170-1b2fb 1bc00-1bc6a 1bc70-1bc7c 1bc80-1bc88 1bc90-1bc99
1d2c0-1d2d3 1d2e0-1d2f3 1d360-1d378 1d400-1d454 1d456-1d49c 1d49e-1d49f 1d4a2-1d4a2
1d4a5-1d4a6 1d4a9-1d4ac 1d4ae-1d4b9 1d4bb-1d4bb 1d4bd-1d4c3 1d4c5-1d505 1d507-1d50a
1d50d-1d514 1d516-1d51c 1d51e-1d539 1d53b-1d53e 1d540-1d544 1d546-1d546 1d54a-1d550
1d552-1d6a5 1d6a8-1d6c0 1d6c2-1d6da 1d6dc-1d6fa 1d6fc-1d714 1d716-1d734 1d736-1d74e
1d750-1d76e 1d770-1d788 1d78a-1d7a8 1d7aa-1d7c2 1d7c4-1d7cb 1d7ce-1d7ff 1df00-1df1e
1df25-1df2a 1e030-1e06d 1e100-1e12c 1e137-1e13d 1e140-1e149 1e14e-1e14e 1e290-1e2ad
1e2c0-1e2eb 1e2f0-1e2f9 1e4d0-1e4eb 1e4f0-1e4f9 1e7e0-1e7e6 1e7e8-1e7eb 1e7ed-1e7ee
1e7f0-1e7fe 1e800-1e8c4 1e8c7-1e8cf 1e900-1e943 1e94b-1e94b 1e950-1e959 1ec71-1ecab
1ecad-1ecaf 1ecb1-1ecb4 1ed01-1ed2d 1ed2f-1ed3d 1ee00-1ee03 1ee05-1ee1f 1ee21-1ee22
1ee24-1ee24 1ee27-1ee27 1ee29-1ee32 1ee34-1ee37 1ee39-1ee39 1ee3b-1ee3b 1ee42-1ee42
1ee47-1ee47 1ee49-1ee49 1ee4b-1ee4b 1ee4d-1ee4f 1ee51-1ee52 1ee54-1ee54 1ee57-1ee57
1ee59-1ee59 1ee5b-1ee5b 1ee5d-1ee5d 1ee5f-1ee5f 1ee61-1ee62 1ee64-1ee64 1ee67-1ee6a
1ee6c-1ee72 1ee74-1ee77 1ee79-1ee7c 1ee7e-1ee7e 1ee80-1ee89 1ee8b-1ee9b 1eea1-1eea3
1eea5-1eea9 1eeab-1eebb 1f100-1f10c 1fbf0-1fbf9 20000-2a6df 2a700-2b739 2b740-2b81d
2b820-2cea1 2ceb0-2ebe0 2f800-2fa1d 30000-3134a 31350-323af`;

// frontend/sugarprompt/core/src/prompt/unicode-intervals.ts
function decodeUnicodeIntervals(value) {
  return value.split(/\s+/u).map((token) => {
    const [from, to] = token.split("-").map((point) => Number.parseInt(point, 16));
    return [
      requireValue(from, "unicode-intervals"),
      requireValue(to, "unicode-intervals")
    ];
  });
}
function unicodeIntervalIncludes(ranges, point) {
  let from = 0, to = ranges.length;
  while (from < to) {
    const middle = from + to >>> 1, range = requireValue(ranges[middle], "unicode-intervals");
    if (point < range[0]) to = middle;
    else if (point > range[1]) from = middle + 1;
    else return true;
  }
  return false;
}

// frontend/sugarprompt/core/src/prompt/alphanumeric.ts
var intervals = decodeUnicodeIntervals(alphanumericIntervals);
function unicodeAlphanumeric(text) {
  if (!text) return false;
  for (const character of text)
    if (!unicodeIntervalIncludes(intervals, requireValue(character.codePointAt(0), "alphanumeric")))
      return false;
  return true;
}

// frontend/sugarprompt/core/src/prompt/text.ts
var whitespace = (
  // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  /[\t-\r\x1c-\x20\x85\xa0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/u
);
function strip(text, side = "both") {
  let from = 0, to = text.length;
  if (side !== "right")
    while (from < to && whitespace.test(requireValue(text[from], "text"))) from++;
  if (side !== "left")
    while (to > from && whitespace.test(requireValue(text[to - 1], "text"))) to--;
  return text.slice(from, to);
}
function caseFold(text) {
  if (!/[^\x00-\x7f]/u.test(text)) return text.toLowerCase();
  let result = "";
  for (const character of text) {
    const point = requireValue(character.codePointAt(0), "text");
    const mapping = foldMappings.get(point);
    if (mapping !== void 0) {
      result += mapping;
      continue;
    }
    const range = foldRanges.find(
      ([from, to, step]) => point >= from && point <= to && (point - from) % step === 0
    );
    result += range ? String.fromCodePoint(point + range[3]) : character;
  }
  return result;
}
function freezeStructure(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freezeStructure(child);
    Object.freeze(value);
  }
  return value;
}
function isStructuralQuote(source, index) {
  if (source[index] !== "'") return true;
  const previous = unicodeAlphanumeric(
    Array.from(source.slice(Math.max(0, index - 2), index)).at(-1) ?? ""
  );
  const next = unicodeAlphanumeric(Array.from(source.slice(index + 1, index + 3))[0] ?? "");
  return !(previous && next);
}

// frontend/sugarprompt/core/src/prompt/wildcards.ts
function wildcardDelimiters(profile) {
  if (profile.style === "curly") return [{ prefix: "{", suffix: "}" }];
  if (profile.style === "double-underscore")
    return [
      { prefix: "__", suffix: "__" },
      { prefix: "{", suffix: "}" }
    ];
  if (profile.style !== "custom" || typeof profile.alsoRecognizeCurly !== "boolean")
    throw new TypeError("Invalid wildcard profile");
  const { prefix, suffix: suffix2 } = profile;
  for (const part of [prefix, suffix2]) {
    if (typeof part !== "string" || !part || [...Array.from(part)].length > 8 || whitespace.test(part) || // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    /[\x00-\x1f'"/\\{}]/u.test(part) || unicodeAlphanumeric(part))
      throw new RangeError("Invalid wildcard delimiter");
  }
  if (prefix === "<" && suffix2 === ">" || prefix === "(" && suffix2 === ")")
    throw new RangeError("Conflicting wildcard delimiters");
  return [
    { prefix, suffix: suffix2 },
    ...profile.alsoRecognizeCurly ? [{ prefix: "{", suffix: "}" }] : []
  ];
}
function identifier(text) {
  if (!text || strip(text) !== text || /[\\:]/u.test(text)) return null;
  return text.split("/").some((part) => !part || strip(part) !== part || part === "." || part === "..") ? null : text;
}
function wildcardSpan(source, from, to, contentFrom, contentTo, curly) {
  const content = source.slice(contentFrom, contentTo);
  if (curly && (!content || strip(content) !== content || /[{}]/u.test(content))) return null;
  const pipe = content.indexOf("|");
  let base = content, tag2 = null;
  if (pipe >= 0) {
    const candidate = content.slice(pipe + 1);
    if (candidate && strip(candidate) === candidate) {
      base = content.slice(0, pipe);
      tag2 = candidate;
    }
  }
  let form = "simple", csvColumn = null;
  if (base.startsWith("csv:")) {
    const fields = base.slice(4).split(":");
    if (fields.length !== 2 || !fields[1] || strip(fields[1]) !== fields[1]) return null;
    base = requireValue(fields[0], "wildcards");
    csvColumn = fields[1];
    form = "csv";
  }
  const name = identifier(base);
  return name === null ? null : {
    outerRange: { from, to },
    contentRange: { from: contentFrom, to: contentTo },
    form,
    identifier: name,
    csvColumn,
    tag: tag2,
    depth: 0
  };
}
function alternateWildcards(source, delimiters) {
  const spans = [];
  for (const { prefix, suffix: suffix2 } of delimiters) {
    if (prefix === "{" && suffix2 === "}") continue;
    let index = 0;
    while (index < source.length) {
      const from = source.indexOf(prefix, index);
      if (from < 0) break;
      const contentFrom = from + prefix.length;
      const contentTo = source.indexOf(suffix2, contentFrom);
      if (contentTo < 0) break;
      index = contentTo + suffix2.length;
      const span = wildcardSpan(source, from, index, contentFrom, contentTo, false);
      if (span) spans.push(span);
    }
  }
  return spans.sort(
    (a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to
  );
}

// frontend/sugarprompt/core/src/prompt/parser.ts
function emphasisSegments(source, from, to, commas, hardBreaks) {
  const segments = [];
  let start = from;
  let breakIndex = 0;
  const endings = (end) => {
    const result = [];
    while (breakIndex < hardBreaks.length && requireValue(hardBreaks[breakIndex], "parser").to <= end) {
      const range = requireValue(hardBreaks[breakIndex++], "parser");
      if (range.from >= start) result.push(range);
    }
    return result;
  };
  for (const comma of commas) {
    if (comma >= to) break;
    let end = comma + 1;
    while (end < to && " 	\r\n".includes(requireValue(source[end], "parser"))) end++;
    segments.push({
      index: segments.length,
      text: source.slice(start, comma),
      contentRange: { from: start, to: comma },
      separatorRange: { from: comma, to: end },
      hardBreaks: endings(comma)
    });
    start = end;
  }
  if (start < to)
    segments.push({
      index: segments.length,
      text: source.slice(start, to),
      contentRange: { from: start, to },
      separatorRange: null,
      hardBreaks: endings(to)
    });
  return segments;
}
var pairs = {
  "(": ")",
  "[": "]",
  "{": "}",
  "<": ">"
};
function withDepth(spans) {
  spans.sort((a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to);
  const ends = [];
  return spans.map((span) => {
    while (ends.length && requireValue(ends[ends.length - 1], "parser") <= span.outerRange.from)
      ends.pop();
    const next = { ...span, depth: ends.length };
    ends.push(span.outerRange.to);
    return next;
  });
}
function parsePrompt(source, profile = { style: "curly" }) {
  const delimiters = wildcardDelimiters(profile);
  const wildcards = alternateWildcards(source, delimiters);
  const alternateAt = new Map(wildcards.map((span) => [span.outerRange.from, span]));
  const curly = delimiters.some(
    (delimiter) => delimiter.prefix === "{" && delimiter.suffix === "}"
  );
  const segments = [], emphasis = [], loras = [], regions = [];
  const stack = [];
  let segmentBreaks = [];
  let quote = null, escape = false, segmentFrom = 0, index = 0, angleDepth = 0;
  while (index < source.length) {
    const alternate = alternateAt.get(index);
    if (alternate && !stack.length && quote === null) {
      index = alternate.outerRange.to;
      continue;
    }
    const character = requireValue(source[index], "parser");
    if (quote !== null) {
      if (escape) escape = false;
      else if (character === "\\") escape = true;
      else if (character === quote && isStructuralQuote(source, index)) quote = null;
      index++;
      continue;
    }
    const top = stack[stack.length - 1];
    if (top?.literal) {
      if (character === "\\" && index + 1 < source.length) {
        const next = source[index + 1];
        if (next === "(") {
          stack.push({
            opener: "\\(",
            closer: ")",
            from: index,
            literal: true,
            colon: null,
            commas: [],
            hardBreaks: []
          });
          index += 2;
          continue;
        }
        if (next === ")") {
          stack.pop();
          index += 2;
          continue;
        }
      }
      index++;
      continue;
    }
    if ((character === "'" || character === '"') && angleDepth === 0 && isStructuralQuote(source, index)) {
      quote = character;
      index++;
      continue;
    }
    if (character === "[") {
      const separator = regionSeparatorAt(source, index);
      if (separator) {
        regions.push(separator);
        stack.length = 0;
        angleDepth = 0;
        index = separator.tokenRange.to;
        continue;
      }
    }
    if (character === "\\") {
      if (source[index + 1] === "(")
        stack.push({
          opener: "\\(",
          closer: ")",
          from: index,
          literal: true,
          colon: null,
          commas: [],
          hardBreaks: []
        });
      index += index + 1 < source.length ? 2 : 1;
      continue;
    }
    const closer = pairs[character];
    if (closer) {
      stack.push({
        opener: character,
        closer,
        from: index,
        literal: false,
        colon: null,
        commas: [],
        hardBreaks: []
      });
      if (character === "<") angleDepth++;
      index++;
      continue;
    }
    if (")]}>".includes(character) && top) {
      if (top.closer === character) {
        stack.pop();
        if (top.opener === "<") {
          angleDepth--;
          const span = loraSpan(source, top.from, index);
          if (span) loras.push(span);
        } else if (top.opener === "(" && top.colon !== null) {
          const weight = weightField(source, top.colon + 1, index, false);
          if (weight)
            emphasis.push({
              outerRange: { from: top.from, to: index + 1 },
              contentRange: { from: top.from + 1, to: top.colon },
              weightRange: weight.range,
              weight: weight.value,
              depth: 0,
              segments: emphasisSegments(
                source,
                top.from + 1,
                top.colon,
                top.commas,
                top.hardBreaks
              )
            });
        } else if (top.opener === "{" && curly && !stack.some((frame) => frame.opener === "{")) {
          const span = wildcardSpan(source, top.from, index + 1, top.from + 1, index, true);
          if (span) wildcards.push(span);
        }
      }
      index++;
      continue;
    }
    if (character === ":" && top) top.colon = index;
    if (character === "," && top?.opener === "(") top.commas.push(index);
    if ((character === "\r" || character === "\n") && !(character === "\n" && source[index - 1] === "\r") && (!top || top.opener === "(")) {
      const to = index + (character === "\r" && source[index + 1] === "\n" ? 2 : 1);
      (top?.hardBreaks ?? segmentBreaks).push({ from: index, to });
      index = to;
      continue;
    }
    if (character === "," && !stack.length) {
      let to = index + 1;
      while (to < source.length && " 	\r\n".includes(requireValue(source[to], "parser"))) to++;
      segments.push({
        index: segments.length,
        text: source.slice(segmentFrom, index),
        contentRange: { from: segmentFrom, to: index },
        separatorRange: { from: index, to },
        hardBreaks: segmentBreaks
      });
      segmentFrom = to;
      segmentBreaks = [];
      index = to;
      continue;
    }
    index++;
  }
  if (segmentFrom < source.length)
    segments.push({
      index: segments.length,
      text: source.slice(segmentFrom),
      contentRange: { from: segmentFrom, to: source.length },
      separatorRange: null,
      hardBreaks: segmentBreaks
    });
  const emphasisSpans = withDepth(emphasis), loraSpans = withDepth(loras);
  const wildcardSpans = wildcards.sort(
    (a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to
  );
  const syntaxSpans = [
    ...emphasisSpans.map(
      (span) => ({
        kind: "emphasis",
        sourceRange: span.outerRange,
        depth: span.depth
      })
    ),
    ...wildcardSpans.map(
      (span) => ({
        kind: "wildcard",
        sourceRange: span.outerRange,
        depth: span.depth
      })
    ),
    ...loraSpans.map(
      (span) => ({
        kind: "lora",
        sourceRange: span.outerRange,
        depth: span.depth
      })
    )
  ].sort((a, b) => a.sourceRange.from - b.sourceRange.from || a.sourceRange.to - b.sourceRange.to);
  return freezeStructure({
    sourceText: source,
    segments,
    emphasisSpans,
    wildcardSpans,
    loraSpans,
    syntaxSpans,
    regions: regionStructure(source.length, regions),
    hasTrailingComma: strip(source, "right").endsWith(",")
  });
}

// frontend/sugarprompt/core/src/prompt/physical-lines.ts
function* physicalLines(source, emptyRecordPolicy = "omit-eof") {
  const lines = (
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    /[^\n\r\v\f\x1c-\x1e\x85\u2028\u2029]*(\r\n|[\n\r\v\f\x1c-\x1e\x85\u2028\u2029]|$)/gu
  );
  let contentAtEof = false;
  for (const match2 of source.matchAll(lines))
    if (match2[0]) {
      const ending = requireValue(match2[1], "physical-lines");
      contentAtEof = ending === "";
      yield Object.freeze({
        from: match2.index,
        to: match2.index + match2[0].length,
        text: match2[0],
        contentTo: match2.index + match2[0].length - ending.length,
        ending
      });
    }
  if (emptyRecordPolicy === "include-eof" && !contentAtEof)
    yield Object.freeze({
      from: source.length,
      to: source.length,
      contentTo: source.length,
      text: "",
      ending: ""
    });
}

// frontend/sugarprompt/core/src/prompt/scenes.ts
function normalizeSceneTitle(title) {
  return caseFold(
    strip(title).replace(
      // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
      /[\t-\r\x1c-\x20\x85\xa0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu,
      " "
    )
  );
}
function parseScenes(source, enabled = true) {
  const candidates = [];
  const seen = /* @__PURE__ */ new Set();
  for (const record2 of enabled ? physicalLines(source) : []) {
    const line = record2.text, from = record2.from, lineEnd = record2.to;
    const contentEnd = lineEnd - (line.endsWith("\r\n") ? 2 : /[\r\n]$/u.test(line) ? 1 : 0);
    const leading = requireValue(/^[ \t]*/u.exec(line), "scenes")[0].length;
    const markerFrom = from + leading, markerTo = markerFrom + 2;
    if (markerTo > contentEnd || source.slice(markerFrom, markerTo) !== "**") continue;
    const raw = source.slice(markerTo, contentEnd), title = strip(raw);
    if (!title) continue;
    const titleFrom = markerTo + raw.length - strip(raw, "left").length;
    const normalizedKey = normalizeSceneTitle(title);
    const marker = {
      title,
      normalizedKey,
      markerRange: { from: markerFrom, to: markerTo },
      titleRange: { from: titleFrom, to: titleFrom + title.length },
      lineRange: { from, to: contentEnd },
      duplicate: seen.has(normalizedKey)
    };
    candidates.push({ marker, contentFrom: lineEnd });
    seen.add(normalizedKey);
  }
  const universalRange = {
    from: 0,
    to: candidates[0]?.marker.lineRange.from ?? source.length
  };
  const scenes = candidates.map((candidate, index) => {
    const contentRange = {
      from: candidate.contentFrom,
      to: candidates[index + 1]?.marker.lineRange.from ?? source.length
    };
    return {
      marker: candidate.marker,
      contentRange,
      text: source.slice(contentRange.from, contentRange.to)
    };
  });
  return freezeStructure({
    sourceText: source,
    universalRange,
    universalText: source.slice(0, universalRange.to),
    scenes
  });
}

// frontend/sugarprompt/core/src/prompt/display.ts
function mapVisibleSource(source, sourceStart = 0) {
  if (!Number.isSafeInteger(sourceStart) || sourceStart < 0)
    throw new RangeError("Invalid visible-source origin");
  const escapes = parenthesisEscapes(source);
  const sourcePositions = [sourceStart], displayPositions = [0], characters = [];
  let escapeIndex = 0, position = 0, displayPosition = 0;
  while (position < source.length) {
    const escaped = escapes[escapeIndex]?.from === position;
    const character = escaped ? requireValue(source[position + 1], "display") : String.fromCodePoint(requireValue(source.codePointAt(position), "display"));
    position += escaped ? 2 : character.length;
    if (escaped) escapeIndex++;
    displayPosition += character.length;
    characters.push(character);
    sourcePositions.push(sourceStart + position);
    displayPositions.push(displayPosition);
  }
  return Object.freeze({
    text: characters.join(""),
    sourcePositions: Object.freeze(sourcePositions),
    displayPositions: Object.freeze(displayPositions)
  });
}
function parenthesisEscapes(source) {
  const ranges = [];
  for (let index = 0; index < source.length - 1; index++) {
    if (source[index] === "\\" && (source[index + 1] === "(" || source[index + 1] === ")")) {
      ranges.push({ from: index, to: index + 2 });
      index++;
    }
  }
  return ranges;
}

// frontend/sugarprompt/core/src/prompt/selection.ts
function segmentSelectionRange(segment) {
  const leading = requireValue(/^[ \t]*/u.exec(segment.text), "selection")[0].length;
  return {
    from: Math.min(segment.contentRange.to, segment.contentRange.from + leading),
    to: segment.contentRange.to
  };
}
function previousBoundary(source, position) {
  const previous = position - 1;
  return previous > 0 && /[\uDC00-\uDFFF]/u.test(requireValue(source[previous], "selection")) && /[\uD800-\uDBFF]/u.test(requireValue(source[previous - 1], "selection")) ? previous - 1 : previous;
}
function wordCharacter(character) {
  return unicodeAlphanumeric(character) || /^[\p{M}_-]$/u.test(character);
}
function promptWordRange(source, position) {
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length || position > 0 && position < source.length && /[\uD800-\uDBFF]/u.test(requireValue(source[position - 1], "selection")) && /[\uDC00-\uDFFF]/u.test(requireValue(source[position], "selection")))
    throw new RangeError("Invalid prompt-word UTF-16 boundary");
  let from = position, to = position;
  while (from > 0) {
    const previous = previousBoundary(source, from);
    if (!wordCharacter(source.slice(previous, from))) break;
    from = previous;
  }
  while (to < source.length) {
    const character = String.fromCodePoint(requireValue(source.codePointAt(to), "selection"));
    if (!wordCharacter(character)) break;
    to += character.length;
  }
  while (from < to) {
    const character = String.fromCodePoint(requireValue(source.codePointAt(from), "selection"));
    if (!/^\p{M}$/u.test(character)) break;
    from += character.length;
  }
  if (position < from) return { from: position, to: position };
  return { from, to };
}

// frontend/sugarprompt/core/src/completion/boundaries.ts
function assertCompletionBoundary(source, position) {
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length || position > 0 && position < source.length && /[\uD800-\uDBFF]/u.test(requireValue(source[position - 1], "boundaries")) && /[\uDC00-\uDFFF]/u.test(requireValue(source[position], "boundaries")) || source[position - 1] === "\r" && source[position] === "\n")
    throw new RangeError("Invalid completion UTF-16 source boundary");
}
function assertTagQuery(source, query) {
  const { from, to } = query.replacementRange;
  for (const position of [from, to, query.activeTagEnd]) assertCompletionBoundary(source, position);
  if (from > to || to > query.activeTagEnd) throw new RangeError("Invalid completion source range");
}

// frontend/sugarprompt/core/src/completion/query.ts
function floorIndex(entries, position, start) {
  let lower = 0, upper = entries.length;
  while (lower < upper) {
    const middle = lower + upper >>> 1;
    if (start(requireValue(entries[middle], "query")) <= position) lower = middle + 1;
    else upper = middle;
  }
  return lower - 1;
}
function indexEmphasis(spans) {
  const indexed = [], stack = [];
  for (const span of spans) {
    while (stack.length && requireValue(indexed[requireValue(stack.at(-1), "query")], "query").span.outerRange.to < span.outerRange.to)
      stack.pop();
    indexed.push({ span, parent: stack.at(-1) ?? -1 });
    stack.push(indexed.length - 1);
  }
  return indexed;
}
function lineRanges(source) {
  const ranges = [];
  let from = 0;
  for (let index = 0; index < source.length; index++) {
    if (source[index] !== "\r" && source[index] !== "\n") continue;
    ranges.push({ from, to: index });
    if (source[index] === "\r" && source[index + 1] === "\n") index++;
    from = index + 1;
  }
  ranges.push({ from, to: source.length });
  return ranges;
}
function visibleIndex(visible, position) {
  const index = floorIndex(visible.sourcePositions, position, (value) => value);
  return visible.sourcePositions[index] === position ? index : -1;
}
function queryForRange(source, visible, range, cursor, minimum) {
  let from = range.from, to = range.to;
  while (from < to && whitespace.test(requireValue(source[from], "query"))) from++;
  while (to > from && whitespace.test(requireValue(source[to - 1], "query"))) to--;
  if (cursor < from) return null;
  range = { from, to: Math.max(cursor, to) };
  const start = visibleIndex(visible, range.from), caret = visibleIndex(visible, cursor);
  if (start < 0 || caret < start || caret - start < minimum) return null;
  const displayStart = requireValue(visible.displayPositions[start], "query");
  const prefix = visible.text.slice(displayStart, visible.displayPositions[caret]);
  let suffixStart = 0;
  for (let index = 0; index < prefix.length; index++)
    if (whitespace.test(requireValue(prefix[index], "query"))) suffixStart = index + 1;
  const suffix2 = prefix.slice(suffixStart);
  let fallback = null;
  if (suffixStart > 0 && Array.from(suffix2).length >= minimum) {
    let localEnd = cursor;
    while (localEnd < range.to && source[localEnd] !== "," && !whitespace.test(requireValue(source[localEnd], "query")))
      localEnd++;
    const suffixOrdinal = start + Array.from(prefix.slice(0, suffixStart)).length;
    fallback = Object.freeze({
      prefix: suffix2,
      replacementRange: Object.freeze({
        from: requireValue(visible.sourcePositions[suffixOrdinal], "query"),
        to: cursor
      }),
      activeTagEnd: localEnd,
      fallback: null
    });
  }
  return Object.freeze({
    prefix,
    replacementRange: Object.freeze({ from: range.from, to: cursor }),
    activeTagEnd: range.to,
    fallback
  });
}
function prepareTagQueries(document) {
  const source = document.sourceText;
  const segments = document.segments.map(segmentSelectionRange);
  const emphasis = indexEmphasis(document.emphasisSpans);
  const lines = lineRanges(source), visible = mapVisibleSource(source);
  return Object.freeze({
    /** Resolve scalar-safe source context; selection and insufficient visible prefix suppress suggestions. */
    queryAt(position, hasSelection, minimumPrefixLength = 2) {
      assertCompletionBoundary(source, position);
      if (!Number.isSafeInteger(minimumPrefixLength) || minimumPrefixLength < 1)
        throw new RangeError("Invalid completion prefix length");
      if (hasSelection) return null;
      const segment = segments[floorIndex(segments, position, (range) => range.from)];
      if (!segment || position > segment.to) return null;
      let spanIndex = floorIndex(emphasis, position, (entry) => entry.span.outerRange.from);
      while (spanIndex >= 0) {
        const entry = requireValue(emphasis[spanIndex], "query");
        const outer = entry.span.outerRange;
        if (position <= outer.to && outer.from >= segment.from && outer.to <= segment.to) {
          const content = entry.span.contentRange;
          return position < content.from || position > content.to ? null : queryForRange(source, visible, content, position, minimumPrefixLength);
        }
        spanIndex = entry.parent;
      }
      const line = requireValue(lines[floorIndex(lines, position, (range) => range.from)], "query");
      return queryForRange(
        source,
        visible,
        { from: Math.max(segment.from, line.from), to: Math.min(segment.to, line.to) },
        position,
        minimumPrefixLength
      );
    }
  });
}

// frontend/sugarprompt/core/src/completion/lora-query.ts
var querySources = /* @__PURE__ */ new WeakMap();
function assertPreparedLoraQuery(source, query) {
  if (querySources.get(query) !== source) throw new TypeError("Stale or unprepared LoRA query");
}
function lowerBound(positions, position) {
  let lower = 0, upper = positions.length;
  while (lower < upper) {
    const middle = lower + upper >>> 1;
    if (requireValue(positions[middle], "lora-query") < position) lower = middle + 1;
    else upper = middle;
  }
  return lower;
}
function prepareLoraQueries(source) {
  if (typeof source !== "string") throw new TypeError("Invalid LoRA completion source");
  const markers = [], less = [], greater = [], colons = [], endings = [];
  for (let position = 0; position < source.length; position++) {
    const character = requireValue(source[position], "lora-query");
    if (character === "<") {
      less.push(position);
      if (/^<lora:$/iu.test(source.slice(position, position + 6))) markers.push(position);
    } else if (character === ">") greater.push(position);
    else if (character === ":") colons.push(position);
    if (",\r\n	 ".includes(character)) endings.push(position);
  }
  const next = (positions, position) => positions[lowerBound(positions, position)] ?? -1;
  const slots = markers.map((from) => {
    const nameFrom = from + 6, nextGreater = next(greater, nameFrom), nextLess = next(less, from + 1);
    const closer = nextGreater >= 0 && (nextLess < 0 || nextGreater < nextLess) ? nextGreater : -1;
    const separator = next(colons, nameFrom);
    let typedWeightText = null, preservedSuffix = null;
    if (separator >= 0 && (closer < 0 || separator < closer)) {
      const openEnd = next(endings, separator), bodyEnd = closer >= 0 ? closer : openEnd < 0 ? source.length : openEnd;
      const second = next(colons, separator + 1);
      const firstEnd = second >= 0 && second < bodyEnd ? second : bodyEnd;
      const weight = strip(source.slice(separator + 1, firstEnd));
      if (weight && parseWeight(weight, true) !== null) typedWeightText = weight;
      if (loraWeightFields(source, separator, bodyEnd))
        preservedSuffix = source.slice(separator, bodyEnd);
    }
    return Object.freeze({
      from,
      closer,
      nextGreater,
      separator,
      typedWeightText,
      preservedSuffix
    });
  });
  return Object.freeze({
    /** Name-only caret lookup preserves exact source offsets even when Unicode case folding expands preceding characters. */
    queryAt(position, hasSelection) {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      const slot = slots[lowerBound(markers, position - 4) - 1];
      if (!slot || position < slot.from + 6 || slot.nextGreater >= 0 && position > slot.nextGreater)
        return null;
      const openEnd = next(endings, position);
      const bodyEnd = slot.closer >= 0 ? slot.closer : openEnd < 0 ? source.length : openEnd;
      const ownsWeight = slot.separator >= 0 && slot.separator < bodyEnd;
      if (ownsWeight && position > slot.separator) return null;
      const nameRange = Object.freeze({
        from: slot.from + 6,
        to: position
      });
      const query = Object.freeze({
        queryText: source.slice(nameRange.from, nameRange.to),
        nameRange,
        nameFieldRange: Object.freeze({
          from: nameRange.from,
          to: ownsWeight ? slot.separator : bodyEnd
        }),
        replacementRange: Object.freeze({
          from: slot.from,
          to: slot.closer >= 0 ? slot.closer + 1 : bodyEnd
        }),
        typedWeightText: ownsWeight ? slot.typedWeightText : null,
        preservedSuffix: ownsWeight ? slot.preservedSuffix : null,
        hasClosingBracket: slot.closer >= 0
      });
      querySources.set(query, source);
      return query;
    }
  });
}

// frontend/sugarprompt/core/src/completion/wildcard-query.ts
var querySources2 = /* @__PURE__ */ new WeakMap();
function assertPreparedQuery(source, query) {
  if (querySources2.get(query) !== source) throw new TypeError("Stale or unprepared wildcard query");
}
function lowerBound2(positions, position) {
  let lower = 0, upper = positions.length;
  while (lower < upper) {
    const middle = lower + upper >>> 1;
    if (requireValue(positions[middle], "wildcard-query") < position) lower = middle + 1;
    else upper = middle;
  }
  return lower;
}
function prepareWildcardQueries(source) {
  if (typeof source !== "string") throw new TypeError("Invalid wildcard completion source");
  const openers = [], closers = [], endings = [], invalid = [], nonblank = [];
  for (let position = source.indexOf("{"); position >= 0 && position < source.length; position++) {
    const character = requireValue(source[position], "wildcard-query");
    if (character === "{") openers.push(position);
    if (character === "}") closers.push(position);
    if (character === "\r" || character === "\n") endings.push(position);
    const blank = whitespace.test(character);
    if (blank || "{},".includes(character)) invalid.push(position);
    if (!blank) nonblank.push(position);
  }
  return Object.freeze({
    /** Binary lookups qualify the nearest opener and first closer without traversing unrelated source. */
    queryAt(position, hasSelection) {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      const from = openers[lowerBound2(openers, position) - 1];
      if (from === void 0) return null;
      const lineFrom = endings[lowerBound2(endings, position) - 1] ?? -1;
      if (from <= lineFrom) return null;
      const contentFrom = from + 1, blocked = invalid[lowerBound2(invalid, contentFrom)];
      if (blocked !== void 0 && blocked < position) return null;
      const lineTo = endings[lowerBound2(endings, position)] ?? source.length, closing = closers[lowerBound2(closers, position)];
      const hasCloser = closing !== void 0 && closing < lineTo;
      if (hasCloser) {
        const right = nonblank[lowerBound2(nonblank, position)];
        if (right !== void 0 && right < closing) return null;
      }
      const query = Object.freeze({
        prefix: source.slice(contentFrom, position),
        contentRange: Object.freeze({ from: contentFrom, to: position }),
        contentFieldRange: Object.freeze({
          from: contentFrom,
          to: hasCloser ? closing : position
        }),
        replacementRange: Object.freeze({
          from,
          to: hasCloser ? closing + 1 : position
        })
      });
      querySources2.set(query, source);
      return query;
    }
  });
}
function isWildcardCompletionBody(body, form) {
  if (typeof body !== "string" || body.length > 4096 || form !== "simple" && form !== "csv")
    return false;
  const text = `{${body}}`, span = wildcardSpan(text, 0, text.length, 1, text.length - 1, true);
  return !!span && span.tag === null && span.form === form;
}
function wildcardReplacement(source, query, body, form) {
  assertPreparedQuery(source, query);
  if (!isWildcardCompletionBody(body, form))
    throw new RangeError("Wildcard completion must preserve one untagged placeholder");
  return Object.freeze({ range: query.replacementRange, text: `{${body}}` });
}
function wildcardGhostText(source, query, body) {
  assertPreparedQuery(source, query);
  const prefix = caseFold(query.prefix);
  if (!caseFold(body).startsWith(prefix)) return "";
  let folded = 0, original = 0;
  for (const character of body) {
    if (folded === prefix.length) break;
    folded += caseFold(character).length;
    original += character.length;
    if (folded > prefix.length) return "";
  }
  const closer = query.replacementRange.to > query.contentFieldRange.to ? "" : "}";
  return body.slice(original) + closer;
}

// frontend/sugarprompt/core/src/completion/scene-query.ts
var querySources3 = /* @__PURE__ */ new WeakMap();
function prepareSceneQueries(source) {
  if (typeof source !== "string") throw new TypeError("Invalid scene query source");
  const slots = [];
  for (let from = 0; from <= source.length; ) {
    let to = from;
    while (to < source.length && source[to] !== "\r" && source[to] !== "\n") to++;
    let markerFrom = from;
    while (markerFrom < to && /[ \t]/u.test(requireValue(source[markerFrom], "scene-query")))
      markerFrom++;
    if (markerFrom + 2 <= to && source.slice(markerFrom, markerFrom + 2) === "**") {
      let nonblankEnd = to;
      while (nonblankEnd > markerFrom + 2 && whitespace.test(requireValue(source[nonblankEnd - 1], "scene-query")))
        nonblankEnd--;
      slots.push({ markerFrom, titleFrom: markerFrom + 2, to, nonblankEnd });
    }
    if (to === source.length) break;
    from = to + (source[to] === "\r" && source[to + 1] === "\n" ? 2 : 1);
  }
  return Object.freeze({
    /** Binary lookup proves the complete right slice is blank before capturing one issued source query. */
    queryAt(position, hasSelection) {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      let lower = 0, upper = slots.length;
      while (lower < upper) {
        const middle = lower + upper >>> 1;
        if (requireValue(slots[middle], "scene-query").titleFrom <= position) lower = middle + 1;
        else upper = middle;
      }
      const slot = slots[lower - 1];
      if (!slot || position > slot.to || position < slot.nonblankEnd) return null;
      const query = Object.freeze({
        prefix: source.slice(slot.titleFrom, position),
        markerRange: Object.freeze({
          from: slot.markerFrom,
          to: slot.titleFrom
        }),
        replacementRange: Object.freeze({ from: slot.titleFrom, to: position })
      });
      querySources3.set(query, source);
      return query;
    }
  });
}
function isSceneTitle(value) {
  return typeof value === "string" && value.length <= 4096 && !!strip(value) && // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  !/[\0\n-\r\x1c-\x1e\x85\u2028\u2029]/u.test(value);
}
function sceneReplacement(source, query, title) {
  if (querySources3.get(query) !== source) throw new TypeError("Stale or unprepared scene query");
  if (!isSceneTitle(title)) throw new TypeError("Invalid scene title");
  return Object.freeze({ range: query.replacementRange, text: title });
}

// frontend/sugarprompt/core/src/scenes/context.ts
function joinTrimmedPrompt(universal, local) {
  return universal && local ? `${universal}

${local}` : universal || local;
}
function prepareSceneContexts(document) {
  const universal = Object.freeze({
    sourceRange: document.universalRange,
    scene: null,
    effectivePromptText: document.universalText
  });
  const trimmedUniversal = strip(document.universalText);
  const includesUniversal = document.universalRange.to > 0 || document.scenes.length === 0;
  const effective = includesUniversal ? [universal.effectivePromptText] : [];
  const localTexts = /* @__PURE__ */ new Set();
  const scenes = Object.freeze(
    document.scenes.map((scene) => {
      const local = strip(scene.text);
      const context = Object.freeze({
        sourceRange: Object.freeze({
          from: scene.marker.lineRange.from,
          to: scene.contentRange.to
        }),
        scene,
        effectivePromptText: joinTrimmedPrompt(trimmedUniversal, local)
      });
      if (!localTexts.has(local) && !(includesUniversal && !local && trimmedUniversal === document.universalText))
        effective.push(context.effectivePromptText);
      localTexts.add(local);
      return context;
    })
  );
  const effectivePromptTexts = Object.freeze(effective);
  return Object.freeze({
    document,
    universal,
    scenes,
    effectivePromptTexts,
    /** Heading indentation belongs to the following scene; only the final scene owns EOF. */
    at(position) {
      if (!Number.isSafeInteger(position)) throw new RangeError("Invalid scene source position");
      const clamped = Math.max(0, Math.min(position, document.sourceText.length));
      let from = 0, to = scenes.length;
      while (from < to) {
        const middle = from + to >>> 1;
        if (requireValue(scenes[middle], "context").sourceRange.from <= clamped) from = middle + 1;
        else to = middle;
      }
      return scenes[from - 1] ?? universal;
    }
  });
}

// frontend/sugarprompt/core/src/wildcard/groups.ts
function wildcardBodyRange(span) {
  return Object.freeze({
    from: span.contentRange.from + (span.form === "csv" ? 4 : 0),
    to: span.contentRange.to - (span.tag === null ? 0 : span.tag.length + 1)
  });
}
function isNumericWildcardTag(tag2) {
  return tag2 !== null && /^[1-9][0-9]*$/u.test(tag2);
}
function prepareWildcardGroups(spans) {
  const counts = /* @__PURE__ */ new Map();
  const keys = spans.map((span) => `${span.form}:${span.identifier}`);
  for (const key of keys) counts.set(key, (counts.get(key) ?? 0) + 1);
  return Object.freeze(
    spans.map((span, index) => {
      const sourceKey = requireValue(keys[index], "groups"), occurrenceCount = requireValue(counts.get(sourceKey), "groups"), displayTag = span.tag ?? (occurrenceCount > 1 ? "1" : null);
      return Object.freeze({
        span,
        sourceKey,
        occurrenceCount,
        displayTag,
        explicit: span.tag !== null,
        numeric: isNumericWildcardTag(displayTag)
      });
    })
  );
}

// frontend/sugarprompt/core/src/document/containers.ts
function admitDocumentContainer(prepared, source, kind, container) {
  if (prepared.source !== source || prepared.kind !== kind)
    return Object.freeze({ status: "rejected", reason: "stale" });
  if (prepared.containersById[container.id] !== container)
    return Object.freeze({ status: "rejected", reason: "foreign" });
  return requireValue(prepared.containerOutcomes[container.id], "containers");
}
function documentContainerAtSource(prepared, source, kind, position) {
  if (prepared.source !== source || prepared.kind !== kind)
    return Object.freeze({ status: "rejected", reason: "stale" });
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length)
    return Object.freeze({ status: "rejected", reason: "missing" });
  let low = 0, high = prepared.containers.length;
  while (low < high) {
    const middle = low + high >>> 1;
    if (requireValue(prepared.containers[middle], "containers").storageRange.to < position)
      low = middle + 1;
    else high = middle;
  }
  const container = prepared.containers[low];
  if (!container || position < container.storageRange.from)
    return Object.freeze({ status: "rejected", reason: "missing" });
  const outcome = requireValue(prepared.containerOutcomes[container.id], "containers");
  if (outcome.status === "protected") return outcome;
  const mapping = requireValue(container.mapping, "containers");
  if (mapping.sourceToDecoded[position - mapping.sourceRange.from] == null)
    return Object.freeze({ status: "rejected", reason: "unmapped" });
  return outcome;
}

// frontend/sugarprompt/core/src/offsets.ts
function codePointBoundaries(source) {
  const boundaries = [0];
  for (const character of source)
    boundaries.push(requireValue(boundaries[boundaries.length - 1], "offsets") + character.length);
  return boundaries;
}
function utf16ToCodePoint(source, position) {
  const value = codePointBoundaries(source).indexOf(position);
  if (!Number.isSafeInteger(position) || value < 0)
    throw new RangeError("Invalid UTF-16 code-point boundary");
  return value;
}

// frontend/sugarprompt/core/src/line-endings.ts
function encodeSource(source) {
  const endings = [];
  const logical = source.replace(/\r\n|\r|\n/gu, (ending) => {
    endings.push(ending);
    return "\n";
  });
  return { logical, endings };
}
function decodeSource(logical, endings) {
  let index = 0;
  const source = logical.replace(/\n/gu, () => {
    const ending = endings[index++];
    if (ending === void 0) throw new Error("Missing line-ending metadata");
    return ending;
  });
  if (index !== endings.length || logical.includes("\r"))
    throw new Error("Invalid logical source representation");
  return source;
}
function documentToSource(logical, endings, position) {
  if (!Number.isSafeInteger(position) || position < 0 || position > logical.length)
    throw new RangeError("Invalid document position");
  let extra = 0, index = 0;
  for (let cursor = 0; cursor < position; cursor++)
    if (logical[cursor] === "\n") extra += (endings[index++]?.length ?? 1) - 1;
  return position + extra;
}
function sourceToDocument(logical, endings, position) {
  return requireValue(sourcePositionsToDocument(logical, endings, [position])[0], "line-endings");
}
function sourcePositionsToDocument(logical, endings, positions) {
  if (!endings.includes("\r\n"))
    return positions.map((position) => {
      if (!Number.isSafeInteger(position) || position < 0 || position > logical.length)
        throw new RangeError("Invalid source position");
      return position;
    });
  const ordered = positions.map((position, index) => {
    if (!Number.isSafeInteger(position) || position < 0)
      throw new RangeError("Invalid source position");
    return { position, index };
  }).sort((left, right) => left.position - right.position);
  const converted = new Array(positions.length);
  let source = 0, cursor = 0, newline = 0;
  for (const boundary2 of ordered) {
    while (source < boundary2.position && cursor < logical.length) {
      source += logical[cursor++] === "\n" ? endings[newline++]?.length ?? 1 : 1;
    }
    if (source > boundary2.position) throw new RangeError("Position is inside CRLF");
    if (source !== boundary2.position) throw new RangeError("Invalid source position");
    converted[boundary2.index] = cursor;
  }
  return converted;
}

// frontend/sugarprompt/core/src/normalization/replacements.ts
function identityNormalization(text) {
  return Object.freeze({
    text,
    boundaryPositions: Object.freeze(Array.from({ length: text.length + 1 }, (_, index) => index)),
    transitions: Object.freeze([]),
    generatedEmphases: Object.freeze([])
  });
}
function applyReplacements(text, edits, transitions = []) {
  if (!edits.length && !transitions.length) return identityNormalization(text);
  const parts = [], boundaries = [];
  let source = 0, target = 0;
  for (const edit of edits) {
    if (edit.from < source || edit.to < edit.from || edit.to > text.length || !Number.isInteger(edit.from) || !Number.isInteger(edit.to))
      throw new RangeError("Invalid normalization replacements");
    parts.push(text.slice(source, edit.from));
    for (let index = source; index <= edit.from; index++)
      boundaries[index] = target + index - source;
    target += edit.from - source;
    parts.push(edit.text);
    const length = edit.to - edit.from;
    if (edit.boundaryPositions && edit.boundaryPositions.length !== length + 1)
      throw new Error("Incomplete local boundary mapping");
    let previous = 0;
    for (let offset = 0; offset <= length; offset++) {
      const position = edit.boundaryPositions ? edit.boundaryPositions[offset] : offset === length ? edit.text.length : Math.min(offset, edit.text.length);
      if (position === void 0 || !Number.isSafeInteger(position) || position < previous || position > edit.text.length)
        throw new Error("Invalid local boundary mapping");
      boundaries[edit.from + offset] = target + position;
      previous = position;
    }
    source = edit.to;
    target += edit.text.length;
  }
  parts.push(text.slice(source));
  for (let index = source; index <= text.length; index++)
    boundaries[index] = target + index - source;
  return Object.freeze({
    text: parts.join(""),
    boundaryPositions: Object.freeze(boundaries),
    transitions: Object.freeze(transitions.map((transition) => Object.freeze({ ...transition }))),
    generatedEmphases: Object.freeze([])
  });
}
function composeNormalizations(first, second) {
  if (second.transitions.length)
    throw new Error("Numeric normalization cannot add parenthesis transitions");
  return Object.freeze({
    text: second.text,
    boundaryPositions: Object.freeze(
      first.boundaryPositions.map(
        (position) => requireValue(second.boundaryPositions[position], "replacements")
      )
    ),
    transitions: first.transitions,
    generatedEmphases: Object.freeze(
      first.generatedEmphases.map(
        (shell) => Object.freeze({
          ...shell,
          from: requireValue(second.boundaryPositions[shell.from], "replacements"),
          to: requireValue(second.boundaryPositions[shell.to], "replacements")
        })
      )
    )
  });
}

// frontend/sugarprompt/core/src/normalization/ranges.ts
function isSourceBoundary(text, position) {
  return Number.isInteger(position) && position >= 0 && position <= text.length && !(position > 0 && text[position - 1] === "\r" && text[position] === "\n");
}
function validateSourceRange(text, range) {
  if (!Number.isInteger(range.from) || !Number.isInteger(range.to) || range.from < 0 || range.to < range.from || range.to > text.length)
    throw new RangeError("Invalid normalization range");
  for (const position of [range.from, range.to])
    if (!isSourceBoundary(text, position))
      throw new RangeError("Normalization boundary splits CRLF");
}
function normalizeRange(text, range, normalize) {
  validateSourceRange(text, range);
  if (range.from === range.to) return identityNormalization(text);
  const result = normalize(text.slice(range.from, range.to));
  const normalized = applyReplacements(
    text,
    [
      {
        ...range,
        text: result.text,
        boundaryPositions: result.boundaryPositions
      }
    ],
    result.transitions.map((transition) => ({
      ...transition,
      from: range.from + transition.from,
      to: range.from + transition.to
    }))
  );
  return Object.freeze({
    ...normalized,
    generatedEmphases: Object.freeze(
      result.generatedEmphases.map(
        (shell) => Object.freeze({
          ...shell,
          from: range.from + shell.from,
          to: range.from + shell.to
        })
      )
    )
  });
}

// frontend/sugarprompt/core/src/document/value-mapping.ts
function prepareSourceOffsets(source) {
  const pythonToUtf16 = codePointBoundaries(source);
  const singleWidthScalars = pythonToUtf16.length === source.length + 1;
  const inverse = singleWidthScalars ? null : Array.from({ length: source.length + 1 }, () => null);
  if (inverse)
    pythonToUtf16.forEach((position, index) => {
      inverse[position] = index;
    });
  return Object.freeze({
    source,
    pythonToUtf16: Object.freeze(pythonToUtf16),
    utf16ToPython: Object.freeze(inverse ?? pythonToUtf16),
    endings: Object.freeze(encodeSource(source).endings)
  });
}
function trimmedValueRange(text) {
  const from = text.length - strip(text, "left").length;
  return Object.freeze({
    from,
    to: Math.max(from, strip(text, "right").length)
  });
}
function prepareIdentityMapping(sourceOffsets, text, from) {
  const sourceRange2 = { from, to: from + text.length };
  validateSourceRange(sourceOffsets.source, sourceRange2);
  if (sourceOffsets.utf16ToPython[from] == null || sourceOffsets.utf16ToPython[sourceRange2.to] == null)
    throw new Error("invalid-value-scalar-extents");
  const complete = from === 0 && text === sourceOffsets.source;
  const pythonToDecoded = complete ? sourceOffsets.pythonToUtf16 : codePointBoundaries(text);
  const identityCoordinates = complete && pythonToDecoded.length === text.length + 1 && !text.includes("\r\n");
  const inverse = complete ? null : Array.from({ length: text.length + 1 }, () => null);
  const decodedBoundaries = identityCoordinates ? null : Array.from({ length: text.length + 1 }, () => null);
  const sourceBoundaries = from === 0 ? decodedBoundaries : Array.from({ length: text.length + 1 }, () => null);
  if (decodedBoundaries && sourceBoundaries)
    pythonToDecoded.forEach((position, python) => {
      if (inverse) inverse[position] = python;
      const absolute = from + position;
      if (!isSourceBoundary(text, position) || !isSourceBoundary(sourceOffsets.source, absolute) || sourceOffsets.utf16ToPython[absolute] == null)
        return;
      decodedBoundaries[position] = absolute;
      sourceBoundaries[position] = position;
    });
  return Object.freeze({
    text,
    sourceRange: Object.freeze(sourceRange2),
    decodedToSource: Object.freeze(decodedBoundaries ?? pythonToDecoded),
    sourceToDecoded: Object.freeze(sourceBoundaries ?? pythonToDecoded),
    pythonToDecoded: Object.freeze(pythonToDecoded),
    decodedToPython: Object.freeze(inverse ?? sourceOffsets.utf16ToPython),
    sourceOffsets: freezeStructure(sourceOffsets)
  });
}
function prepareValueMapping(sourceOffsets, text, characterRanges, emptyAnchor) {
  if (characterRanges.length !== text.length) throw new Error("invalid-value-extents");
  const sourceRange2 = {
    from: characterRanges[0]?.from ?? emptyAnchor,
    to: characterRanges.at(-1)?.to ?? emptyAnchor
  };
  validateSourceRange(sourceOffsets.source, sourceRange2);
  if (sourceOffsets.utf16ToPython[sourceRange2.from] == null || sourceOffsets.utf16ToPython[sourceRange2.to] == null)
    throw new Error("invalid-value-scalar-extents");
  for (let index = 0; index < characterRanges.length; index++) {
    const range = requireValue(characterRanges[index], "value-mapping");
    if (range.from >= range.to || range.from < sourceRange2.from || range.to > sourceRange2.to || index > 0 && requireValue(characterRanges[index - 1], "value-mapping").to !== range.from)
      throw new Error("invalid-value-extents");
  }
  const pythonToDecoded = codePointBoundaries(text);
  const decodedToPython = Array.from({ length: text.length + 1 }, () => null);
  const decodedToSource = Array.from({ length: text.length + 1 }, () => null);
  const sourceToDecoded = Array.from(
    { length: sourceRange2.to - sourceRange2.from + 1 },
    () => null
  );
  pythonToDecoded.forEach((position, python) => {
    decodedToPython[position] = python;
    const absolute = characterRanges[position]?.from ?? sourceRange2.to;
    if (!isSourceBoundary(text, position) || !isSourceBoundary(sourceOffsets.source, absolute))
      return;
    if (sourceOffsets.utf16ToPython[absolute] == null) return;
    decodedToSource[position] = absolute;
    sourceToDecoded[absolute - sourceRange2.from] = position;
  });
  return Object.freeze({
    text,
    sourceRange: Object.freeze(sourceRange2),
    decodedToSource: Object.freeze(decodedToSource),
    sourceToDecoded: Object.freeze(sourceToDecoded),
    pythonToDecoded: Object.freeze(pythonToDecoded),
    decodedToPython: Object.freeze(decodedToPython),
    sourceOffsets: freezeStructure(sourceOffsets)
  });
}
function validRange(range, maximum) {
  return Number.isSafeInteger(range.from) && Number.isSafeInteger(range.to) && range.from >= 0 && range.from <= range.to && range.to <= maximum;
}
function decodedRangeToSource(mapping, range) {
  if (!validRange(range, mapping.text.length)) return null;
  const from = mapping.decodedToSource[range.from], to = mapping.decodedToSource[range.to];
  return from == null || to == null ? null : Object.freeze({ from, to });
}
function sourceRangeToDecoded(mapping, range) {
  if (!validRange(range, mapping.sourceRange.to) || range.from < mapping.sourceRange.from)
    return null;
  const from = mapping.sourceToDecoded[range.from - mapping.sourceRange.from], to = mapping.sourceToDecoded[range.to - mapping.sourceRange.from];
  return from == null || to == null ? null : Object.freeze({ from, to });
}

// frontend/sugarprompt/core/src/document/prompt-scopes.ts
function prepareDocumentPromptScopes(values, syntax, previous) {
  const profileKey = JSON.stringify(wildcardDelimiters(syntax));
  if (previous?.values === values && previous.profileKey === profileKey) return previous;
  const scopes = [];
  const byId = /* @__PURE__ */ Object.create(null);
  for (const container of values.containers) {
    if (admitDocumentContainer(values, values.source, values.kind, container).status !== "editable")
      continue;
    const fullMapping = container.mapping;
    if (!fullMapping) throw new Error("missing-editable-container-map");
    const anchor = decodedRangeToSource(fullMapping, container.valueRange);
    if (!anchor) throw new Error("invalid-container-value-map");
    const mapping = container.value?.mapping ?? (values.kind === "prompt" ? fullMapping : prepareIdentityMapping(values.sourceOffsets, "", anchor.from));
    const previousScope = previous?.values === values && previous.byId[container.id]?.container === container ? previous.byId[container.id] : void 0;
    const document = parsePrompt(mapping.text, syntax);
    const scope = freezeStructure({
      id: container.id,
      container,
      value: container.value,
      mapping,
      document,
      wildcardGroups: prepareWildcardGroups(document.wildcardSpans),
      tagQueries: prepareTagQueries(document),
      loraQueries: prepareLoraQueries(mapping.text),
      wildcardQueries: prepareWildcardQueries(mapping.text),
      sceneContexts: values.kind === "prompt" ? previousScope?.sceneContexts ?? prepareSceneContexts(parseScenes(mapping.text)) : null,
      sceneQueries: values.kind === "prompt" ? previousScope?.sceneQueries ?? prepareSceneQueries(mapping.text) : null
    });
    scopes.push(scope);
    byId[scope.id] = scope;
  }
  return freezeStructure({ values, profileKey, scopes, byId });
}
function admitDocumentPromptScope(prepared, source, kind, scope) {
  if (prepared.values.source !== source || prepared.values.kind !== kind)
    return { status: "rejected", reason: "stale" };
  if (prepared.byId[scope.id] !== scope) return { status: "rejected", reason: "foreign" };
  const admitted = admitDocumentContainer(prepared.values, source, kind, scope.container);
  if (admitted.status !== "editable") return { status: "rejected", reason: admitted.reason };
  return {
    status: "ready",
    scope,
    decodedRange: { from: 0, to: scope.mapping.text.length }
  };
}
function documentPromptScopeAtSource(prepared, source, kind, exactRange) {
  if (prepared.values.source !== source || prepared.values.kind !== kind)
    return { status: "rejected", reason: "stale" };
  if (!Number.isSafeInteger(exactRange.from) || !Number.isSafeInteger(exactRange.to) || exactRange.from > exactRange.to)
    return { status: "rejected", reason: "unmapped" };
  const first = documentContainerAtSource(prepared.values, source, kind, exactRange.from);
  const last = documentContainerAtSource(prepared.values, source, kind, exactRange.to);
  if (first.status !== "editable") return { status: "rejected", reason: first.reason };
  if (last.status !== "editable") return { status: "rejected", reason: last.reason };
  if (first.container !== last.container) return { status: "rejected", reason: "cross-value" };
  const scope = prepared.byId[first.container.id];
  if (!scope) throw new Error("missing-editable-document-scope");
  const decodedRange = sourceRangeToDecoded(scope.mapping, exactRange);
  if (!decodedRange) return { status: "rejected", reason: "padding" };
  return { status: "ready", scope, decodedRange };
}

// frontend/sugarprompt/core/src/document/csv.ts
function prepareWildcardCsvEncoding(cell, value, forceQuoted = false) {
  const quoted = cell.quoted || forceQuoted || /[,"\r\n]/u.test(value), parts = [], ranges = [];
  let cursor = quoted ? 1 : 0;
  for (let at = 0; at < value.length; at++) {
    const literal = quoted && value[at] === '"' ? '""' : requireValue(value[at], "csv");
    parts.push(literal);
    ranges.push({ from: cursor, to: cursor + literal.length });
    cursor += literal.length;
  }
  const content = parts.join(""), text = quoted ? `"${content}"${cell.postQuoteText}` : content;
  return freezeStructure({
    text,
    quoted,
    mapping: prepareValueMapping(prepareSourceOffsets(text), value, ranges, quoted ? 1 : 0)
  });
}
function parseWildcardCsv(source) {
  const records = [];
  let row = [], index = 0, valid = true;
  while (index <= source.length) {
    const from = index, characters = [], characterRanges = [];
    const quoted = source[index] === '"';
    let postQuoteText = "";
    if (quoted) {
      index++;
      let closed = false;
      while (index < source.length) {
        if (source[index] !== '"') {
          characters.push(requireValue(source[index], "csv"));
          characterRanges.push({ from: index, to: index + 1 });
          index++;
        } else if (source[index + 1] === '"') {
          characters.push('"');
          characterRanges.push({ from: index, to: index + 2 });
          index += 2;
        } else {
          index++;
          closed = true;
          break;
        }
      }
      if (!closed) valid = false;
      const postQuoteStart = index;
      while (index < source.length && !/[,\r\n]/u.test(requireValue(source[index], "csv"))) {
        if (!whitespace.test(requireValue(source[index], "csv"))) valid = false;
        index++;
      }
      postQuoteText = source.slice(postQuoteStart, index);
    } else {
      while (index < source.length && !/[,\r\n]/u.test(requireValue(source[index], "csv"))) {
        if (source[index] === '"') valid = false;
        characters.push(requireValue(source[index], "csv"));
        characterRanges.push({ from: index, to: index + 1 });
        index++;
      }
    }
    row.push({
      range: { from, to: index },
      quoted,
      postQuoteText,
      value: characters.join(""),
      characterRanges
    });
    if (index >= source.length) {
      records.push(row);
      break;
    }
    if (source[index] === ",") {
      index++;
      continue;
    }
    if (source[index++] === "\r" && source[index] === "\n") index++;
    records.push(row);
    row = [];
    if (index === source.length) break;
  }
  return freezeStructure({ valid, records });
}

// frontend/sugarprompt/core/src/document/values.ts
function prepareDocumentValues(source, kind, previous) {
  if (previous?.source === source && previous.kind === kind) return previous;
  const sourceOffsets = prepareSourceOffsets(source);
  const values = [];
  const containers = [];
  const csv = kind === "wildcard-csv" ? parseWildcardCsv(source) : null;
  if (kind === "wildcard-txt") {
    let lineIndex = 0;
    for (const line of physicalLines(source, "include-eof")) {
      const mapping = prepareIdentityMapping(
        sourceOffsets,
        source.slice(line.from, line.contentTo),
        line.from
      );
      let value = null;
      const decodedRange = trimmedValueRange(mapping.text);
      if (decodedRange.from < decodedRange.to) {
        const text = mapping.text.slice(decodedRange.from, decodedRange.to);
        const from = line.from + decodedRange.from;
        value = {
          id: `txt-line:${lineIndex}`,
          mapping: prepareIdentityMapping(sourceOffsets, text, from),
          decodedRange,
          location: { kind: "txt", line: lineIndex }
        };
        values.push(value);
      }
      containers.push({
        id: `txt-container:${lineIndex}`,
        storageRange: { from: line.from, to: line.contentTo },
        decodedRange: { from: 0, to: mapping.text.length },
        valueRange: decodedRange,
        mapping,
        value,
        editability: "editable",
        location: { kind: "txt", line: lineIndex, record: line }
      });
      lineIndex++;
    }
  } else if (csv) {
    csv.records.forEach((record2, row) => {
      record2.forEach((cell, column) => {
        const mapping = csv.valid ? prepareValueMapping(
          sourceOffsets,
          cell.value,
          cell.characterRanges,
          cell.range.from + Number(cell.quoted)
        ) : null;
        let value = null;
        const decodedRange = trimmedValueRange(cell.value);
        if (csv.valid && row > 0) {
          const characterRanges = cell.characterRanges.slice(decodedRange.from, decodedRange.to);
          const anchor = cell.characterRanges[decodedRange.from]?.from ?? cell.characterRanges.at(-1)?.to ?? cell.range.from + Number(cell.quoted);
          value = {
            id: `csv-cell:${row}:${column}`,
            mapping: prepareValueMapping(
              sourceOffsets,
              cell.value.slice(decodedRange.from, decodedRange.to),
              characterRanges,
              anchor
            ),
            decodedRange,
            location: { kind: "csv", row, column, cell }
          };
          values.push(value);
        }
        containers.push({
          id: `csv-container:${row}:${column}`,
          storageRange: cell.range,
          decodedRange: { from: 0, to: cell.value.length },
          valueRange: decodedRange,
          mapping,
          value,
          editability: !csv.valid ? "malformed" : row === 0 ? "header" : "editable",
          location: { kind: "csv", row, column, cell }
        });
      });
    });
  } else {
    containers.push({
      id: "prompt-container",
      storageRange: { from: 0, to: source.length },
      decodedRange: { from: 0, to: source.length },
      valueRange: { from: 0, to: source.length },
      mapping: prepareIdentityMapping(sourceOffsets, source, 0),
      value: null,
      editability: "editable",
      location: { kind: "prompt" }
    });
  }
  const byId = /* @__PURE__ */ Object.create(null);
  for (const value of values) byId[value.id] = value;
  const containersById = /* @__PURE__ */ Object.create(null);
  const containerOutcomes = /* @__PURE__ */ Object.create(null);
  for (const container of containers) {
    containersById[container.id] = container;
    containerOutcomes[container.id] = container.editability === "editable" ? { status: "editable", container } : { status: "protected", container, reason: container.editability };
  }
  return freezeStructure({
    source,
    kind,
    valid: csv?.valid ?? true,
    scenesEnabled: kind === "prompt",
    usesStructuredPromptValues: kind === "wildcard-csv",
    isolatesDuplicates: kind !== "prompt",
    values,
    byId,
    csv,
    sourceOffsets,
    containers,
    containersById,
    containerOutcomes,
    containerState: !csv ? "ready" : !csv.valid ? "malformed" : source.length === 0 ? "no-data" : csv.records.length === 1 ? "header-only" : "ready"
  });
}
function documentValueById(prepared, source, kind, id) {
  if (prepared.source !== source || prepared.kind !== kind || !prepared.valid) return null;
  return prepared.byId[id] ?? null;
}

// frontend/sugarprompt/core/src/document/semantics.ts
function validateDocumentKind(kind) {
  if (kind !== "prompt" && kind !== "wildcard-txt" && kind !== "wildcard-csv")
    throw new TypeError("Invalid prompt document kind");
}
function prepareDocumentSemantics(source, kind, previous, seed) {
  validateDocumentKind(kind);
  if (seed && (seed.source !== source || seed.kind !== kind))
    throw new Error("document-semantics-seed-mismatch");
  if (previous?.preparedValues.source === source && previous.kind === kind && (!seed || previous.preparedValues === seed))
    return previous;
  const ranges = [];
  const preparedValues = seed ?? prepareDocumentValues(source, kind);
  for (const value of preparedValues.values)
    if (value.mapping.text.startsWith("**")) {
      const range = decodedRangeToSource(value.mapping, { from: 0, to: 2 });
      if (!range) throw new Error("invalid-scene-marker-mapping");
      ranges.push(range);
    }
  return freezeStructure({
    kind,
    scenesEnabled: preparedValues.scenesEnabled,
    valid: preparedValues.valid,
    unsupportedSceneMarkerRanges: ranges,
    preparedValues
  });
}

// frontend/sugarprompt/core/src/document/replacement.ts
function ownFields(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const actual = Reflect.ownKeys(value);
  if (actual.length !== keys.length || actual.some((key) => typeof key !== "string" || !keys.includes(key)))
    return null;
  const fields = /* @__PURE__ */ Object.create(null);
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !descriptor.enumerable || !Object.hasOwn(descriptor, "value")) return null;
    fields[key] = descriptor.value;
  }
  return fields;
}
function request(value) {
  const fields = ownFields(value, ["kind", "range", "source", "text"]);
  if (!fields || typeof fields.source !== "string" || typeof fields.text !== "string") return null;
  try {
    validateDocumentKind(fields.kind);
  } catch (error) {
    if (error instanceof TypeError) return null;
    throw error;
  }
  const range = ownFields(fields.range, ["from", "to"]);
  if (!range || typeof range.from !== "number" || typeof range.to !== "number") return null;
  return {
    source: fields.source,
    text: fields.text,
    kind: fields.kind,
    range: { from: range.from, to: range.to }
  };
}
function boundaryFailure(source, range) {
  if (!Number.isSafeInteger(range.from) || !Number.isSafeInteger(range.to) || range.from < 0 || range.to < range.from || range.to > source.length)
    return "invalid-range";
  try {
    validateSourceRange(source, range);
    utf16ToCodePoint(source, range.from);
    utf16ToCodePoint(source, range.to);
  } catch (error) {
    if (error instanceof RangeError) return "invalid-boundary";
    throw error;
  }
  return null;
}
function prepareDocumentReplacement(input, prepared) {
  const intent = request(input);
  if (!intent) return { status: "rejected", reason: "invalid-input" };
  const { source, kind, range, text } = intent;
  const failure = boundaryFailure(source, range);
  if (failure) return { status: "rejected", reason: failure };
  if (prepared && (prepared.source !== source || prepared.kind !== kind))
    return { status: "rejected", reason: "unavailable-target" };
  if (kind !== "wildcard-csv") {
    const updated2 = source.slice(0, range.from) + text + source.slice(range.to);
    const caret2 = range.from + text.length;
    if (boundaryFailure(updated2, { from: range.from, to: caret2 }))
      return { status: "rejected", reason: "invalid-boundary" };
    return freezeStructure({
      status: "ready",
      range,
      text,
      caret: caret2,
      target: null,
      preparedValues: prepareDocumentValues(updated2, kind, prepared)
    });
  }
  const values = prepared ?? prepareDocumentValues(source, kind);
  const document = values.csv;
  if (!document) throw new Error("missing-prepared-csv");
  if (!document.valid) return { status: "rejected", reason: "malformed-csv" };
  const candidates = document.records.flatMap(
    (record2, row2) => record2.flatMap(
      (cell2, column2) => cell2.range.from <= range.from && range.to <= cell2.range.to ? [{ cell: cell2, row: row2, column: column2 }] : []
    )
  );
  if (candidates.length > 1) return { status: "rejected", reason: "ambiguous-target" };
  const candidate = candidates[0];
  if (!candidate) {
    const intersects = document.records.flat().some(
      (cell2) => range.from < range.to && cell2.range.from <= range.to && range.from <= cell2.range.to
    );
    return {
      status: "rejected",
      reason: intersects ? "cross-value" : "unavailable-target"
    };
  }
  const { cell, row, column } = candidate;
  if (row === 0) return { status: "rejected", reason: "header-target" };
  const selected = documentValueById(values, source, kind, `csv-cell:${row}:${column}`);
  if (!selected) throw new Error("missing-prepared-value");
  const decoded = sourceRangeToDecoded(selected.mapping, range);
  if (!decoded) return { status: "rejected", reason: "ambiguous-target" };
  const decodedRange = {
    from: selected.decodedRange.from + decoded.from,
    to: selected.decodedRange.from + decoded.to
  };
  const value = cell.value.slice(0, decodedRange.from) + text + cell.value.slice(decodedRange.to);
  const decodedCaret = decodedRange.from + text.length;
  if (boundaryFailure(value, { from: decodedRange.from, to: decodedCaret }))
    return { status: "rejected", reason: "invalid-boundary" };
  let encoded = prepareWildcardCsvEncoding(cell, value).text;
  const preservesStructure = (updated2) => updated2.valid && updated2.records.length === document.records.length && document.records.every(
    (record2, r) => record2.length === requireValue(updated2.records[r], "replacement").length && record2.every(
      (original, c) => requireValue(requireValue(updated2.records[r], "replacement")[c], "replacement").value === (r === row && c === column ? value : original.value)
    )
  );
  let updatedSource = source.slice(0, cell.range.from) + encoded + source.slice(cell.range.to);
  let updatedValues = prepareDocumentValues(updatedSource, kind, values);
  if (!updatedValues.csv) throw new Error("missing-post-edit-csv");
  if (!preservesStructure(updatedValues.csv) && !cell.quoted) {
    encoded = prepareWildcardCsvEncoding(cell, value, true).text;
    updatedSource = source.slice(0, cell.range.from) + encoded + source.slice(cell.range.to);
    updatedValues = prepareDocumentValues(updatedSource, kind, values);
  }
  const updated = updatedValues.csv;
  if (!updated) throw new Error("missing-post-edit-csv");
  if (!preservesStructure(updated)) return { status: "rejected", reason: "invalid-boundary" };
  const updatedCell = requireValue(
    requireValue(updated.records[row], "replacement")[column],
    "replacement"
  );
  if (updatedCell.range.from !== cell.range.from || updatedCell.range.to !== cell.range.from + encoded.length)
    return { status: "rejected", reason: "invalid-boundary" };
  const updatedMapping = updatedValues.containersById[`csv-container:${row}:${column}`]?.mapping;
  if (!updatedMapping) throw new Error("missing-post-edit-container-map");
  const mappedCaret = decodedRangeToSource(updatedMapping, {
    from: decodedCaret,
    to: decodedCaret
  });
  if (!mappedCaret) return { status: "rejected", reason: "invalid-boundary" };
  const caret = mappedCaret.from;
  if (boundaryFailure(updatedSource, {
    from: cell.range.from,
    to: cell.range.from + encoded.length
  }) || boundaryFailure(updatedSource, { from: caret, to: caret }))
    return { status: "rejected", reason: "invalid-boundary" };
  return freezeStructure({
    status: "ready",
    range: cell.range,
    text: encoded,
    caret,
    target: { row, column, range: decodedRange },
    preparedValues: updatedValues
  });
}

// frontend/sugarprompt/core/src/prompt/unicode-lower-data.ts
var lowercaseMappings = `41:61 42:62 43:63 44:64 45:65 46:66 47:67 48:68 49:69 4a:6a 4b:6b 4c:6c 4d:6d 4e:6e 4f:6f
50:70 51:71 52:72 53:73 54:74 55:75 56:76 57:77 58:78 59:79 5a:7a c0:e0 c1:e1 c2:e2 c3:e3
c4:e4 c5:e5 c6:e6 c7:e7 c8:e8 c9:e9 ca:ea cb:eb cc:ec cd:ed ce:ee cf:ef d0:f0 d1:f1 d2:f2
d3:f3 d4:f4 d5:f5 d6:f6 d8:f8 d9:f9 da:fa db:fb dc:fc dd:fd de:fe 100:101 102:103 104:105
106:107 108:109 10a:10b 10c:10d 10e:10f 110:111 112:113 114:115 116:117 118:119 11a:11b
11c:11d 11e:11f 120:121 122:123 124:125 126:127 128:129 12a:12b 12c:12d 12e:12f 130:69,307
132:133 134:135 136:137 139:13a 13b:13c 13d:13e 13f:140 141:142 143:144 145:146 147:148
14a:14b 14c:14d 14e:14f 150:151 152:153 154:155 156:157 158:159 15a:15b 15c:15d 15e:15f
160:161 162:163 164:165 166:167 168:169 16a:16b 16c:16d 16e:16f 170:171 172:173 174:175
176:177 178:ff 179:17a 17b:17c 17d:17e 181:253 182:183 184:185 186:254 187:188 189:256
18a:257 18b:18c 18e:1dd 18f:259 190:25b 191:192 193:260 194:263 196:269 197:268 198:199
19c:26f 19d:272 19f:275 1a0:1a1 1a2:1a3 1a4:1a5 1a6:280 1a7:1a8 1a9:283 1ac:1ad 1ae:288
1af:1b0 1b1:28a 1b2:28b 1b3:1b4 1b5:1b6 1b7:292 1b8:1b9 1bc:1bd 1c4:1c6 1c5:1c6 1c7:1c9
1c8:1c9 1ca:1cc 1cb:1cc 1cd:1ce 1cf:1d0 1d1:1d2 1d3:1d4 1d5:1d6 1d7:1d8 1d9:1da 1db:1dc
1de:1df 1e0:1e1 1e2:1e3 1e4:1e5 1e6:1e7 1e8:1e9 1ea:1eb 1ec:1ed 1ee:1ef 1f1:1f3 1f2:1f3
1f4:1f5 1f6:195 1f7:1bf 1f8:1f9 1fa:1fb 1fc:1fd 1fe:1ff 200:201 202:203 204:205 206:207
208:209 20a:20b 20c:20d 20e:20f 210:211 212:213 214:215 216:217 218:219 21a:21b 21c:21d
21e:21f 220:19e 222:223 224:225 226:227 228:229 22a:22b 22c:22d 22e:22f 230:231 232:233
23a:2c65 23b:23c 23d:19a 23e:2c66 241:242 243:180 244:289 245:28c 246:247 248:249 24a:24b
24c:24d 24e:24f 370:371 372:373 376:377 37f:3f3 386:3ac 388:3ad 389:3ae 38a:3af 38c:3cc
38e:3cd 38f:3ce 391:3b1 392:3b2 393:3b3 394:3b4 395:3b5 396:3b6 397:3b7 398:3b8 399:3b9
39a:3ba 39b:3bb 39c:3bc 39d:3bd 39e:3be 39f:3bf 3a0:3c0 3a1:3c1 3a3:3c3 3a4:3c4 3a5:3c5
3a6:3c6 3a7:3c7 3a8:3c8 3a9:3c9 3aa:3ca 3ab:3cb 3cf:3d7 3d8:3d9 3da:3db 3dc:3dd 3de:3df
3e0:3e1 3e2:3e3 3e4:3e5 3e6:3e7 3e8:3e9 3ea:3eb 3ec:3ed 3ee:3ef 3f4:3b8 3f7:3f8 3f9:3f2
3fa:3fb 3fd:37b 3fe:37c 3ff:37d 400:450 401:451 402:452 403:453 404:454 405:455 406:456
407:457 408:458 409:459 40a:45a 40b:45b 40c:45c 40d:45d 40e:45e 40f:45f 410:430 411:431
412:432 413:433 414:434 415:435 416:436 417:437 418:438 419:439 41a:43a 41b:43b 41c:43c
41d:43d 41e:43e 41f:43f 420:440 421:441 422:442 423:443 424:444 425:445 426:446 427:447
428:448 429:449 42a:44a 42b:44b 42c:44c 42d:44d 42e:44e 42f:44f 460:461 462:463 464:465
466:467 468:469 46a:46b 46c:46d 46e:46f 470:471 472:473 474:475 476:477 478:479 47a:47b
47c:47d 47e:47f 480:481 48a:48b 48c:48d 48e:48f 490:491 492:493 494:495 496:497 498:499
49a:49b 49c:49d 49e:49f 4a0:4a1 4a2:4a3 4a4:4a5 4a6:4a7 4a8:4a9 4aa:4ab 4ac:4ad 4ae:4af
4b0:4b1 4b2:4b3 4b4:4b5 4b6:4b7 4b8:4b9 4ba:4bb 4bc:4bd 4be:4bf 4c0:4cf 4c1:4c2 4c3:4c4
4c5:4c6 4c7:4c8 4c9:4ca 4cb:4cc 4cd:4ce 4d0:4d1 4d2:4d3 4d4:4d5 4d6:4d7 4d8:4d9 4da:4db
4dc:4dd 4de:4df 4e0:4e1 4e2:4e3 4e4:4e5 4e6:4e7 4e8:4e9 4ea:4eb 4ec:4ed 4ee:4ef 4f0:4f1
4f2:4f3 4f4:4f5 4f6:4f7 4f8:4f9 4fa:4fb 4fc:4fd 4fe:4ff 500:501 502:503 504:505 506:507
508:509 50a:50b 50c:50d 50e:50f 510:511 512:513 514:515 516:517 518:519 51a:51b 51c:51d
51e:51f 520:521 522:523 524:525 526:527 528:529 52a:52b 52c:52d 52e:52f 531:561 532:562
533:563 534:564 535:565 536:566 537:567 538:568 539:569 53a:56a 53b:56b 53c:56c 53d:56d
53e:56e 53f:56f 540:570 541:571 542:572 543:573 544:574 545:575 546:576 547:577 548:578
549:579 54a:57a 54b:57b 54c:57c 54d:57d 54e:57e 54f:57f 550:580 551:581 552:582 553:583
554:584 555:585 556:586 10a0:2d00 10a1:2d01 10a2:2d02 10a3:2d03 10a4:2d04 10a5:2d05
10a6:2d06 10a7:2d07 10a8:2d08 10a9:2d09 10aa:2d0a 10ab:2d0b 10ac:2d0c 10ad:2d0d 10ae:2d0e
10af:2d0f 10b0:2d10 10b1:2d11 10b2:2d12 10b3:2d13 10b4:2d14 10b5:2d15 10b6:2d16 10b7:2d17
10b8:2d18 10b9:2d19 10ba:2d1a 10bb:2d1b 10bc:2d1c 10bd:2d1d 10be:2d1e 10bf:2d1f 10c0:2d20
10c1:2d21 10c2:2d22 10c3:2d23 10c4:2d24 10c5:2d25 10c7:2d27 10cd:2d2d 13a0:ab70 13a1:ab71
13a2:ab72 13a3:ab73 13a4:ab74 13a5:ab75 13a6:ab76 13a7:ab77 13a8:ab78 13a9:ab79 13aa:ab7a
13ab:ab7b 13ac:ab7c 13ad:ab7d 13ae:ab7e 13af:ab7f 13b0:ab80 13b1:ab81 13b2:ab82 13b3:ab83
13b4:ab84 13b5:ab85 13b6:ab86 13b7:ab87 13b8:ab88 13b9:ab89 13ba:ab8a 13bb:ab8b 13bc:ab8c
13bd:ab8d 13be:ab8e 13bf:ab8f 13c0:ab90 13c1:ab91 13c2:ab92 13c3:ab93 13c4:ab94 13c5:ab95
13c6:ab96 13c7:ab97 13c8:ab98 13c9:ab99 13ca:ab9a 13cb:ab9b 13cc:ab9c 13cd:ab9d 13ce:ab9e
13cf:ab9f 13d0:aba0 13d1:aba1 13d2:aba2 13d3:aba3 13d4:aba4 13d5:aba5 13d6:aba6 13d7:aba7
13d8:aba8 13d9:aba9 13da:abaa 13db:abab 13dc:abac 13dd:abad 13de:abae 13df:abaf 13e0:abb0
13e1:abb1 13e2:abb2 13e3:abb3 13e4:abb4 13e5:abb5 13e6:abb6 13e7:abb7 13e8:abb8 13e9:abb9
13ea:abba 13eb:abbb 13ec:abbc 13ed:abbd 13ee:abbe 13ef:abbf 13f0:13f8 13f1:13f9 13f2:13fa
13f3:13fb 13f4:13fc 13f5:13fd 1c90:10d0 1c91:10d1 1c92:10d2 1c93:10d3 1c94:10d4 1c95:10d5
1c96:10d6 1c97:10d7 1c98:10d8 1c99:10d9 1c9a:10da 1c9b:10db 1c9c:10dc 1c9d:10dd 1c9e:10de
1c9f:10df 1ca0:10e0 1ca1:10e1 1ca2:10e2 1ca3:10e3 1ca4:10e4 1ca5:10e5 1ca6:10e6 1ca7:10e7
1ca8:10e8 1ca9:10e9 1caa:10ea 1cab:10eb 1cac:10ec 1cad:10ed 1cae:10ee 1caf:10ef 1cb0:10f0
1cb1:10f1 1cb2:10f2 1cb3:10f3 1cb4:10f4 1cb5:10f5 1cb6:10f6 1cb7:10f7 1cb8:10f8 1cb9:10f9
1cba:10fa 1cbd:10fd 1cbe:10fe 1cbf:10ff 1e00:1e01 1e02:1e03 1e04:1e05 1e06:1e07 1e08:1e09
1e0a:1e0b 1e0c:1e0d 1e0e:1e0f 1e10:1e11 1e12:1e13 1e14:1e15 1e16:1e17 1e18:1e19 1e1a:1e1b
1e1c:1e1d 1e1e:1e1f 1e20:1e21 1e22:1e23 1e24:1e25 1e26:1e27 1e28:1e29 1e2a:1e2b 1e2c:1e2d
1e2e:1e2f 1e30:1e31 1e32:1e33 1e34:1e35 1e36:1e37 1e38:1e39 1e3a:1e3b 1e3c:1e3d 1e3e:1e3f
1e40:1e41 1e42:1e43 1e44:1e45 1e46:1e47 1e48:1e49 1e4a:1e4b 1e4c:1e4d 1e4e:1e4f 1e50:1e51
1e52:1e53 1e54:1e55 1e56:1e57 1e58:1e59 1e5a:1e5b 1e5c:1e5d 1e5e:1e5f 1e60:1e61 1e62:1e63
1e64:1e65 1e66:1e67 1e68:1e69 1e6a:1e6b 1e6c:1e6d 1e6e:1e6f 1e70:1e71 1e72:1e73 1e74:1e75
1e76:1e77 1e78:1e79 1e7a:1e7b 1e7c:1e7d 1e7e:1e7f 1e80:1e81 1e82:1e83 1e84:1e85 1e86:1e87
1e88:1e89 1e8a:1e8b 1e8c:1e8d 1e8e:1e8f 1e90:1e91 1e92:1e93 1e94:1e95 1e9e:df 1ea0:1ea1
1ea2:1ea3 1ea4:1ea5 1ea6:1ea7 1ea8:1ea9 1eaa:1eab 1eac:1ead 1eae:1eaf 1eb0:1eb1 1eb2:1eb3
1eb4:1eb5 1eb6:1eb7 1eb8:1eb9 1eba:1ebb 1ebc:1ebd 1ebe:1ebf 1ec0:1ec1 1ec2:1ec3 1ec4:1ec5
1ec6:1ec7 1ec8:1ec9 1eca:1ecb 1ecc:1ecd 1ece:1ecf 1ed0:1ed1 1ed2:1ed3 1ed4:1ed5 1ed6:1ed7
1ed8:1ed9 1eda:1edb 1edc:1edd 1ede:1edf 1ee0:1ee1 1ee2:1ee3 1ee4:1ee5 1ee6:1ee7 1ee8:1ee9
1eea:1eeb 1eec:1eed 1eee:1eef 1ef0:1ef1 1ef2:1ef3 1ef4:1ef5 1ef6:1ef7 1ef8:1ef9 1efa:1efb
1efc:1efd 1efe:1eff 1f08:1f00 1f09:1f01 1f0a:1f02 1f0b:1f03 1f0c:1f04 1f0d:1f05 1f0e:1f06
1f0f:1f07 1f18:1f10 1f19:1f11 1f1a:1f12 1f1b:1f13 1f1c:1f14 1f1d:1f15 1f28:1f20 1f29:1f21
1f2a:1f22 1f2b:1f23 1f2c:1f24 1f2d:1f25 1f2e:1f26 1f2f:1f27 1f38:1f30 1f39:1f31 1f3a:1f32
1f3b:1f33 1f3c:1f34 1f3d:1f35 1f3e:1f36 1f3f:1f37 1f48:1f40 1f49:1f41 1f4a:1f42 1f4b:1f43
1f4c:1f44 1f4d:1f45 1f59:1f51 1f5b:1f53 1f5d:1f55 1f5f:1f57 1f68:1f60 1f69:1f61 1f6a:1f62
1f6b:1f63 1f6c:1f64 1f6d:1f65 1f6e:1f66 1f6f:1f67 1f88:1f80 1f89:1f81 1f8a:1f82 1f8b:1f83
1f8c:1f84 1f8d:1f85 1f8e:1f86 1f8f:1f87 1f98:1f90 1f99:1f91 1f9a:1f92 1f9b:1f93 1f9c:1f94
1f9d:1f95 1f9e:1f96 1f9f:1f97 1fa8:1fa0 1fa9:1fa1 1faa:1fa2 1fab:1fa3 1fac:1fa4 1fad:1fa5
1fae:1fa6 1faf:1fa7 1fb8:1fb0 1fb9:1fb1 1fba:1f70 1fbb:1f71 1fbc:1fb3 1fc8:1f72 1fc9:1f73
1fca:1f74 1fcb:1f75 1fcc:1fc3 1fd8:1fd0 1fd9:1fd1 1fda:1f76 1fdb:1f77 1fe8:1fe0 1fe9:1fe1
1fea:1f7a 1feb:1f7b 1fec:1fe5 1ff8:1f78 1ff9:1f79 1ffa:1f7c 1ffb:1f7d 1ffc:1ff3 2126:3c9
212a:6b 212b:e5 2132:214e 2160:2170 2161:2171 2162:2172 2163:2173 2164:2174 2165:2175
2166:2176 2167:2177 2168:2178 2169:2179 216a:217a 216b:217b 216c:217c 216d:217d 216e:217e
216f:217f 2183:2184 24b6:24d0 24b7:24d1 24b8:24d2 24b9:24d3 24ba:24d4 24bb:24d5 24bc:24d6
24bd:24d7 24be:24d8 24bf:24d9 24c0:24da 24c1:24db 24c2:24dc 24c3:24dd 24c4:24de 24c5:24df
24c6:24e0 24c7:24e1 24c8:24e2 24c9:24e3 24ca:24e4 24cb:24e5 24cc:24e6 24cd:24e7 24ce:24e8
24cf:24e9 2c00:2c30 2c01:2c31 2c02:2c32 2c03:2c33 2c04:2c34 2c05:2c35 2c06:2c36 2c07:2c37
2c08:2c38 2c09:2c39 2c0a:2c3a 2c0b:2c3b 2c0c:2c3c 2c0d:2c3d 2c0e:2c3e 2c0f:2c3f 2c10:2c40
2c11:2c41 2c12:2c42 2c13:2c43 2c14:2c44 2c15:2c45 2c16:2c46 2c17:2c47 2c18:2c48 2c19:2c49
2c1a:2c4a 2c1b:2c4b 2c1c:2c4c 2c1d:2c4d 2c1e:2c4e 2c1f:2c4f 2c20:2c50 2c21:2c51 2c22:2c52
2c23:2c53 2c24:2c54 2c25:2c55 2c26:2c56 2c27:2c57 2c28:2c58 2c29:2c59 2c2a:2c5a 2c2b:2c5b
2c2c:2c5c 2c2d:2c5d 2c2e:2c5e 2c2f:2c5f 2c60:2c61 2c62:26b 2c63:1d7d 2c64:27d 2c67:2c68
2c69:2c6a 2c6b:2c6c 2c6d:251 2c6e:271 2c6f:250 2c70:252 2c72:2c73 2c75:2c76 2c7e:23f
2c7f:240 2c80:2c81 2c82:2c83 2c84:2c85 2c86:2c87 2c88:2c89 2c8a:2c8b 2c8c:2c8d 2c8e:2c8f
2c90:2c91 2c92:2c93 2c94:2c95 2c96:2c97 2c98:2c99 2c9a:2c9b 2c9c:2c9d 2c9e:2c9f 2ca0:2ca1
2ca2:2ca3 2ca4:2ca5 2ca6:2ca7 2ca8:2ca9 2caa:2cab 2cac:2cad 2cae:2caf 2cb0:2cb1 2cb2:2cb3
2cb4:2cb5 2cb6:2cb7 2cb8:2cb9 2cba:2cbb 2cbc:2cbd 2cbe:2cbf 2cc0:2cc1 2cc2:2cc3 2cc4:2cc5
2cc6:2cc7 2cc8:2cc9 2cca:2ccb 2ccc:2ccd 2cce:2ccf 2cd0:2cd1 2cd2:2cd3 2cd4:2cd5 2cd6:2cd7
2cd8:2cd9 2cda:2cdb 2cdc:2cdd 2cde:2cdf 2ce0:2ce1 2ce2:2ce3 2ceb:2cec 2ced:2cee 2cf2:2cf3
a640:a641 a642:a643 a644:a645 a646:a647 a648:a649 a64a:a64b a64c:a64d a64e:a64f a650:a651
a652:a653 a654:a655 a656:a657 a658:a659 a65a:a65b a65c:a65d a65e:a65f a660:a661 a662:a663
a664:a665 a666:a667 a668:a669 a66a:a66b a66c:a66d a680:a681 a682:a683 a684:a685 a686:a687
a688:a689 a68a:a68b a68c:a68d a68e:a68f a690:a691 a692:a693 a694:a695 a696:a697 a698:a699
a69a:a69b a722:a723 a724:a725 a726:a727 a728:a729 a72a:a72b a72c:a72d a72e:a72f a732:a733
a734:a735 a736:a737 a738:a739 a73a:a73b a73c:a73d a73e:a73f a740:a741 a742:a743 a744:a745
a746:a747 a748:a749 a74a:a74b a74c:a74d a74e:a74f a750:a751 a752:a753 a754:a755 a756:a757
a758:a759 a75a:a75b a75c:a75d a75e:a75f a760:a761 a762:a763 a764:a765 a766:a767 a768:a769
a76a:a76b a76c:a76d a76e:a76f a779:a77a a77b:a77c a77d:1d79 a77e:a77f a780:a781 a782:a783
a784:a785 a786:a787 a78b:a78c a78d:265 a790:a791 a792:a793 a796:a797 a798:a799 a79a:a79b
a79c:a79d a79e:a79f a7a0:a7a1 a7a2:a7a3 a7a4:a7a5 a7a6:a7a7 a7a8:a7a9 a7aa:266 a7ab:25c
a7ac:261 a7ad:26c a7ae:26a a7b0:29e a7b1:287 a7b2:29d a7b3:ab53 a7b4:a7b5 a7b6:a7b7
a7b8:a7b9 a7ba:a7bb a7bc:a7bd a7be:a7bf a7c0:a7c1 a7c2:a7c3 a7c4:a794 a7c5:282 a7c6:1d8e
a7c7:a7c8 a7c9:a7ca a7d0:a7d1 a7d6:a7d7 a7d8:a7d9 a7f5:a7f6 ff21:ff41 ff22:ff42 ff23:ff43
ff24:ff44 ff25:ff45 ff26:ff46 ff27:ff47 ff28:ff48 ff29:ff49 ff2a:ff4a ff2b:ff4b ff2c:ff4c
ff2d:ff4d ff2e:ff4e ff2f:ff4f ff30:ff50 ff31:ff51 ff32:ff52 ff33:ff53 ff34:ff54 ff35:ff55
ff36:ff56 ff37:ff57 ff38:ff58 ff39:ff59 ff3a:ff5a 10400:10428 10401:10429 10402:1042a
10403:1042b 10404:1042c 10405:1042d 10406:1042e 10407:1042f 10408:10430 10409:10431
1040a:10432 1040b:10433 1040c:10434 1040d:10435 1040e:10436 1040f:10437 10410:10438
10411:10439 10412:1043a 10413:1043b 10414:1043c 10415:1043d 10416:1043e 10417:1043f
10418:10440 10419:10441 1041a:10442 1041b:10443 1041c:10444 1041d:10445 1041e:10446
1041f:10447 10420:10448 10421:10449 10422:1044a 10423:1044b 10424:1044c 10425:1044d
10426:1044e 10427:1044f 104b0:104d8 104b1:104d9 104b2:104da 104b3:104db 104b4:104dc
104b5:104dd 104b6:104de 104b7:104df 104b8:104e0 104b9:104e1 104ba:104e2 104bb:104e3
104bc:104e4 104bd:104e5 104be:104e6 104bf:104e7 104c0:104e8 104c1:104e9 104c2:104ea
104c3:104eb 104c4:104ec 104c5:104ed 104c6:104ee 104c7:104ef 104c8:104f0 104c9:104f1
104ca:104f2 104cb:104f3 104cc:104f4 104cd:104f5 104ce:104f6 104cf:104f7 104d0:104f8
104d1:104f9 104d2:104fa 104d3:104fb 10570:10597 10571:10598 10572:10599 10573:1059a
10574:1059b 10575:1059c 10576:1059d 10577:1059e 10578:1059f 10579:105a0 1057a:105a1
1057c:105a3 1057d:105a4 1057e:105a5 1057f:105a6 10580:105a7 10581:105a8 10582:105a9
10583:105aa 10584:105ab 10585:105ac 10586:105ad 10587:105ae 10588:105af 10589:105b0
1058a:105b1 1058c:105b3 1058d:105b4 1058e:105b5 1058f:105b6 10590:105b7 10591:105b8
10592:105b9 10594:105bb 10595:105bc 10c80:10cc0 10c81:10cc1 10c82:10cc2 10c83:10cc3
10c84:10cc4 10c85:10cc5 10c86:10cc6 10c87:10cc7 10c88:10cc8 10c89:10cc9 10c8a:10cca
10c8b:10ccb 10c8c:10ccc 10c8d:10ccd 10c8e:10cce 10c8f:10ccf 10c90:10cd0 10c91:10cd1
10c92:10cd2 10c93:10cd3 10c94:10cd4 10c95:10cd5 10c96:10cd6 10c97:10cd7 10c98:10cd8
10c99:10cd9 10c9a:10cda 10c9b:10cdb 10c9c:10cdc 10c9d:10cdd 10c9e:10cde 10c9f:10cdf
10ca0:10ce0 10ca1:10ce1 10ca2:10ce2 10ca3:10ce3 10ca4:10ce4 10ca5:10ce5 10ca6:10ce6
10ca7:10ce7 10ca8:10ce8 10ca9:10ce9 10caa:10cea 10cab:10ceb 10cac:10cec 10cad:10ced
10cae:10cee 10caf:10cef 10cb0:10cf0 10cb1:10cf1 10cb2:10cf2 118a0:118c0 118a1:118c1
118a2:118c2 118a3:118c3 118a4:118c4 118a5:118c5 118a6:118c6 118a7:118c7 118a8:118c8
118a9:118c9 118aa:118ca 118ab:118cb 118ac:118cc 118ad:118cd 118ae:118ce 118af:118cf
118b0:118d0 118b1:118d1 118b2:118d2 118b3:118d3 118b4:118d4 118b5:118d5 118b6:118d6
118b7:118d7 118b8:118d8 118b9:118d9 118ba:118da 118bb:118db 118bc:118dc 118bd:118dd
118be:118de 118bf:118df 16e40:16e60 16e41:16e61 16e42:16e62 16e43:16e63 16e44:16e64
16e45:16e65 16e46:16e66 16e47:16e67 16e48:16e68 16e49:16e69 16e4a:16e6a 16e4b:16e6b
16e4c:16e6c 16e4d:16e6d 16e4e:16e6e 16e4f:16e6f 16e50:16e70 16e51:16e71 16e52:16e72
16e53:16e73 16e54:16e74 16e55:16e75 16e56:16e76 16e57:16e77 16e58:16e78 16e59:16e79
16e5a:16e7a 16e5b:16e7b 16e5c:16e7c 16e5d:16e7d 16e5e:16e7e 16e5f:16e7f 1e900:1e922
1e901:1e923 1e902:1e924 1e903:1e925 1e904:1e926 1e905:1e927 1e906:1e928 1e907:1e929
1e908:1e92a 1e909:1e92b 1e90a:1e92c 1e90b:1e92d 1e90c:1e92e 1e90d:1e92f 1e90e:1e930
1e90f:1e931 1e910:1e932 1e911:1e933 1e912:1e934 1e913:1e935 1e914:1e936 1e915:1e937
1e916:1e938 1e917:1e939 1e918:1e93a 1e919:1e93b 1e91a:1e93c 1e91b:1e93d 1e91c:1e93e
1e91d:1e93f 1e91e:1e940 1e91f:1e941 1e920:1e942 1e921:1e943`;
var casedIntervals = `41-5a 61-7a aa-aa b5-b5 ba-ba c0-d6 d8-f6 f8-1ba 1bc-1bf 1c4-293 295-2b8 2c0-2c1 2e0-2e4
345-345 370-373 376-377 37a-37d 37f-37f 386-386 388-38a 38c-38c 38e-3a1 3a3-3f5 3f7-481
48a-52f 531-556 560-588 10a0-10c5 10c7-10c7 10cd-10cd 10d0-10fa 10fc-10ff 13a0-13f5
13f8-13fd 1c80-1c88 1c90-1cba 1cbd-1cbf 1d00-1dbf 1e00-1f15 1f18-1f1d 1f20-1f45 1f48-1f4d
1f50-1f57 1f59-1f59 1f5b-1f5b 1f5d-1f5d 1f5f-1f7d 1f80-1fb4 1fb6-1fbc 1fbe-1fbe 1fc2-1fc4
1fc6-1fcc 1fd0-1fd3 1fd6-1fdb 1fe0-1fec 1ff2-1ff4 1ff6-1ffc 2071-2071 207f-207f 2090-209c
2102-2102 2107-2107 210a-2113 2115-2115 2119-211d 2124-2124 2126-2126 2128-2128 212a-212d
212f-2134 2139-2139 213c-213f 2145-2149 214e-214e 2160-217f 2183-2184 24b6-24e9 2c00-2ce4
2ceb-2cee 2cf2-2cf3 2d00-2d25 2d27-2d27 2d2d-2d2d a640-a66d a680-a69d a722-a787 a78b-a78e
a790-a7ca a7d0-a7d1 a7d3-a7d3 a7d5-a7d9 a7f2-a7f6 a7f8-a7fa ab30-ab5a ab5c-ab69 ab70-abbf
fb00-fb06 fb13-fb17 ff21-ff3a ff41-ff5a 10400-1044f 104b0-104d3 104d8-104fb 10570-1057a
1057c-1058a 1058c-10592 10594-10595 10597-105a1 105a3-105b1 105b3-105b9 105bb-105bc
10780-10780 10783-10785 10787-107b0 107b2-107ba 10c80-10cb2 10cc0-10cf2 118a0-118df
16e40-16e7f 1d400-1d454 1d456-1d49c 1d49e-1d49f 1d4a2-1d4a2 1d4a5-1d4a6 1d4a9-1d4ac
1d4ae-1d4b9 1d4bb-1d4bb 1d4bd-1d4c3 1d4c5-1d505 1d507-1d50a 1d50d-1d514 1d516-1d51c
1d51e-1d539 1d53b-1d53e 1d540-1d544 1d546-1d546 1d54a-1d550 1d552-1d6a5 1d6a8-1d6c0
1d6c2-1d6da 1d6dc-1d6fa 1d6fc-1d714 1d716-1d734 1d736-1d74e 1d750-1d76e 1d770-1d788
1d78a-1d7a8 1d7aa-1d7c2 1d7c4-1d7cb 1df00-1df09 1df0b-1df1e 1df25-1df2a 1e030-1e06d
1e900-1e943 1f130-1f149 1f150-1f169 1f170-1f189`;
var caseIgnorableIntervals = `27-27 2e-2e 3a-3a 5e-5e 60-60 a8-a8 ad-ad af-af b4-b4 b7-b8 2b0-36f 374-375 37a-37a
384-385 387-387 483-489 559-559 55f-55f 591-5bd 5bf-5bf 5c1-5c2 5c4-5c5 5c7-5c7 5f4-5f4
600-605 610-61a 61c-61c 640-640 64b-65f 670-670 6d6-6dd 6df-6e8 6ea-6ed 70f-70f 711-711
730-74a 7a6-7b0 7eb-7f5 7fa-7fa 7fd-7fd 816-82d 859-85b 888-888 890-891 898-89f 8c9-902
93a-93a 93c-93c 941-948 94d-94d 951-957 962-963 971-971 981-981 9bc-9bc 9c1-9c4 9cd-9cd
9e2-9e3 9fe-9fe a01-a02 a3c-a3c a41-a42 a47-a48 a4b-a4d a51-a51 a70-a71 a75-a75 a81-a82
abc-abc ac1-ac5 ac7-ac8 acd-acd ae2-ae3 afa-aff b01-b01 b3c-b3c b3f-b3f b41-b44 b4d-b4d
b55-b56 b62-b63 b82-b82 bc0-bc0 bcd-bcd c00-c00 c04-c04 c3c-c3c c3e-c40 c46-c48 c4a-c4d
c55-c56 c62-c63 c81-c81 cbc-cbc cbf-cbf cc6-cc6 ccc-ccd ce2-ce3 d00-d01 d3b-d3c d41-d44
d4d-d4d d62-d63 d81-d81 dca-dca dd2-dd4 dd6-dd6 e31-e31 e34-e3a e46-e4e eb1-eb1 eb4-ebc
ec6-ec6 ec8-ece f18-f19 f35-f35 f37-f37 f39-f39 f71-f7e f80-f84 f86-f87 f8d-f97 f99-fbc
fc6-fc6 102d-1030 1032-1037 1039-103a 103d-103e 1058-1059 105e-1060 1071-1074 1082-1082
1085-1086 108d-108d 109d-109d 10fc-10fc 135d-135f 1712-1714 1732-1733 1752-1753 1772-1773
17b4-17b5 17b7-17bd 17c6-17c6 17c9-17d3 17d7-17d7 17dd-17dd 180b-180f 1843-1843 1885-1886
18a9-18a9 1920-1922 1927-1928 1932-1932 1939-193b 1a17-1a18 1a1b-1a1b 1a56-1a56 1a58-1a5e
1a60-1a60 1a62-1a62 1a65-1a6c 1a73-1a7c 1a7f-1a7f 1aa7-1aa7 1ab0-1ace 1b00-1b03 1b34-1b34
1b36-1b3a 1b3c-1b3c 1b42-1b42 1b6b-1b73 1b80-1b81 1ba2-1ba5 1ba8-1ba9 1bab-1bad 1be6-1be6
1be8-1be9 1bed-1bed 1bef-1bf1 1c2c-1c33 1c36-1c37 1c78-1c7d 1cd0-1cd2 1cd4-1ce0 1ce2-1ce8
1ced-1ced 1cf4-1cf4 1cf8-1cf9 1d2c-1d6a 1d78-1d78 1d9b-1dff 1fbd-1fbd 1fbf-1fc1 1fcd-1fcf
1fdd-1fdf 1fed-1fef 1ffd-1ffe 200b-200f 2018-2019 2024-2024 2027-2027 202a-202e 2060-2064
2066-206f 2071-2071 207f-207f 2090-209c 20d0-20f0 2c7c-2c7d 2cef-2cf1 2d6f-2d6f 2d7f-2d7f
2de0-2dff 2e2f-2e2f 3005-3005 302a-302d 3031-3035 303b-303b 3099-309e 30fc-30fe a015-a015
a4f8-a4fd a60c-a60c a66f-a672 a674-a67d a67f-a67f a69c-a69f a6f0-a6f1 a700-a721 a770-a770
a788-a78a a7f2-a7f4 a7f8-a7f9 a802-a802 a806-a806 a80b-a80b a825-a826 a82c-a82c a8c4-a8c5
a8e0-a8f1 a8ff-a8ff a926-a92d a947-a951 a980-a982 a9b3-a9b3 a9b6-a9b9 a9bc-a9bd a9cf-a9cf
a9e5-a9e6 aa29-aa2e aa31-aa32 aa35-aa36 aa43-aa43 aa4c-aa4c aa70-aa70 aa7c-aa7c aab0-aab0
aab2-aab4 aab7-aab8 aabe-aabf aac1-aac1 aadd-aadd aaec-aaed aaf3-aaf4 aaf6-aaf6 ab5b-ab5f
ab69-ab6b abe5-abe5 abe8-abe8 abed-abed fb1e-fb1e fbb2-fbc2 fe00-fe0f fe13-fe13 fe20-fe2f
fe52-fe52 fe55-fe55 feff-feff ff07-ff07 ff0e-ff0e ff1a-ff1a ff3e-ff3e ff40-ff40 ff70-ff70
ff9e-ff9f ffe3-ffe3 fff9-fffb 101fd-101fd 102e0-102e0 10376-1037a 10780-10785 10787-107b0
107b2-107ba 10a01-10a03 10a05-10a06 10a0c-10a0f 10a38-10a3a 10a3f-10a3f 10ae5-10ae6
10d24-10d27 10eab-10eac 10efd-10eff 10f46-10f50 10f82-10f85 11001-11001 11038-11046
11070-11070 11073-11074 1107f-11081 110b3-110b6 110b9-110ba 110bd-110bd 110c2-110c2
110cd-110cd 11100-11102 11127-1112b 1112d-11134 11173-11173 11180-11181 111b6-111be
111c9-111cc 111cf-111cf 1122f-11231 11234-11234 11236-11237 1123e-1123e 11241-11241
112df-112df 112e3-112ea 11300-11301 1133b-1133c 11340-11340 11366-1136c 11370-11374
11438-1143f 11442-11444 11446-11446 1145e-1145e 114b3-114b8 114ba-114ba 114bf-114c0
114c2-114c3 115b2-115b5 115bc-115bd 115bf-115c0 115dc-115dd 11633-1163a 1163d-1163d
1163f-11640 116ab-116ab 116ad-116ad 116b0-116b5 116b7-116b7 1171d-1171f 11722-11725
11727-1172b 1182f-11837 11839-1183a 1193b-1193c 1193e-1193e 11943-11943 119d4-119d7
119da-119db 119e0-119e0 11a01-11a0a 11a33-11a38 11a3b-11a3e 11a47-11a47 11a51-11a56
11a59-11a5b 11a8a-11a96 11a98-11a99 11c30-11c36 11c38-11c3d 11c3f-11c3f 11c92-11ca7
11caa-11cb0 11cb2-11cb3 11cb5-11cb6 11d31-11d36 11d3a-11d3a 11d3c-11d3d 11d3f-11d45
11d47-11d47 11d90-11d91 11d95-11d95 11d97-11d97 11ef3-11ef4 11f00-11f01 11f36-11f3a
11f40-11f40 11f42-11f42 13430-13440 13447-13455 16af0-16af4 16b30-16b36 16b40-16b43
16f4f-16f4f 16f8f-16f9f 16fe0-16fe1 16fe3-16fe4 1aff0-1aff3 1aff5-1affb 1affd-1affe
1bc9d-1bc9e 1bca0-1bca3 1cf00-1cf2d 1cf30-1cf46 1d167-1d169 1d173-1d182 1d185-1d18b
1d1aa-1d1ad 1d242-1d244 1da00-1da36 1da3b-1da6c 1da75-1da75 1da84-1da84 1da9b-1da9f
1daa1-1daaf 1e000-1e006 1e008-1e018 1e01b-1e021 1e023-1e024 1e026-1e02a 1e030-1e06d
1e08f-1e08f 1e130-1e13d 1e2ae-1e2ae 1e2ec-1e2ef 1e4eb-1e4ef 1e8d0-1e8d6 1e944-1e94b
1f3fb-1f3ff e0001-e0001 e0020-e007f e0100-e01ef`;

// frontend/sugarprompt/core/src/prompt/lower.ts
var cased = decodeUnicodeIntervals(casedIntervals);
var ignorable = decodeUnicodeIntervals(caseIgnorableIntervals);
var mappings = new Map(
  lowercaseMappings.split(/\s+/u).map((token) => {
    const [source, target] = token.split(":");
    return [
      Number.parseInt(requireValue(source, "lower"), 16),
      String.fromCodePoint(
        ...requireValue(target, "lower").split(",").map((point) => Number.parseInt(point, 16))
      )
    ];
  })
);
function unicodeLowerCase(text) {
  const characters = Array.from(text), followingCased = [];
  let following = false;
  for (let index = characters.length - 1; index >= 0; index--) {
    const point = requireValue(requireValue(characters[index], "lower").codePointAt(0), "lower");
    followingCased[index] = following;
    if (!unicodeIntervalIncludes(ignorable, point))
      following = unicodeIntervalIncludes(cased, point);
  }
  let previous = false, result = "";
  for (let index = 0; index < characters.length; index++) {
    const character = requireValue(characters[index], "lower"), point = requireValue(character.codePointAt(0), "lower");
    result += point === 931 && previous && !followingCased[index] ? "\u03C2" : mappings.get(point) ?? character;
    if (!unicodeIntervalIncludes(ignorable, point))
      previous = unicodeIntervalIncludes(cased, point);
  }
  return result;
}

// frontend/sugarprompt/core/src/wildcard/column-aliases.ts
function prepareWildcardColumnAliases(span, matchedColumn) {
  if (span.form !== "csv" || matchedColumn === null) return Object.freeze([]);
  const column = span.csvColumn;
  if (column === null || unicodeLowerCase(column) !== unicodeLowerCase(matchedColumn))
    throw new RangeError("Canonical wildcard column identity mismatch");
  if (column === matchedColumn) return Object.freeze([]);
  const authored = Array.from(column), canonical = Array.from(matchedColumn), aliases = [];
  let sourceIndex = 0, displayIndex = 0, from = span.contentRange.from + 5 + span.identifier.length;
  while (sourceIndex < authored.length && displayIndex < canonical.length) {
    let sourceText2 = "", text = "", sourceWidth = 0, displayWidth = 0;
    do {
      if (sourceWidth <= displayWidth) {
        const character = authored[sourceIndex++];
        if (character === void 0) throw new Error("Canonical wildcard source boundary mismatch");
        sourceText2 += character;
        sourceWidth += unicodeLowerCase(character).length;
      }
      if (displayWidth < sourceWidth) {
        const character = canonical[displayIndex++];
        if (character === void 0)
          throw new Error("Canonical wildcard display boundary mismatch");
        text += character;
        displayWidth += unicodeLowerCase(character).length;
      }
    } while (sourceWidth !== displayWidth);
    const to = from + sourceText2.length;
    if (sourceText2 !== text)
      aliases.push(Object.freeze({ range: Object.freeze({ from, to }), text }));
    from = to;
  }
  if (sourceIndex !== authored.length || displayIndex !== canonical.length)
    throw new Error("Canonical wildcard terminal boundary mismatch");
  return Object.freeze(aliases);
}

// frontend/sugarprompt/core/src/wildcard/tag-mutations.ts
function tagReplacement(source, span, tag2) {
  if (!tag2 || strip(tag2) !== tag2) return null;
  const prefix = source.slice(span.outerRange.from, span.contentRange.from), suffix2 = source.slice(span.contentRange.to, span.outerRange.to), base = requireValue(
    source.slice(span.contentRange.from, span.contentRange.to).split("|", 1)[0],
    "tag-mutations"
  );
  const replacement = `${base}|${tag2}`, candidate = `${prefix}${replacement}${suffix2}`;
  const profile = prefix === "{" && suffix2 === "}" ? { style: "curly" } : { style: "custom", prefix, suffix: suffix2, alsoRecognizeCurly: false };
  const parsed = parsePrompt(candidate, profile).wildcardSpans;
  return parsed.length === 1 && requireValue(parsed[0], "tag-mutations").outerRange.from === 0 && requireValue(parsed[0], "tag-mutations").outerRange.to === candidate.length && requireValue(parsed[0], "tag-mutations").tag === tag2 ? replacement : null;
}
function mutateWildcardTag(document, outer, change, preparedGroups) {
  const source = document.sourceText;
  validateSourceRange(source, outer);
  utf16ToCodePoint(source, outer.from);
  utf16ToCodePoint(source, outer.to);
  if (preparedGroups && (preparedGroups.length !== document.wildcardSpans.length || preparedGroups.some((group2, index) => group2.span !== document.wildcardSpans[index])))
    throw new TypeError("foreign-wildcard-groups");
  const group = (preparedGroups ?? prepareWildcardGroups(document.wildcardSpans)).find(
    ({ span }) => span.outerRange.from === outer.from && span.outerRange.to === outer.to
  );
  if (!group) return null;
  let tag2;
  if (change.kind === "set") tag2 = change.tag;
  else {
    if (!group.numeric || group.displayTag === null) return null;
    const stepped = BigInt(group.displayTag) + (change.direction === "increase" ? 1n : -1n);
    tag2 = String(stepped < 1n ? 1n : stepped);
  }
  const replacement = tagReplacement(source, group.span, tag2);
  if (replacement === null) return null;
  const range = group.span.contentRange, caret = range.from + replacement.length;
  return Object.freeze({
    text: source.slice(0, range.from) + replacement + source.slice(range.to),
    selectionRange: Object.freeze({ from: caret, to: caret })
  });
}

// frontend/sugarprompt/core/src/lora/trigger-insertion.ts
function validBoundary(source, position) {
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length) return false;
  return !(source[position - 1] === "\r" && source[position] === "\n" || /[\uD800-\uDBFF]/u.test(source[position - 1] ?? "") && /[\uDC00-\uDFFF]/u.test(source[position] ?? ""));
}
function prepareTriggerInsertionSlot(source, target) {
  if (target.kind === "selection") {
    const { from, to } = target.range;
    if (from >= to || !validBoundary(source, from) || !validBoundary(source, to)) return null;
    return Object.freeze({
      range: Object.freeze({ from, to }),
      prefix: ""
    });
  }
  if (!validBoundary(source, target.position)) return null;
  let end = target.position;
  while (end < source.length && ![",", "\r", "\n"].includes(requireValue(source[end], "trigger-insertion")))
    end++;
  const lineStart = Math.max(source.lastIndexOf("\r", end - 1), source.lastIndexOf("\n", end - 1)) + 1;
  const left = strip(source.slice(lineStart, end), "right");
  return Object.freeze({
    range: Object.freeze({ from: end, to: end }),
    prefix: left ? left.endsWith(",") ? " " : ", " : ""
  });
}
function triggerInsertionText(slot, words) {
  const text = strip(words);
  return text ? `${slot.prefix}${text}` : null;
}

// frontend/sugarprompt/core/src/completion/lora-text.ts
function loraScheduleWeight(weight) {
  if (weight === null || weight === void 0) return "1.00";
  if (typeof weight !== "string") throw new TypeError("Invalid LoRA weight input");
  const value = strip(weight);
  return value && parseWeight(value, true) !== null ? value : "1.00";
}
function loraNameContent(text) {
  return !/[<>:\r\n\u0000]/u.test(text);
}
function canonicalName(name) {
  if (typeof name !== "string" || !strip(name) || name.length > 4096 || !loraNameContent(name))
    throw new TypeError("Invalid LoRA schedule identity");
  return name;
}
function loraScheduleText(name, weight) {
  return `<lora:${canonicalName(name)}:${loraScheduleWeight(weight)}>`;
}
function loraReplacement(source, query, name) {
  assertPreparedLoraQuery(source, query);
  const range = query.replacementRange;
  for (const position of [range.from, range.to, query.nameRange.from, query.nameRange.to])
    assertCompletionBoundary(source, position);
  if (range.from > query.nameRange.from || query.nameRange.from > query.nameRange.to || query.nameRange.to > range.to)
    throw new RangeError("Invalid LoRA completion range");
  return Object.freeze({
    range: Object.freeze({ ...range }),
    text: query.preservedSuffix === null ? loraScheduleText(name, query.typedWeightText) : `<lora:${canonicalName(name)}${query.preservedSuffix}>`
  });
}
function suffix(candidate, prefix) {
  const proposed = Array.from(candidate), typed = Array.from(prefix);
  if (typed.length > proposed.length) return "";
  for (const [index, character] of typed.entries()) {
    const left = caseFold(character.replaceAll("\\", "/")), right = caseFold(requireValue(proposed[index], "lora-text").replaceAll("\\", "/"));
    if (left !== right && !(left === "_" && right === " " || left === " " && right === "_"))
      return "";
  }
  return proposed.slice(typed.length).join("");
}
function loraCompletionSuffix(query, display, name) {
  if (!query) return display;
  const displaySuffix = suffix(display, query);
  return displaySuffix || (/[\\/]/u.test(query) ? suffix(name, query) : "");
}

// frontend/sugarprompt/core/src/normalization/lexicon.ts
var identityWhitespace = new RegExp(whitespace.source + "+", "gu");
function normalizeTagIdentity(text) {
  return strip(caseFold(text.replaceAll("_", " "))).replace(identityWhitespace, " ");
}
function prepareTagLexicon(tags) {
  const identities = new Set(tags.map(normalizeTagIdentity));
  identities.delete("");
  return Object.freeze({
    /** Inspect one normalized exact identity without traversing the catalog. */
    contains(text) {
      return identities.has(normalizeTagIdentity(text));
    }
  });
}

// frontend/sugarprompt/core/src/completion/text.ts
function tagReplacementText(tag2) {
  return tag2.replaceAll("_", " ").replace(/[()]/gu, "\\$&");
}
function charactersMatch(left, right) {
  return left === right || left === "_" && right === " " || left === " " && right === "_";
}
function completionSuffix(suggestion2, prefix) {
  const proposed = Array.from(suggestion2), count = matchingPrefixLength(proposed, prefix);
  return count < 0 ? "" : proposed.slice(count).join("");
}
function matchingPrefixLength(proposed, prefix) {
  const typed = Array.from(prefix);
  return typed.length > proposed.length || typed.some(
    (character, index) => !charactersMatch(character, requireValue(proposed[index], "text"))
  ) ? -1 : typed.length;
}
function suffixWithoutRightText(suffix2, rightText) {
  const right = Array.from(rightText.replace(/[ \t]+$/u, "")), proposed = Array.from(suffix2);
  if (!right.length || right.length > proposed.length) return suffix2;
  const start = proposed.length - right.length;
  return right.every(
    (character, index) => charactersMatch(character, requireValue(proposed[start + index], "text"))
  ) ? proposed.slice(0, start).join("") : suffix2;
}
function tagIsNoop(source, query, tag2) {
  assertTagQuery(source, query);
  return normalizeTagIdentity(
    mapVisibleSource(source.slice(query.replacementRange.from, query.activeTagEnd)).text
  ) === normalizeTagIdentity(mapVisibleSource(tagReplacementText(tag2)).text);
}
function tagGhostText(source, query, tag2) {
  assertTagQuery(source, query);
  const suffix2 = completionSuffix(mapVisibleSource(tagReplacementText(tag2)).text, query.prefix);
  return suffixWithoutRightText(
    suffix2,
    mapVisibleSource(source.slice(query.replacementRange.to, query.activeTagEnd)).text
  );
}
function continuationReplacement(source, query, replacementText, to, addComma) {
  let end = to;
  let continuation = addComma ? ", " : "";
  if (addComma && to === query.activeTagEnd) {
    let comma = to;
    while (source[comma] === " " || source[comma] === "	") comma++;
    if (source[comma] === ",") {
      end = comma + 1;
      while (source[end] === " " || source[end] === "	") end++;
      continuation = source.slice(to, end);
    }
  }
  return Object.freeze({
    range: Object.freeze({ from: query.replacementRange.from, to: end }),
    text: replacementText + continuation
  });
}
function prepareTagReplacement(source, query, tag2, addComma) {
  assertTagQuery(source, query);
  const replacementText = tagReplacementText(tag2);
  const suggestion2 = mapVisibleSource(replacementText).text;
  const right = mapVisibleSource(source.slice(query.replacementRange.to, query.activeTagEnd)).text;
  const beforeRight = suffixWithoutRightText(suggestion2, right);
  const retainRight = continuationReplacement(
    source,
    query,
    replacementText,
    query.replacementRange.to,
    addComma
  );
  if (beforeRight === suggestion2) return Object.freeze({ retainRight, right: null });
  const proposed = Object.freeze(Array.from(beforeRight));
  const maximumSourcePrefixLength = beforeRight.length * 2;
  function consumesVisiblePrefix(prefix) {
    return matchingPrefixLength(proposed, prefix) >= 0;
  }
  return Object.freeze({
    retainRight,
    right: Object.freeze({
      replacement: continuationReplacement(
        source,
        query,
        replacementText,
        query.activeTagEnd,
        addComma
      ),
      match: Object.freeze({
        maximumSourcePrefixLength,
        consumesVisiblePrefix,
        /** Reject oversized input before any visible-source decoding. */
        consumesSourcePrefix(prefix) {
          return prefix.length <= maximumSourcePrefixLength && consumesVisiblePrefix(mapVisibleSource(prefix).text);
        }
      })
    })
  });
}

// frontend/sugarprompt/core/src/diagnostics/duplicate-segments.ts
function occurrence(document, range, emphasis) {
  const source = document.sourceText;
  let content = range;
  let weight = "1.00";
  for (; ; ) {
    const span = emphasis.get(content.from);
    if (!span || span.outerRange.to !== content.to) break;
    if (span.weightRange.from !== span.weightRange.to) weight = span.weight;
    content = span.contentRange;
  }
  const text = source.slice(content.from, content.to);
  if (/[<>{}[\]|\\\r\n]/u.test(text)) return null;
  const key = normalizeTagIdentity(text);
  if (!key || key.includes("://") || key.includes("/") || /(?:\.safetensors|\.ckpt|\.pt|\.pth|\.onnx)$/u.test(key) || /^[a-f0-9]{8,}$/u.test(key))
    return null;
  return freezeStructure({ range, contentRange: content, key, weight });
}
function occurrences(document, emphasis, blocked) {
  const result = [];
  let blockedIndex = 0;
  for (const segment of document.segments) {
    let start = segment.contentRange.from;
    for (const end of [
      ...segment.hardBreaks,
      { from: segment.contentRange.to, to: segment.contentRange.to }
    ]) {
      let from = start, to = end.from;
      start = end.to;
      while (from < to && whitespace.test(requireValue(document.sourceText[from], "duplicate-segments")))
        from++;
      while (to > from && whitespace.test(requireValue(document.sourceText[to - 1], "duplicate-segments")))
        to--;
      if (from >= to) continue;
      while (blocked[blockedIndex] && requireValue(blocked[blockedIndex], "duplicate-segments").sourceRange.to <= from)
        blockedIndex++;
      if (blocked[blockedIndex] && requireValue(blocked[blockedIndex], "duplicate-segments").sourceRange.from < to)
        continue;
      const value = occurrence(document, { from, to }, emphasis);
      if (value) result.push(value);
    }
  }
  return result;
}
function firstOccurrence(first, key) {
  for (let current = first; current; current = current.parent) {
    const found = current.own.get(key);
    if (found) return found;
  }
  return void 0;
}
function occurrenceStart(values, from) {
  let low = 0, high = values.length;
  while (low < high) {
    const middle = low + high >>> 1;
    if (requireValue(values[middle], "duplicate-segments").range.from < from) low = middle + 1;
    else high = middle;
  }
  return low;
}
function prepareDuplicateSegments(document, ownership, previous) {
  if (previous?.document === document && previous.ownership === ownership) return previous;
  const emphasis = new Map(document.emphasisSpans.map((span) => [span.outerRange.from, span]));
  const blocked = document.syntaxSpans.filter((span) => span.kind !== "emphasis");
  const values = occurrences(document, emphasis, blocked);
  const findings = [];
  const collect = (range, id, parent) => {
    const first = { own: /* @__PURE__ */ new Map(), parent };
    for (let index = occurrenceStart(values, range.from); index < values.length; index++) {
      const value = requireValue(values[index], "duplicate-segments");
      if (value.range.to > range.to) break;
      const previous2 = firstOccurrence(first, value.key);
      if (previous2) findings.push({ scopeId: id, first: previous2, duplicate: value });
      else first.own.set(value.key, value);
    }
    return first;
  };
  if (ownership.kind === "value") {
    collect({ from: 0, to: document.sourceText.length }, "value", null);
  } else if (ownership.kind === "scenes") {
    const contexts = ownership.contexts;
    if (contexts.document.sourceText !== document.sourceText)
      throw new Error("foreign-duplicate-scene-source");
    const universal = collect(contexts.document.universalRange, "universal", null);
    for (const context of contexts.scenes) {
      const scene = context.scene;
      if (!scene) throw new Error("missing-duplicate-scene");
      collect(scene.contentRange, `scene:${scene.marker.markerRange.from}`, universal);
    }
  } else {
    const topology = ownership.topology;
    if (topology.scopes.length !== document.regions.partitions.length || topology.scopes.some(
      (scope, index) => scope.range.from !== requireValue(document.regions.partitions[index], "duplicate-segments").sourceRange.from || scope.range.to !== requireValue(document.regions.partitions[index], "duplicate-segments").sourceRange.to
    ))
      throw new Error("foreign-duplicate-conditioning-source");
    const visit = (id, parent) => {
      const scope = topology.scope(id), first = collect(scope.range, scope.id, parent);
      for (const child of topology.children(id)) visit(child.id, first);
    };
    for (const root of topology.roots) visit(root.id, null);
  }
  return freezeStructure({ document, ownership, findings });
}

// frontend/sugarprompt/core/src/normalization/decimals.ts
function increment(digits) {
  const characters = [...Array.from(digits)];
  for (let index = characters.length - 1; index >= 0; index--) {
    if (characters[index] !== "9") {
      characters[index] = String(Number(characters[index]) + 1);
      return characters.join("");
    }
    characters[index] = "0";
  }
  return "1" + characters.join("");
}
function decimalParts(value) {
  const matched = /^(-?)(\d+)(?:\.(\d+))?(?:E([+-]?\d+))?$/u.exec(value);
  if (!matched) throw new TypeError("Invalid parsed decimal");
  const sign = requireValue(matched[1], "decimals"), fraction = matched[3] ?? "";
  const coefficient = (requireValue(matched[2], "decimals") + fraction).replace(/^0+/u, "") || "0";
  const scale = fraction.length - Number(matched[4] ?? 0);
  if (!Number.isSafeInteger(scale)) throw new RangeError("Invalid decimal scale");
  return { sign, coefficient, scale };
}
function formatWeight(value) {
  const { sign, coefficient, scale } = decimalParts(value);
  let rounded;
  if (scale <= 2) rounded = coefficient + "0".repeat(2 - scale);
  else {
    const cut = scale - 2, boundary2 = coefficient.length - cut;
    rounded = boundary2 > 0 ? coefficient.slice(0, boundary2) : "0";
    const remainder = boundary2 >= 0 ? coefficient.slice(boundary2) : "0";
    const greater = requireValue(remainder[0], "decimals") > "5" || remainder.startsWith("5") && /[1-9]/u.test(remainder.slice(1));
    const tie = remainder.startsWith("5") && !/[1-9]/u.test(remainder.slice(1));
    if (greater || tie && Number(rounded.at(-1)) % 2 === 1) rounded = increment(rounded);
  }
  rounded = rounded.padStart(3, "0");
  return sign + rounded.slice(0, -2) + "." + rounded.slice(-2);
}
function implicitWeight(depth) {
  if (!Number.isSafeInteger(depth) || depth < 1)
    throw new RangeError("Invalid implicit emphasis depth");
  const coefficient = (11n ** BigInt(depth)).toString().padStart(depth + 1, "0");
  return coefficient.slice(0, -depth) + "." + coefficient.slice(-depth).padEnd(2, "0");
}
function addWeights(left, right) {
  const a = decimalParts(left), b = decimalParts(right);
  const precision = Math.max(0, a.scale, b.scale);
  const first = BigInt(a.coefficient) * 10n ** BigInt(precision - a.scale);
  const second = BigInt(b.coefficient) * 10n ** BigInt(precision - b.scale);
  const sum = (a.sign === "-" ? -first : first) + (b.sign === "-" ? -second : second);
  const digits = (sum < 0n ? -sum : sum).toString().padStart(precision + 1, "0");
  return (sum < 0n ? "-" : "") + (precision ? digits.slice(0, -precision) + "." + digits.slice(-precision) : digits);
}
function stepWeight(value, direction) {
  return formatWeight(addWeights(value, direction === "increase" ? "0.05" : "-0.05"));
}

// frontend/sugarprompt/core/src/diagnostics/duplicate-edits.ts
function removal(source, finding) {
  const range = finding.duplicate.range;
  let after = range.to;
  while (source[after] === " " || source[after] === "	") after++;
  if (source[after] === ",") {
    after++;
    while (source[after] === " " || source[after] === "	") after++;
    return { from: range.from, to: after, text: "" };
  }
  let before = range.from;
  while (source[before - 1] === " " || source[before - 1] === "	") before--;
  return {
    from: source[before - 1] === "," ? before - 1 : range.from,
    to: range.to,
    text: ""
  };
}
function prepareDuplicateEdit(prepared, source, finding, action) {
  if (source !== prepared.document.sourceText) return { status: "rejected", reason: "stale" };
  if (!prepared.findings.includes(finding)) return { status: "rejected", reason: "foreign" };
  const edits = [removal(source, finding)];
  if (action === "emphasize-first") {
    const excess = addWeights(finding.duplicate.weight, "-1.00");
    const weight = formatWeight(
      addWeights(addWeights(finding.first.weight, "0.10"), excess.startsWith("-") ? "0" : excess)
    );
    const content = finding.first.contentRange;
    edits.push({
      ...finding.first.range,
      text: `(${source.slice(content.from, content.to)}:${weight})`,
      boundaryPositions: Array.from(
        { length: finding.first.range.to - finding.first.range.from + 1 },
        (_, offset) => 1 + Math.max(
          0,
          Math.min(content.to - content.from, finding.first.range.from + offset - content.from)
        )
      )
    });
  }
  edits.sort((a, b) => a.from - b.from);
  if (edits.some(
    (edit, index) => index > 0 && edit.from < requireValue(edits[index - 1], "duplicate-edits").to
  ))
    return { status: "rejected", reason: "overlap" };
  return freezeStructure({
    status: "ready",
    edits,
    output: applyReplacements(source, edits)
  });
}

// frontend/sugarprompt/core/src/diagnostics/visibility.ts
var punctuation = new Set(Array.from(`,;:.!?)]}"'`));
function isDiagnosticCommitBoundary(character) {
  const point = character.codePointAt(0);
  if (point === void 0 || character.length !== (point > 65535 ? 2 : 1)) return false;
  return whitespace.test(character) || punctuation.has(character);
}
function diagnosticIsActive(from, to, caret, committedAtEnd) {
  if (![from, to, caret].every(Number.isSafeInteger) || from < 0 || to < from || caret < 0)
    throw new RangeError("Invalid diagnostic position");
  return from < caret && caret < to || caret === to && !committedAtEnd;
}

// frontend/sugarprompt/core/src/normalization/weights.ts
function weightReplacement(source, range, value) {
  const text = formatWeight(value), before = source.slice(range.from, range.to), boundaries = [];
  let units = 0, scalar = 0;
  for (const character of before) {
    const position = Math.min(scalar, text.length);
    boundaries[units] = position;
    if (character.length === 2) boundaries[units + 1] = Math.min(position + 1, text.length);
    units += character.length;
    scalar++;
  }
  boundaries[units] = text.length;
  return { ...range, text, boundaryPositions: boundaries };
}
function normalizeWeights(text) {
  const document = parsePrompt(text);
  const edits = document.emphasisSpans.map(
    (span) => weightReplacement(text, span.weightRange, span.weight)
  );
  for (const span of document.loraSpans) {
    edits.push(weightReplacement(text, span.firstWeightRange, span.firstWeight));
    if (span.secondWeightRange && span.secondWeight !== null)
      edits.push(weightReplacement(text, span.secondWeightRange, span.secondWeight));
  }
  return applyReplacements(
    text,
    edits.sort((a, b) => a.from - b.from)
  );
}

// frontend/sugarprompt/core/src/normalization/parenthesis-pairs.ts
function parenthesisPairs(source) {
  const stack = [], pairs2 = [];
  let quote = null, angles = 0;
  for (let index = 0; index < source.length; index++) {
    const character = requireValue(source[index], "parenthesis-pairs");
    if (character === "\\") {
      index++;
      continue;
    }
    if (quote !== null) {
      if (character === quote && isStructuralQuote(source, index)) quote = null;
      continue;
    }
    if (character === "<") {
      angles++;
      continue;
    }
    if (character === ">" && angles) {
      angles--;
      continue;
    }
    if ((character === "'" || character === '"') && !angles && isStructuralQuote(source, index)) {
      quote = character;
      continue;
    }
    if (angles) continue;
    if (character === "(") stack.push(index);
    else if (character === ")" && stack.length)
      pairs2.push({
        from: requireValue(stack.pop(), "parenthesis-pairs"),
        close: index,
        depth: stack.length + 1
      });
  }
  return pairs2.sort((a, b) => a.from - b.from);
}

// frontend/sugarprompt/core/src/normalization/sequence-map.ts
function longest(source, target, from, to, start, end) {
  const states = [{ next: /* @__PURE__ */ new Map(), length: 0, link: -1, firstEnd: Infinity }];
  let last = 0;
  for (let index = start; index < end; index++) {
    const character = requireValue(target[index], "sequence-map"), current = states.length;
    states.push({
      next: /* @__PURE__ */ new Map(),
      length: requireValue(states[last], "sequence-map").length + 1,
      link: 0,
      firstEnd: index
    });
    let parent = last;
    while (parent >= 0 && !requireValue(states[parent], "sequence-map").next.has(character)) {
      requireValue(states[parent], "sequence-map").next.set(character, current);
      parent = requireValue(states[parent], "sequence-map").link;
    }
    if (parent >= 0) {
      const next = requireValue(
        requireValue(states[parent], "sequence-map").next.get(character),
        "sequence-map"
      );
      if (requireValue(states[parent], "sequence-map").length + 1 === requireValue(states[next], "sequence-map").length)
        requireValue(states[current], "sequence-map").link = next;
      else {
        const clone = states.length;
        states.push({
          ...requireValue(states[next], "sequence-map"),
          next: new Map(requireValue(states[next], "sequence-map").next),
          length: requireValue(states[parent], "sequence-map").length + 1
        });
        while (parent >= 0 && requireValue(states[parent], "sequence-map").next.get(character) === next) {
          requireValue(states[parent], "sequence-map").next.set(character, clone);
          parent = requireValue(states[parent], "sequence-map").link;
        }
        requireValue(states[next], "sequence-map").link = clone;
        requireValue(states[current], "sequence-map").link = clone;
      }
    }
    last = current;
  }
  for (const state2 of [...states].sort((a, b) => b.length - a.length)) {
    if (state2.link >= 0)
      requireValue(states[state2.link], "sequence-map").firstEnd = Math.min(
        requireValue(states[state2.link], "sequence-map").firstEnd,
        state2.firstEnd
      );
  }
  let best = { source: from, target: start, length: 0 }, state = 0, length = 0;
  for (let index = from; index < to; index++) {
    const character = requireValue(source[index], "sequence-map");
    while (state && !requireValue(states[state], "sequence-map").next.has(character)) {
      state = requireValue(states[state], "sequence-map").link;
      length = requireValue(states[state], "sequence-map").length;
    }
    const next = requireValue(states[state], "sequence-map").next.get(character);
    if (next === void 0) {
      state = 0;
      length = 0;
      continue;
    }
    state = next;
    length++;
    if (length > best.length) {
      best = {
        source: index - length + 1,
        target: requireValue(states[state], "sequence-map").firstEnd - length + 1,
        length
      };
    }
  }
  return best;
}
function sequenceBoundaryMap(before, after) {
  const source = [...Array.from(before)], target = [...Array.from(after)];
  const queue = [[0, source.length, 0, target.length]], matches = [];
  while (queue.length) {
    const [from2, to, start2, end] = requireValue(queue.pop(), "sequence-map");
    const match2 = longest(source, target, from2, to, start2, end);
    if (!match2.length) continue;
    matches.push(match2);
    if (from2 < match2.source && start2 < match2.target)
      queue.push([from2, match2.source, start2, match2.target]);
    const nextSource = match2.source + match2.length, nextTarget = match2.target + match2.length;
    if (nextSource < to && nextTarget < end) queue.push([nextSource, to, nextTarget, end]);
  }
  const blocks = [];
  for (const match2 of matches.sort((a, b) => a.source - b.source || a.target - b.target)) {
    const previous = blocks.at(-1);
    if (previous && previous.source + previous.length === match2.source && previous.target + previous.length === match2.target)
      blocks[blocks.length - 1] = {
        ...previous,
        length: previous.length + match2.length
      };
    else blocks.push(match2);
  }
  blocks.push({ source: source.length, target: target.length, length: 0 });
  const points = Array.from({ length: source.length + 1 }, () => 0);
  let from = 0, start = 0;
  for (const block of blocks) {
    if (from < block.source || start < block.target) {
      points[from] = start;
      points[block.source] = block.target;
      for (let offset = 1; offset < block.source - from; offset++)
        points[from + offset] = start + Math.min(offset, block.target - start);
    }
    for (let offset = 0; offset <= block.length; offset++)
      points[block.source + offset] = block.target + offset;
    from = block.source + block.length;
    start = block.target + block.length;
  }
  const targetUnits = [0];
  target.forEach(
    (character) => targetUnits.push(requireValue(targetUnits.at(-1), "sequence-map") + character.length)
  );
  const boundaries = [];
  let units = 0;
  source.forEach((character, index) => {
    const mapped = requireValue(
      targetUnits[requireValue(points[index], "sequence-map")],
      "sequence-map"
    );
    boundaries[units] = mapped;
    for (let offset = 1; offset < character.length; offset++)
      boundaries[units + offset] = mapped + Math.min(
        offset,
        requireValue(
          targetUnits[requireValue(points[index + 1], "sequence-map")],
          "sequence-map"
        ) - mapped
      );
    units += character.length;
  });
  boundaries[units] = requireValue(
    targetUnits[requireValue(points[source.length], "sequence-map")],
    "sequence-map"
  );
  return boundaries;
}

// frontend/sugarprompt/core/src/normalization/parentheses.ts
function renderUnknown(text, pairs2, explicit, generated) {
  const byOpen = new Map(pairs2.map((pair) => [pair.from, pair]));
  const tasks = [{ kind: "range", from: 0, to: text.length }], parts = [], generatedShells = [];
  let maximum = 1, length = 0;
  function append(value) {
    parts.push(value);
    length += value.length;
  }
  while (tasks.length) {
    const task = requireValue(tasks.pop(), "parentheses");
    if (task.kind === "close") {
      append(":" + task.weight + ")");
      if (task.depth !== null)
        generatedShells.push({
          from: task.from,
          to: length,
          nestingDepth: task.depth
        });
      continue;
    }
    let cursor = task.from;
    while (cursor < task.to) {
      const pair = byOpen.get(cursor);
      if (!pair || pair.close >= task.to) {
        append(requireValue(text[cursor], "parentheses"));
        cursor++;
        continue;
      }
      const weighted = explicit.get(pair.from);
      let from = pair.from + 1, to = pair.close, weight, generatedDepth = null;
      if (weighted?.outerRange.to === pair.close + 1) {
        from = weighted.contentRange.from;
        to = weighted.contentRange.to;
        weight = text.slice(weighted.weightRange.from, weighted.weightRange.to);
        const authored = generated.get(pair.from);
        if (authored?.to === pair.close + 1) generatedDepth = authored.nestingDepth;
      } else {
        let depth = 1;
        while (from < to) {
          const nested = byOpen.get(from);
          if (!nested || nested.close !== to - 1) break;
          const inner = explicit.get(nested.from);
          if (inner?.outerRange.to === nested.close + 1) {
            const authored = generated.get(nested.from);
            if (!authored || authored.to !== nested.close + 1) break;
            depth += authored.nestingDepth;
            from = inner.contentRange.from;
            to = inner.contentRange.to;
          } else {
            depth++;
            from = nested.from + 1;
            to = nested.close;
          }
        }
        maximum = Math.max(maximum, depth);
        weight = implicitWeight(depth);
        generatedDepth = depth;
      }
      const shellFrom = length;
      append("(");
      tasks.push(
        { kind: "range", from: pair.close + 1, to: task.to },
        { kind: "close", weight, from: shellFrom, depth: generatedDepth },
        { kind: "range", from, to }
      );
      break;
    }
  }
  return {
    text: parts.join(""),
    depth: maximum,
    generated: generatedShells.sort((a, b) => a.from - b.from)
  };
}
function escapeLiteral(text, pairs2) {
  const positions = new Set(pairs2.flatMap((pair) => [pair.from, pair.close]));
  const result = [];
  for (let index = 0; index < text.length; index++)
    result.push((positions.has(index) ? "\\" : "") + requireValue(text[index], "parentheses"));
  return result.join("");
}
function canonicalizeParentheses(text, options = {}) {
  const document = parsePrompt(text), edits = [], transitions = [], generatedShells = [];
  let delta = 0;
  for (const generated of options.generatedEmphases ?? []) {
    if (!Number.isSafeInteger(generated.nestingDepth) || generated.nestingDepth < 1 || !Number.isInteger(generated.from) || !Number.isInteger(generated.to) || generated.from < 0 || generated.to <= generated.from || generated.to > text.length)
      throw new RangeError("Invalid generated emphasis");
  }
  for (const segment of document.segments) {
    const leading = requireValue(/^[ \t]*/u.exec(segment.text), "parentheses")[0].length, trailing = requireValue(/[ \t]*$/u.exec(segment.text), "parentheses")[0].length;
    const from = segment.contentRange.from + leading, to = segment.contentRange.to - trailing;
    if (from >= to) continue;
    const before = text.slice(from, to), pairs2 = parenthesisPairs(before);
    if (!pairs2.length) continue;
    const literal = options.forceLiteral === true || options.lexicon?.contains(before) === true;
    const explicit = new Map(
      document.emphasisSpans.filter((span) => span.outerRange.from >= from && span.outerRange.to <= to).map((span) => [
        span.outerRange.from - from,
        {
          ...span,
          outerRange: {
            from: span.outerRange.from - from,
            to: span.outerRange.to - from
          },
          contentRange: {
            from: span.contentRange.from - from,
            to: span.contentRange.to - from
          },
          weightRange: {
            from: span.weightRange.from - from,
            to: span.weightRange.to - from
          }
        }
      ])
    );
    const generated = new Map(
      (options.generatedEmphases ?? []).filter((span) => span.from >= from && span.to <= to).map((span) => [span.from - from, { ...span, from: span.from - from, to: span.to - from }])
    );
    const rendered = literal ? { text: escapeLiteral(before, pairs2), depth: 1, generated: [] } : renderUnknown(before, pairs2, explicit, generated);
    generatedShells.push(
      ...rendered.generated.map((shell) => ({
        ...shell,
        from: from + delta + shell.from,
        to: from + delta + shell.to
      }))
    );
    if (rendered.text === before) continue;
    delta += rendered.text.length - before.length;
    edits.push({
      from,
      to,
      text: rendered.text,
      boundaryPositions: sequenceBoundaryMap(before, rendered.text)
    });
    transitions.push({
      from,
      to,
      kind: literal ? "known_literal_tag" : "implicit_emphasis",
      nestingDepth: rendered.depth
    });
  }
  return Object.freeze({
    ...applyReplacements(text, edits, transitions),
    generatedEmphases: Object.freeze(generatedShells.map((shell) => Object.freeze(shell)))
  });
}
function isExplicitEmphasis(text) {
  const document = parsePrompt(text);
  return document.segments.length === 1 && !document.hasTrailingComma && document.emphasisSpans.some(
    (span) => span.outerRange.from === 0 && span.outerRange.to === text.length
  );
}

// frontend/sugarprompt/core/src/normalization/storage.ts
function normalizeForStorage(text, options = {}) {
  const parentheses = canonicalizeParentheses(text, options);
  return composeNormalizations(parentheses, normalizeWeights(parentheses.text));
}

// frontend/sugarprompt/core/src/normalization/insertions.ts
function prepareInsertions(text, insertions) {
  if (!insertions.length) {
    const normalization = identityNormalization(text);
    return Object.freeze({
      normalization,
      originalPositions: normalization.boundaryPositions
    });
  }
  const changes = /* @__PURE__ */ new Map();
  for (const insertion of insertions) {
    if (!Number.isInteger(insertion.position) || insertion.position < 0 || insertion.position > text.length || changes.has(insertion.position))
      throw new RangeError("Invalid source insertion");
    changes.set(insertion.position, insertion);
  }
  const parts = [], boundaries = [], original = [];
  let target = 0;
  for (let position = 0; position <= text.length; position++) {
    const insertion = changes.get(position);
    if (insertion?.affinity === "before") boundaries[position] = target;
    original[target] = position;
    if (insertion) {
      parts.push(insertion.text);
      for (let offset = 1; offset <= insertion.text.length; offset++)
        original[target + offset] = position;
      target += insertion.text.length;
    }
    if (!insertion || insertion.affinity === "after") boundaries[position] = target;
    if (position < text.length) {
      parts.push(requireValue(text[position], "insertions"));
      target++;
    }
  }
  return Object.freeze({
    normalization: Object.freeze({
      text: parts.join(""),
      boundaryPositions: Object.freeze(boundaries),
      transitions: Object.freeze([]),
      generatedEmphases: Object.freeze([])
    }),
    originalPositions: Object.freeze(original)
  });
}

// frontend/sugarprompt/core/src/normalization/region-context.ts
function isEscaped(source, position) {
  let start = position;
  while (start > 0 && source[start - 1] === "\\") start--;
  return (position - start) % 2 === 1;
}
function sourceLineStart(source, position) {
  if (position <= 0) return 0;
  return Math.max(source.lastIndexOf("\r", position - 1), source.lastIndexOf("\n", position - 1)) + 1;
}
function nextBreak(source, from) {
  const values = [source.indexOf("\r", from), source.indexOf("\n", from)].filter(
    (position) => position >= 0
  );
  return values.length ? Math.min(...values) : null;
}
function separatorWindow(source, position) {
  const from = sourceLineStart(source, position), first = nextBreak(source, position);
  if (first === null) return { from, to: source.length };
  const next = first + (source.startsWith("\r\n", first) ? 2 : 1);
  return { from, to: nextBreak(source, next) ?? source.length };
}
function contextBoundary(source, position, forward) {
  for (let count = 0; count < 512 && (forward ? position < source.length : position > 0); count++) {
    if (forward)
      position += requireValue(source.codePointAt(position), "region-context") > 65535 ? 2 : 1;
    else {
      position--;
      if (position > 0 && /[\udc00-\udfff]/u.test(requireValue(source[position], "region-context")) && /[\ud800-\udbff]/u.test(requireValue(source[position - 1], "region-context")))
        position--;
    }
  }
  return position;
}
function preferredRegionEnding(source, range) {
  const from = contextBoundary(source, range.from, false), to = contextBoundary(source, range.to, true);
  const context = [...Array.from(source.slice(from, to))], marker = [...Array.from(source.slice(from, range.from))].length;
  let ending = "\n", distance = Infinity;
  for (let index = 0; index < context.length; index++) {
    const character = context[index];
    if (character !== "\r" && character !== "\n") continue;
    const candidate = character === "\r" && context[index + 1] === "\n" ? "\r\n" : character;
    if (Math.abs(index - marker) < distance) {
      ending = candidate;
      distance = Math.abs(index - marker);
    }
    if (candidate === "\r\n") index++;
  }
  return ending;
}

// frontend/sugarprompt/core/src/normalization/regions.ts
function prepareMarkers(text, tokens, trailing, beforeBoundary) {
  const insertions = /* @__PURE__ */ new Map();
  for (const token of tokens) {
    const range = token.tokenRange;
    if (isEscaped(text, range.from)) continue;
    const ending = preferredRegionEnding(text, range);
    if (range.from > 0 && !/[\r\n]/u.test(requireValue(text[range.from - 1], "regions"))) {
      if (!insertions.has(range.from))
        insertions.set(range.from, {
          position: range.from,
          text: ending,
          affinity: range.from === beforeBoundary ? "before" : "after"
        });
    }
    if (range.to === text.length && trailing || range.to < text.length && !/[\r\n]/u.test(requireValue(text[range.to], "regions"))) {
      if (!insertions.has(range.to))
        insertions.set(range.to, {
          position: range.to,
          text: ending,
          affinity: range.to === beforeBoundary ? "before" : "after"
        });
    }
  }
  return prepareInsertions(text, [...insertions.values()]);
}
function preparePastedRegions(text, range) {
  validateSourceRange(text, range);
  return prepareMarkers(text, regionTokensInRange(text, range.from, range.to), false);
}
function prepareEmptyRegionInsertion(text, range) {
  validateSourceRange(text, range);
  if (range.from < range.to) {
    for (const ending of ["\r\n", "\n", "\r"]) {
      const tokenEnd = range.from - ending.length;
      if (tokenEnd < 0 || text.slice(tokenEnd, range.from) !== ending) continue;
      const previous = regionSeparatorAt(text, sourceLineStart(text, tokenEnd)), next = regionTokenAt(text, range.to);
      if (previous?.tokenRange.to === tokenEnd && next && (next.tokenRange.to === text.length || /[\r\n]/u.test(requireValue(text[next.tokenRange.to], "regions"))) && !isEscaped(text, range.to))
        return prepareInsertions(text, [{ position: range.to, text: ending, affinity: "before" }]);
    }
  }
  return prepareInsertions(text, []);
}
function prepareTypedRegions(text, range, replacement) {
  validateSourceRange(text, range);
  let token = null;
  if (replacement === "]" && range.to - range.from === 1) {
    const start = text.lastIndexOf("[SEP", range.to - 1);
    if (start >= sourceLineStart(text, range.to - 1)) token = regionTokenAt(text, start);
  }
  const tokens = replacement.length > 1 && range.to - range.from > 1 ? regionTokensInRange(text, sourceLineStart(text, range.from), range.to).filter(
    (item) => item.tokenRange.to > range.from
  ) : token && token.tokenRange.to === range.to && !isEscaped(text, token.tokenRange.from) ? [token] : [];
  if (!tokens.length) return prepareInsertions(text, []);
  const prepared = prepareMarkers(text, tokens, true), result = prepared.normalization;
  if (!tokens.some((item) => item.tokenRange.to === range.to)) return prepared;
  const after = range.to + (text.startsWith("\r\n", range.to) ? 2 : /[\r\n]/u.test(text[range.to] ?? "") ? 1 : 0), boundaries = [...result.boundaryPositions];
  for (let position = range.to; position <= after; position++)
    boundaries[position] = requireValue(result.boundaryPositions[after], "regions");
  return Object.freeze({
    ...prepared,
    normalization: Object.freeze({
      ...result,
      boundaryPositions: Object.freeze(boundaries)
    })
  });
}
function normalizeTypedDeletion(text, position) {
  validateSourceRange(text, { from: position, to: position });
  const window = separatorWindow(text, position);
  const beforeBoundary = regionTokenAt(text, position) && position > 0 && !/[\r\n]/u.test(requireValue(text[position - 1], "regions")) ? position : void 0;
  return prepareMarkers(
    text,
    regionTokensInRange(text, window.from, window.to),
    false,
    beforeBoundary
  ).normalization;
}

// frontend/sugarprompt/core/src/normalization/typed-parentheses.ts
function matchingParenthesis(text, position, forward) {
  let depth = 0;
  for (let index = position; index >= 0 && index < text.length; index += forward ? 1 : -1) {
    if (isEscaped(text, index)) continue;
    const character = text[index];
    if (character === (forward ? "(" : ")")) depth++;
    else if (character === (forward ? ")" : "(")) {
      depth--;
      if (depth === 0) return index;
    }
  }
  return null;
}
function containingSegment(text, range) {
  return parsePrompt(text).segments.find(
    (segment) => segment.contentRange.from <= range.from && range.to <= segment.contentRange.to
  )?.contentRange ?? range;
}
function typedParenthesisScope(text, edit) {
  if (edit.to - edit.from > 1 && edit.replacementText.length > 1) {
    if (!/[()]/u.test(edit.replacementText)) return null;
    const completed = parenthesisPairs(text).filter(
      (pair) => pair.depth === 1 && pair.from < edit.to && pair.close >= edit.from
    );
    if (!completed.length) return null;
    const first = containingSegment(text, {
      from: requireValue(completed[0], "typed-parentheses").from,
      to: requireValue(completed[0], "typed-parentheses").close + 1
    }), last = containingSegment(text, {
      from: requireValue(completed.at(-1), "typed-parentheses").from,
      to: requireValue(completed.at(-1), "typed-parentheses").close + 1
    });
    return { from: first.from, to: last.to };
  }
  if (edit.to - edit.from !== 1 || edit.replacementText.length !== 1) return null;
  if (edit.replacementText === "(") {
    const close = matchingParenthesis(text, edit.from, true);
    return close === null ? null : containingSegment(text, { from: edit.from, to: close + 1 });
  }
  if (edit.replacementText !== ")") return null;
  const open = matchingParenthesis(text, edit.from, false);
  if (open === null) return null;
  const scope = containingSegment(text, { from: open, to: edit.to }), pairs2 = parenthesisPairs(text.slice(scope.from, scope.to));
  const closed = pairs2.find(
    (pair) => pair.from === open - scope.from && pair.close === edit.from - scope.from
  );
  if (closed && closed.depth > pairs2.filter((pair) => pair.from < closed.from && pair.close > closed.close).length + 1)
    return null;
  return scope;
}
function enclosingEscapedGroup(text, position) {
  const stack = [];
  let result = null;
  for (let index = 0; index < text.length - 1; index++) {
    if (text[index] !== "\\" || isEscaped(text, index)) continue;
    if (text[index + 1] === "(") {
      stack.push(index);
      index++;
    } else if (text[index + 1] === ")" && stack.length) {
      const from = requireValue(stack.pop(), "typed-parentheses"), to = index + 2;
      if (from < position && position < to && (!result || to - from < result.to - result.from))
        result = { from, to };
      index++;
    }
  }
  return result;
}
function reclassifyTypedEmphasis(text, edit) {
  if (edit.to <= edit.from || !/[\d:.]$/u.test(edit.replacementText)) return null;
  const scope = enclosingEscapedGroup(text, edit.to - 1);
  if (!scope) return null;
  const candidate = "(" + text.slice(scope.from + 2, scope.to - 2) + ")";
  if (!isExplicitEmphasis(candidate)) return null;
  return applyReplacements(
    text,
    [
      { from: scope.from, to: scope.from + 1, text: "" },
      { from: scope.to - 2, to: scope.to - 1, text: "" }
    ],
    [{ ...scope, kind: "escaped_literal_to_emphasis", nestingDepth: 1 }]
  );
}

// frontend/sugarprompt/core/src/normalization/local.ts
function normalizePasteRange(text, range, options = {}) {
  validateSourceRange(text, range);
  const empty3 = prepareEmptyRegionInsertion(text, range), first = empty3.normalization;
  const separators = preparePastedRegions(first.text, {
    from: requireValue(first.boundaryPositions[range.from], "local"),
    to: requireValue(first.boundaryPositions[range.to], "local")
  }), second = separators.normalization;
  const canonical = normalizeRange(
    second.text,
    {
      from: requireValue(
        second.boundaryPositions[requireValue(first.boundaryPositions[range.from], "local")],
        "local"
      ),
      to: requireValue(
        second.boundaryPositions[requireValue(first.boundaryPositions[range.to], "local")],
        "local"
      )
    },
    (source) => normalizeForStorage(source, options)
  );
  return Object.freeze({
    text: canonical.text,
    boundaryPositions: Object.freeze(
      first.boundaryPositions.map(
        (position) => requireValue(
          canonical.boundaryPositions[requireValue(second.boundaryPositions[position], "local")],
          "local"
        )
      )
    ),
    transitions: Object.freeze(
      canonical.transitions.map(
        (transition) => Object.freeze({
          ...transition,
          from: requireValue(
            empty3.originalPositions[requireValue(separators.originalPositions[transition.from], "local")],
            "local"
          ),
          to: requireValue(
            empty3.originalPositions[requireValue(separators.originalPositions[transition.to], "local")],
            "local"
          )
        })
      )
    ),
    generatedEmphases: canonical.generatedEmphases
  });
}
function normalizeTypedEdit(text, edit, options = {}) {
  validateSourceRange(text, edit);
  const empty3 = prepareEmptyRegionInsertion(text, edit), preparation = empty3.normalization.text !== text ? empty3 : prepareTypedRegions(text, edit, edit.replacementText), region = preparation.normalization;
  if (region.text !== text) {
    if (edit.replacementText.length === 1) return region;
    const from = requireValue(region.boundaryPositions[edit.from], "local"), to = requireValue(region.boundaryPositions[edit.to], "local");
    const authored = normalizeTypedEdit(
      region.text,
      { from, to, replacementText: region.text.slice(from, to) },
      {
        ...options,
        ...options.generatedEmphases ? {
          generatedEmphases: options.generatedEmphases.map((shell) => ({
            ...shell,
            from: requireValue(region.boundaryPositions[shell.from], "local"),
            to: requireValue(region.boundaryPositions[shell.to], "local")
          }))
        } : {}
      }
    );
    return Object.freeze({
      text: authored.text,
      boundaryPositions: Object.freeze(
        region.boundaryPositions.map(
          (position) => requireValue(authored.boundaryPositions[position], "local")
        )
      ),
      transitions: Object.freeze(
        authored.transitions.map(
          (transition) => Object.freeze({
            ...transition,
            from: requireValue(preparation.originalPositions[transition.from], "local"),
            to: requireValue(preparation.originalPositions[transition.to], "local")
          })
        )
      ),
      generatedEmphases: authored.generatedEmphases
    });
  }
  const explicit = reclassifyTypedEmphasis(text, edit);
  if (explicit) return explicit;
  const scope = typedParenthesisScope(text, edit);
  if (!scope) return region;
  const generated = (options.generatedEmphases ?? []).filter((span) => span.from >= scope.from && span.to <= scope.to).map((span) => ({
    ...span,
    from: span.from - scope.from,
    to: span.to - scope.from
  }));
  return normalizeRange(
    text,
    scope,
    (source) => canonicalizeParentheses(source, {
      ...options,
      generatedEmphases: generated
    })
  );
}

// frontend/sugarprompt/core/src/scenes/headings.ts
function prepareSceneHeadings(document, errorKeys = /* @__PURE__ */ new Set()) {
  return Object.freeze(
    document.scenes.map(({ marker }) => {
      const hostError = errorKeys.has(marker.normalizedKey);
      return Object.freeze({
        marker,
        concealedRange: Object.freeze({
          from: marker.lineRange.from,
          to: marker.titleRange.from
        }),
        visibleRange: Object.freeze({
          from: marker.titleRange.from,
          to: Math.max(marker.titleRange.to, marker.lineRange.to)
        }),
        hostError,
        error: marker.duplicate || hostError
      });
    })
  );
}

// frontend/sugarprompt/core/src/prompt/region-naming.ts
function validRegionName(name) {
  return !/[\]\r\n]/u.test(name);
}
function regionNameReplacement(document, ordinal, name) {
  if (typeof name !== "string" || !validRegionName(name))
    throw new TypeError("Invalid region name");
  if (!Number.isSafeInteger(ordinal) || ordinal < 0) throw new RangeError("Invalid region ordinal");
  const separator = document.regions.separators[ordinal];
  if (!separator) throw new RangeError("Region unavailable");
  const range = name && separator.nameRange ? separator.nameRange : separator.tokenRange;
  return Object.freeze({
    ...range,
    text: name ? separator.nameRange ? name : `[SEP|${name}]` : "[SEP]"
  });
}

// frontend/sugarprompt/core/src/prompt/region-name-identity.ts
function regionNameIdentities(document) {
  return Object.freeze(
    document.regions.separators.map(
      (separator, ordinal) => Object.freeze({ ordinal, name: strip(separator.name ?? "") })
    )
  );
}
function changedRegionIdentities(previous, current) {
  const before = new Map(previous.map((value) => [value.ordinal, value.name]));
  return Object.freeze(
    current.filter(
      (value) => before.has(value.ordinal) ? before.get(value.ordinal) !== value.name : value.name !== ""
    )
  );
}
function applyRegionNames(document, names) {
  const replacements = [], seen = /* @__PURE__ */ new Set();
  for (const { ordinal, name } of names) {
    if (!Number.isSafeInteger(ordinal) || ordinal < 0 || seen.has(ordinal))
      throw new RangeError("Invalid region identity");
    seen.add(ordinal);
    const separator = document.regions.separators[ordinal];
    if (separator && strip(separator.name ?? "") !== name)
      replacements.push(regionNameReplacement(document, ordinal, name));
  }
  replacements.sort((left, right) => left.from - right.from);
  let source = document.sourceText;
  for (let index = replacements.length - 1; index >= 0; index--) {
    const edit = requireValue(replacements[index], "region-name-identity");
    source = source.slice(0, edit.from) + edit.text + source.slice(edit.to);
  }
  return Object.freeze({ source, replacements: Object.freeze(replacements) });
}

// frontend/sugarprompt/core/src/prompt/region-palette.ts
var REGION_PALETTE_BASE = Object.freeze({ r: 79, g: 109, b: 245 });
function regionHue(base, ordinal, total) {
  const hue = (base % 360 + 360) % 360;
  const offset = ordinal <= 0 ? 0 : total === 2 ? 180 : total === 3 ? ordinal === 1 ? 150 : 210 : total === 4 ? ordinal * 90 : total >= 5 ? ordinal * 360 / total : 0;
  return Math.floor((hue + offset) % 360);
}
function regionPaletteColor(base, ordinal, total) {
  if (ordinal <= 0) return Object.freeze({ ...base });
  const { h, s, v } = rgbToHsv(base);
  return hsvToRgb(regionHue(h, ordinal, total), Math.max(s, 200), Math.max(v, 220));
}
function parseRegionHex(value) {
  const match2 = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(value.trim());
  if (!match2?.[1] || !match2[2] || !match2[3]) return null;
  return Object.freeze({
    r: Number.parseInt(match2[1], 16),
    g: Number.parseInt(match2[2], 16),
    b: Number.parseInt(match2[3], 16)
  });
}
function formatRegionHex({ r, g, b }) {
  return "#" + [r, g, b].map((channel) => channel.toString(16).padStart(2, "0")).join("");
}
function rgbToHsv({ r, g, b }) {
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min;
  if (delta === 0) return { h: -1, s: 0, v: max };
  let hue;
  if (max === r) hue = (g - b) / delta % 6;
  else if (max === g) hue = (b - r) / delta + 2;
  else hue = (r - g) / delta + 4;
  return {
    h: Math.round((hue * 60 + 360) % 360) % 360,
    s: Math.round(delta * 255 / max),
    v: max
  };
}
function hsvToRgb(h, s, v) {
  const saturation = s / 255, value = v / 255, sector = h % 360 / 60, chroma = saturation * value, x = chroma * (1 - Math.abs(sector % 2 - 1)), minimum = value - chroma;
  const [r, g, b] = sector < 1 ? [chroma, x, 0] : sector < 2 ? [x, chroma, 0] : sector < 3 ? [0, chroma, x] : sector < 4 ? [0, x, chroma] : sector < 5 ? [x, 0, chroma] : [chroma, 0, x];
  return Object.freeze({
    r: Math.round((r + minimum) * 255),
    g: Math.round((g + minimum) * 255),
    b: Math.round((b + minimum) * 255)
  });
}

// frontend/sugarprompt/core/src/prompt/conditioning.ts
var ConditioningTopology = class {
  mode;
  scopes;
  roots;
  emitted;
  identities;
  inherited;
  emptyChildren = Object.freeze([]);
  /** Derive one topology from canonical source partitions, including leading, adjacent and trailing empties. */
  constructor(mode, regions) {
    this.mode = mode;
    const regional = mode === "regional" && regions.separators.length > 0;
    this.scopes = Object.freeze(
      regions.partitions.map(
        (partition) => Object.freeze({
          id: `partition:${partition.index}`,
          partition: partition.index,
          role: regional ? partition.index === 0 ? "global" : "region" : "condition",
          range: Object.freeze({ ...partition.sourceRange }),
          parent: regional && partition.index !== 0 ? "partition:0" : null,
          emits: !regional || partition.index !== 0
        })
      )
    );
    this.roots = Object.freeze(this.scopes.filter((scope) => scope.parent === null));
    this.emitted = Object.freeze(this.scopes.filter((scope) => scope.emits));
    this.identities = new Map(this.scopes.map((scope) => [scope.id, scope]));
    this.inherited = new Map(
      this.scopes.map((scope) => [
        scope.id,
        Object.freeze(scope.parent ? [this.scope(scope.parent).range, scope.range] : [scope.range])
      ])
    );
    Object.freeze(this);
  }
  /** Resolve a stable scope identity; unknown ownership rejects rather than returning an unrelated partition. */
  scope(id) {
    const scope = this.identities.get(id);
    if (!scope) throw new RangeError("Unknown conditioning scope");
    return scope;
  }
  /** Resolve one zero-based canonical source partition. */
  partition(index) {
    return this.scope(`partition:${index}`);
  }
  /** Binary source lookup excludes separator lines, empty intervals and the terminal source boundary. */
  at(position) {
    let from = 0, to = this.scopes.length;
    while (from < to) {
      const middle = from + to >>> 1;
      if (requireValue(this.scopes[middle], "conditioning").range.from <= position)
        from = middle + 1;
      else to = middle;
    }
    const scope = this.scopes[from - 1];
    return scope && scope.range.from <= position && position < scope.range.to ? scope : null;
  }
  /** Source-ordered direct children share immutable prepared scope objects. */
  children(id) {
    this.scope(id);
    return id === "partition:0" && this.scopes[0]?.role === "global" ? this.emitted : this.emptyChildren;
  }
  /** Reuse prepared inherited/local exact-source UTF-16 ranges; siblings never contribute to one another. */
  effectiveRanges(id) {
    this.scope(id);
    return requireValue(this.inherited.get(id), "conditioning");
  }
};

// frontend/sugarprompt/core/src/reorder/source-pieces.ts
function isReorderBoundary(text, at) {
  if (!Number.isSafeInteger(at) || at < 0 || at > text.length) return false;
  const previous = text.charCodeAt(at - 1), next = text.charCodeAt(at);
  return !(previous >= 55296 && previous <= 56319 && next >= 56320 && next <= 57343) && !(text[at - 1] === "\r" && text[at] === "\n");
}
function sourceText(scope, from, to) {
  const text = scope.document.sourceText;
  if (from > to || !isReorderBoundary(text, from) || !isReorderBoundary(text, to))
    throw new Error("reorder-source-boundary");
  return freezeStructure({
    scope,
    text: text.slice(from, to),
    pieces: from === to ? [] : [{ kind: "source", sourceRange: { from, to } }]
  });
}
function derivedText(scope, text, reason) {
  return freezeStructure({
    scope,
    text,
    pieces: text ? [{ kind: "derived", text, reason }] : []
  });
}
function concatText(first, ...rest) {
  if (rest.some((value) => value.scope !== first.scope)) throw new Error("reorder-carrier-scope");
  return freezeStructure({
    scope: first.scope,
    text: [first, ...rest].map((value) => value.text).join(""),
    pieces: [first, ...rest].flatMap((value) => value.pieces)
  });
}
function sliceText(value, from, to = value.text.length) {
  if (from > to || !isReorderBoundary(value.text, from) || !isReorderBoundary(value.text, to))
    throw new Error("reorder-carrier-boundary");
  const pieces = [];
  let cursor = 0;
  for (const piece of value.pieces) {
    const length = piece.kind === "source" ? piece.sourceRange.to - piece.sourceRange.from : piece.text.length;
    const start = Math.max(from, cursor), end = Math.min(to, cursor + length);
    if (start < end)
      pieces.push(
        piece.kind === "source" ? {
          kind: "source",
          sourceRange: {
            from: piece.sourceRange.from + start - cursor,
            to: piece.sourceRange.from + end - cursor
          }
        } : { ...piece, text: piece.text.slice(start - cursor, end - cursor) }
      );
    cursor += length;
  }
  return freezeStructure({
    scope: value.scope,
    text: value.text.slice(from, to),
    pieces
  });
}
function validReorderText(scope, value) {
  if (!value || value.scope !== scope || typeof value.text !== "string" || !Array.isArray(value.pieces))
    return false;
  const source = scope.document.sourceText;
  const texts = [];
  for (const piece of [...value.pieces]) {
    if (!piece) return false;
    if (piece.kind === "source") {
      const range = piece.sourceRange;
      if (!range || range.from >= range.to || !isReorderBoundary(source, range.from) || !isReorderBoundary(source, range.to))
        return false;
      texts.push(source.slice(range.from, range.to));
    } else if (piece.kind === "derived" && piece.text && typeof piece.text === "string" && (piece.reason === "row-separator" ? /^(?: |, )$/u.test(piece.text) : piece.reason === "blank-line-comma" ? piece.text === "," : piece.reason === "empty-partition-ending" && /^(?:\r\n|\r|\n)$/u.test(piece.text)))
      texts.push(piece.text);
    else return false;
  }
  return texts.join("") === value.text;
}
function atomicOriginPieces(text, pieces) {
  const joined = new Set(
    pieces.slice(1).map((piece) => piece.outputRange.from).filter((at) => !isReorderBoundary(text, at))
  );
  if (!joined.size) return freezeStructure(pieces);
  const split = [];
  for (const piece of pieces) {
    if (piece.kind === "composed") throw new Error("reorder-origin-already-composed");
    const cuts = [
      piece.outputRange.from,
      ...joined.has(piece.outputRange.from) ? [piece.outputRange.from + 1] : [],
      ...joined.has(piece.outputRange.to) ? [piece.outputRange.to - 1] : [],
      piece.outputRange.to
    ].sort((a, b) => a - b);
    for (let index = 0; index < cuts.length - 1; index++) {
      const from = requireValue(cuts[index], "source-pieces"), to = requireValue(cuts[index + 1], "source-pieces");
      if (from === to) continue;
      const outputRange = { from, to };
      if (piece.kind === "derived") split.push({ ...piece, outputRange });
      else {
        const sourceRange2 = {
          from: piece.sourceRange.from + from - piece.outputRange.from,
          to: piece.sourceRange.from + to - piece.outputRange.from
        };
        split.push({
          kind: sourceRange2.from === from && sourceRange2.to === to ? "unchanged" : "copied",
          sourceRange: sourceRange2,
          outputRange
        });
      }
    }
  }
  const result = [];
  function component(piece) {
    if (piece.kind === "composed") throw new Error("reorder-origin-component");
    return piece.kind === "derived" ? {
      kind: "derived",
      text: text.slice(piece.outputRange.from, piece.outputRange.to),
      reason: piece.reason
    } : { kind: piece.kind, sourceRange: piece.sourceRange };
  }
  for (let index = 0; index < split.length; index++) {
    const left = requireValue(split[index], "source-pieces");
    if (joined.has(left.outputRange.to)) {
      const right = split[++index];
      if (!right || left.outputRange.to !== right.outputRange.from || left.outputRange.to - left.outputRange.from !== 1 || right.outputRange.to - right.outputRange.from !== 1)
        throw new Error("reorder-origin-atomic-unit");
      result.push({
        kind: "composed",
        outputRange: { from: left.outputRange.from, to: right.outputRange.to },
        components: [component(left), component(right)]
      });
    } else result.push(left);
  }
  return freezeStructure(result);
}
function reorderOriginBoundary(output, position, association) {
  if (!isReorderBoundary(output.origin.scope.document.sourceText, position))
    return freezeStructure({
      status: "rejected",
      reason: "invalid-source-boundary"
    });
  const images = [];
  const seen = /* @__PURE__ */ new Set();
  function observe(component, from, unitRange) {
    if (component.kind === "derived") return;
    const source = component.sourceRange;
    const included = association < 0 ? source.from < position && position <= source.to : source.from <= position && position < source.to;
    if (!included) return;
    const at = from + position - source.from;
    const image = isReorderBoundary(output.text, at) ? { kind: "boundary", position: at } : { kind: "interior", unitRange };
    const key = image.kind === "boundary" ? "b:" + String(image.position) : "i:" + String(image.unitRange.from) + ":" + String(image.unitRange.to);
    if (!seen.has(key)) {
      seen.add(key);
      images.push(image);
    }
  }
  for (const piece of output.origin.pieces) {
    if (piece.kind === "derived") continue;
    if (piece.kind === "composed") {
      let from = piece.outputRange.from;
      for (const component of piece.components) {
        observe(component, from, piece.outputRange);
        from += component.kind === "derived" ? component.text.length : component.sourceRange.to - component.sourceRange.from;
      }
    } else observe(piece, piece.outputRange.from, piece.outputRange);
  }
  return freezeStructure({ status: "observed", images });
}

// frontend/sugarprompt/core/src/reorder/derivation-spans.ts
var ReorderDerivationSpans = class {
  #emphasis = /* @__PURE__ */ new Map();
  #loras;
  /** Preserve first exact-shell identity and original LoRA enumeration order independently of query sorting. */
  constructor(document) {
    for (const span of document.emphasisSpans) {
      let endings = this.#emphasis.get(span.outerRange.from);
      if (!endings) {
        endings = /* @__PURE__ */ new Map();
        this.#emphasis.set(span.outerRange.from, endings);
      }
      if (!endings.has(span.outerRange.to)) endings.set(span.outerRange.to, span);
    }
    this.#loras = document.loraSpans.map((span, order) => ({ span, order })).sort(
      (left, right) => left.span.outerRange.from - right.span.outerRange.from || left.order - right.order
    );
  }
  /** Resolve only a shell whose complete original endpoints match this trimmed segment. */
  emphasis(range) {
    return this.#emphasis.get(range.from)?.get(range.to);
  }
  /** Stop at the first contained original LoRA without scanning unrelated earlier source. */
  hasLora(range) {
    for (let index = this.#first(range.from); index < this.#loras.length; index++) {
      const entry = this.#loras[index];
      if (!entry || entry.span.outerRange.from > range.to) break;
      if (entry.span.outerRange.to <= range.to) return true;
    }
    return false;
  }
  /** Enumerate contained original spans in exactly the parser's order, including overlap precedence. */
  loras(range) {
    const matches = [];
    for (let index = this.#first(range.from); index < this.#loras.length; index++) {
      const entry = this.#loras[index];
      if (!entry || entry.span.outerRange.from > range.to) break;
      if (entry.span.outerRange.to <= range.to) matches.push(entry);
    }
    return matches.sort((left, right) => left.order - right.order).map((entry) => entry.span);
  }
  /** Find the first candidate source start in logarithmic time. */
  #first(position) {
    let from = 0, to = this.#loras.length;
    while (from < to) {
      const middle = from + to >>> 1;
      if ((this.#loras[middle]?.span.outerRange.from ?? Infinity) < position) from = middle + 1;
      else to = middle;
    }
    return from;
  }
};

// frontend/sugarprompt/core/src/reorder/derivation.ts
function horizontalRange(source, range) {
  let { from, to } = range;
  while (from < to && " 	".includes(requireValue(source[from], "derivation"))) from++;
  while (to > from && " 	".includes(requireValue(source[to - 1], "derivation"))) to--;
  return { from, to };
}
function splitLoras(documentScope, document, spans, chip) {
  const eligible = spans.loras(chip.contentRange);
  const children = [];
  let cursor = chip.contentRange.from;
  const textChild = (to) => {
    const range = horizontalRange(document.sourceText, { from: cursor, to });
    if (range.from !== range.to)
      children.push({
        ...chip,
        text: document.sourceText.slice(range.from, range.to),
        contentRange: range,
        leading: sourceText(documentScope, 0, 0),
        trailing: sourceText(documentScope, 0, 0)
      });
  };
  for (const span of eligible) {
    if (span.outerRange.from < cursor) continue;
    textChild(span.outerRange.from);
    children.push({
      ...chip,
      text: document.sourceText.slice(span.outerRange.from, span.outerRange.to),
      contentRange: span.outerRange,
      leading: sourceText(documentScope, 0, 0),
      trailing: sourceText(documentScope, 0, 0)
    });
    cursor = span.outerRange.to;
  }
  if (!children.length) return [chip];
  textChild(chip.contentRange.to);
  return children.map((child, index) => ({
    ...child,
    separatorRange: index + 1 < children.length ? {
      from: child.contentRange.to,
      to: requireValue(children[index + 1], "derivation").contentRange.from
    } : chip.separatorRange,
    leading: index === 0 ? concatText(
      chip.leading,
      sourceText(documentScope, chip.contentRange.from, child.contentRange.from)
    ) : sourceText(documentScope, 0, 0),
    trailing: index === children.length - 1 ? concatText(
      sourceText(documentScope, child.contentRange.to, chip.contentRange.to),
      chip.trailing
    ) : sourceText(documentScope, 0, 0)
  }));
}
function expandSegment(documentScope, document, spans, segment, partition, envelopes) {
  const source = document.sourceText;
  const range = horizontalRange(source, segment.contentRange);
  const shell = spans.emphasis(range);
  if (shell && !strip(source.slice(shell.contentRange.from, shell.contentRange.to), "right").endsWith(",") && (shell.segments.length > 1 || spans.hasLora(range))) {
    const envelope = {
      span: shell,
      opening: source.slice(shell.outerRange.from, shell.contentRange.from),
      closing: source.slice(shell.contentRange.to, shell.outerRange.to)
    };
    const children = shell.segments.flatMap(
      (child) => expandSegment(documentScope, document, spans, child, partition, [...envelopes, envelope])
    );
    return children.map((child, index) => ({
      ...child,
      leading: index === 0 ? concatText(
        sourceText(documentScope, segment.contentRange.from, range.from),
        child.leading
      ) : child.leading,
      trailing: index === children.length - 1 ? concatText(child.trailing, sourceText(documentScope, range.to, segment.contentRange.to)) : child.trailing,
      separatorRange: index === children.length - 1 ? segment.separatorRange : child.separatorRange
    }));
  }
  const base = {
    index: -1,
    partition,
    text: segment.text,
    contentRange: segment.contentRange,
    separatorRange: segment.separatorRange,
    envelopes,
    leading: sourceText(documentScope, 0, 0),
    trailing: sourceText(documentScope, 0, 0)
  };
  if (!segment.hardBreaks.length) return splitLoras(documentScope, document, spans, base);
  const lines = [];
  let cursor = segment.contentRange.from;
  const endings = [
    ...segment.hardBreaks,
    { from: segment.contentRange.to, to: segment.contentRange.to }
  ];
  for (const ending of endings) {
    const from = cursor, to = ending.from;
    if (horizontalRange(source, { from, to }).from !== to) {
      const separatorRange = ending.to > to ? ending : segment.separatorRange;
      lines.push({
        ...base,
        text: source.slice(from, to),
        contentRange: { from, to },
        separatorRange
      });
    } else if (lines.length && ending.to > to) {
      const previous = requireValue(lines.at(-1), "derivation");
      lines[lines.length - 1] = {
        ...previous,
        separatorRange: {
          from: previous.separatorRange?.from ?? previous.contentRange.to,
          to: ending.to
        }
      };
    }
    cursor = ending.to;
  }
  const lastLine = lines.at(-1);
  if (lastLine && lastLine.contentRange.to < segment.contentRange.to && horizontalRange(source, {
    from: lastLine.separatorRange?.to ?? lastLine.contentRange.to,
    to: segment.contentRange.to
  }).from === segment.contentRange.to)
    lines[lines.length - 1] = {
      ...lastLine,
      separatorRange: {
        from: lastLine.contentRange.to,
        to: segment.separatorRange?.to ?? segment.contentRange.to
      }
    };
  return lines.flatMap((line) => splitLoras(documentScope, document, spans, line));
}
function prepareDocumentValueReorders(scopes) {
  if (Object.keys(scopes.byId).length !== scopes.scopes.length) return null;
  const result = [], seen = /* @__PURE__ */ new Set();
  for (const scope of scopes.scopes) {
    if (!scope || !Object.hasOwn(scopes.byId, scope.id) || scopes.byId[scope.id] !== scope || seen.has(scope.id))
      return null;
    seen.add(scope.id);
    result.push(deriveValueReorder(scope));
  }
  return Object.freeze(result);
}
function deriveValueReorder(scope) {
  const document = scope.document;
  const spans = new ReorderDerivationSpans(document);
  const raw = [];
  for (const partition of document.regions.partitions)
    for (const segment of document.segments) {
      const from = Math.max(partition.sourceRange.from, segment.contentRange.from);
      const to = Math.min(partition.sourceRange.to, segment.contentRange.to);
      if (from >= to) continue;
      raw.push(
        ...expandSegment(
          scope,
          document,
          spans,
          {
            ...segment,
            text: document.sourceText.slice(from, to),
            contentRange: { from, to },
            separatorRange: to === segment.contentRange.to ? segment.separatorRange : null,
            hardBreaks: segment.hardBreaks.filter(
              (ending) => ending.from >= from && ending.to <= to
            )
          },
          partition.index,
          []
        )
      );
    }
  const chips = raw.map((chip, index) => ({ ...chip, index }));
  const first = chips[0], last = chips.at(-1);
  const start = first ? first.contentRange.from - first.leading.text.length - first.envelopes.reduce((n, envelope) => n + envelope.opening.length, 0) : document.sourceText.length;
  const end = last ? last.contentRange.to + last.trailing.text.length + last.envelopes.reduce((n, envelope) => n + envelope.closing.length, 0) : start;
  const slots = chips.slice(0, -1).map((chip, index) => {
    const next = requireValue(chips[index + 1], "derivation");
    if (chip.partition !== next.partition)
      return sourceText(
        scope,
        chip.contentRange.to + chip.trailing.text.length + chip.envelopes.reduce((n, envelope) => n + envelope.closing.length, 0),
        next.contentRange.from - next.leading.text.length - next.envelopes.reduce((n, envelope) => n + envelope.opening.length, 0)
      );
    return chip.separatorRange ? sourceText(scope, chip.separatorRange.from, chip.separatorRange.to) : sourceText(scope, 0, 0);
  });
  return freezeStructure({
    scope,
    chips,
    initial: {
      order: chips.map((chip) => chip.index),
      partitions: chips.map((chip) => chip.partition),
      slots,
      prefix: sourceText(scope, 0, start),
      suffix: sourceText(scope, end, document.sourceText.length)
    }
  });
}

// frontend/sugarprompt/core/src/reorder/layout.ts
function reorderBlankOffsets(text) {
  const lines = [...physicalLines(text)];
  return lines.slice(1).filter((line) => line.ending && /^[ \t]*[\r\n]/u.test(line.text)).map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0));
}
function fencedBlankOffsets(carrier, partition, side) {
  const range = carrier.scope.document.regions.separators[side === "before" ? partition : partition - 1]?.lineRange;
  if (!range) return [];
  let cursor = 0, boundary2 = null;
  for (const piece of carrier.pieces) {
    if (piece.kind === "source") {
      if (range.from >= piece.sourceRange.from && range.to <= piece.sourceRange.to) {
        boundary2 = cursor + (side === "before" ? range.from : range.to) - piece.sourceRange.from;
        break;
      }
      cursor += piece.sourceRange.to - piece.sourceRange.from;
    } else cursor += piece.text.length;
  }
  if (boundary2 === null) return [];
  return [...physicalLines(carrier.text)].slice(1).filter((line) => line.ending && /^[ \t,]*(?:\r\n|\r|\n)$/u.test(line.text)).map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0)).filter((offset) => side === "before" ? offset < boundary2 : offset >= boundary2);
}
function valueReorderVacancy(order, hidden, held) {
  const remainder = order.order.filter((chip) => chip !== held);
  if (hidden.order.length !== remainder.length || hidden.order.some((chip, at2) => chip !== remainder[at2]))
    return null;
  const row = valueReorderLayout(order).rows.find((r) => r.chips.includes(held));
  if (!row) return null;
  const layout = valueReorderLayout(hidden), at = row.chips.indexOf(held), next = row.chips[at + 1], previous = row.chips[at - 1], survivor = next ?? previous;
  if (survivor !== void 0) {
    const destination = layout.rows.find((r) => r.chips.includes(survivor));
    return destination ? {
      kind: "row",
      row: destination.index,
      insertion: destination.chips.indexOf(survivor) + (next === void 0 ? 1 : 0)
    } : null;
  }
  if (!layout.rows.some((r) => r.partition === row.partition))
    return { kind: "partition", partition: row.partition };
  const index = order.order.indexOf(held), before = order.order[index - 1], carrier = index === 0 ? "prefix" : index === order.order.length - 1 ? "suffix" : "slot", slot = before === void 0 ? -1 : hidden.order.indexOf(before), offset = index === 0 ? order.prefix.text.length : carrier === "suffix" ? hidden.suffix.text.length - order.suffix.text.length : requireValue(order.slots[index - 1], "layout").text.length, gap = layout.gaps.find(
    (g) => g.carrier === carrier && (carrier !== "slot" || g.slot === slot) && g.partition === row.partition
  ), blank = gap?.blankOffsets.indexOf(offset);
  return gap && blank !== void 0 && blank >= 0 ? { kind: "gap", gap: gap.index, blank } : null;
}
function valueReorderAdjacentTarget(hidden, origin, candidate, direction) {
  if (origin.kind === "row" || candidate.kind !== "row") return candidate;
  const row = valueReorderLayout(hidden).rows[candidate.row];
  if (!row) return null;
  const insertion = candidate.insertion + (direction > 0 && candidate.insertion === 0 ? 1 : direction < 0 && candidate.insertion === row.chips.length ? -1 : 0);
  return { kind: "row", row: row.index, insertion };
}
function valueReorderLayout(state) {
  const rows = [], gaps = [];
  let start = 0;
  for (let offset = 0; offset < state.order.length; offset++) {
    const id = requireValue(state.order[offset], "layout");
    const next = state.order[offset + 1];
    const slot = state.slots[offset]?.text ?? "";
    const partition = requireValue(state.partitions[id], "layout");
    if (next !== void 0 && state.partitions[next] === partition && !/[\r\n]/u.test(slot))
      continue;
    rows.push({
      index: rows.length,
      partition,
      start,
      chips: state.order.slice(start, offset + 1)
    });
    if (next !== void 0 && state.partitions[next] === partition && /[\r\n]/u.test(slot))
      gaps.push({
        index: gaps.length,
        partition,
        slot: offset,
        carrier: "slot",
        text: slot,
        blankOffsets: reorderBlankOffsets(slot)
      });
    else if (next !== void 0 && state.partitions[next] !== partition) {
      for (const side of ["before", "after"]) {
        const blankOffsets = fencedBlankOffsets(
          requireValue(state.slots[offset], "layout"),
          side === "before" ? partition : requireValue(state.partitions[next], "layout"),
          side
        );
        if (blankOffsets.length)
          gaps.push({
            index: gaps.length,
            partition: side === "before" ? partition : requireValue(state.partitions[next], "layout"),
            slot: offset,
            carrier: "slot",
            text: slot,
            blankOffsets
          });
      }
    }
    start = offset + 1;
  }
  for (const carrier of ["prefix", "suffix"]) {
    const text = state[carrier].text;
    if (!/[\r\n]/u.test(text) || !state.order.length) continue;
    const partition = requireValue(
      state.partitions[carrier === "prefix" ? requireValue(state.order[0], "layout") : requireValue(state.order.at(-1), "layout")],
      "layout"
    );
    const blankOffsets = /^[ \t,\r\n]*$/u.test(text) ? [...physicalLines(text, "include-eof")].filter((line) => /^[ \t,]*(?:\r\n|\r|\n)?$/u.test(line.text)).map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0)) : fencedBlankOffsets(state[carrier], partition, carrier === "prefix" ? "after" : "before");
    if (!blankOffsets.length) continue;
    gaps.push({
      index: gaps.length,
      partition,
      slot: carrier === "prefix" ? -1 : state.slots.length,
      carrier,
      text,
      blankOffsets
    });
  }
  return freezeStructure({ rows, gaps });
}
function emptyValueReorderPartitions(prepared, state) {
  const carriers = [state.prefix, ...state.slots, state.suffix].map((value) => value.text);
  const structural = carriers.join("");
  const locations = [];
  let cursor = 0;
  for (const separator of prepared.scope.document.regions.separators) {
    const source = prepared.scope.document.sourceText.slice(
      separator.lineRange.from,
      separator.lineRange.to
    );
    const from = structural.indexOf(source, cursor);
    if (from < 0) return [];
    const ending = source.match(/(?:\r\n|\r|\n)$/u)?.[0] ?? "";
    locations.push({ from, to: from + source.length, ending });
    cursor = from + source.length;
  }
  const empty3 = [];
  for (const partition of prepared.scope.document.regions.partitions) {
    if (state.order.some((chip) => state.partitions[chip] === partition.index)) continue;
    const begin = partition.index === 0 ? 0 : requireValue(locations[partition.index - 1], "layout").to;
    let offset = begin, carrier = 0;
    while (carrier < carriers.length - 1 && offset > requireValue(carriers[carrier], "layout").length)
      offset -= requireValue(carriers[carrier++], "layout").length;
    const nextHeader = locations[partition.index];
    const needsEnding = nextHeader?.from === begin;
    empty3.push({
      partition: partition.index,
      carrier: carrier === 0 ? "prefix" : carrier === carriers.length - 1 ? "suffix" : "slot",
      slot: carrier - 1,
      offset,
      ending: needsEnding ? locations[partition.index - 1]?.ending || nextHeader?.ending || "\n" : ""
    });
  }
  return freezeStructure(empty3);
}

// frontend/sugarprompt/core/src/reorder/serialization.ts
function sharedDepth(left, right) {
  let depth = 0;
  while (depth < left.length && left[depth]?.span === right[depth]?.span) depth++;
  return depth;
}
function validReorderState(prepared, state, complete = true) {
  const scope = prepared.scope, source = scope.document.sourceText, emphasis = new Set(scope.document.emphasisSpans);
  return state.slots.length === Math.max(0, state.order.length - 1) && [...state.slots, state.prefix, state.suffix].every((value) => validReorderText(scope, value)) && prepared.chips.every(
    (chip) => chip.text === source.slice(chip.contentRange.from, chip.contentRange.to) && isReorderBoundary(source, chip.contentRange.from) && isReorderBoundary(source, chip.contentRange.to) && validReorderText(scope, chip.leading) && validReorderText(scope, chip.trailing) && chip.envelopes.every(
      (envelope) => emphasis.has(envelope.span) && envelope.opening === source.slice(envelope.span.outerRange.from, envelope.span.contentRange.from) && envelope.closing === source.slice(envelope.span.contentRange.to, envelope.span.outerRange.to)
    )
  ) && state.partitions.length === prepared.chips.length && (!complete || state.order.length === prepared.chips.length) && new Set(state.order).size === state.order.length && [...state.order].every((id) => Number.isSafeInteger(id) && !!prepared.chips[id]) && [...state.partitions].every(
    (partition) => Number.isSafeInteger(partition) && partition >= 0 && partition < scope.document.regions.partitions.length
  );
}
function serializeValueReorder(prepared, state = prepared.initial) {
  return validReorderState(prepared, state) ? serializeState(prepared, state) : null;
}
function serializeReorderPreview(prepared, hidden) {
  return validReorderState(prepared, hidden, false) && hidden.order.length === prepared.chips.length - 1 ? serializeState(prepared, hidden) : null;
}
function chipParts(chip, shared, trailingShared) {
  const parts = [], leading = [...chip.leading.pieces], trailing = [...chip.trailing.pieces];
  function text(piece) {
    parts.push({
      kind: "text",
      value: piece.kind === "source" ? sourceText(chip.leading.scope, piece.sourceRange.from, piece.sourceRange.to) : derivedText(chip.leading.scope, piece.text, piece.reason)
    });
  }
  for (const envelope of chip.envelopes.slice(shared)) {
    while (leading[0] && (leading[0].kind === "derived" || leading[0].sourceRange.to <= envelope.span.outerRange.from))
      text(requireValue(leading.shift(), "serialization"));
    parts.push({ kind: "opening", envelope });
  }
  for (const piece of leading) text(piece);
  parts.push({
    kind: "content",
    value: sourceText(chip.leading.scope, chip.contentRange.from, chip.contentRange.to)
  });
  for (const envelope of chip.envelopes.slice(trailingShared).reverse()) {
    while (trailing[0] && (trailing[0].kind === "derived" || trailing[0].sourceRange.from < envelope.span.outerRange.to))
      text(requireValue(trailing.shift(), "serialization"));
    parts.push({ kind: "closing", envelope });
  }
  for (const piece of trailing) text(piece);
  return parts;
}
function serializeState(prepared, state) {
  const parts = [], pieces = [], envelopes = [];
  let cursor = 0, open = [];
  const active = [];
  const contentRanges = /* @__PURE__ */ Object.create(null);
  const renderedRanges = /* @__PURE__ */ Object.create(null);
  const slotRanges = [];
  function append(value) {
    for (const piece of value.pieces) {
      const text2 = piece.kind === "source" ? prepared.scope.document.sourceText.slice(piece.sourceRange.from, piece.sourceRange.to) : piece.text;
      const outputRange = { from: cursor, to: cursor + text2.length };
      pieces.push(
        piece.kind === "source" ? {
          kind: piece.sourceRange.from === outputRange.from && piece.sourceRange.to === outputRange.to ? "unchanged" : "copied",
          sourceRange: piece.sourceRange,
          outputRange
        } : { kind: "derived", outputRange, reason: piece.reason }
      );
      parts.push(text2);
      cursor += text2.length;
    }
  }
  function opening(envelope) {
    const from = cursor;
    append(
      sourceText(prepared.scope, envelope.span.outerRange.from, envelope.span.contentRange.from)
    );
    active.push({ envelope, from, contentFrom: cursor });
  }
  function closing(envelope) {
    const occurrence2 = active.pop();
    if (occurrence2?.envelope.span !== envelope.span) throw new Error("reorder-envelope-pair");
    const contentTo = cursor;
    append(sourceText(prepared.scope, envelope.span.contentRange.to, envelope.span.outerRange.to));
    envelopes.push({
      span: envelope.span,
      outerRange: { from: occurrence2.from, to: cursor },
      contentRange: { from: occurrence2.contentFrom, to: contentTo }
    });
  }
  append(state.prefix);
  for (const [ordinal, id] of state.order.entries()) {
    const chip = requireValue(prepared.chips[id], "serialization"), shared = sharedDepth(open, chip.envelopes);
    for (const envelope of open.slice(shared).reverse()) closing(envelope);
    const renderedFrom = cursor;
    const next = prepared.chips[state.order[ordinal + 1] ?? -1], trailingShared = sharedDepth(chip.envelopes, next?.envelopes ?? []);
    for (const part of chipParts(chip, shared, trailingShared)) {
      if (part.kind === "opening") opening(part.envelope);
      else if (part.kind === "closing") closing(part.envelope);
      else {
        const from = cursor;
        append(part.value);
        if (part.kind === "content") contentRanges[id] = { from, to: cursor };
      }
    }
    renderedRanges[id] = { from: renderedFrom, to: cursor };
    open = chip.envelopes.slice(0, trailingShared);
    const slot = state.slots[ordinal];
    if (slot !== void 0) {
      const slotFrom = cursor;
      append(slot);
      slotRanges.push({ from: slotFrom, to: cursor });
    }
  }
  append(state.suffix);
  if (active.length) throw new Error("reorder-envelope-unclosed");
  const text = parts.join("");
  const origins = atomicOriginPieces(text, pieces);
  return freezeStructure({
    text,
    contentRanges,
    renderedRanges,
    slotRanges,
    origin: { scope: prepared.scope, pieces: origins, envelopes }
  });
}

// frontend/sugarprompt/core/src/reorder/movement.ts
function mergeSlots(left, right) {
  if (!right.text.startsWith(",")) {
    if (/[\r\n]/u.test(right.text))
      return /[\r\n]/u.test(left.text) ? concatText(left, right) : right;
    if (!/[,\r\n]/u.test(left.text)) return right.text ? right : left;
    if (!/^[ \t]*$/u.test(right.text)) return null;
  }
  let suffix2 = right.text.startsWith(",") ? sliceText(right, 1) : right;
  if (/[\r\n]/u.test(left.text))
    suffix2 = sliceText(suffix2, suffix2.text.match(/^[ \t]+/u)?.[0].length ?? 0);
  if (!/[\r\n]/u.test(left.text) && suffix2.text && /^[ \t]+$/u.test(suffix2.text) && /[ \t]$/u.test(left.text))
    return left;
  return concatText(left, suffix2);
}
function hideReorderChip(prepared, state, chip) {
  if (!validReorderState(prepared, state)) return null;
  const at = state.order.indexOf(chip);
  if (at < 0) return null;
  const order = [...state.order], slots = [...state.slots];
  let prefix = state.prefix, suffix2 = state.suffix;
  order.splice(at, 1);
  if (slots.length) {
    if (at === 0) {
      if (state.partitions[requireValue(state.order[1], "movement")] !== state.partitions[chip] || /[\r\n]/u.test(requireValue(slots[0], "movement").text))
        prefix = concatText(
          prefix,
          sliceText(
            requireValue(slots[0], "movement"),
            requireValue(slots[0], "movement").text.match(/^,[ \t]*/u)?.[0].length ?? 0
          )
        );
      slots.shift();
    } else if (at === state.order.length - 1) {
      if (state.partitions[requireValue(state.order[at - 1], "movement")] !== state.partitions[chip] || /[\r\n]/u.test(requireValue(slots.at(-1), "movement").text))
        suffix2 = concatText(
          sliceText(
            requireValue(slots.at(-1), "movement"),
            requireValue(slots.at(-1), "movement").text.match(/^,[ \t]*/u)?.[0].length ?? 0
          ),
          suffix2
        );
      slots.pop();
    } else {
      const merged = mergeSlots(
        requireValue(slots[at - 1], "movement"),
        requireValue(slots[at], "movement")
      );
      if (merged === null) return null;
      slots[at - 1] = merged;
      slots.splice(at, 1);
    }
  }
  return freezeStructure({ ...state, order, slots, prefix, suffix: suffix2 });
}
function rowSeparator(state, row) {
  const slots = state.slots.slice(row.start, row.start + row.chips.length - 1);
  const comma = slots.find((slot) => slot.text.includes(",") && !/[\r\n]/u.test(slot.text));
  if (comma !== void 0) return comma;
  if (slots.some((slot) => !!slot.text))
    return derivedText(state.prefix.scope, " ", "row-separator");
  return derivedText(state.prefix.scope, slots.length ? "" : ", ", "row-separator");
}
function dropReorderChip(prepared, hidden, chip, target) {
  if (!validReorderState(prepared, hidden, false) || hidden.order.length !== prepared.chips.length - 1 || hidden.order.includes(chip) || !prepared.chips[chip])
    return null;
  const { rows, gaps } = valueReorderLayout(hidden);
  const order = [...hidden.order], slots = [...hidden.slots], partitions = [...hidden.partitions];
  let prefix = hidden.prefix, suffix2 = hidden.suffix;
  if (target.kind === "row") {
    const row = rows[target.row];
    if (!Number.isSafeInteger(target.row) || !row || !Number.isSafeInteger(target.insertion) || target.insertion < 0 || target.insertion > row.chips.length)
      return null;
    const at = row.start + target.insertion;
    order.splice(at, 0, chip);
    if (target.insertion === 0) slots.splice(at, 0, rowSeparator(hidden, row));
    else if (target.insertion === row.chips.length)
      slots.splice(at - 1, 0, rowSeparator(hidden, row));
    else {
      const original = requireValue(slots[at - 1], "movement");
      slots[at - 1] = rowSeparator(hidden, row);
      slots.splice(at, 0, original);
    }
    partitions[chip] = row.partition;
  } else if (target.kind === "gap") {
    const gap = gaps[target.gap];
    const offset = gap?.blankOffsets[target.blank];
    if (!Number.isSafeInteger(target.gap) || !Number.isSafeInteger(target.blank) || !gap || offset === void 0)
      return null;
    const carrier = gap.carrier === "prefix" ? hidden.prefix : gap.carrier === "suffix" ? hidden.suffix : requireValue(hidden.slots[gap.slot], "movement");
    if (gap.carrier === "prefix") {
      prefix = sliceText(carrier, 0, offset);
      slots.unshift(sliceText(carrier, offset));
      order.unshift(chip);
    } else if (gap.carrier === "suffix") {
      slots.push(sliceText(carrier, 0, offset));
      suffix2 = sliceText(carrier, offset);
      order.push(chip);
    } else {
      order.splice(gap.slot + 1, 0, chip);
      slots[gap.slot] = sliceText(carrier, 0, offset);
      slots.splice(
        gap.slot + 1,
        0,
        concatText(
          derivedText(prepared.scope, ",", "blank-line-comma"),
          sliceText(carrier, offset)
        )
      );
    }
    partitions[chip] = gap.partition;
  } else {
    if (!Number.isSafeInteger(target.partition)) return null;
    const destination = emptyValueReorderPartitions(prepared, hidden).find(
      (item) => item.partition === target.partition
    );
    if (!destination) return null;
    const carrier = destination.carrier === "prefix" ? prefix : destination.carrier === "suffix" ? suffix2 : requireValue(slots[destination.slot], "movement");
    const left = sliceText(carrier, 0, destination.offset), right = concatText(
      derivedText(prepared.scope, destination.ending, "empty-partition-ending"),
      sliceText(carrier, destination.offset)
    );
    if (!order.length) {
      prefix = destination.carrier === "prefix" ? left : concatText(prefix, left);
      suffix2 = destination.carrier === "prefix" ? concatText(right, suffix2) : right;
      order.push(chip);
    } else if (destination.carrier === "prefix") {
      prefix = left;
      slots.unshift(right);
      order.unshift(chip);
    } else if (destination.carrier === "suffix") {
      suffix2 = right;
      slots.push(left);
      order.push(chip);
    } else {
      slots[destination.slot] = left;
      slots.splice(destination.slot + 1, 0, right);
      order.splice(destination.slot + 1, 0, chip);
    }
    partitions[chip] = destination.partition;
  }
  const result = freezeStructure({
    ...hidden,
    order,
    slots,
    partitions,
    prefix,
    suffix: suffix2
  });
  return validReorderState(prepared, result) ? result : null;
}

// frontend/sugarprompt/core/src/reorder/selection.ts
function boundary(text, at) {
  return Number.isSafeInteger(at) && codePointBoundaries(text).includes(at) && !(at > 0 && text[at - 1] === "\r" && text[at] === "\n");
}
function captureReorderSelection(prepared, anchor, head) {
  const source = prepared.scope.document.sourceText;
  if (!boundary(source, anchor) || !boundary(source, head)) return null;
  const primary = Math.min(anchor, head);
  const boundaries = codePointBoundaries(source);
  const last = Math.max(anchor, head);
  const secondary = anchor === head ? primary : requireValue(boundaries[Math.max(0, boundaries.indexOf(last) - 1)], "selection");
  const containing = (position) => prepared.chips.find(
    (item) => item.contentRange.from <= position && position <= item.contentRange.to
  );
  const preceding = (position) => prepared.chips.findLast((item) => item.contentRange.to <= position);
  const chip = containing(primary) ?? containing(secondary) ?? preceding(primary) ?? preceding(secondary);
  if (!chip) return null;
  const offset = (position) => chip.contentRange.from <= position && position <= chip.contentRange.to ? position - chip.contentRange.from : null;
  return freezeStructure({
    chip: chip.index,
    anchorOffset: offset(anchor),
    headOffset: offset(head),
    reversed: anchor > head
  });
}
function mapReorderSelection(prepared, output, selection) {
  const chip = prepared.chips[selection.chip], range = output.contentRanges[selection.chip];
  if (!chip || !range || output.text.slice(range.from, range.to) !== chip.text) return null;
  const anchor = selection.anchorOffset === null || selection.headOffset === null ? selection.reversed ? range.to : range.from : range.from + selection.anchorOffset;
  const head = selection.anchorOffset === null || selection.headOffset === null ? selection.reversed ? range.from : range.to : range.from + selection.headOffset;
  if (anchor < range.from || anchor > range.to || head < range.from || head > range.to || !boundary(output.text, anchor) || !boundary(output.text, head))
    return null;
  return freezeStructure({ anchor, head });
}

// frontend/sugarprompt/services/src/requests/latest.ts
var LatestRequest = class {
  active;
  generation = 0;
  disposed = false;
  /** Start only the latest invocation, including reentrant submissions from host abort listeners. */
  run(work) {
    if (this.disposed) return Promise.resolve({ status: "disposed" });
    const generation = ++this.generation;
    this.retire("cancelled");
    if (this.disposed) return Promise.resolve({ status: "disposed" });
    if (generation !== this.generation) return Promise.resolve({ status: "cancelled" });
    return new Promise((resolve, reject) => {
      const active = {
        cancellation: new AbortController(),
        retire: (status) => resolve({ status })
      };
      this.active = active;
      const complete = (value) => {
        if (this.active !== active) return;
        this.active = void 0;
        resolve({ status: "ready", value });
      };
      const fail = (cause) => {
        if (this.active !== active) return;
        this.active = void 0;
        reject(cause);
      };
      try {
        void work(active.cancellation.signal).then(complete, fail);
      } catch (cause) {
        fail(cause);
      }
    });
  }
  /** Retire the current operation before invoking host cancellation callbacks. */
  cancel() {
    ++this.generation;
    this.retire("cancelled");
  }
  /** Settle owned work immediately and prohibit new external invocation after disposal. */
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    ++this.generation;
    this.retire("disposed");
  }
  /** Release the lane before aborting, so synchronous host reentrancy cannot revive the old request. */
  retire(status) {
    const active = this.active;
    this.active = void 0;
    active?.retire(status);
    active?.cancellation.abort();
  }
};

// frontend/sugarprompt/services/src/scenes/queue.ts
var authorities = /* @__PURE__ */ new WeakMap();
function validatePreparedSceneQueue(value) {
  if (value !== null && (typeof value !== "object" || !value || !authorities.has(value)))
    throw new TypeError("Invalid prepared scene queue capability");
}
function prepareSceneQueue(adapter) {
  const { revision, sourceRevision, queueableKeys, queue } = adapter;
  if (typeof revision !== "string" || !revision || revision.length > 256 || !Number.isSafeInteger(sourceRevision) || sourceRevision < 0 || !Array.isArray(queueableKeys) || queueableKeys.length > 1e4 || typeof queue !== "function")
    throw new TypeError("Invalid scene queue publication");
  let total = 0;
  const keys = Array.from(queueableKeys, (key) => {
    if (typeof key !== "string" || !key || key.length > 1e6)
      throw new TypeError("Scene queue requires normalized nonempty keys");
    total += key.length;
    if (total > 8e6) throw new RangeError("Scene queue preparation limit exceeded");
    if (normalizeSceneTitle(key) !== key)
      throw new TypeError("Scene queue requires normalized nonempty keys");
    return key;
  });
  if (new Set(keys).size !== keys.length) throw new TypeError("Duplicate scene queue key");
  const requests2 = new Map(
    keys.map((sceneKey) => [sceneKey, Object.freeze({ revision, sourceRevision, sceneKey })])
  );
  const prepared = Object.freeze({
    revision,
    sourceRevision,
    queueableKeys: Object.freeze(keys),
    /** Lookup reuses the captured request without retaining source or traversing the host catalog. */
    request: (key) => requests2.get(key) ?? null
  });
  authorities.set(prepared, {
    requests: new Set(requests2.values()),
    execute: (request2, signal) => queue.call(adapter, request2, signal)
  });
  return prepared;
}
var SceneQueueSession = class {
  lane = new LatestRequest();
  authority;
  disposed = false;
  /** A capability replacement creates a complete new host lifetime; forged capabilities reject. */
  constructor(capability) {
    validatePreparedSceneQueue(capability);
    this.authority = capability ? authorities.get(capability) : void 0;
  }
  /** Accept only requests prepared by this exact publication; current malformed replies and failures remain observable. */
  async queue(request2) {
    if (this.disposed) return { status: "disposed" };
    const authority = this.authority;
    if (!authority) return { status: "unavailable" };
    if (!authority.requests.has(request2))
      throw new TypeError("Invalid prepared scene queue request");
    const outcome = await this.lane.run((signal) => authority.execute(request2, signal));
    if (outcome.status !== "ready") return outcome;
    const value = outcome.value;
    if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).join() !== "status")
      throw new TypeError("Invalid scene queue outcome");
    const status = value.status;
    if (status !== "queued" && status !== "cancelled" && status !== "unavailable")
      throw new TypeError("Invalid scene queue outcome");
    return Object.freeze({ status });
  }
  /** Source/context retirement releases pending work without revoking the immutable host metadata. */
  cancel() {
    this.lane.cancel();
  }
  /** Release the captured host receiver and settle owned work before native teardown. */
  dispose() {
    this.disposed = true;
    this.authority = void 0;
    this.lane.dispose();
  }
};

// frontend/sugarprompt/services/src/text/substring-index.ts
function grams(value, size) {
  const characters = Array.from(value), result = [];
  for (let start = 0; start <= characters.length - size; start++)
    result.push(characters.slice(start, start + size).join(""));
  return result;
}
var ScalarSubstringIndex = class {
  postings = /* @__PURE__ */ new Map();
  items;
  /** Build once from bounded captured fields, deduplicating each item across metadata and identity aliases. */
  constructor(items, fields) {
    this.items = Object.freeze([...items]);
    const postings = /* @__PURE__ */ new Map();
    for (const item of items) {
      const keys = /* @__PURE__ */ new Set();
      for (const field of fields(item)) {
        for (const size of [1, 2, 3])
          for (const gram of grams(field, size)) keys.add(`${size}:${gram}`);
      }
      for (const key of keys) {
        const posting = postings.get(key);
        if (posting) posting.push(item);
        else postings.set(key, [item]);
      }
    }
    for (const [key, values] of postings) this.postings.set(key, Object.freeze(values));
  }
  /** Empty queries deliberately include every model; other lookups use the rarest scalar gram, with final exact matching downstream. */
  candidates(query) {
    if (!query) return this.items;
    const size = Math.min(3, Array.from(query).length);
    let selected = null;
    for (const gram of grams(query, size)) {
      const posting = this.postings.get(`${size}:${gram}`);
      if (!posting) return [];
      if (!selected || posting.length < selected.length) selected = posting;
    }
    return selected ?? [];
  }
};

// frontend/sugarprompt/services/src/text/scalar.ts
function scalarTextLength(value) {
  let length = 0;
  for (let index = 0; index < value.length; length++)
    index += requireValue(value.codePointAt(index), "scalar") > 65535 ? 2 : 1;
  return length;
}
function compareScalarText(left, right) {
  if (left === right) return 0;
  let leftIndex = 0, rightIndex = 0;
  while (leftIndex < left.length && rightIndex < right.length) {
    const a = requireValue(left.codePointAt(leftIndex), "scalar"), b = requireValue(right.codePointAt(rightIndex), "scalar"), difference = a - b;
    if (difference) return difference;
    leftIndex += a > 65535 ? 2 : 1;
    rightIndex += b > 65535 ? 2 : 1;
  }
  let remaining = 0;
  while (leftIndex < left.length) {
    leftIndex += requireValue(left.codePointAt(leftIndex), "scalar") > 65535 ? 2 : 1;
    remaining++;
  }
  while (rightIndex < right.length) {
    rightIndex += requireValue(right.codePointAt(rightIndex), "scalar") > 65535 ? 2 : 1;
    remaining--;
  }
  return remaining;
}

// frontend/sugarprompt/services/src/wildcards/catalog.ts
var noColumns = Object.freeze([]);
var noMatches = Object.freeze({
  status: "ready",
  suggestions: Object.freeze([])
});

// frontend/sugarprompt/services/src/text/ranked-prefix.ts
var RankedPrefixIndex = class {
  entries;
  minima;
  size;
  /** Capture original ranked values and build the separate lexical range index once. */
  constructor(ranked, key) {
    this.entries = ranked.map((value, rank2) => ({
      key: key(value),
      rank: rank2,
      value
    })).sort((a, b) => compareScalarText(a.key, b.key));
    this.size = 2 ** Math.ceil(Math.log2(Math.max(1, ranked.length)));
    this.minima = new Int32Array(this.size * 2).fill(-1);
    for (let index = 0; index < this.entries.length; index++)
      this.minima[this.size + index] = index;
    for (let index = this.size - 1; index > 0; index--)
      this.minima[index] = this.best(
        requireValue(this.minima[index * 2], "ranked-prefix"),
        requireValue(this.minima[index * 2 + 1], "ranked-prefix")
      );
  }
  /** Select the lower original rank, treating an empty range as absent. */
  best(left, right) {
    return left < 0 ? right : right < 0 ? left : requireValue(this.entries[left], "ranked-prefix").rank < requireValue(this.entries[right], "ranked-prefix").rank ? left : right;
  }
  /** Find the highest-ranked lexical entry in one half-open interval. */
  minimum(from, to) {
    let lower = from + this.size, upper = to + this.size, result = -1;
    while (lower < upper) {
      if (lower & 1)
        result = this.best(result, requireValue(this.minima[lower++], "ranked-prefix"));
      if (upper & 1)
        result = this.best(result, requireValue(this.minima[--upper], "ranked-prefix"));
      lower >>>= 1;
      upper >>>= 1;
    }
    return result;
  }
  /** Return the next ranked row from a bounded set of disjoint candidate intervals. */
  search(prefix, limit) {
    let lower = 0, upper = this.entries.length;
    while (lower < upper) {
      const middle = lower + upper >>> 1;
      if (compareScalarText(requireValue(this.entries[middle], "ranked-prefix").key, prefix) < 0)
        lower = middle + 1;
      else upper = middle;
    }
    const from = lower;
    upper = this.entries.length;
    while (lower < upper) {
      const middle = lower + upper >>> 1;
      if (requireValue(this.entries[middle], "ranked-prefix").key.startsWith(prefix))
        lower = middle + 1;
      else upper = middle;
    }
    const pending = [{ from, to: lower, best: this.minimum(from, lower) }];
    const result = [];
    while (result.length < limit && pending.length) {
      let chosen = -1, best = -1;
      for (let index = 0; index < pending.length; index++) {
        const candidate = requireValue(pending[index], "ranked-prefix").best;
        if (candidate >= 0 && (best < 0 || this.best(best, candidate) === candidate)) {
          chosen = index;
          best = candidate;
        }
      }
      if (chosen < 0) break;
      const interval = requireValue(pending.splice(chosen, 1)[0], "ranked-prefix");
      result.push(requireValue(this.entries[best], "ranked-prefix").value);
      if (interval.from < best)
        pending.push({
          from: interval.from,
          to: best,
          best: this.minimum(interval.from, best)
        });
      if (best + 1 < interval.to)
        pending.push({
          from: best + 1,
          to: interval.to,
          best: this.minimum(best + 1, interval.to)
        });
    }
    return Object.freeze(result);
  }
};

// frontend/sugarprompt/services/src/completion/catalog.ts
var empty = Object.freeze([]);
function record(value, keys) {
  return value !== null && typeof value === "object" && !Array.isArray(value) && Object.keys(value).sort().join() === [...keys].sort().join();
}
function tag(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 1024 && // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  !/[\r\n\u0000]/u.test(value) && normalizeTagIdentity(value).length > 0;
}
function readRows(input) {
  if (!record(input, ["revision", "bundled", "customTags", "censoredTags"]) || typeof input.revision !== "string" || input.revision.length < 1 || input.revision.length > 256 || !Array.isArray(input.bundled) || !Array.isArray(input.customTags) || !Array.isArray(input.censoredTags) || input.bundled.length + input.customTags.length + input.censoredTags.length > 25e4 || !input.customTags.every(tag) || !input.censoredTags.every(tag))
    throw new TypeError("Invalid tag catalog");
  const bundled = [];
  for (const row of input.bundled) {
    if (!record(row, ["tag", "popularity"]) || !tag(row.tag) || row.popularity !== null && (typeof row.popularity !== "number" || !Number.isSafeInteger(row.popularity) || row.popularity < 0))
      throw new TypeError("Invalid bundled tag metadata");
    bundled.push(
      Object.freeze({
        tag: strip(row.tag).replaceAll("_", " "),
        popularity: row.popularity,
        sourceLabel: null
      })
    );
  }
  bundled.sort(
    (a, b) => (b.popularity ?? 0) - (a.popularity ?? 0) || scalarTextLength(a.tag) - scalarTextLength(b.tag) || compareScalarText(caseFold(a.tag), caseFold(b.tag))
  );
  const censored = new Set(input.censoredTags.map(normalizeTagIdentity));
  const seen = /* @__PURE__ */ new Set(), ranked = [];
  const custom = input.customTags.map(
    (value) => Object.freeze({
      tag: strip(value.replaceAll("_", " ")).replace(
        new RegExp(whitespace.source + "+", "gu"),
        " "
      ),
      popularity: null,
      sourceLabel: "custom"
    })
  );
  for (const row of [...custom, ...bundled]) {
    const key = normalizeTagIdentity(row.tag);
    if (seen.has(key) || censored.has(key)) continue;
    seen.add(key);
    ranked.push(row);
  }
  return { revision: input.revision, ranked: Object.freeze(ranked) };
}
function prepareTagCompletionCatalog(input) {
  const prepared = readRows(input), index = new RankedPrefixIndex(prepared.ranked, (row) => normalizeTagIdentity(row.tag));
  const memo = /* @__PURE__ */ new Map();
  return Object.freeze({
    revision: prepared.revision,
    lexicon: prepareTagLexicon(prepared.ranked.map((row) => row.tag)),
    /** Reuse actual immutable output for normalized prefix/limit identities; the bounded memo retains at most 64 queries. */
    search(prefix, limit = 10) {
      if (typeof prefix !== "string" || !Number.isSafeInteger(limit) || limit < 0 || limit > 50)
        throw new TypeError("Invalid tag lookup");
      const key = normalizeTagIdentity(prefix);
      if (!key || !limit) return empty;
      const memoKey = JSON.stringify([key, limit]), cached = memo.get(memoKey);
      if (cached) {
        memo.delete(memoKey);
        memo.set(memoKey, cached);
        return cached;
      }
      const result = index.search(key, limit);
      memo.set(memoKey, result);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, "catalog"));
      return result;
    }
  });
}

// frontend/sugarprompt/services/src/completion/scene-catalog.ts
var empty2 = Object.freeze([]);

// frontend/sugarprompt/services/src/features/registry.ts
function definition(id, label, aliases, syntax = null, dependencies2 = [], legacyBase = false) {
  return Object.freeze({
    id,
    label,
    aliases: Object.freeze(aliases),
    syntax,
    dependencies: Object.freeze(dependencies2),
    conflicts: Object.freeze([]),
    legacyBase
  });
}
var featureDefinitions = Object.freeze([
  definition("emphasis", "Emphasis weights", ["emphasis"], "emphasis"),
  definition("danbooru_url_import", "Danbooru URL import", ["danbooru_url_import"], null, [], true),
  definition("wildcard_syntax", "Wildcard syntax", ["wildcard", "wildcard_syntax"], "wildcard"),
  definition(
    "wildcard_autocomplete",
    "Wildcard autocomplete",
    ["wildcard", "wildcard_autocomplete"],
    null,
    ["wildcard_syntax"]
  ),
  definition("autocomplete_ghost_text", "Autocomplete ghost text", [], null, [], true),
  definition("lora_syntax", "LoRA syntax", ["lora", "lora_syntax"], "lora"),
  definition("lora_autocomplete", "LoRA autocomplete", ["lora", "lora_autocomplete"], null, [
    "lora_syntax"
  ]),
  definition("lora_picker", "LoRA picker", ["lora", "lora_picker"], null, ["lora_syntax"]),
  definition("lora_trigger_words", "LoRA trigger words", ["lora", "lora_trigger_words"]),
  definition("segment_reorder", "Segment reorder", ["segment_reorder"], null, [], true),
  definition("spellcheck", "Spellcheck", ["spellcheck"], null, [], true),
  definition(
    "duplicate_segment_diagnostics",
    "Duplicate segment warnings",
    ["duplicate_segment_diagnostics"],
    null,
    [],
    true
  )
]);
function isFeatureId(value) {
  return featureDefinitions.some((entry) => entry.id === value);
}

// frontend/sugarprompt/services/src/features/profile.ts
function fieldAllows(context) {
  const field = context.field;
  if (field?.prompt_features)
    return new Set(
      field.prompt_features.flatMap((entry) => {
        const id = typeof entry === "string" ? entry.trim().toLowerCase() : entry;
        return isFeatureId(id) ? [id] : [];
      })
    );
  if (field?.prompt_syntaxes) {
    const aliases = field.prompt_syntaxes.filter((entry) => typeof entry === "string").map((entry) => entry.trim().toLowerCase());
    return new Set(
      featureDefinitions.filter(
        (entry) => entry.legacyBase || entry.aliases.some((alias) => aliases.includes(alias))
      ).map(({ id }) => id)
    );
  }
  return new Set(featureDefinitions.map(({ id }) => id));
}
function validateFeatureContext(context) {
  if (!context || typeof context !== "object" || Array.isArray(context) || Object.keys(context).some(
    (key) => !["mode", "field", "loraScheduling", "unavailableFeatures"].includes(key)
  ) || !["library", "workflow"].includes(context.mode) || "loraScheduling" in context && typeof context.loraScheduling !== "boolean" || "unavailableFeatures" in context && (!Array.isArray(context.unavailableFeatures) || Array.from(context.unavailableFeatures).some((id) => !isFeatureId(id))))
    throw new TypeError("Invalid feature context");
  if ("field" in context && (!context.field || typeof context.field !== "object" || Array.isArray(context.field) || Object.keys(context.field).some(
    (key) => !["prompt_features", "prompt_syntaxes"].includes(key)
  ) || ["prompt_features", "prompt_syntaxes"].some(
    (key) => key in requireValue(context.field, "profile") && !Array.isArray(requireValue(context.field, "profile")[key])
  )))
    throw new TypeError("Invalid field metadata");
}
function dependencies(decisions, definitions) {
  let changed = true;
  while (changed) {
    changed = false;
    for (const entry of definitions) {
      if (!decisions.get(entry.id)?.enabled) continue;
      const disabled = entry.dependencies.map((id) => decisions.get(id)).find((decision) => decision && !decision.enabled);
      if (disabled && !disabled.enabled) {
        decisions.set(entry.id, {
          feature: entry.id,
          enabled: false,
          disabledReason: disabled.disabledReason,
          detail: `Requires ${disabled.feature}.`
        });
        changed = true;
      }
    }
  }
}
function resolveFeatureProfile(preferences, context, definitions = featureDefinitions) {
  validateFeatureContext(context);
  const allowed = fieldAllows(context);
  const decisions = /* @__PURE__ */ new Map();
  for (const entry of definitions) {
    const reason = !allowed.has(entry.id) ? "field_disabled" : !preferences.allowed[entry.id] ? "user_disabled" : null;
    decisions.set(
      entry.id,
      reason ? {
        feature: entry.id,
        enabled: false,
        disabledReason: reason,
        detail: null
      } : {
        feature: entry.id,
        enabled: true,
        disabledReason: null,
        detail: null
      }
    );
  }
  dependencies(decisions, definitions);
  for (const entry of definitions) {
    const schedulingMissing = context.mode === "workflow" && !context.loraScheduling && ["lora_autocomplete", "lora_picker", "lora_trigger_words"].includes(entry.id);
    if (decisions.get(entry.id)?.enabled && (schedulingMissing || context.unavailableFeatures?.includes(entry.id)))
      decisions.set(entry.id, {
        feature: entry.id,
        enabled: false,
        disabledReason: "missing_service",
        detail: schedulingMissing ? "Prompt Control LoRA scheduling is not available for this prompt field." : "The host has not supplied this capability."
      });
  }
  dependencies(decisions, definitions);
  for (const entry of definitions) {
    if (!decisions.get(entry.id)?.enabled) continue;
    for (const conflict of entry.conflicts)
      if (decisions.get(conflict)?.enabled)
        decisions.set(conflict, {
          feature: conflict,
          enabled: false,
          disabledReason: "conflict",
          detail: `Conflicts with ${entry.id}.`
        });
  }
  dependencies(decisions, definitions);
  const ordered = definitions.map(
    ({ id }) => Object.freeze(requireValue(decisions.get(id), "profile"))
  );
  return Object.freeze({
    decisions: Object.freeze(ordered),
    rendererSyntax: Object.freeze(
      definitions.filter((entry) => decisions.get(entry.id)?.enabled && entry.syntax !== null).map((entry) => requireValue(entry.syntax, "profile")).filter((syntax, index, all) => all.indexOf(syntax) === index)
    )
  });
}
function featureEnabled(profile, id) {
  return profile.decisions.some((decision) => decision.feature === id && decision.enabled);
}

// frontend/sugarprompt/services/src/completion/query-routing.ts
function resolveCompletionQuery(indexes, profile, position, hasSelection) {
  if (featureEnabled(profile, "lora_autocomplete")) {
    const query = indexes.loraQueries.queryAt(position, hasSelection);
    if (query) return Object.freeze({ kind: "lora", query });
  }
  if (featureEnabled(profile, "wildcard_autocomplete")) {
    const query = indexes.wildcardQueries.queryAt(position, hasSelection);
    if (query) return Object.freeze({ kind: "wildcard", query });
  }
  const scene = indexes.sceneQueries?.queryAt(position, hasSelection);
  if (scene) return Object.freeze({ kind: "scene", query: scene });
  const tag2 = indexes.tagQueries.queryAt(position, hasSelection);
  return tag2 ? Object.freeze({ kind: "tag", query: tag2 }) : null;
}

// frontend/sugarprompt/services/src/completion/replacement-key.ts
function completionReplacementKey(word) {
  return strip(
    caseFold(
      tagReplacementText(word).replaceAll("\\(", "(").replaceAll("\\)", ")").replaceAll("_", " ")
    )
  );
}

// frontend/sugarprompt/services/src/completion/result.ts
function findPromptTextCompletions(catalog, triggers, source, query, limit = 10) {
  if (!Number.isSafeInteger(limit) || limit < 0 || limit > 50)
    throw new RangeError("Invalid prompt completion limit");
  const candidates = [], seen = /* @__PURE__ */ new Set();
  function add2(row, word) {
    const key = completionReplacementKey(word);
    if (seen.has(key)) return;
    seen.add(key);
    candidates.push(Object.freeze(row));
  }
  for (const value of triggers?.search(query.prefix) ?? []) {
    if (candidates.length === limit) break;
    add2({ kind: "trigger", value }, value.word);
  }
  if (candidates.length < limit)
    for (const value of catalog?.search(query.prefix, limit) ?? []) {
      if (candidates.length === limit) break;
      add2({ kind: "tag", value }, value.tag);
    }
  const result = Object.freeze({
    query,
    hadCandidates: candidates.length > 0,
    matches: Object.freeze(
      candidates.filter(
        (row) => !tagIsNoop(source, query, row.kind === "tag" ? row.value.tag : row.value.word)
      )
    )
  });
  if (result.hadCandidates || !query.fallback) return result;
  const fallback = findPromptTextCompletions(catalog, triggers, source, query.fallback, limit);
  return fallback.matches.length ? fallback : result;
}

// frontend/sugarprompt/services/src/lora/metadata-validation.ts
var validatedThumbnails = /* @__PURE__ */ new WeakSet();
function loraRecord(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("Invalid LoRA resource record");
  const actual = Object.keys(value);
  if (actual.length !== keys.length || actual.some((key) => !keys.includes(key)))
    throw new TypeError("Invalid LoRA resource record");
  return value;
}
function loraText(value, maximum, empty3 = false) {
  if (typeof value !== "string" || !empty3 && value.length === 0 || value.length > maximum)
    throw new TypeError("Invalid LoRA resource text");
  return value;
}
function parseUrl(value) {
  try {
    return new URL(value);
  } catch (cause) {
    if (cause instanceof TypeError) throw new TypeError("Invalid LoRA resource URL");
    throw cause;
  }
}
function loraUrl(value, blob) {
  const url = loraText(value, 4096), parsed = parseUrl(url);
  const target = parsed.protocol === "blob:" && blob ? parseUrl(parsed.pathname) : parsed;
  if (!blob && parsed.protocol === "blob:" || !["http:", "https:", ...blob ? ["blob:"] : []].includes(parsed.protocol) || !["http:", "https:"].includes(target.protocol) || !target.hostname || target.username || target.password)
    throw new TypeError("Invalid LoRA resource URL");
  return url;
}
function readLoraModelPageUrl(value) {
  return loraUrl(strip(loraText(value, 4096)), false);
}
function thumbnail(value) {
  if (value && typeof value === "object" && validatedThumbnails.has(value))
    return value;
  const raw = loraRecord(
    value,
    value && typeof value === "object" && "status" in value && value.status === "ready" ? ["status", "url", "width", "height"] : ["status"]
  );
  if (raw.status === "unavailable") return Object.freeze({ status: "unavailable" });
  if (raw.status !== "ready") throw new TypeError("Invalid LoRA thumbnail status");
  const url = loraUrl(raw.url, true);
  for (const size of [raw.width, raw.height])
    if (typeof size !== "number" || !Number.isSafeInteger(size) || size < 1 || size > 8192)
      throw new TypeError("Invalid LoRA thumbnail dimensions");
  const prepared = Object.freeze({
    status: "ready",
    url,
    width: raw.width,
    height: raw.height
  });
  validatedThumbnails.add(prepared);
  return prepared;
}
function readLoraModelIdentity(value) {
  const raw = loraRecord(value, ["promptName", "backendValue", "basename"]);
  const basename2 = loraText(raw.basename, 4096);
  if (/[\\/]/u.test(basename2)) throw new TypeError("Invalid LoRA basename");
  return Object.freeze({
    promptName: loraText(raw.promptName, 4096),
    backendValue: loraText(raw.backendValue, 4096),
    basename: basename2
  });
}
function readLoraItemFields(raw) {
  if (!Array.isArray(raw.triggerWords) || raw.triggerWords.length > 256)
    throw new TypeError("Invalid LoRA trigger words");
  return Object.freeze({
    model: readLoraModelIdentity(raw.model),
    title: loraText(raw.title, 256, true),
    subtitle: raw.subtitle === null ? null : loraText(raw.subtitle, 256, true),
    triggerWords: Object.freeze(raw.triggerWords.map((word) => loraText(word, 1024))),
    thumbnail: thumbnail(raw.thumbnail),
    modelPageUrl: raw.modelPageUrl === null ? null : readLoraModelPageUrl(raw.modelPageUrl)
  });
}
var loraItemKeys = Object.freeze([
  "model",
  "title",
  "subtitle",
  "triggerWords",
  "thumbnail",
  "modelPageUrl"
]);

// frontend/sugarprompt/services/src/lora/scheduled-context.ts
var preparedContexts = /* @__PURE__ */ new WeakSet();
function assertPreparedLoraSchedulingContext(context) {
  if (!preparedContexts.has(context)) throw new TypeError("Unprepared LoRA scheduling context");
}

// frontend/sugarprompt/services/src/lora/names.ts
function withoutLoraExtension(value) {
  const folded = caseFold(value);
  for (const suffix2 of [".safetensors", ".ckpt", ".pt"])
    if (folded.endsWith(suffix2)) return value.slice(0, -suffix2.length);
  return value;
}
function loraNameKey(value, extensionless = false) {
  return caseFold((extensionless ? withoutLoraExtension(value) : value).replaceAll("\\", "/"));
}

// frontend/sugarprompt/services/src/lora/trigger-index.ts
function lookupKey(text) {
  return caseFold(text.replaceAll("_", " "));
}
function prepareLoraTriggerIndex(models) {
  const rows = [];
  let units = 0;
  for (const model of models)
    for (const entry of model.triggerWords) {
      units += entry.length;
      if (units > 8e6) throw new TypeError("LoRA trigger index exceeds preparation budget");
      for (const part of entry.split(",")) {
        const word = strip(part);
        if (word) {
          if (rows.length === 1e5)
            throw new TypeError("LoRA trigger index exceeds preparation budget");
          rows.push({
            key: lookupKey(word),
            replacementKey: completionReplacementKey(word),
            rank: rows.length,
            suggestion: Object.freeze({
              word,
              display: model.display,
              backendValue: model.model.backendValue
            })
          });
        }
      }
    }
  rows.sort((a, b) => compareScalarText(a.key, b.key) || a.rank - b.rank);
  const results = /* @__PURE__ */ new Map(), empty3 = Object.freeze([]);
  let retainedReferences = 0;
  return Object.freeze({
    /** Binary prefix bounds inspect only matching prepared rows; the last 32 normalized queries retain actual frozen output. */
    search(prefix) {
      if (typeof prefix !== "string" || prefix.length > 4096)
        throw new TypeError("Invalid LoRA trigger prefix");
      const key = lookupKey(prefix);
      if (!key) return empty3;
      const cached = results.get(key);
      if (cached) {
        results.delete(key);
        results.set(key, cached);
        return cached;
      }
      let lower = 0, upper = rows.length;
      while (lower < upper) {
        const middle = lower + upper >>> 1;
        if (compareScalarText(requireValue(rows[middle], "trigger-index").key, key) < 0)
          lower = middle + 1;
        else upper = middle;
      }
      const from = lower;
      upper = rows.length;
      while (lower < upper) {
        const middle = lower + upper >>> 1;
        if (requireValue(rows[middle], "trigger-index").key.startsWith(key)) lower = middle + 1;
        else upper = middle;
      }
      const seen = /* @__PURE__ */ new Set(), matches = rows.slice(from, lower).sort((a, b) => a.rank - b.rank).filter((row) => {
        if (seen.has(row.replacementKey)) return false;
        seen.add(row.replacementKey);
        return true;
      }).map((row) => row.suggestion);
      const result = matches.length ? Object.freeze(matches) : empty3;
      if (result.length <= 8192) {
        while (results.size === 32 || retainedReferences + result.length > 8192) {
          const oldest = requireValue(results.keys().next().value, "trigger-index");
          retainedReferences -= requireValue(results.get(oldest), "trigger-index").length;
          results.delete(oldest);
        }
        results.set(key, result);
        retainedReferences += result.length;
      }
      return result;
    }
  });
}

// frontend/sugarprompt/services/src/lora/validation.ts
var validatedResources = /* @__PURE__ */ new WeakSet();
function readLoraResources(value, names) {
  if (!Array.isArray(value) || value.length !== names.length)
    throw new TypeError("Invalid LoRA resource count");
  const expected = new Set(names), results = /* @__PURE__ */ new Map();
  for (const item of value) {
    const resolution = item && typeof item === "object" && "resolution" in item && item.resolution;
    const raw = loraRecord(
      item,
      resolution === "found" ? ["name", "resolution", ...loraItemKeys] : resolution === "ambiguous" ? ["name", "resolution", "candidateCount"] : ["name", "resolution"]
    );
    if (typeof raw.name !== "string") throw new TypeError("Invalid LoRA resource identity");
    const name = raw.name;
    if (!expected.has(name) || results.has(name))
      throw new TypeError("Invalid LoRA resource identity");
    if (validatedResources.has(item)) {
      results.set(name, item);
      continue;
    }
    if (raw.resolution === "found") {
      results.set(
        name,
        Object.freeze({
          name,
          resolution: "found",
          ...readLoraItemFields(raw)
        })
      );
    } else if (raw.resolution === "ambiguous") {
      const count = raw.candidateCount;
      if (count !== null && (typeof count !== "number" || !Number.isSafeInteger(count) || count < 2))
        throw new TypeError("Invalid LoRA ambiguity count");
      results.set(name, Object.freeze({ name, resolution: "ambiguous", candidateCount: count }));
    } else if (raw.resolution === "missing" || raw.resolution === "pending-authority" || raw.resolution === "unavailable")
      results.set(name, Object.freeze({ name, resolution: raw.resolution }));
    else throw new TypeError("Invalid LoRA resource resolution");
  }
  return Object.freeze(
    names.map((name) => {
      const resource = requireValue(results.get(name), "validation");
      validatedResources.add(resource);
      return resource;
    })
  );
}

// frontend/sugarprompt/services/src/lora/scheduled.ts
function scheduled(item, source) {
  const title = strip(item.title) || strip(item.model.basename), subtitle = item.subtitle === null ? "" : strip(item.subtitle);
  return Object.freeze({
    model: item.model,
    display: title && subtitle ? `${title} - ${subtitle}` : title || subtitle,
    triggerWords: item.triggerWords,
    source,
    insertionText: item.triggerWords.map((word) => strip(word)).filter(Boolean).join(", ")
  });
}
function prepareEffectiveLoras(names, resources, catalogRevision, context) {
  const prepared = prepareEffectiveLoraModels(names, resources, catalogRevision, context);
  return prepared.status === "ready" ? Object.freeze({
    ...prepared,
    triggers: prepareLoraTriggerIndex(prepared.models)
  }) : prepared;
}
function prepareEffectiveLoraModels(names, resources, catalogRevision, context) {
  if (context === null) return Object.freeze({ status: "unavailable", reason: "missing-context" });
  assertPreparedLoraSchedulingContext(context);
  if (!context.supported)
    return Object.freeze({
      status: "unavailable",
      reason: "unsupported-scheduling"
    });
  if (catalogRevision !== context.catalogRevision)
    return Object.freeze({ status: "unavailable", reason: "stale-catalog" });
  if (resources.status !== "ready")
    return Object.freeze({
      status: "unavailable",
      reason: "resources-unavailable"
    });
  const resolved = readLoraResources(resources.resources, names), models = [], seen = /* @__PURE__ */ new Set();
  function add2(item, source) {
    const key = loraNameKey(
      strip(item.model.backendValue) ? item.model.backendValue : item.model.promptName
    );
    if (seen.has(key)) return;
    seen.add(key);
    models.push(scheduled(item, source));
  }
  for (const item of resolved) if (item.resolution === "found") add2(item, "inline");
  for (const item of context.upstream) add2(item, "upstream");
  for (const item of context.fields) add2(item, "field");
  const captured = Object.freeze(models);
  return Object.freeze({
    status: "ready",
    context,
    models: captured
  });
}

// frontend/sugarprompt/services/src/lora/resource-names.ts
function loraResourceNames(document) {
  return Object.freeze([
    ...new Set(
      document.loraSpans.map(
        (span) => document.sourceText.slice(span.nameRange.from, span.nameRange.to)
      )
    )
  ]);
}

// frontend/sugarprompt/services/src/lora/scheduled-session.ts
var EffectiveLoraSession = class {
  previous = null;
  disposed = false;
  /** Capture current input and retain actual output only when every ordered name, validated descriptor and host identity matches. */
  prepare(document, resources, revision, context) {
    return this.prepareNames(loraResourceNames(document), resources, revision, context);
  }
  /** Reuse this same finite memo for parser-prepared scene names, with exact scoped resource coverage. */
  prepareNames(names, resources, revision, context) {
    if (this.disposed) throw new Error("Effective LoRA session disposed");
    if (!context || !context.supported || context.catalogRevision !== revision || resources.status !== "ready") {
      this.previous = null;
      return prepareEffectiveLoras(names, resources, revision, context);
    }
    const captured = readLoraResources(resources.resources, names);
    const previous = this.previous;
    if (previous && previous.revision === revision && previous.context === context && names.length === previous.names.length && names.every((name, index) => name === previous.names[index]) && captured.every((item, index) => item === previous.resources[index]))
      return previous.snapshot;
    const snapshot = prepareEffectiveLoras(
      names,
      { status: "ready", resources: captured },
      revision,
      context
    );
    this.previous = snapshot.status === "ready" ? { revision, context, names, resources: captured, snapshot } : null;
    return snapshot;
  }
  /** Release captured metadata and actual lookup output when the native preparation owner is destroyed. */
  dispose() {
    this.disposed = true;
    this.previous = null;
  }
};

// frontend/sugarprompt/services/src/features/preferences.ts
var defaultPreferences = Object.freeze({
  allowed: Object.freeze(
    Object.fromEntries(featureDefinitions.map(({ id }) => [id, true]))
  ),
  wheelMode: "hover_dwell",
  resolveOnGeneration: true
});

// frontend/sugarprompt/services/src/lora/resource-validity.ts
function isCatalogStableLoraResource(resource) {
  return resource.resolution === "found" || resource.resolution === "missing" || resource.resolution === "ambiguous";
}

// frontend/sugarprompt/services/src/lora/session.ts
var LoraResourceSession = class {
  lane = new LatestRequest();
  cache = /* @__PURE__ */ new Map();
  adapter;
  revision;
  observe;
  capacity;
  current;
  epoch = 0;
  disposed = false;
  /** Bind one immutable catalog and finite observer; capacity bounds process memoization, never persisted user data. */
  constructor(adapter, observe = () => {
  }, capacity = 64) {
    if (!Number.isSafeInteger(capacity) || capacity < 1 || capacity > 1024)
      throw new RangeError("Invalid LoRA cache capacity");
    if (adapter && (typeof adapter.revision !== "string" || !adapter.revision.length || adapter.revision.length > 256))
      throw new TypeError("Invalid LoRA catalog revision");
    this.adapter = adapter;
    this.revision = adapter?.revision;
    this.observe = observe;
    this.capacity = capacity;
    this.current = Object.freeze({
      status: adapter ? "idle" : "unavailable",
      resources: Object.freeze([])
    });
  }
  /** Expose immutable external preparation, without retaining an editable source document. */
  snapshot() {
    return this.current;
  }
  /** Prepare unique exact names from parsed source, reuse actual validated records and supersede older host work. */
  async warmup(document) {
    return this.warmupNames(loraResourceNames(document));
  }
  /** Resolve exact parser-prepared names across the source and its effective scenes through the same finite catalog lifetime. */
  async warmupNames(names) {
    if (names.some((name) => typeof name !== "string") || new Set(names).size !== names.length)
      throw new TypeError("Invalid LoRA resource names");
    names = Object.freeze([...names]);
    if (this.disposed) return { status: "disposed" };
    if (!this.adapter) return { status: "unavailable" };
    if (this.adapter.revision !== this.revision)
      throw new Error("LoRA catalog mutated without replacement");
    const epoch = ++this.epoch;
    this.lane.cancel();
    if (this.disposed) return { status: "disposed" };
    if (epoch !== this.epoch) return { status: "cancelled" };
    const reused = /* @__PURE__ */ new Map();
    const retained = new Map(
      this.current.resources.filter(isCatalogStableLoraResource).map((resource) => [resource.name, resource])
    );
    for (const name of names) {
      const cached = this.cache.get(name);
      const value = cached ?? retained.get(name);
      if (value) {
        if (cached) {
          this.cache.delete(name);
          this.cache.set(name, cached);
        }
        reused.set(name, value);
      }
    }
    const missing = Object.freeze(names.filter((name) => !reused.has(name)));
    try {
      if (missing.length) {
        this.publish("loading", [...reused.values()]);
        if (this.disposed) return { status: "disposed" };
        if (epoch !== this.epoch) return { status: "cancelled" };
        const result = await this.lane.run(
          (signal) => requireValue(this.adapter, "session").resolve(missing, signal)
        );
        if (result.status !== "ready") return result;
        if (this.disposed) return { status: "disposed" };
        if (epoch !== this.epoch) return { status: "cancelled" };
        if (this.adapter.revision !== this.revision)
          throw new Error("LoRA catalog mutated during request");
        for (const value of readLoraResources(result.value, missing)) {
          reused.set(value.name, value);
          if (isCatalogStableLoraResource(value)) {
            this.cache.set(value.name, value);
            while (this.cache.size > this.capacity)
              this.cache.delete(requireValue(this.cache.keys().next().value, "session"));
          }
        }
      }
      const resources = Object.freeze(
        names.map((name) => requireValue(reused.get(name), "session"))
      );
      this.publish("ready", resources);
      if (this.disposed) return { status: "disposed" };
      if (epoch !== this.epoch) return { status: "cancelled" };
      return { status: "ready", resources };
    } catch (cause) {
      if (this.disposed) return { status: "disposed" };
      if (epoch !== this.epoch) return { status: "cancelled" };
      this.publish("failed", [...reused.values()]);
      throw cause;
    }
  }
  /** Settle retired source/profile work while retaining published, catalog-valid records; replacing the session retires catalog authority. */
  invalidate() {
    if (this.disposed) return;
    const epoch = ++this.epoch;
    this.lane.cancel();
    if (this.disposed || epoch !== this.epoch) return;
    this.publish(
      this.adapter ? "idle" : "unavailable",
      this.current.resources.filter(isCatalogStableLoraResource)
    );
  }
  /** Release cached metadata, settle ignored-abort work and prohibit publication after teardown. */
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    ++this.epoch;
    this.lane.dispose();
    this.cache.clear();
    this.current = Object.freeze({
      status: "disposed",
      resources: Object.freeze([])
    });
  }
  /** Only this finite owner publishes immutable external records; no source is included in lifecycle diagnostics. */
  publish(status, resources) {
    if (this.disposed) return;
    if (this.current.status === status && this.current.resources.length === resources.length && resources.every((value, index) => this.current.resources[index] === value))
      return;
    this.current = Object.freeze({
      status,
      resources: Object.freeze(resources)
    });
    this.observe(this.current);
  }
};

// frontend/sugarprompt/services/src/lora/scene-scopes.ts
function prepareSceneLoraScopes(document, contexts, profile = { style: "curly" }) {
  if (document.sourceText !== contexts.document.sourceText)
    throw new Error("Scene LoRA source mismatch");
  const sourceNames = loraResourceNames(document), all = new Set(sourceNames), names = /* @__PURE__ */ new Map(), memo = /* @__PURE__ */ new Map();
  for (const context of [contexts.universal, ...contexts.scenes]) {
    const text = context.effectivePromptText;
    let prepared = memo.get(text);
    if (!prepared) {
      prepared = text === document.sourceText ? sourceNames : loraResourceNames(parsePrompt(text, profile));
      memo.set(text, prepared);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, "scene-scopes"));
    }
    names.set(context, prepared);
    for (const name of prepared) all.add(name);
  }
  const resourceNames = all.size === sourceNames.length ? sourceNames : Object.freeze([...all]);
  return Object.freeze({
    contexts,
    resourceNames,
    entries: Object.freeze(
      [contexts.universal, ...contexts.scenes].map(
        (context) => Object.freeze({ names: requireValue(names.get(context), "scene-scopes") })
      )
    ),
    /** This lookup performs no parsing or metadata traversal. */
    namesAt(position) {
      assertCompletionBoundary(document.sourceText, position);
      return requireValue(names.get(contexts.at(position)), "scene-scopes");
    }
  });
}
function prepareDocumentLoraScopes(scopes, profile = { style: "curly" }, previous) {
  if (JSON.stringify(wildcardDelimiters(profile)) !== scopes.profileKey)
    throw new Error("document-lora-profile-mismatch");
  if (previous?.documentScopes === scopes) return previous;
  const prompt = scopes.values.kind === "prompt" ? scopes.scopes[0] : null;
  const scenes = prompt?.sceneContexts ? prepareSceneLoraScopes(prompt.document, prompt.sceneContexts, profile) : null;
  const names = new Map(scopes.scopes.map((scope) => [scope, loraResourceNames(scope.document)]));
  const entries = scenes?.entries ?? Object.freeze(
    scopes.scopes.map(
      (scope) => Object.freeze({ names: requireValue(names.get(scope), "scene-scopes") })
    )
  );
  const resourceNames = scenes?.resourceNames ?? Object.freeze([...new Set(entries.flatMap((entry) => entry.names))]);
  return Object.freeze({
    documentScopes: scopes,
    entries,
    resourceNames,
    /** Canonical admission excludes storage and unsafe exact endpoints without parsing on lookup. */
    namesAt(position) {
      const admitted = documentPromptScopeAtSource(
        scopes,
        scopes.values.source,
        scopes.values.kind,
        { from: position, to: position }
      );
      if (admitted.status !== "ready") return null;
      return scenes ? scenes.namesAt(position) : requireValue(names.get(admitted.scope), "scene-scopes");
    }
  });
}

// frontend/sugarprompt/services/src/lora/trigger-actions.ts
function prepareLoraTriggerActions(snapshot) {
  return new Map(
    snapshot.status === "ready" ? snapshot.models.filter((model) => model.insertionText).map((model) => [loraNameKey(model.model.backendValue), model]) : []
  );
}

// frontend/sugarprompt/services/src/lora/scene-schedules.ts
var SceneLoraSession = class {
  cache = /* @__PURE__ */ new Map();
  disposed = false;
  /** Preserve the public prompt adapter while delegating actual preparation to the shared finite owner. */
  prepare(scopes, resources, revision, context) {
    const prepared = this.prepareEntries(scopes, resources, revision, context);
    return Object.freeze({
      /** Prompt namesAt validates boundaries; an absent admitted prompt entry is an invariant violation. */
      at(position) {
        const schedule = prepared.at(position);
        if (!schedule) throw new Error("missing-prompt-lora-schedule");
        return schedule;
      },
      canInsertAt: prepared.canInsertAt
    });
  }
  /** Prepare every admitted canonical value/scene entry under this same session and metadata lifetime. */
  prepareDocument(scopes, resources, revision, context) {
    return this.prepareEntries(scopes, resources, revision, context);
  }
  /** Eagerly build each distinct ordered-name result; hot facades perform only scope and actual-object lookups. */
  prepareEntries(scopes, resources, revision, context) {
    if (this.disposed) throw new Error("Scene LoRA session disposed");
    const status = resources.status;
    const pool = new Map(
      (status === "ready" ? readLoraResources(resources.resources, scopes.resourceNames) : []).map(
        (item) => [item.name, item]
      )
    );
    const results = /* @__PURE__ */ new Map();
    const distinct = /* @__PURE__ */ new Map();
    for (const { names } of scopes.entries) {
      const key = JSON.stringify(names);
      let result2 = distinct.get(key);
      if (!result2) {
        let retained = this.cache.get(key);
        if (!retained) retained = { session: new EffectiveLoraSession(), result: null };
        const snapshot = retained.session.prepareNames(
          names,
          {
            status,
            resources: status === "ready" ? names.map((name) => requireValue(pool.get(name), "scene-schedules")) : []
          },
          revision,
          context
        );
        result2 = retained.result?.snapshot === snapshot ? retained.result : Object.freeze({
          snapshot,
          insertions: prepareLoraTriggerActions(snapshot)
        });
        retained.result = result2;
        this.cache.delete(key);
        this.cache.set(key, retained);
        if (this.cache.size > 64) {
          const oldest = requireValue(this.cache.keys().next().value, "scene-schedules");
          requireValue(this.cache.get(oldest), "scene-schedules").session.dispose();
          this.cache.delete(oldest);
        }
        distinct.set(key, result2);
      }
      results.set(names, result2);
    }
    const at = (position) => {
      if (this.disposed) throw new Error("Scene LoRA session disposed");
      const names = scopes.namesAt(position);
      if (names === null) return null;
      const result2 = results.get(names);
      if (!result2) throw new Error("missing-prepared-lora-schedule");
      return result2;
    };
    const result = Object.freeze({
      at,
      /** Trigger indexes are actual snapshot-derived preparation output, never rebuilt during input. */
      canInsertAt(position, backendKey) {
        const schedule = at(position);
        return schedule !== null && (backendKey === void 0 ? schedule.insertions.size > 0 : schedule.insertions.has(backendKey));
      }
    });
    return result;
  }
  /** Dispose each retained effective session and reject all issued facades, including already evicted entries. */
  dispose() {
    this.disposed = true;
    for (const entry of this.cache.values()) entry.session.dispose();
    this.cache.clear();
  }
};

// frontend/sugarprompt/services/src/lora/model-page.ts
var preparedRequests = /* @__PURE__ */ new WeakSet();
function prepareLoraPage(resource) {
  if (resource.resolution !== "found" || resource.modelPageUrl === null) return null;
  const url = readLoraModelPageUrl(resource.modelPageUrl), host = new URL(url).hostname.toLowerCase();
  const request2 = Object.freeze({
    model: readLoraModelIdentity(resource.model),
    url,
    label: host === "civitai.com" || host.endsWith(".civitai.com") ? "Go to CivitAI page" : "Open model page"
  });
  preparedRequests.add(request2);
  return request2;
}
var LoraPageSession = class {
  lane = new LatestRequest();
  adapter;
  disposed = false;
  /** Adapter replacement creates a new complete external capability lifetime. */
  constructor(adapter) {
    this.adapter = adapter;
  }
  /** New actions supersede old host work, and unknown replies cannot masquerade as success. */
  async open(request2) {
    if (this.disposed) return { status: "disposed" };
    if (!preparedRequests.has(request2)) throw new TypeError("Invalid prepared LoRA page request");
    const adapter = this.adapter;
    if (!adapter) return { status: "unavailable" };
    const outcome = await this.lane.run((signal) => adapter.open(request2, signal));
    if (outcome.status !== "ready") return outcome;
    const raw = loraRecord(outcome.value, ["status"]);
    if (raw.status !== "opened" && raw.status !== "cancelled" && raw.status !== "unavailable")
      throw new TypeError("Invalid LoRA page outcome");
    return Object.freeze({ status: raw.status });
  }
  /** Source/context/resource retirement settles pending work even when the host ignores abort. */
  cancel() {
    this.lane.cancel();
  }
  /** Release host references and owned pending work before the native view disappears. */
  dispose() {
    this.disposed = true;
    this.adapter = void 0;
    this.lane.dispose();
  }
};

// frontend/sugarprompt/services/src/lora/metadata-capability.ts
var capabilities = /* @__PURE__ */ new WeakSet();
function isPreparedLoraMetadata(value) {
  return typeof value === "object" && value !== null && capabilities.has(value);
}
function validatePreparedLoraMetadata(value) {
  if (value !== void 0 && !isPreparedLoraMetadata(value))
    throw new TypeError("Invalid prepared metadata capability");
}

// frontend/sugarprompt/services/src/wildcards/diagnostics.ts
function prepareWildcardDiagnostic(span, resolution) {
  if (resolution.status !== "missing") return null;
  const message = span.form === "csv" && resolution.reason === "column" ? `Missing CSV wildcard column: ${span.identifier}:${span.csvColumn}` : `Missing wildcard: ${span.identifier}`;
  return Object.freeze({
    kind: "wildcard",
    severity: "error",
    range: span.outerRange,
    message,
    payload: Object.freeze({
      form: span.form,
      identifier: span.identifier,
      csvColumn: span.csvColumn,
      matchedCsvColumn: null,
      availableColumns: resolution.availableColumns
    })
  });
}

// frontend/sugarprompt/services/src/lora/catalog-validation.ts
function readLoraCatalog(value) {
  const raw = loraRecord(value, ["revision", "authoritative", "items"]);
  if (typeof raw.authoritative !== "boolean" || !Array.isArray(raw.items) || raw.items.length > 1e5)
    throw new TypeError("Invalid LoRA catalog");
  const physical = /* @__PURE__ */ new Set();
  const thumbnails = /* @__PURE__ */ new Map();
  const items = raw.items.map((item) => {
    const prepared = readLoraItemFields(loraRecord(item, loraItemKeys));
    if (physical.has(prepared.model.backendValue))
      throw new TypeError("Duplicate LoRA backend identity");
    physical.add(prepared.model.backendValue);
    if (prepared.thumbnail.status === "ready") {
      const image = prepared.thumbnail;
      const key = JSON.stringify([image.url, image.width, image.height]);
      const existing = thumbnails.get(key);
      if (existing) return Object.freeze({ ...prepared, thumbnail: existing });
      thumbnails.set(key, image);
    }
    return prepared;
  });
  return Object.freeze({
    revision: loraText(raw.revision, 256),
    authoritative: raw.authoritative,
    items: Object.freeze(items)
  });
}

// frontend/sugarprompt/services/src/lora/catalog-index.ts
function basename(value) {
  return value.replaceAll("\\", "/").split("/").filter((part) => part && part !== ".").at(-1) ?? "";
}
function backendQuery(value) {
  const name = basename(value), dot = name.lastIndexOf(".");
  return loraNameKey(dot > 0 && dot < name.length - 1 ? value : value + ".safetensors");
}
function add(index, value, item) {
  let bucket = index.get(value);
  if (!bucket) {
    bucket = /* @__PURE__ */ new Set();
    index.set(value, bucket);
  }
  bucket.add(item);
}
function match(items) {
  if (!items?.size) return null;
  return items.size === 1 ? { status: "found", item: requireValue(items.values().next().value, "catalog-index") } : { status: "ambiguous", candidateCount: items.size };
}
var LoraCatalogIndex = class {
  prompt = /* @__PURE__ */ new Map();
  backend = /* @__PURE__ */ new Map();
  backendPrompt = /* @__PURE__ */ new Map();
  aliases = /* @__PURE__ */ new Map();
  collisions = /* @__PURE__ */ new Map();
  suffixes = {
    children: /* @__PURE__ */ new Map(),
    items: /* @__PURE__ */ new Set()
  };
  /** Build only from validated immutable metadata; no filesystem, network, media decoding or source document is retained. */
  constructor(items) {
    for (const item of items) {
      const model = item.model;
      add(this.prompt, loraNameKey(model.promptName, true), item);
      add(this.backend, loraNameKey(model.backendValue), item);
      add(this.backendPrompt, loraNameKey(model.backendValue, true), item);
      add(this.collisions, loraNameKey(model.basename), item);
      for (const alias of [
        model.promptName,
        withoutLoraExtension(model.backendValue),
        item.title,
        model.basename
      ])
        if (alias) add(this.aliases, loraNameKey(alias), item);
      for (const path of [model.promptName, model.backendValue]) this.indexSuffixes(path, item);
    }
  }
  /** Keep donor lookup priority for definite matches; an explicit installed backend filename disambiguates a colliding canonical stem. */
  find(name) {
    const normalized = loraNameKey(name, true);
    const canonical = match(this.prompt.get(normalized));
    if (canonical?.status === "ambiguous" && withoutLoraExtension(name) !== name) {
      const filename = match(this.backend.get(loraNameKey(name)));
      if (filename?.status === "found") return filename;
    }
    const exact = canonical ?? match(this.backend.get(backendQuery(name))) ?? match(this.backendPrompt.get(normalized));
    if (exact) return exact;
    const alias = match(this.aliases.get(normalized));
    if (alias) return alias;
    const parts = normalized.split("/");
    let suffix2 = this.suffixes;
    if (parts.some((part) => !part)) suffix2 = void 0;
    else for (const part of parts.reverse()) suffix2 = suffix2?.children.get(part);
    const path = match(suffix2?.items);
    if (path) return path;
    const bare = loraNameKey(basename(name), true);
    return match(this.aliases.get(bare)) ?? match(this.collisions.get(bare)) ?? { status: "missing" };
  }
  /** Store each component once along its reverse path; all proper suffixes receive the same immutable model identity. */
  indexSuffixes(path, item) {
    const parts = loraNameKey(path, true).split("/").filter(Boolean).reverse();
    let node = this.suffixes;
    for (const [depth, part] of parts.entries()) {
      let child = node.children.get(part);
      if (!child) {
        child = { children: /* @__PURE__ */ new Map(), items: /* @__PURE__ */ new Set() };
        node.children.set(part, child);
      }
      node = child;
      if (depth < parts.length - 1) node.items.add(item);
    }
  }
};

// frontend/sugarprompt/services/src/lora/completion-ranking.ts
function rankedLoraItem(item, metadata, scheduleName) {
  const display = item.title || item.model.basename;
  return Object.freeze({
    empty: Object.freeze({
      item,
      scheduleName,
      score: 900,
      matchKind: "empty",
      display
    }),
    basename: loraNameKey(item.model.basename),
    display: loraNameKey(item.title),
    name: loraNameKey(item.model.promptName),
    backend: loraNameKey(item.model.backendValue, true),
    metadata: loraNameKey(metadata),
    displaySort: caseFold(display),
    pathSort: caseFold(item.model.backendValue)
  });
}
function scalarBoundary(text, position) {
  const before = text.charCodeAt(position - 1), after = text.charCodeAt(position);
  return !(before >= 55296 && before <= 56319 && after >= 56320 && after <= 57343);
}
function contains(text, query, prefix = false) {
  for (let position = text.indexOf(query); position >= 0; position = text.indexOf(query, position + 1)) {
    if (prefix && position !== 0) return false;
    if (scalarBoundary(text, position) && scalarBoundary(text, position + query.length))
      return true;
  }
  return false;
}
function rankLora(item, query) {
  if (!query) return item.empty;
  const result = (score, matchKind) => Object.freeze({ ...item.empty, score, matchKind });
  if ([item.name, item.backend, item.display, item.basename].includes(query))
    return result(0, "exact");
  if (contains(item.basename, query, true)) return result(100, "basename_prefix");
  if (contains(item.display, query, true)) return result(110, "display_prefix");
  if (query.includes("/") && contains(item.name, query, true)) return result(120, "path_prefix");
  if (contains(item.basename, query)) return result(300, "basename_substring");
  if (contains(item.display, query)) return result(310, "display_substring");
  if (contains(item.name, query)) return result(330, "path_substring");
  return contains(item.metadata, query) ? result(700, "metadata") : null;
}
function compareRankedLoras(left, right) {
  return compareScalarText(left.displaySort, right.displaySort) || compareScalarText(left.pathSort, right.pathSort);
}

// frontend/sugarprompt/services/src/lora/completion-catalog.ts
function searchMetadata(input, catalog) {
  const raw = loraRecord(input, ["searchMetadata"]);
  if (!Array.isArray(raw.searchMetadata) || raw.searchMetadata.length > catalog.items.length)
    throw new TypeError("Invalid LoRA completion metadata");
  const identities = new Set(catalog.items.map((item) => item.model.backendValue)), result = /* @__PURE__ */ new Map();
  for (const value of raw.searchMetadata) {
    const record2 = loraRecord(value, ["backendValue", "text"]), identity = loraText(record2.backendValue, 4096), text = loraText(record2.text, 16384, true);
    if (!identities.has(identity) || result.has(identity))
      throw new TypeError("Unknown or duplicate LoRA completion identity");
    result.set(identity, text);
  }
  return result;
}
function schedulingName(item, index) {
  loraScheduleText(item.model.promptName);
  for (const name of [item.model.promptName, item.model.backendValue]) {
    const match2 = index.find(name);
    if (match2.status === "found" && match2.item === item) {
      loraScheduleText(name);
      return name;
    }
  }
  throw new TypeError("LoRA completion identity is not source-representable");
}
function prepareLoraCompletion(catalog, options, resolution) {
  const metadata = searchMetadata(options, catalog), items = [];
  let total = 0;
  for (const item of catalog.items) {
    const ranked = rankedLoraItem(
      item,
      metadata.get(item.model.backendValue) ?? [item.title, item.subtitle ?? "", ...item.triggerWords].join(" "),
      schedulingName(item, resolution)
    );
    total += ranked.basename.length + ranked.display.length + ranked.name.length + ranked.backend.length + ranked.metadata.length;
    if (total > 8e6) throw new TypeError("LoRA completion catalog exceeds prepared text bound");
    items.push(ranked);
  }
  Object.freeze(items);
  const index = new ScalarSubstringIndex(items, (item) => [
    item.basename,
    item.display,
    item.name,
    item.backend,
    item.metadata
  ]), ordered = Object.freeze([...items].sort(compareRankedLoras).map((item) => item.empty)), memo = /* @__PURE__ */ new Map();
  let retained = 0;
  return Object.freeze({
    revision: catalog.revision,
    authoritative: catalog.authoritative,
    /** Lookup returns every match, with at most 64 queries/8192 candidate references retained for actual output reuse. */
    search(query) {
      if (typeof query !== "string" || query.length > 4096)
        throw new TypeError("Invalid LoRA completion query");
      const key = loraNameKey(query);
      if (!key) return ordered;
      const cached = memo.get(key);
      if (cached) {
        memo.delete(key);
        memo.set(key, cached);
        return cached;
      }
      const ranked = index.candidates(key).flatMap((item) => {
        const result2 = rankLora(item, key);
        return result2 ? [{ item, result: result2 }] : [];
      });
      ranked.sort((a, b) => a.result.score - b.result.score || compareRankedLoras(a.item, b.item));
      const result = Object.freeze(ranked.map((row) => row.result));
      if (result.length <= 8192) {
        memo.set(key, result);
        retained += result.length;
        while (memo.size > 64 || retained > 8192) {
          const oldest = requireValue(memo.keys().next().value, "completion-catalog");
          retained -= requireValue(memo.get(oldest), "completion-catalog").length;
          memo.delete(oldest);
        }
      }
      return result;
    }
  });
}

// frontend/sugarprompt/services/src/lora/catalog-adapter.ts
function createLoraCatalogAdapter(input, completion) {
  const catalog = readLoraCatalog(input), index = new LoraCatalogIndex(catalog.items);
  return Object.freeze({
    revision: catalog.revision,
    ...completion === void 0 ? {} : {
      completionCatalog: prepareLoraCompletion(catalog, completion, index)
    },
    /** Resolve current names from prepared indexes; cancelled requests have no external work or publication side effects. */
    async resolve(names, signal) {
      if (!Array.isArray(names) || names.some((name) => typeof name !== "string" || !name.length) || new Set(names).size !== names.length)
        throw new TypeError("Invalid LoRA lookup names");
      return Object.freeze(
        names.map((name) => {
          if (signal.aborted) return Object.freeze({ name, resolution: "unavailable" });
          if (!catalog.authoritative)
            return Object.freeze({ name, resolution: "pending-authority" });
          const result = index.find(name);
          if (result.status === "found")
            return Object.freeze({ name, resolution: "found", ...result.item });
          return result.status === "ambiguous" ? Object.freeze({
            name,
            resolution: "ambiguous",
            candidateCount: result.candidateCount
          }) : Object.freeze({ name, resolution: "missing" });
        })
      );
    }
  });
}

// frontend/sugarprompt/services/src/lora/thumbnail-menu.ts
function sources(choices, leaf) {
  return [...new Set(choices.map((value) => value.sourceKey))].map((key) => {
    const group = choices.filter((value) => value.sourceKey === key), label = requireValue(group[0], "thumbnail-menu").sourceLabel || "Output";
    return group.length === 1 ? leaf(label, requireValue(group[0], "thumbnail-menu")) : {
      kind: "submenu",
      label,
      children: group.map((value, index) => leaf(`Image ${index + 1}`, value))
    };
  });
}
function batches(choices, leaf) {
  const indices = [...new Set(choices.map((value) => value.setIndex))].sort((a, b) => a - b);
  if (indices.length <= 1) return sources(choices, leaf);
  return indices.map((index) => {
    const group = choices.filter((value) => value.setIndex === index), label = `Batch ${index}`;
    return group.length === 1 ? leaf(label, requireValue(group[0], "thumbnail-menu")) : { kind: "submenu", label, children: sources(group, leaf) };
  });
}
function prepareThumbnailMenu(choices, activeImageId, leaf) {
  const label = "Set thumbnail from canvas";
  if (!choices.length) return { kind: "disabled", label };
  if (choices.length === 1) return leaf(label, requireValue(choices[0], "thumbnail-menu"));
  const keys = new Set(choices.map((value) => value.sceneKey).filter(Boolean));
  const meaningful = keys.size > 1 || keys.size > 0 && choices.some((value) => value.sceneTitle && value.sceneTitle !== "Scene");
  const sorted = meaningful ? [...choices].sort(
    (a, b) => a.sceneOrder - b.sceneOrder || compareScalarText(a.sceneTitle, b.sceneTitle) || compareScalarText(a.sceneKey, b.sceneKey)
  ) : choices;
  const hierarchy = meaningful ? [...new Set(sorted.map((value) => value.sceneKey))].map((key) => sorted.filter((value) => value.sceneKey === key)).map((group) => {
    const sceneLabel = requireValue(group[0], "thumbnail-menu").sceneTitle || "Scene";
    return group.length === 1 ? leaf(sceneLabel, requireValue(group[0], "thumbnail-menu")) : {
      kind: "submenu",
      label: sceneLabel,
      children: batches(group, leaf)
    };
  }) : batches(choices, leaf);
  const active = choices.find((value) => value.imageId === activeImageId);
  return {
    kind: "submenu",
    label,
    children: active ? [leaf("Current image", active), { kind: "separator" }, ...hierarchy] : hierarchy
  };
}

// frontend/sugarprompt/services/src/lora/metadata-actions.ts
var requests = /* @__PURE__ */ new WeakMap();
function freezeMenu(item) {
  return item.kind === "submenu" ? Object.freeze({
    ...item,
    children: Object.freeze(item.children.map(freezeMenu))
  }) : Object.freeze(item);
}
function prepareLoraMetadataMenu(model, catalogRevision, capability) {
  if (!isPreparedLoraMetadata(capability))
    throw new TypeError("Invalid prepared metadata capability");
  const canonical = readLoraModelIdentity(model), revision = loraText(catalogRevision, 256);
  function action(label, intent) {
    const request2 = Object.freeze({
      model: canonical,
      catalogRevision: revision,
      ...intent
    });
    requests.set(request2, capability);
    return Object.freeze({ kind: "action", label, request: request2 });
  }
  const items = [];
  if (capability.refresh) items.push(action("Refresh CivitAI metadata", { action: "refresh" }));
  if (capability.thumbnail)
    items.push(
      prepareThumbnailMenu(
        capability.choices,
        capability.activeImageId,
        (label, image) => action(label, { action: "thumbnail", imageId: image.imageId })
      )
    );
  return Object.freeze(items.map(freezeMenu));
}
var LoraMetadataSession = class {
  lane = new LatestRequest();
  capability;
  disposed = false;
  /** The native capability publication already owns copied immutable host choices and method identity. */
  constructor(capability) {
    validatePreparedLoraMetadata(capability);
    this.capability = capability;
  }
  /** Retired/foreign/disabled requests cannot invoke a host; current unknown replies and failures remain observable. */
  async execute(request2) {
    if (this.disposed) return { status: "disposed" };
    const capability = this.capability;
    if (!capability) return { status: "unavailable" };
    if (requests.get(request2) !== capability)
      throw new TypeError("Invalid prepared metadata request");
    const result = await this.lane.run((signal) => capability.execute(request2, signal));
    if (result.status !== "ready") return result;
    const value = result.value;
    const raw = loraRecord(
      value,
      value && typeof value === "object" && "status" in value && value.status === "updated" ? ["status", "catalog", "completion"] : ["status"]
    );
    if (raw.status === "cancelled" || raw.status === "unavailable") return { status: raw.status };
    if (raw.status !== "updated") throw new TypeError("Invalid metadata action outcome");
    const catalog = readLoraCatalog(raw.catalog);
    const model = catalog.items.find(
      (item) => item.model.backendValue === request2.model.backendValue
    )?.model;
    if (catalog.revision === request2.catalogRevision || !catalog.authoritative || !model || model.promptName !== request2.model.promptName || model.basename !== request2.model.basename)
      throw new TypeError("Invalid metadata replacement identity");
    const adapter = createLoraCatalogAdapter(
      catalog,
      raw.completion === null ? void 0 : raw.completion
    );
    return { status: "updated", adapter };
  }
  /** Retire stale source/profile/catalog work and settle hosts ignoring abort. */
  cancel() {
    this.lane.cancel();
  }
  /** Release the captured host method and settle owned work before native teardown. */
  dispose() {
    this.disposed = true;
    this.capability = void 0;
    this.lane.dispose();
  }
};

// frontend/sugarprompt/services/src/document-diagnostics.ts
function unsupportedSceneDiagnostics(semantics) {
  return Object.freeze(
    semantics.unsupportedSceneMarkerRanges.map(
      (range) => Object.freeze({
        kind: "unsupported-scene-marker",
        severity: "error",
        range,
        message: "Scenes aren\u2019t supported in wildcard values."
      })
    )
  );
}

// frontend/sugarprompt/services/src/diagnostics/duplicates.ts
function sourceRange(scope, range) {
  const mapped = decodedRangeToSource(scope.mapping, range);
  if (!mapped) throw new Error("unmapped-duplicate-diagnostic");
  return mapped;
}
function prepareDuplicateDiagnostics(scopes, context, previous) {
  if (previous?.scopes === scopes && previous.context.key === context.key && previous.context.mode === context.mode)
    return previous;
  const groups = [];
  const findings = [];
  for (const scope of scopes.scopes) {
    const document = scope.document;
    const scenes = scope.sceneContexts;
    const prepared = prepareDuplicateSegments(
      document,
      scopes.values.kind !== "prompt" ? { kind: "value" } : scenes?.scenes.length && !document.regions.separators.length ? { kind: "scenes", contexts: scenes } : {
        kind: "conditioning",
        topology: new ConditioningTopology(context.mode, document.regions)
      }
    );
    const group = Object.freeze({ scope, prepared });
    groups.push(group);
    for (const finding of prepared.findings) {
      findings.push(
        Object.freeze({
          kind: "duplicate-segment",
          severity: "error",
          group,
          finding,
          range: sourceRange(scope, finding.duplicate.range),
          firstRange: sourceRange(scope, finding.first.range),
          message: `Duplicate prompt segment: ${finding.duplicate.key}`
        })
      );
    }
  }
  return Object.freeze({
    scopes,
    context: Object.freeze({ key: context.key, mode: context.mode }),
    groups: Object.freeze(groups),
    findings: Object.freeze(findings)
  });
}

// frontend/sugarprompt/services/src/regions/context.ts
var unresolvedConditioning = Object.freeze({
  key: null,
  mode: "unresolved"
});
function captureConditioningContext(value) {
  if (value === null || value === void 0) return unresolvedConditioning;
  if (!value || typeof value !== "object" || Object.keys(value).sort().join() !== "key,mode" || !("key" in value) || typeof value.key !== "string" || !value.key || !("mode" in value) || value.mode !== "unresolved" && value.mode !== "independent" && value.mode !== "regional")
    throw new TypeError("Invalid conditioning context");
  return Object.freeze({ key: value.key, mode: value.mode });
}

// frontend/sugarprompt/services/src/regions/names.ts
function captureRegionNames(value) {
  if (!Array.isArray(value)) throw new TypeError("Invalid shared region names");
  const seen = /* @__PURE__ */ new Set();
  return Object.freeze(
    value.map((item) => {
      if (!item || typeof item !== "object" || Object.keys(item).sort().join() !== "name,ordinal" || !("ordinal" in item) || typeof item.ordinal !== "number" || !Number.isSafeInteger(item.ordinal) || item.ordinal < 0 || seen.has(item.ordinal) || !("name" in item) || typeof item.name !== "string" || !validRegionName(item.name))
        throw new TypeError("Invalid shared region identity");
      seen.add(item.ordinal);
      return Object.freeze({ ordinal: item.ordinal, name: strip(item.name) });
    })
  );
}

// frontend/sugarprompt/services/src/clipboard/session.ts
function readResult(value) {
  if (typeof value !== "object" || value === null || Array.isArray(value) || !("status" in value))
    throw new TypeError("Invalid clipboard read result");
  if (value.status === "read" && "value" in value) {
    const transfer = value.value;
    if (typeof transfer === "object" && transfer !== null && !Array.isArray(transfer) && "kind" in transfer && (transfer.kind === "exact" || transfer.kind === "plain") && "text" in transfer && typeof transfer.text === "string")
      return {
        status: "read",
        value: { kind: transfer.kind, text: transfer.text }
      };
  } else if (value.status === "empty" || value.status === "denied" || value.status === "unavailable")
    return { status: value.status };
  throw new TypeError("Invalid clipboard read result");
}
function writeResult(value) {
  if (typeof value === "object" && value !== null && !Array.isArray(value) && "status" in value && (value.status === "written" || value.status === "denied" || value.status === "unavailable"))
    return { status: value.status };
  throw new TypeError("Invalid clipboard write result");
}
var ClipboardSession = class {
  lane = new LatestRequest();
  gateway;
  /** The native adapter owns actual browser capabilities; this owner validates its publication. */
  constructor(gateway) {
    this.gateway = gateway;
  }
  /** Supersede any earlier clipboard work and publish only a current validated transfer. */
  async read(nativeTransfer) {
    const outcome = await this.lane.run(
      async (signal) => this.gateway.readable ? readResult(await this.gateway.read(signal, nativeTransfer)) : { status: "unavailable" }
    );
    return outcome.status === "ready" ? outcome.value : outcome;
  }
  /** Preserve the deliberate captured text while preventing late completion from authorizing a source edit. */
  async write(text) {
    const outcome = await this.lane.run(
      async (signal) => this.gateway.writable ? writeResult(await this.gateway.write(text, signal)) : { status: "unavailable" }
    );
    return outcome.status === "ready" ? outcome.value : outcome;
  }
  /** Settle current work and release publication authority on source, selection or focus retirement. */
  cancel() {
    this.lane.cancel();
  }
  /** Settle ignored-abort work before native editor teardown. */
  dispose() {
    this.lane.dispose();
  }
};

// frontend/sugarprompt/core/src/diagnostics/spellcheck-candidates.ts
var nondecimalDigits = [
  [178, 179],
  [185, 185],
  [4969, 4977],
  [6618, 6618],
  [8304, 8304],
  [8308, 8313],
  [8320, 8329],
  [9312, 9320],
  [9332, 9340],
  [9352, 9360],
  [9450, 9450],
  [9461, 9469],
  [9471, 9471],
  [10102, 10110],
  [10112, 10120],
  [10122, 10130],
  [68160, 68163],
  [69216, 69224],
  [69714, 69722],
  [127232, 127242]
];
function containsDigit(text) {
  for (const character of text) {
    const point = requireValue(character.codePointAt(0), "spellcheck-candidates");
    if (decimalZeros.some((zero) => point >= zero && point < zero + 10) || nondecimalDigits.some(([from, to]) => point >= from && point <= to))
      return true;
  }
  return false;
}
function skippedToken(token) {
  const text = strip(token);
  return !text || /[_/\\<>:]/u.test(text) || text.startsWith("#") || text.startsWith("**") || /^(?:#|0x)?[0-9a-fA-F]{8,}$/u.test(text) || /^[A-Za-z0-9_-]+\.[A-Za-z0-9]{1,8}$/u.test(text) || /[a-z][A-Z]|[A-Z]{2,}[a-z]/u.test(text) || containsDigit(text);
}
function tokenAt(source, from, to) {
  while (from > 0 && source[from - 1] !== "," && !whitespace.test(requireValue(source[from - 1], "spellcheck-candidates")))
    from--;
  while (to < source.length && source[to] !== "," && !whitespace.test(requireValue(source[to], "spellcheck-candidates")))
    to++;
  return { from, to };
}
function prepareSpellcheckCandidates(scope, lexicon, previous) {
  if (previous?.scope === scope && previous.lexicon === (lexicon ?? null)) return previous;
  const document = scope.document, source = document.sourceText;
  const decisions = /* @__PURE__ */ new Map();
  const known = (text) => {
    if (!lexicon) return false;
    const key = normalizeTagIdentity(text);
    if (!key) return false;
    const cached = decisions.get(key);
    if (cached !== void 0) return cached;
    const value = lexicon.contains(text);
    decisions.set(key, value);
    return value;
  };
  const segmentSkipped = /* @__PURE__ */ new Map();
  const candidates = [];
  let syntaxIndex = 0, segmentIndex = 0;
  let tokenRange = { from: 0, to: 0 }, tokenSkipped = false;
  for (const match2 of source.matchAll(/[A-Za-z][A-Za-z']*/gu)) {
    const from = match2.index, to = from + match2[0].length;
    while (document.syntaxSpans[syntaxIndex] && requireValue(document.syntaxSpans[syntaxIndex], "spellcheck-candidates").sourceRange.to <= from)
      syntaxIndex++;
    const syntax = document.syntaxSpans[syntaxIndex];
    if (syntax && syntax.sourceRange.from <= from && to <= syntax.sourceRange.to) continue;
    if (from >= tokenRange.to) {
      tokenRange = tokenAt(source, from, to);
      tokenSkipped = skippedToken(source.slice(tokenRange.from, tokenRange.to));
    }
    if (tokenSkipped || known(match2[0])) continue;
    while (document.segments[segmentIndex] && requireValue(document.segments[segmentIndex], "spellcheck-candidates").contentRange.to < from)
      segmentIndex++;
    const segment = document.segments[segmentIndex];
    if (segment && segment.contentRange.from <= from) {
      let skipped = segmentSkipped.get(segmentIndex);
      if (skipped === void 0) {
        const text = source.slice(segment.contentRange.from, segment.contentRange.to);
        skipped = skippedToken(text) || known(text);
        segmentSkipped.set(segmentIndex, skipped);
      }
      if (skipped) continue;
    }
    candidates.push({ range: { from, to }, text: match2[0] });
  }
  return freezeStructure({ scope, lexicon: lexicon ?? null, candidates });
}

// frontend/sugarprompt/services/src/diagnostics/spelling-preparation.ts
var preparations = /* @__PURE__ */ new WeakSet();
function prepareSpellingCandidates(scopes, lexicon = null, previous) {
  const reusable = previous && preparations.has(previous) ? previous : void 0;
  if (reusable?.scopes === scopes && reusable.lexicon === lexicon) return reusable;
  const groups = new Map(reusable?.groups.map((group) => [group.scope, group]));
  const prepared = Object.freeze({
    scopes,
    lexicon,
    groups: Object.freeze(
      scopes.scopes.map(
        (scope) => prepareSpellcheckCandidates(scope, lexicon ?? void 0, groups.get(scope))
      )
    )
  });
  preparations.add(prepared);
  return prepared;
}
function isPreparedSpellingCandidates(prepared) {
  return preparations.has(prepared);
}

// frontend/sugarprompt/services/src/diagnostics/spelling-session.ts
var sessions = /* @__PURE__ */ new WeakSet();
function isSpellcheckSession(value) {
  return value instanceof SpellcheckSession && sessions.has(value);
}
function unavailableCode(value) {
  if (value !== "backend-unavailable" && value !== "dictionary-unavailable" && value !== "unsupported")
    throw new TypeError("spelling-unavailable-code");
  return value;
}
function lookup(value) {
  if (!value || typeof value !== "object") throw new TypeError("spelling-lookup");
  const status = value.status;
  if (status === "ready") return { status, value: value.value };
  if (status === "unavailable") return { status, code: unavailableCode(value.code) };
  if (status === "cancelled") return { status };
  throw new TypeError("spelling-lookup-status");
}
function cells(value, maximum) {
  if (!Array.isArray(value)) throw new TypeError("spelling-payload-array");
  const length = value.length;
  if (!Number.isSafeInteger(length) || length < 0 || length > maximum)
    throw new TypeError("spelling-payload-length");
  const result = [];
  for (let index = 0; index < length; index++) {
    if (!Object.hasOwn(value, index)) throw new TypeError("spelling-payload-hole");
    result.push(value[index]);
  }
  return Object.freeze(result);
}
function suggestion(value) {
  if (typeof value !== "string" || !value || value.length > 4096 || /[\uD800-\uDFFF]/u.test(value))
    throw new TypeError("spelling-suggestion");
  return value;
}
var SpellcheckSession = class {
  identity = Symbol();
  gatewayIdentity;
  languageTag;
  gateway;
  ignored = /* @__PURE__ */ new Set();
  active = /* @__PURE__ */ new Set();
  subscribers = /* @__PURE__ */ new Set();
  snapshots = /* @__PURE__ */ new WeakSet();
  currentRevision = 0;
  disposed = false;
  /** Capture adapter identity and methods before callers can mutate its public descriptor. */
  constructor(gateway) {
    const identity = gateway.identity, languageTag = gateway.languageTag, availability = gateway.availability, checkWords = gateway.checkWords, suggest = gateway.suggest, add2 = gateway.addToDictionary, ignore = gateway.ignoreForSession;
    if (typeof identity !== "symbol" || typeof languageTag !== "string" || !languageTag || languageTag.length > 128 || typeof availability !== "function" || typeof checkWords !== "function" || typeof suggest !== "function" || add2 !== void 0 && typeof add2 !== "function" || ignore !== void 0 && typeof ignore !== "function")
      throw new TypeError("spelling-gateway");
    this.gatewayIdentity = identity;
    this.languageTag = languageTag;
    this.gateway = Object.freeze({
      identity,
      languageTag,
      availability: availability.bind(gateway),
      checkWords: checkWords.bind(gateway),
      suggest: suggest.bind(gateway),
      ...add2 ? { addToDictionary: add2.bind(gateway) } : {},
      ...ignore ? { ignoreForSession: ignore.bind(gateway) } : {}
    });
    sessions.add(this);
  }
  /** Monotonic dictionary/ignore revision is observed rather than mirrored by editor consumers. */
  get revision() {
    return this.currentRevision;
  }
  /** External property reads may retire or replace the session before any later adaptation or side effect. */
  retirement(signal, revision) {
    if (this.disposed) return { status: "disposed" };
    return signal.aborted || revision !== this.currentRevision ? { status: "cancelled" } : null;
  }
  /** One mounted consumer owns the returned release function; disposed sessions retain no subscribers. */
  subscribe(listener) {
    if (this.disposed) return () => {
    };
    this.subscribers.add(listener);
    return () => {
      this.subscribers.delete(listener);
    };
  }
  /** Capture all capability cells once and reject a promised operation without its actual method. */
  availability() {
    const value = this.gateway.availability(), status = value.status;
    if (status === "unavailable") return { status, code: unavailableCode(value.code) };
    if (status !== "available") throw new TypeError("spelling-availability");
    const dictionaryAdd = value.dictionaryAdd, sessionIgnore = value.sessionIgnore;
    if (typeof dictionaryAdd !== "boolean" || typeof sessionIgnore !== "boolean" || dictionaryAdd && !this.gateway.addToDictionary || sessionIgnore && !this.gateway.ignoreForSession)
      throw new TypeError("spelling-capability");
    return Object.freeze({ status, dictionaryAdd, sessionIgnore });
  }
  /** Retire earlier work before notifying consumers; one failing observer does not prevent other retirement notifications. */
  changed() {
    this.currentRevision++;
    for (const operation of [...this.active]) operation.cancel();
    const causes = [];
    for (const listener of [...this.subscribers]) {
      try {
        listener();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, "spelling-session-observers");
  }
  /** Each concurrent request retains a separate finite lane; payload capture is followed by an abort/revision publication fence. */
  async run(signal, revision, work, adapt) {
    if (this.disposed) return { status: "disposed" };
    if (signal.aborted || revision !== this.currentRevision) return { status: "cancelled" };
    const lane = new LatestRequest();
    this.active.add(lane);
    const abort = () => lane.cancel();
    signal.addEventListener("abort", abort, { once: true });
    try {
      if (signal.aborted) lane.cancel();
      const outcome = await lane.run(async (ownedSignal) => {
        if (signal.aborted || revision !== this.currentRevision)
          return { status: "cancelled" };
        const captured = lookup(await work(ownedSignal));
        if (captured.status !== "ready") return captured;
        const value = adapt(captured.value);
        if (ownedSignal.aborted || signal.aborted || this.disposed || revision !== this.currentRevision)
          return { status: "cancelled" };
        return { status: "ready", value };
      });
      if (this.disposed) return { status: "disposed" };
      if (outcome.status !== "ready") return { status: "cancelled" };
      if (signal.aborted || revision !== this.currentRevision) return { status: "cancelled" };
      return outcome.value;
    } finally {
      signal.removeEventListener("abort", abort);
      this.active.delete(lane);
      lane.dispose();
    }
  }
  /** Project unique dictionary decisions onto actual canonical candidates; expected absence never becomes an empty ready snapshot. */
  async check(prepared, signal) {
    if (this.disposed) return { status: "disposed" };
    if (!isPreparedSpellingCandidates(prepared)) return { status: "rejected" };
    const revision = this.currentRevision, availability = this.availability();
    const retired = this.retirement(signal, revision);
    if (retired) return retired;
    if (availability.status === "unavailable") return availability;
    const words = Object.freeze(
      [
        ...new Set(
          prepared.groups.flatMap(
            (group) => group.candidates.map((candidate) => caseFold(candidate.text))
          )
        )
      ].filter((word) => !this.ignored.has(word))
    );
    return this.run(
      signal,
      revision,
      (owned) => words.length ? this.gateway.checkWords(words, owned) : Promise.resolve({ status: "ready", value: [] }),
      (payload) => {
        const answers = cells(payload, words.length);
        if (answers.length !== words.length || answers.some((value) => typeof value !== "boolean"))
          throw new TypeError("spelling-check-payload");
        const rejected = new Set(words.filter((_, index) => !answers[index]));
        const issues = [];
        for (const group of prepared.groups)
          for (const candidate of group.candidates) {
            if (!rejected.has(caseFold(candidate.text))) continue;
            const range = decodedRangeToSource(group.scope.mapping, candidate.range);
            if (!range) throw new Error("spelling-range-unmapped");
            issues.push(
              Object.freeze({
                kind: "spelling",
                group,
                candidate,
                range: Object.freeze(range)
              })
            );
          }
        const snapshot = Object.freeze({
          prepared,
          gateway: this.gatewayIdentity,
          languageTag: this.languageTag,
          revision,
          dictionaryAdd: availability.dictionaryAdd,
          issues: Object.freeze(issues)
        });
        this.snapshots.add(snapshot);
        return snapshot;
      }
    );
  }
  /** Only actual current issue membership can request suggestions or dictionary operations. */
  current(snapshot, issue) {
    return !this.disposed && this.snapshots.has(snapshot) && snapshot.revision === this.currentRevision && snapshot.issues.includes(issue);
  }
  /** Lazy suggestions retain literal provider order and the actual issue; no shared suggestion cache. */
  async suggestions(snapshot, issue, signal) {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? "disposed" : "rejected" };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    if (availability.status === "unavailable") return availability;
    return this.run(
      signal,
      snapshot.revision,
      (owned) => this.gateway.suggest(issue.candidate.text, 8, owned),
      (payload) => Object.freeze({
        snapshot,
        issue,
        suggestions: Object.freeze(cells(payload, 8).map(suggestion))
      })
    );
  }
  /** Word-wide local suppression is accepted before optional external delegation; backend absence cannot undo it. */
  async ignore(snapshot, issue, signal) {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? "disposed" : "rejected" };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    this.ignored.add(caseFold(issue.candidate.text));
    const revision = this.currentRevision + 1;
    this.changed();
    const changedRetirement = this.retirement(signal, revision);
    if (changedRetirement) return changedRetirement;
    if (availability.status === "unavailable" || !availability.sessionIgnore)
      return {
        status: "ready",
        value: Object.freeze({
          revision,
          delegation: availability.status === "unavailable" ? "unavailable" : "unsupported"
        })
      };
    const delegated = await this.run(
      signal,
      revision,
      (owned) => requireValue(this.gateway.ignoreForSession, "spelling-session")(
        issue.candidate.text,
        owned
      ),
      (payload) => {
        if (payload !== true) throw new TypeError("spelling-ignore-payload");
        return Object.freeze({ revision, delegation: "accepted" });
      }
    );
    const delegationRetirement = this.retirement(signal, revision);
    if (delegationRetirement) return delegationRetirement;
    return delegated.status === "unavailable" ? {
      status: "ready",
      value: Object.freeze({ revision, delegation: "unavailable" })
    } : delegated;
  }
  /** Only successful supported persistence suppresses the word; failed adds never change native source/history. */
  async add(snapshot, issue, signal) {
    if (!this.current(snapshot, issue)) return { status: this.disposed ? "disposed" : "rejected" };
    const availability = this.availability();
    const retired = this.retirement(signal, snapshot.revision);
    if (retired) return retired;
    if (availability.status === "unavailable") return availability;
    if (!availability.dictionaryAdd) return { status: "unavailable", code: "unsupported" };
    const outcome = await this.run(
      signal,
      snapshot.revision,
      (owned) => requireValue(this.gateway.addToDictionary, "spelling-session")(issue.candidate.text, owned),
      (payload) => {
        if (typeof payload !== "boolean") throw new TypeError("spelling-add-payload");
        return payload;
      }
    );
    const publicationRetirement = this.retirement(signal, snapshot.revision);
    if (publicationRetirement) return publicationRetirement;
    if (!this.current(snapshot, issue)) return { status: "rejected" };
    if (outcome.status === "ready" && outcome.value) {
      this.ignored.add(caseFold(issue.candidate.text));
      this.changed();
    }
    return outcome;
  }
  /** Host session teardown retires every consumer operation and releases all retained ignore/subscriber state. */
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const lane of [...this.active]) lane.dispose();
    this.active.clear();
    this.subscribers.clear();
    this.ignored.clear();
  }
};

// frontend/sugarprompt/services/src/saved-segments/capture.ts
function captureRecord(value, keys) {
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("invalid-saved-segment-record");
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== null && prototype !== Object.prototype)
    throw new TypeError("invalid-saved-segment-record");
  const names = Reflect.ownKeys(value);
  if (names.length !== keys.length || names.some((key) => typeof key !== "string" || !keys.includes(key)))
    throw new TypeError("invalid-saved-segment-record");
  const result = /* @__PURE__ */ Object.create(null);
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !("value" in descriptor))
      throw new TypeError("invalid-saved-segment-record");
    result[key] = descriptor.value;
  }
  return Object.freeze(result);
}
function captureArray(value, limit) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype)
    throw new TypeError("invalid-saved-segment-array");
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
  const length = lengthDescriptor?.value;
  if (typeof length !== "number" || !Number.isSafeInteger(length) || length < 0 || length > limit)
    throw new TypeError("invalid-saved-segment-array");
  if (Reflect.ownKeys(value).length !== length + 1)
    throw new TypeError("invalid-saved-segment-array");
  const result = [];
  for (let index = 0; index < length; index++) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor || !("value" in descriptor)) throw new TypeError("invalid-saved-segment-array");
    result.push(descriptor.value);
  }
  return Object.freeze(result);
}
function captureText(value, limit) {
  if (typeof value !== "string" || value.length > limit || !strip(value))
    throw new TypeError("invalid-saved-segment-text");
  return value;
}
function captureReply(value, allowed) {
  if (value === null || typeof value !== "object")
    throw new TypeError("invalid-saved-segment-reply");
  const descriptor = Object.getOwnPropertyDescriptor(value, "status");
  const status = descriptor && "value" in descriptor ? descriptor.value : void 0;
  if (typeof status !== "string" || !allowed.includes(status))
    throw new TypeError("invalid-saved-segment-reply");
  const result = captureRecord(value, status === "ready" ? ["status", "segments"] : ["status"]);
  if (result.status !== status) throw new TypeError("invalid-saved-segment-reply");
  return result;
}

// frontend/sugarprompt/services/src/saved-segments/preparation.ts
function sameTarget(left, right) {
  return left.kind === right.kind && left.provider === right.provider && left.key === right.key;
}
function rank(scope) {
  return scope.kind === "global" ? 0 : scope.kind === "family" ? 1 : 2;
}
function captureSavedSegmentScope(value) {
  const item = captureRecord(value, ["kind", "provider", "key", "label"]);
  const kind = item.kind;
  if (kind !== "global" && kind !== "family" && kind !== "provider-model" && kind !== "model-version" && kind !== "local-model")
    throw new TypeError("invalid-saved-segment-scope");
  const key = captureText(item.key, 4096), label = captureText(item.label, 2048);
  if (kind === "global") {
    if (item.provider !== null || key !== "global")
      throw new TypeError("invalid-saved-segment-scope");
    return Object.freeze({ kind, provider: null, key, label });
  }
  const provider = captureText(item.provider, 4096);
  if (kind === "local-model" && provider !== "local")
    throw new TypeError("invalid-saved-segment-scope");
  return Object.freeze({ kind, provider, key, label });
}
function captureSavedSegmentContext(value) {
  const item = captureRecord(value, ["mode", "scopes"]), mode = item.mode;
  if (mode !== "library" && mode !== "workflow")
    throw new TypeError("invalid-saved-segment-context");
  const scopes = captureArray(item.scopes, 32).map(captureSavedSegmentScope);
  if (!scopes.length || scopes.filter((scope) => scope.kind === "global").length !== 1 || scopes.filter((scope) => rank(scope) === 2).length > 1)
    throw new TypeError("invalid-saved-segment-context");
  for (let index = 0; index < scopes.length; index++) {
    const scope = requireValue(scopes[index], "preparation");
    if (scope.kind === "provider-model" || mode === "library" && scope.kind !== "global" || scopes.slice(0, index).some((prior) => sameTarget(prior, scope)))
      throw new TypeError("invalid-saved-segment-context");
  }
  return Object.freeze({
    mode,
    scopes: Object.freeze(scopes.sort((a, b) => rank(a) - rank(b)))
  });
}
function captureSavedSegmentAdapter(value) {
  const item = captureRecord(value, ["read", "save"]);
  if (typeof item.read !== "function" || typeof item.save !== "function")
    throw new TypeError("invalid-saved-segment-adapter");
  const read = item.read, save = item.save;
  return Object.freeze({
    read(context, signal) {
      return Reflect.apply(read, value, [context, signal]);
    },
    save(request2, signal) {
      return Reflect.apply(save, value, [request2, signal]);
    }
  });
}
function validateSavedSegmentAdapter(value) {
  captureSavedSegmentAdapter(value);
}
function prepareSavedSegmentCatalog(context, value) {
  const segments = [];
  let remaining = 8e6;
  for (const raw of captureArray(value, 4096)) {
    const item = captureRecord(raw, ["id", "label", "text", "associations"]);
    const id = captureText(item.id, 4096), label = captureText(item.label, 2048), text = captureText(item.text, remaining);
    remaining -= text.length;
    const associations = captureArray(item.associations, 32).map(captureSavedSegmentScope);
    if (!associations.length) throw new TypeError("invalid-saved-segment-catalog");
    segments.push(
      Object.freeze({
        id,
        label,
        text,
        associations: Object.freeze(associations)
      })
    );
  }
  const sorted = segments.map((segment) => ({
    segment,
    label: caseFold(segment.label),
    text: caseFold(segment.text)
  }));
  sorted.sort(
    (a, b) => compareScalarText(a.label, b.label) || compareScalarText(a.text, b.text) || compareScalarText(a.segment.id, b.segment.id)
  );
  const shown = /* @__PURE__ */ new Set(), groups = [];
  const scopes = [...context.scopes].sort((a, b) => rank(b) - rank(a));
  for (const scope of scopes) {
    const matching = [];
    for (const { segment } of sorted) {
      if (!shown.has(segment.id) && segment.associations.some((association) => sameTarget(association, scope))) {
        matching.push(segment);
        shown.add(segment.id);
      }
    }
    if (matching.length) groups.push(Object.freeze({ scope, segments: Object.freeze(matching) }));
  }
  return Object.freeze({ context, groups: Object.freeze(groups) });
}

// frontend/sugarprompt/services/src/saved-segments/session.ts
var SavedSegmentSession = class {
  adapter;
  lane = new LatestRequest();
  context;
  /** Capture the host context before accepting any storage data. */
  constructor(adapter, context) {
    this.adapter = captureSavedSegmentAdapter(adapter);
    this.context = captureSavedSegmentContext(context);
  }
  /** A finite deadline aborts the provider and settles as unavailable without publishing late data. */
  async run(work) {
    let expired = false;
    const timer = setTimeout(() => {
      expired = true;
      this.lane.cancel();
    }, 1e4);
    try {
      const outcome = await this.lane.run(work);
      return expired ? { status: "unavailable" } : outcome;
    } finally {
      clearTimeout(timer);
    }
  }
  /** Listing traversal and scope matching happen once outside native menu input. */
  async read() {
    const outcome = await this.run(async (signal) => {
      const raw = await this.adapter.read(this.context, signal);
      if (signal.aborted) return { status: "cancelled" };
      const reply = captureReply(raw, ["ready", "unavailable"]);
      const result = reply.status === "unavailable" ? Object.freeze({ status: "unavailable" }) : Object.freeze({
        status: "ready",
        catalog: prepareSavedSegmentCatalog(this.context, reply.segments)
      });
      return signal.aborted ? { status: "cancelled" } : result;
    });
    if (outcome.status !== "ready") return outcome;
    return outcome.value;
  }
  /** Exact captured text and literal label reach only an admitted current scope. */
  async save(label, text, scope) {
    const capturedLabel = strip(captureText(label, 2048)), capturedText = captureText(text, 8e6);
    if (!this.context.scopes.includes(scope)) throw new TypeError("invalid-saved-segment-save");
    const outcome = await this.run(async (signal) => {
      const raw = await this.adapter.save(
        Object.freeze({ label: capturedLabel, text: capturedText, scope }),
        signal
      );
      if (signal.aborted) return { status: "cancelled" };
      const reply = captureReply(raw, ["saved", "unavailable"]);
      const status = reply.status === "saved" ? "saved" : "unavailable";
      return signal.aborted ? { status: "cancelled" } : Object.freeze({ status });
    });
    if (outcome.status !== "ready") return outcome;
    return outcome.value;
  }
  /** Supersession/source retirement releases the current host request immediately. */
  cancel() {
    this.lane.cancel();
  }
  /** Teardown prevents subsequent provider invocation and releases held request data. */
  dispose() {
    this.lane.dispose();
  }
};

// frontend/comfyui/ui/prompt_editor/runtime/PromptTagCatalog.ts
function preparePromptTagCatalog(source, revision) {
  const bundled = [];
  for (const [index, line] of source.split(/\r?\n/u).entries()) {
    if (!line.trim()) continue;
    const row = /^([^,]+),(\d+)(?:\s|$)/u.exec(line);
    const tag2 = row?.[1];
    const popularity = Number(row?.[2]);
    if (!tag2 || !Number.isSafeInteger(popularity) || popularity < 0)
      throw new TypeError(`Invalid bundled prompt tag at line ${String(index + 1)}.`);
    bundled.push({ tag: tag2, popularity });
  }
  if (!bundled.length) throw new TypeError("The bundled prompt tag catalog is empty.");
  return prepareTagCompletionCatalog({ revision, bundled, customTags: [], censoredTags: [] });
}

export {
  requireValue,
  parseWeight,
  unicodeAlphanumeric,
  whitespace,
  strip,
  wildcardDelimiters,
  normalizeSceneTitle,
  parseScenes,
  parenthesisEscapes,
  segmentSelectionRange,
  promptWordRange,
  isWildcardCompletionBody,
  wildcardReplacement,
  wildcardGhostText,
  sceneReplacement,
  wildcardBodyRange,
  utf16ToCodePoint,
  encodeSource,
  decodeSource,
  documentToSource,
  sourceToDocument,
  sourcePositionsToDocument,
  identityNormalization,
  validateSourceRange,
  decodedRangeToSource,
  prepareDocumentPromptScopes,
  admitDocumentPromptScope,
  prepareWildcardCsvEncoding,
  validateDocumentKind,
  prepareDocumentSemantics,
  prepareDocumentReplacement,
  prepareWildcardColumnAliases,
  mutateWildcardTag,
  prepareTriggerInsertionSlot,
  triggerInsertionText,
  loraReplacement,
  loraCompletionSuffix,
  tagReplacementText,
  completionSuffix,
  tagGhostText,
  prepareTagReplacement,
  formatWeight,
  stepWeight,
  prepareDuplicateEdit,
  isDiagnosticCommitBoundary,
  diagnosticIsActive,
  normalizeTypedDeletion,
  normalizePasteRange,
  normalizeTypedEdit,
  prepareSceneHeadings,
  validRegionName,
  regionNameReplacement,
  regionNameIdentities,
  changedRegionIdentities,
  applyRegionNames,
  REGION_PALETTE_BASE,
  regionPaletteColor,
  parseRegionHex,
  formatRegionHex,
  ConditioningTopology,
  reorderOriginBoundary,
  prepareDocumentValueReorders,
  valueReorderVacancy,
  valueReorderAdjacentTarget,
  valueReorderLayout,
  emptyValueReorderPartitions,
  serializeValueReorder,
  serializeReorderPreview,
  hideReorderChip,
  dropReorderChip,
  captureReorderSelection,
  mapReorderSelection,
  LatestRequest,
  validatePreparedSceneQueue,
  prepareSceneQueue,
  SceneQueueSession,
  featureDefinitions,
  resolveFeatureProfile,
  featureEnabled,
  resolveCompletionQuery,
  findPromptTextCompletions,
  assertPreparedLoraSchedulingContext,
  loraNameKey,
  defaultPreferences,
  isCatalogStableLoraResource,
  LoraResourceSession,
  prepareDocumentLoraScopes,
  SceneLoraSession,
  prepareLoraPage,
  LoraPageSession,
  validatePreparedLoraMetadata,
  prepareWildcardDiagnostic,
  prepareLoraMetadataMenu,
  LoraMetadataSession,
  unsupportedSceneDiagnostics,
  prepareDuplicateDiagnostics,
  unresolvedConditioning,
  captureConditioningContext,
  captureRegionNames,
  ClipboardSession,
  prepareSpellingCandidates,
  isPreparedSpellingCandidates,
  isSpellcheckSession,
  captureRecord,
  captureSavedSegmentContext,
  validateSavedSegmentAdapter,
  SavedSegmentSession,
  preparePromptTagCatalog
};
