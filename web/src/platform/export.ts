import { registerPlugin } from '@capacitor/core';

const exporter = registerPlugin<{ save(input: { session: string }): Promise<{ saved: boolean }> }>(
  'AccountExport',
);
export const saveExport = (session: string) => exporter.save({ session });
