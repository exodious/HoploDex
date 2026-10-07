import type { SVGProps } from "react";

// A small stroked icon set drawn on a 20px grid to match the type drawings'
// line weight. Decorative by default (aria-hidden); pair with visible text
// or an aria-label on the control.
const PATHS = {
  search: "M8.5 3.5a5 5 0 1 0 0 10 5 5 0 0 0 0-10ZM12.2 12.2 16.5 16.5",
  plus: "M10 4v12M4 10h12",
  list: "M7 5.5h9M7 10h9M7 14.5h9M3.5 5.5h.5M3.5 10h.5M3.5 14.5h.5",
  tiles: "M3.5 3.5h5v5h-5zM11.5 3.5h5v5h-5zM3.5 11.5h5v5h-5zM11.5 11.5h5v5h-5z",
  back: "M11.5 5 6.5 10l5 5",
  up: "M5 11.5 10 6.5l5 5",
  chevronLeft: "M12 5 7 10l5 5",
  chevronRight: "M8 5l5 5-5 5",
  chevronDown: "M5.5 8l4.5 4.5L14.5 8",
  more: "M5 10h.01M10 10h.01M15 10h.01",
  pencil: "M12.5 4.5 15.5 7.5 7.5 15.5H4.5V12.5ZM10.5 6.5l3 3",
  trash: "M4 6h12M8 6V4.5h4V6M5.5 6l.8 9.5h7.4l.8-9.5M8.5 9v4M11.5 9v4",
  file: "M5.5 3h6l3 3v11h-9zM11.5 3v3h3M8 10h4.5M8 13h4.5",
  image: "M3.5 4.5h13v11h-13zM3.5 13l4-4 3.5 3.5 2-2 3.5 3.5M12.5 7.5h.01",
  alert: "M10 3.5 17 16H3ZM10 8v3.5M10 13.8v.2",
  check: "M4.5 10.5 8 14l7.5-8",
  calendar: "M3.5 5h13v11h-13zM3.5 8.5h13M7 3v3.5M13 3v3.5",
  upload: "M10 13V3.5M6 7.5l4-4 4 4M4 13v3h12v-3",
  download: "M10 3.5V13M6 9l4 4 4-4M4 13v3h12v-3",
  close: "M5 5l10 10M15 5 5 15",
  open: "M11 3.5h5.5V9M16.5 3.5 9 11M14 11.5v5h-10.5v-10.5h5",
  shield: "M10 3 16 5v5c0 3.5-2.6 6-6 7-3.4-1-6-3.5-6-7V5Z",
  lock: "M6.5 9V6.5a3.5 3.5 0 0 1 7 0V9M4.5 9h11v7.5h-11zM10 12v2",
  archive: "M3.5 4h13v3.5h-13zM4.5 7.5v8.5h11V7.5M8 10.5h4",
  star: "M10 3.5l1.9 4 4.4.5-3.3 3 .9 4.3L10 13.2l-3.9 2.1.9-4.3-3.3-3 4.4-.5Z",
  folder: "M3.5 5h4.5l1.5 1.5h7v9h-13z",
  sun: "M10 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6ZM10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4",
  moon: "M16 11.5A6.5 6.5 0 0 1 8.5 4 6.5 6.5 0 1 0 16 11.5Z",
  auto: "M3.5 4.5h13v9h-13zM7.5 16.5h5M10 13.5v3",
  eye: "M2.5 10S5.5 4.5 10 4.5s7.5 5.5 7.5 5.5-3 5.5-7.5 5.5S2.5 10 2.5 10ZM10 7.8a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4Z",
  minus: "M4 10h12",
  pageFirst: "M5 4.5v11M14.5 5 9.5 10l5 5",
  pageLast: "M15 4.5v11M5.5 5l5 5-5 5",
  fitWidth: "M3.5 4.5v11M16.5 4.5v11M6.5 10h7M8.5 8l-2 2 2 2M11.5 8l2 2-2 2",
  fitPage: "M5.5 3.5h9v13h-9zM10 6.5v7M8.3 8.2 10 6.5l1.7 1.7M8.3 11.8 10 13.5l1.7-1.7",
} as const;

export type IconName = keyof typeof PATHS;

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 18, ...props }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable={false}
      {...props}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
