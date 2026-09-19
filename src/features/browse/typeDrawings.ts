/*
 * The artwork behind TypeDrawing: one side elevation per firearm type,
 * muzzle to the right, in a 320×200 box. Each drawing is a list of parts
 * painted back to front, then a bore axis.
 */

export type Part =
  | { d: string; role: "part" | "open" | "detail" }
  | { circle: [number, number, number]; role: "part" };

export interface Drawing {
  parts: Part[];
  /** Bore axis: [x start, y, x end]. */
  axis: [number, number, number];
}

export const DRAWINGS: Record<string, Drawing> = {
  handgun: {
    parts: [
      {
        d: "M68 58C60 60 57 65 57 72L44 170Q43 176 49 177L94 183Q99 183.5 100 178L108 112Q110 100 120 99H158Q168 99 170 90L175 74H262L266 68V58Z",
        role: "part",
      },
      { d: "M119 77H165L162 88Q160 94 154 94H124Q118 94 118 88Z", role: "open" },
      { d: "M143 77C141 84 138 89 137 93", role: "open" },
      { d: "M132 77V81H138V77", role: "detail" },
      { d: "M46 164L101 170", role: "detail" },
      { d: "M62 84L51.5 160M101 118L96 160", role: "detail" },
      {
        d: "M107 124Q110.5 127 107 130M106.3 133Q109.8 136 106.3 139M105.5 142Q109 145 105.5 148",
        role: "detail",
      },
      { d: "M188 66H208M216 66H236", role: "detail" },
      { d: "M146 58V63H178L182 60V58Z", role: "part" },
      { d: "M116 58V63H134V58Z", role: "part" },
      { circle: [110, 88, 3], role: "part" },
      { d: "M62 24H256L274 33V58H56V30Q56 24 62 24Z", role: "part" },
      { d: "M64 19.5H78V24H64Z", role: "part" },
      { d: "M69.5 19.5V22", role: "detail" },
      { d: "M248 18.5H258V24H248Z", role: "part" },
      { d: "M67 31V52M71.5 31V52M76 31V52M80.5 31V52M85 31V52", role: "detail" },
      { d: "M226 36V52M230.5 36V52M235 36V52M239.5 36V52", role: "detail" },
      { d: "M130 28H172L176 32V46H130Z", role: "open" },
      { d: "M138 46V38H168", role: "detail" },
      { d: "M56 52H270", role: "detail" },
    ],
    axis: [42, 38, 302],
  },
  rifle: {
    parts: [
      { d: "M139 90.2L304 91.6V96.4L139 97.8Z", role: "part" },
      { d: "M300.5 91.6V96.4", role: "detail" },
      { d: "M286 91.8L291 88.6H295.5V91.7Z", role: "part" },
      {
        d: "M16 97C16 95 17.4 94.4 20 94C30 92.6 38 91.4 50 91L88 89.6H136V95.5H233Q237 95.5 237.2 98.5L236.6 107.6Q236.2 110 233.5 110L138 113.5L122 115L112 116Q108.5 125 104.5 131Q103 135 97 135C92 135 88.5 130.5 83 125C77 119.5 66 121 44 125.5L24 130.6Q21 131.2 20.6 128Z",
        role: "part",
      },
      { d: "M18.4 100L22.8 125.6", role: "detail" },
      { d: "M223 95.5V110", role: "detail" },
      { d: "M95 132.4L103.4 129.4", role: "detail" },
      { d: "M87.5 112L100.5 115L103.5 123L92.5 120.6Z", role: "detail" },
      { d: "M91.5 114.4L98.5 120.4M95.5 113.6L101 118.4", role: "detail" },
      { d: "M168 100H214V106.6H168Z", role: "detail" },
      {
        d: "M174 100L180 106.6M182 100L188 106.6M190 100L196 106.6M198 100L204 106.6M206 100L212 106.6",
        role: "detail",
      },
      { circle: [200, 111.6, 1.6], role: "part" },
      { circle: [34, 131.4, 1.6], role: "part" },
      { d: "M103 99.4V110Q103 114.6 108 114.6H119Q125 114.6 126 109L127 99.4Z", role: "part" },
      { d: "M112 99.4C111 104 112 107 113.4 110.4", role: "detail" },
      { d: "M102 99.4H129V102.6H102Z", role: "part" },
      { d: "M82 90.6H90V96.2H82Z", role: "part" },
      { d: "M91 88.6H134V99.4H91Z", role: "part" },
      { d: "M107 91.2H125V96.4H107Z", role: "open" },
      { d: "M93 92.2H105", role: "detail" },
      { d: "M134 87.6H142V100.4H134Z", role: "part" },
      { d: "M102 95.6L97 97.2L86 106.4L83.4 105.4L94.5 95Z", role: "part" },
      { circle: [82.6, 108.6, 3.4], role: "part" },
      { d: "M94 75.6H100V88.6H94Z", role: "part" },
      { d: "M124 75.6H130V88.6H124Z", role: "part" },
      { d: "M71 75.2L84 76.4V84.6L71 85.8Z", role: "part" },
      { d: "M84 76.4H139V84.6H84Z", role: "part" },
      { d: "M139 76.4L151 73.8H161V87.2H151L139 84.6Z", role: "part" },
      { d: "M159 75.2V85.8M154.5 74.6V86.4", role: "detail" },
      { d: "M108 76.4V72.2H117V76.4Z", role: "part" },
      { d: "M110.4 72.2V76.4M112.5 72.2V76.4M114.6 72.2V76.4", role: "detail" },
      { d: "M94 75.6H100V84.6H94Z", role: "part" },
      { d: "M124 75.6H130V84.6H124Z", role: "part" },
      { d: "M87 76.4V84.6M90 76.4V84.6", role: "detail" },
    ],
    axis: [8, 94, 320],
  },
  shotgun: {
    parts: [
      { d: "M152 81.8L308 82.4V89.6L152 90.2Z", role: "part" },
      { d: "M152 79.4H306V81.9H152Z", role: "part" },
      {
        d: "M160 79.4V81.6M166 79.4V81.6M172 79.4V81.6M178 79.4V81.6M184 79.4V81.6M190 79.4V81.6M196 79.4V81.6M202 79.4V81.6M208 79.4V81.6M214 79.4V81.6M220 79.4V81.6M226 79.4V81.6M232 79.4V81.6M238 79.4V81.6M244 79.4V81.6M250 79.4V81.6M256 79.4V81.6M262 79.4V81.6M268 79.4V81.6M274 79.4V81.6M280 79.4V81.6M286 79.4V81.6M292 79.4V81.6M298 79.4V81.6",
        role: "detail",
      },
      { d: "M298 82.4V89.6", role: "detail" },
      { d: "M306.4 79.4V77.4", role: "detail" },
      { circle: [306.4, 76.8, 1.4], role: "part" },
      { d: "M232 79.4V78", role: "detail" },
      { circle: [232, 77.4, 1.1], role: "part" },
      { d: "M152 90.4H283V95.8H152Z", role: "part" },
      { d: "M227 89.6H233V96.6H227Z", role: "part" },
      { d: "M283 89.8H290V96.6H283Z", role: "part" },
      {
        d: "M10 96.6C10 94.4 11.4 93.6 14 93.2C28 91.6 40 89.4 52 87.6L92 80.6V96H101C100 106 98 116 96 122Q95 129.4 87 129.4C80 129.4 78 122 72 119.4C62 117 44 123 22 131.6Q17.6 132.6 17 129.6Z",
        role: "part",
      },
      { d: "M12.6 98L18 127", role: "detail" },
      { d: "M88.6 127.4L97 124", role: "detail" },
      { d: "M101 94V104Q101 108.6 105.6 108.6H116Q122 108.6 123 104L124 94Z", role: "part" },
      { d: "M110 94C109 99 110 102 111.4 105", role: "detail" },
      { d: "M118 94V97H126V94Z", role: "part" },
      { d: "M88 80.6H148Q153 80.6 153 85V90Q153 94 149 94H92Q88 94 88 90Z", role: "part" },
      { d: "M111 83H133Q135.4 83 135.4 85.4V87.6H111Z", role: "open" },
      { d: "M118 87.6V83", role: "detail" },
      { d: "M103 94V91.6H120V94", role: "open" },
      { d: "M92 84.6H106", role: "detail" },
      { d: "M153 92.2H168M153 94.6H168", role: "detail" },
      {
        d: "M168 91.4H223Q228.6 91.4 228.6 96V102Q228.6 106.4 224 106.4H172.6Q168 106.4 168 102Z",
        role: "part",
      },
      {
        d: "M178 94.4V103.6M182.5 94.4V103.6M187 94.4V103.6M191.5 94.4V103.6M196 94.4V103.6M200.5 94.4V103.6M205 94.4V103.6M209.5 94.4V103.6M214 94.4V103.6M218.5 94.4V103.6",
        role: "detail",
      },
    ],
    axis: [4, 86, 316],
  },
  // "Other" has no single silhouette, so it gets a cartridge instead.
  other: {
    parts: [
      {
        d: "M34.5 81.4H38L40 78.6H179L196 83.5H216V116.5H196L179 121.4H40L38 118.6H34.5Z",
        role: "part",
      },
      { d: "M179 79.2V120.8M196 83.5V116.5", role: "detail" },
      { d: "M40 78.6V121.4", role: "detail" },
      { d: "M30.4 78H34.5V122H30.4Q29 122 29 120.6V79.4Q29 78 30.4 78Z", role: "part" },
      { d: "M216 85.7H253C271 85.7 282 92 292 100C282 108 271 114.3 253 114.3H216Z", role: "part" },
      { d: "M225 86.6V113.4M228.5 86.6V113.4", role: "detail" },
      { d: "M216 83.5V116.5", role: "detail" },
    ],
    axis: [23, 100, 313],
  },
};
