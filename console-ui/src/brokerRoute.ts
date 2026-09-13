const parameters = new URLSearchParams(window.location.search);
export const fleetDevice = parameters.get('device');
export const brokerBase = fleetDevice && /^[0-9a-f-]{36}$/.test(fleetDevice)
  ? `/api/v1/devices/${fleetDevice}/broker` : '/api';
const view = parameters.get('view') === 'terminal' ? 'terminal' : 'console';
export const brokerHome = (fleetDevice ? `/?device=${encodeURIComponent(fleetDevice)}&managed` : '/?managed') + `&view=${view}`;
export function desktopURL(id: string, kind = view) { const url = new URL(brokerHome,location.origin); url.searchParams.set('view',kind); url.searchParams.set('desktop',id); return url.pathname + url.search; }
