export type Journey = {
  level: number;
  xp: number;
  next_xp: number | null;
  tutorial_step: number;
  recruits?: number;
  locations: Record<string, number>;
};

export function requiredLevel(journey: Journey | undefined, place: string): number {
  return journey?.locations[place.startsWith('clan:') ? 'clan' : place] ?? 30;
}
export function canEnter(journey: Journey | undefined, place: string): boolean {
  return Boolean(journey && journey.level >= requiredLevel(journey, place));
}
