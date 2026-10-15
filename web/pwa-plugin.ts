import { createHash } from 'node:crypto';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Plugin } from 'vite';

export function offlinePlugin(): Plugin {
  let output = '';
  let android = false;
  return {
    name: 'offline-assets',
    apply: 'build',
    configResolved(config) {
      output = config.build.outDir;
      android = config.env.VITE_PLATFORM === 'android';
    },
    async closeBundle() {
      if (android) return;
      async function walk(directory: string): Promise<string[]> {
        const entries = await readdir(directory, { withFileTypes: true });
        const paths = await Promise.all(
          entries
            .filter((entry) => !entry.name.startsWith('.'))
            .map(async (entry) =>
              entry.isDirectory()
                ? walk(join(directory, entry.name))
                : [join(directory, entry.name)],
            ),
        );
        return paths.flat();
      }
      const files = (await walk(output))
        .filter((file) => !file.endsWith('sw.js') && !file.endsWith('.map'))
        .sort();
      const digest = createHash('sha256');
      for (const file of files) digest.update(await readFile(file));
      const assets = files.map(
        (file) => `./${file.slice(output.length + 1).replaceAll('\\', '/')}`,
      );
      const source = await readFile(new URL('./sw.template.js', import.meta.url), 'utf8');
      await writeFile(
        join(output, 'sw.js'),
        source
          .replace('__CACHE__', `game-${digest.digest('hex').slice(0, 20)}`)
          .replace('__ASSETS__', JSON.stringify(assets)),
      );
    },
  };
}
