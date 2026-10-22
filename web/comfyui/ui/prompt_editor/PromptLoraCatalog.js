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

// frontend/sugarprompt/core/src/prompt/alphanumeric.ts
var intervals = decodeUnicodeIntervals(alphanumericIntervals);

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

// frontend/sugarprompt/core/src/normalization/lexicon.ts
var identityWhitespace = new RegExp(whitespace.source + "+", "gu");

// frontend/sugarprompt/core/src/diagnostics/visibility.ts
var punctuation = new Set(Array.from(`,;:.!?)]}"'`));

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

// frontend/sugarprompt/services/src/completion/catalog.ts
var empty = Object.freeze([]);

// frontend/sugarprompt/services/src/completion/scene-catalog.ts
var empty2 = Object.freeze([]);

// frontend/sugarprompt/services/src/features/registry.ts
function definition(id, label, aliases, syntax = null, dependencies = [], legacyBase = false) {
  return Object.freeze({
    id,
    label,
    aliases: Object.freeze(aliases),
    syntax,
    dependencies: Object.freeze(dependencies),
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

// frontend/sugarprompt/services/src/lora/names.ts
function withoutLoraExtension(value) {
  const folded = caseFold(value);
  for (const suffix of [".safetensors", ".ckpt", ".pt"])
    if (folded.endsWith(suffix)) return value.slice(0, -suffix.length);
  return value;
}
function loraNameKey(value, extensionless = false) {
  return caseFold((extensionless ? withoutLoraExtension(value) : value).replaceAll("\\", "/"));
}

// frontend/sugarprompt/services/src/features/preferences.ts
var defaultPreferences = Object.freeze({
  allowed: Object.freeze(
    Object.fromEntries(featureDefinitions.map(({ id }) => [id, true]))
  ),
  wheelMode: "hover_dwell",
  resolveOnGeneration: true
});

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
    let suffix = this.suffixes;
    if (parts.some((part) => !part)) suffix = void 0;
    else for (const part of parts.reverse()) suffix = suffix?.children.get(part);
    const path = match(suffix?.items);
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
    const record = loraRecord(value, ["backendValue", "text"]), identity = loraText(record.backendValue, 4096), text = loraText(record.text, 16384, true);
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

// frontend/sugarprompt/services/src/regions/context.ts
var unresolvedConditioning = Object.freeze({
  key: null,
  mode: "unresolved"
});

// frontend/comfyui/ui/prompt_editor/PromptLoraCatalog.ts
function preparePromptLoraCatalog(snapshot, baseUrl, prioritize) {
  const models = new Map(
    snapshot.items.filter((item) => item.kind === "loras").map((item) => [item.value, item])
  );
  const adapter = createLoraCatalogAdapter({
    revision: `models:${String(snapshot.revision)}`,
    authoritative: !snapshot.scanning,
    items: snapshot.items.filter((item) => item.kind === "loras").map((item) => {
      const name = item.value.replace(/\.(safetensors|ckpt|pt|pth|bin)$/i, "");
      const thumbnail2 = item.thumbnails.find((image) => image.role === "banner") ?? item.thumbnails[0];
      const url = thumbnail2 ? safeHttpUrl(thumbnail2.url, baseUrl) : null;
      return {
        model: {
          promptName: name,
          backendValue: item.value,
          basename: name.split(/[\\/]/).pop() ?? name
        },
        title: item.title || item.value,
        subtitle: item.version || null,
        triggerWords: item.trainedWords,
        thumbnail: thumbnail2 && url && thumbnail2.width > 0 && thumbnail2.height > 0 ? { status: "ready", url, width: thumbnail2.width, height: thumbnail2.height } : { status: "unavailable" },
        modelPageUrl: modelPage(item, baseUrl)
      };
    })
  });
  return {
    ...adapter,
    /** Prioritize only names resolved by the native catalog; never acquire metadata from rendering. */
    async resolve(names, signal) {
      const resources = await adapter.resolve(names, signal);
      if (!signal.aborted) {
        const items = /* @__PURE__ */ new Set();
        for (const resource of resources) {
          if (resource.resolution !== "found") continue;
          const model = models.get(resource.model.backendValue);
          if (model) items.add(model);
        }
        prioritize?.([...items]);
      }
      return resources;
    }
  };
}
function safeHttpUrl(value, baseUrl) {
  if (!value) return null;
  try {
    const url = new URL(value, baseUrl);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
  } catch {
    return null;
  }
}
function modelPage(item, baseUrl) {
  return safeHttpUrl(item.providerLinks[0]?.modelPageUrl ?? item.providerUrl, baseUrl);
}
export {
  preparePromptLoraCatalog
};
