/*
 * How the tuner page and its server agree on a Save token: the server puts
 * this run's in a <meta> in the page it serves (vite.config.ts), and the
 * page sends it back in a header (main.tsx), checked in saveRequest.ts.
 */
export const TOKEN_META = "plate-tuner-token";
export const TOKEN_HEADER = "x-plate-tuner-token";
