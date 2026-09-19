/*
 * The artwork behind TypeDrawing: one side elevation per firearm type,
 * muzzle to the right, in a 320×200 box. Each drawing is a list of parts
 * painted back to front, then a bore axis.
 *
 * Each firearm is traced from a side-on photograph of a real model so its
 * proportions hold up. Muzzle-right puts the right side toward the viewer,
 * so only right-side features are drawn (ejection ports, not slide stops).
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
  // Glock 17, right side: ejection port and extractor show; the slide stop
  // lever and magazine catch are on the left side only.
  handgun: {
    parts: [
      { d: "M67.8 24L71 19Q71.5 18.3 72.4 18.3H75.2L80.8 24Z", role: "part" },
      { d: "M262.8 24.3L264.3 21.2H267.4L270.6 24.3Z", role: "part" },
      {
        d: "M54.8 168H101.4V175.8L98.5 178.8Q96.5 181.3 92.6 181.4L53 181.1Q51.6 181 51.6 179.8L51.8 175.8L54.8 173Z",
        role: "part",
      },
      {
        d: "M58.5 49.5L57.2 50.7Q56.2 55.6 59.5 57.8C66 58.6 72.5 60.5 75.4 66.5Q77.2 70.5 76.4 76C74.8 84 70 100 64.7 109.1C57.5 121.5 46 142 43.2 154Q42 161 42.8 165Q44 170 49 171L101.4 171.3L108.6 164.9C108.4 157 110.5 148.5 115 143.5Q118.6 140.5 117.4 136.5Q116.4 132 119.5 127Q122 123.5 125.5 121.5Q126.6 116 126.5 111Q127 105.5 131 101.5Q135 98.6 140 99.8Q144 102.4 149 103L194.7 101.9Q196 101 195.4 99.8L193.4 93Q192.9 86 193.3 79Q194 74.5 197 71.6Q199.5 69.4 203 69L265.7 67.3Q272 66.6 275.4 62.5Q277.3 59.5 277.2 55L277 49.5Z",
        role: "part",
      },
      {
        d: "M150 71.8Q146 72.5 142 76Q139.5 78.8 139.6 83V90Q140 96 146 97.5H181Q186.2 97.5 186.2 92V75Q186.2 69.6 181 69.6Z",
        role: "open",
      },
      {
        d: "M143 73.5Q149 71.5 152.5 72.2C149.5 78 148.5 86 152.4 95.2Q151.8 96.6 150.2 96C145 90 142.5 85 140.6 79Q140.5 75.5 143 73.5Z",
        role: "part",
      },
      { d: "M149.4 73.6C147.2 80.5 147.2 88 150.8 95.4", role: "detail" },
      { d: "M208.5 51H213.5V59.5H208.5ZM214 60H268M214 62.4H266", role: "detail" },
      { circle: [151.3, 52.9, 1.5], role: "part" },
      { circle: [153, 61.8, 1.9], role: "part" },
      { d: "M113.5 49.5V56.5H127.5V49.5", role: "part" },
      { d: "M117 50.5V55.5M120.5 50.5V55.5M124 50.5V55.5", role: "detail" },
      { d: "M58.5 49.5L58.3 28Q58.5 24 62.5 23.6L271 24.3Q277.8 24.6 278 30V49.5Z", role: "part" },
      {
        d: "M65.5 27V47M71 27V47M76.5 27V47M82 27V47M87.5 27V47M93 27V47M98.5 27V47M104 27V47",
        role: "detail",
      },
      { d: "M141 24.1V36.5H184V24.4", role: "open" },
      { d: "M145.5 24.2V36.5M145.5 33H184", role: "detail" },
      { d: "M127 33H141M127 33V37.5H141", role: "detail" },
    ],
    axis: [46, 37, 300],
  },
  // AR-15 in the A2 pattern, right side: carry handle, triangular front
  // sight, round ribbed handguard, fixed stock, 30-round magazine; the
  // ejection port, forward assist and magazine release face this way.
  rifle: {
    parts: [
      { d: "M259 76.6H297V81.4H259Z", role: "part" },
      { d: "M296.5 75.8H300V82.2H296.5Z", role: "part" },
      { d: "M300 76.2H309.8V81.8H300Z", role: "part" },
      { d: "M302.5 77.6H307.5M302.5 80.4H307.5", role: "detail" },
      {
        d: "M246 82.4V76.5L246.8 75L253 60.8Q253.6 59.6 255 59.6H258Q259.8 59.6 259.8 61.4V82.4Z",
        role: "part",
      },
      { d: "M249.6 73L255.3 64.8H257.6V73Z", role: "open" },
      { d: "M246 74.6H259.8", role: "detail" },
      { d: "M256 82.4V85.6H259.8V82.4", role: "part" },
      { circle: [249.2, 85.2, 1.6], role: "part" },
      { d: "M155.8 70.2H245.5V87.8H155.8Z", role: "part" },
      {
        d: "M160 70.8V87.2 M166 70.8V87.2 M172 70.8V87.2 M178 70.8V87.2 M184 70.8V87.2 M190 70.8V87.2 M196 70.8V87.2 M202 70.8V87.2 M208 70.8V87.2 M214 70.8V87.2 M220 70.8V87.2 M226 70.8V87.2 M232 70.8V87.2 M238 70.8V87.2 M244 70.8V87.2",
        role: "detail",
      },
      { d: "M155.8 79H245.5", role: "detail" },
      { d: "M245.5 71H247.8V87H245.5", role: "part" },
      { d: "M148.5 72L150.5 69.4H155.8V88.6H150.5L148.5 86Z", role: "part" },
      { d: "M151.8 70V88M153.6 70V88", role: "detail" },
      { d: "M11 77H9Q8.2 77 8.2 78.2V112.4Q8.2 113.6 9.6 113.6H11Z", role: "part" },
      {
        d: "M90.5 75.8L12.6 76.9Q10.2 77 10.2 79V111.5Q10.2 113.2 12 113L90.5 94.4Z",
        role: "part",
      },
      {
        d: "M123.3 97H145.8V100Q147.2 115 149.6 129.8L150.6 131.2L132.7 136L131.9 134.5Q127.4 118 125.8 101Z",
        role: "part",
      },
      { d: "M131.3 131.9L149.4 127.3", role: "detail" },
      { d: "M131 101.5Q132.4 117 135.9 131M138.2 101Q139.2 115.5 142.2 129.4", role: "detail" },
      {
        d: "M97.5 93L81.3 118.8Q80.3 120.5 82.3 121L95.8 124.3Q98.3 124.8 98.4 122.3Q98.4 119.5 96.2 116.2Q95.2 113.8 97.5 112Q101.8 110.8 104 109Q105.3 107.5 105 104L106 93Z",
        role: "part",
      },
      {
        d: "M90.5 83.5H148.5V94.5Q148.5 96 147 96.5L146.5 99.5H121V102.4Q121.3 104.3 119.3 104.3H108Q106 104.3 105.3 102L105 96.5H97Q92 96 90.5 92Z",
        role: "part",
      },
      { d: "M108 96.5H120V101.2Q120 102.6 118.6 102.6H109.5Q108 102.6 108 101Z", role: "open" },
      {
        d: "M111.3 96.5H112.8C112.6 99 113.2 100.8 115.2 101.8L114.8 102.6C112.2 101.8 111.2 99.6 111.3 96.5Z",
        role: "part",
      },
      { d: "M122 86.2H125.3V90.6H122Z", role: "part" },
      { circle: [98.3, 86.3, 1.2], role: "part" },
      { circle: [104.2, 89.4, 0.9], role: "part" },
      { circle: [113.3, 91.2, 0.8], role: "part" },
      { circle: [147, 85.8, 1], role: "part" },
      { d: "M91.3 74.6L91.8 71.2Q92.2 70.3 93.2 70.5L95.6 71.3L96.2 74.6Z", role: "part" },
      {
        d: "M100 74.5V61.4Q100 60 101.5 60H108L110 59.6L144 62.3Q149.6 62.8 149.6 68V74.5Z",
        role: "part",
      },
      {
        d: "M112.5 64.6H144.5Q147.4 64.6 147.4 67.5V69.2Q147.4 72 144.5 72H112.5Q110.2 72 110.2 69.6V67Q110.2 64.6 112.5 64.6Z",
        role: "open",
      },
      { d: "M110 60V74.5M100.5 66.8H107.5V69.8H100.5Z", role: "detail" },
      { circle: [103.8, 62.9, 2.1], role: "part" },
      { d: "M90.5 74.5H149V83.5H90.5Z", role: "part" },
      { d: "M121 75.6H146.8V80.6H121Z", role: "open" },
      { d: "M121.5 81.8H146.5", role: "detail" },
      {
        d: "M112.8 73.4H118.6Q119.8 73.4 119.8 74.6V78.2Q119.8 79.4 118.6 79.4H112.8Z",
        role: "part",
      },
    ],
    axis: [4, 79, 316],
  },
  // Remington 870 with a 20" barrel, right side: the ejection port is on
  // this side; ribbed forend, magazine cap and vented recoil pad.
  shotgun: {
    parts: [
      {
        d: "M18.5 88L13.2 88.3Q11.3 88.5 11 90.3Q8.6 104 8.8 122Q9 124.6 11.2 124.8L16.8 125.3Z",
        role: "part",
      },
      {
        d: "M14.6 90.5L12.8 93.1L14.8 95.7L12.8 98.3L14.8 100.9L12.8 103.5L14.8 106.1L12.8 108.7L14.8 111.3L12.8 113.9L14.8 116.5L12.8 119.1L14.8 121.7",
        role: "detail",
      },
      {
        d: "M112 73.2L107.5 73.6C99 75.8 90.5 81 84 82.1Q81.5 82.5 78 82.6L18.3 88.2L16.6 125.2Q16.6 125.6 17.2 125.3L70 101.6C77 99.8 82.5 101.3 86.3 103.2Q89 104.5 90.9 102.2C94 97 101 89.5 110 85.4H112Z",
        role: "part",
      },
      { d: "M163 71.2H308.4Q309.8 71.2 309.8 72.6V76.2Q309.8 77.6 308.4 77.6H163Z", role: "part" },
      { circle: [302.3, 70.2, 0.9], role: "part" },
      { d: "M163 78.6H247V87H163Z", role: "part" },
      { d: "M140 86.6H191V88.2H140Z", role: "part" },
      {
        d: "M246.2 79.2Q246.2 78.3 247.1 78.3H250Q251 78.3 251 79.3V86.3Q251 87.3 250 87.3H247.1Q246.2 87.3 246.2 86.4Z",
        role: "part",
      },
      { d: "M247.8 78.8V86.8M249.4 78.8V86.8", role: "detail" },
      {
        d: "M193 75.2H240Q245.8 75.2 245.8 80V86Q245.8 90.6 240.5 90.6H193Q190.4 90.6 190.4 88V77.8Q190.4 75.2 193 75.2Z",
        role: "part",
      },
      {
        d: "M197 78.6V87.4M200.6 78.6V87.4M204.2 78.6V87.4M207.8 78.6V87.4M211.4 78.6V87.4M215 78.6V87.4M218.6 78.6V87.4M222.2 78.6V87.4M225.8 78.6V87.4M229.4 78.6V87.4M233 78.6V87.4M236.6 78.6V87.4",
        role: "detail",
      },
      { d: "M111 89.5Q111 95.8 116 95.8H121.5Q125.8 95.6 126.2 90.5V88.5Z", role: "part" },
      { d: "M113.3 90.2Q113.5 94.3 116.5 94.4H121.3Q124.3 94.2 124.4 90.2", role: "open" },
      {
        d: "M114.4 89.4H115.8C115.6 91.6 116.2 93 118 93.8L117.7 94.5C115.3 93.9 114.3 92.2 114.4 89.4Z",
        role: "part",
      },
      { d: "M109.6 85.4L110.2 90.3L139.5 87.8L140.5 85.4Z", role: "part" },
      { d: "M107.9 75.6Q108.2 72.4 111.2 72.3L163.3 71.4V85.4H109.6Z", role: "part" },
      {
        d: "M141 73.2H157.2Q159.7 73.2 159.7 75.7Q159.7 78.2 157.2 78.2H141Q138.4 78.2 138.4 75.7Q138.4 73.2 141 73.2Z",
        role: "open",
      },
      { circle: [112.5, 82.7, 0.9], role: "part" },
      { circle: [127.1, 82.7, 0.9], role: "part" },
    ],
    axis: [2, 74.4, 318],
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
