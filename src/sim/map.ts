// The tile map. Terrain never changes during a game; resource amounts do, so they live in the game state.

export const Terrain = { open: 0, rock: 1, cliff: 2 } as const;
export type TerrainKind = (typeof Terrain)[keyof typeof Terrain];

export interface MapData {
  width: number;
  height: number;
  terrain: TerrainKind[];
  resource: number[];        // amount left on each tile
  start: { x: number; y: number }[];
}

export const TILE = 256;     // sub-tile units per tile; positions are integers in these units
export const RESOURCE_PER_TILE = 300;

/**
 * Parse an ASCII map:
 *   .  open ground   #  rock (buildable)   X  cliff (impassable)
 *   ~  open ground with resource           1-8  player start (on rock)
 */
export function parseMap(text: string): MapData {
  const rows = text.split("\n").map((r) => r.trimEnd()).filter((r) => r.length > 0 && !r.startsWith(";"));
  const height = rows.length;
  const width = Math.max(...rows.map((r) => r.length));
  const terrain: TerrainKind[] = [];
  const resource: number[] = [];
  const start: { x: number; y: number }[] = [];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const c = rows[y][x] ?? ".";
      if (c === "#") terrain.push(Terrain.rock);
      else if (c === "X") terrain.push(Terrain.cliff);
      else if (c >= "1" && c <= "8") {
        terrain.push(Terrain.rock);
        start[Number(c) - 1] = { x, y };
      } else if (c === "." || c === "~") terrain.push(Terrain.open);
      else throw new Error(`map: unknown tile '${c}' at ${x},${y}`);
      resource.push(c === "~" ? RESOURCE_PER_TILE : 0);
    }
  }
  return { width, height, terrain, resource, start };
}

export function inBounds(map: { width: number; height: number }, x: number, y: number): boolean {
  return x >= 0 && y >= 0 && x < map.width && y < map.height;
}

export function passable(map: MapData, x: number, y: number): boolean {
  return inBounds(map, x, y) && map.terrain[y * map.width + x] !== Terrain.cliff;
}
