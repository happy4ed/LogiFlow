/* Shared inline SVG icons (stroke = currentColor). size via s(px). */
function svg(inner, size, extra) {
  return `<svg viewBox="0 0 24 24" fill="none" width="${size}" height="${size}" `
    + `stroke="currentColor" stroke-width="1.7" stroke-linecap="round" `
    + `stroke-linejoin="round" ${extra||''}>${inner}</svg>`;
}
const ICON = {
  // app mark: two screens with a flow arc transferring between them
  mark: (s) => `<svg viewBox="0 0 24 24" fill="none" width="${s}" height="${s}">`
    + `<rect x="2.3" y="9.2" width="8.3" height="7.8" rx="2" stroke="currentColor" stroke-width="1.8"/>`
    + `<rect x="13.4" y="9.2" width="8.3" height="7.8" rx="2" stroke="currentColor" stroke-width="1.8"/>`
    + `<path d="M8.6 9.2 C 9.4 4.9, 14.6 4.9, 15.4 9.2" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/>`
    + `<path d="M15.7 9.6 L 13.1 8.7 L 14.4 11.1 Z" fill="currentColor"/></svg>`,
  // nav: switch / transfer
  swap: (s) => svg(`<path d="M4 8h13M13 4l4 4-4 4"/><path d="M20 16H7M11 20l-4-4 4-4"/>`, s),
  diag: (s) => svg(`<path d="M3 12h3l2.5 6 4-13 2.5 7H21"/>`, s),
  gear: (s) => svg(`<circle cx="12" cy="12" r="3.2"/><path d="M12 3v2.2M12 18.8V21M4.6 4.6l1.6 1.6M17.8 17.8l1.6 1.6M3 12h2.2M18.8 12H21M4.6 19.4l1.6-1.6M17.8 6.2l1.6-1.6"/>`, s),
  refresh: (s) => svg(`<path d="M20 11a8 8 0 1 0-1.6 5"/><path d="M20 4v5h-5"/>`, s),
  plus: (s) => svg(`<path d="M12 5v14M5 12h14"/>`, s),
  minus: (s) => svg(`<path d="M5 12h14"/>`, s),
  fit: (s) => svg(`<path d="M8 3H4v4M16 3h4v4M8 21H4v-4M16 21h4v-4"/>`, s),
  mouse: (s) => svg(`<rect x="7" y="3" width="10" height="18" rx="5"/><path d="M12 7v3"/>`, s),
  keyboard: (s) => svg(`<rect x="2.5" y="6.5" width="19" height="11" rx="2"/><path d="M6 10h0M9 10h0M12 10h0M15 10h0M18 10h0M7 14h10"/>`, s),
  plug: (s) => svg(`<path d="M9 3v5M15 3v5M6.5 8h11v3a5.5 5.5 0 0 1-11 0z"/><path d="M12 16.5V21"/>`, s),
  chevron: (s) => svg(`<path d="M6 9l6 6 6-6"/>`, s),
  pencil: (s) => svg(`<path d="M4 20h4L19 9l-4-4L4 16v4z"/><path d="M14 6l4 4"/>`, s),
  check: (s) => svg(`<path d="M5 12l4.5 4.5L19 7"/>`, s, 'stroke-width="2.4"'),
  monitor: (s) => svg(`<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>`, s),
  tower: (s) => svg(`<rect x="7" y="3" width="10" height="18" rx="1.6"/><path d="M10 6h4M10 9h4"/><circle cx="12" cy="17" r="1"/>`, s),
  folder: (s) => svg(`<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>`, s),
  power: (s) => svg(`<path d="M12 4v8"/><path d="M7.5 7a7 7 0 1 0 9 0"/>`, s),
  edge: (s) => svg(`<rect x="3" y="4" width="13" height="10" rx="1.6"/><path d="M16 9h5M18 6l3 3-3 3"/>`, s),
  // window controls
  wmin: (s) => `<svg viewBox="0 0 12 12" width="${s}" height="${s}"><path d="M2 6h8" stroke="currentColor" stroke-width="1.1"/></svg>`,
  wmax: (s) => `<svg viewBox="0 0 12 12" width="${s}" height="${s}" fill="none"><rect x="2.2" y="2.2" width="7.6" height="7.6" stroke="currentColor" stroke-width="1.1"/></svg>`,
  wclose: (s) => `<svg viewBox="0 0 12 12" width="${s}" height="${s}"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.1"/></svg>`,
};
