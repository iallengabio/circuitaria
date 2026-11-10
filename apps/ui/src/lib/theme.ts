// Canvas colour tokens (the DOM uses the CSS variables in app.css; keep both in sync).

export interface Pal {
  bg: string;
  grid: string;
  fill: string;
  fill2: string;
  stroke: string;
  text: string;
  dim: string;
  off: string;
  on: string;
  z: string;
  x: string;
  bus: string;
  busOn: string;
  accent: string;
  select: string;
}

export const PALETTES: Record<'light' | 'dark', Pal> = {
  light: {
    bg: '#f4f5fb', grid: '#dfe3f1', fill: '#ffffff', fill2: '#eef0fa', stroke: '#2b3152', text: '#2b3152', dim: '#8a92b5',
    off: '#9aa3c4', on: '#12b76a', z: '#c3c8de', x: '#f0436a', bus: '#7d8fd6', busOn: '#3b5bff', accent: '#6c5ce7', select: '#ff9f1c',
  },
  dark: {
    bg: '#11142a', grid: '#1c2142', fill: '#1f2549', fill2: '#283060', stroke: '#c9d1ff', text: '#e8ecff', dim: '#7781b3',
    off: '#4d5788', on: '#3ddc97', z: '#333b68', x: '#ff5470', bus: '#5a6bb8', busOn: '#8fa6ff', accent: '#9d8cff', select: '#ffb443',
  },
};

export const LED_COLORS: Record<string, string> = {
  red: '#ff4d6d', green: '#2fd67b', blue: '#4c8dff', yellow: '#ffd23f', purple: '#b06cff', white: '#f4f6ff',
};

export const IC_COLORS: Record<string, string> = {
  teal: '#1aa6a6', purple: '#7c5ce7', orange: '#f08a24', pink: '#e8589c', blue: '#3b82f6', green: '#2ea86b', yellow: '#d6a800', red: '#e5484d',
};

/** 16-colour palette of the pixel screen (index = 4-bit colour). */
export const PIXEL_COLORS = [
  '#0b0d1c', '#1d2b53', '#7e2553', '#008751', '#ab5236', '#5f574f', '#c2c3c7', '#fff1e8',
  '#ff004d', '#ffa300', '#ffec27', '#00e436', '#29adff', '#83769c', '#ff77a8', '#ffffff',
];

export const FONT = 'ui-rounded, "SF Pro Rounded", "Nunito", "Segoe UI", system-ui, sans-serif';
export const MONO = 'ui-monospace, "SF Mono", "Cascadia Code", Menlo, Consolas, monospace';
