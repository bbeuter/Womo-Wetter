import type { AppData, BerlinNow } from './data';
import type { SunMap } from './map';

export interface State {
  data: AppData;
  map: SunMap;
  now: BerlinNow;
  nowIdx: number;          // Index der aktuellen Stunde in latest.hours
  idx: number;             // gewählte Stunde
  region: string | null;   // geöffnete Region
  listMode: 'heute' | 'we';
}
export interface Ctx {
  st: State;
  setIdx(idx: number): void;
  open(id: string | null): void;
  renderAll(): void;
}
