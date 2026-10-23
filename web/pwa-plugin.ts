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
      const manifest = JSON.parse(
        await readFile(join(output, '.vite/manifest.json'), 'utf8'),
      ) as Record<
        string,
        {
          file: string;
          imports?: string[];
          css?: string[];
          assets?: string[];
          isEntry?: boolean;
          isDynamicEntry?: boolean;
        }
      >;
      function graph(key: string, files = new Set<string>()): Set<string> {
        const entry = manifest[key];
        if (!entry || files.has(entry.file)) return files;
        files.add(entry.file);
        for (const file of [...(entry.css ?? []), ...(entry.assets ?? [])]) files.add(file);
        for (const imported of entry.imports ?? []) graph(imported, files);
        return files;
      }
      const onlineKeys = [
        'src/features/lobby/Lobby.tsx',
        'src/features/lobby/JourneyGate.tsx',
        'src/features/lobby/OnlineDuel.tsx',
        'src/features/lobby/OnlineRecords.tsx',
        'src/features/lobby/Trade.tsx',
        'src/features/account/Account.tsx',
        'src/features/privacy/Privacy.tsx',
        'src/features/legal/Legal.tsx',
        'src/features/legal/software.ts',
        'src/features/store/Store.tsx',
        'src/features/clans/Clans.tsx',
        'src/features/tournaments/Tournaments.tsx',
        'src/features/expeditions/Expeditions.tsx',
        'src/features/admin/Admin.tsx',
        'src/features/moderation/Report.tsx',
        'src/features/moderation/Notices.tsx',
        'src/app/CardCatalogView.tsx',
        'src/app/pathArt.ts',
        'src/app/pathCatalog.ts',
        'src/features/collection/Progression.tsx',
        'src/features/collection/Recruitment.tsx',
        'src/features/collection/Advancement.tsx',
      ];
      const offline = new Set<string>();
      for (const [key, entry] of Object.entries(manifest)) {
        if (!onlineKeys.includes(key) && (entry.isEntry || entry.isDynamicEntry))
          graph(key, offline);
      }
      const excluded = [...new Set(onlineKeys.flatMap((key) => [...graph(key)]))].filter(
        (file) => !offline.has(file),
      );
      excluded.push(...JSON.parse(await readFile(join(output, 'legal-assets.json'), 'utf8')));
      const content = JSON.parse(
        await readFile(new URL('./src/generated/content.json', import.meta.url), 'utf8'),
      ) as { version: string };
      const workspace = await readFile(new URL('../Cargo.toml', import.meta.url), 'utf8');
      const engine = workspace.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
      if (!engine) throw new Error('Missing engine version');
      await writeFile(
        join(output, '.vite/release.json'),
        JSON.stringify({
          release: process.env.GAME_RELEASE ?? 'development',
          engine,
          content: content.version,
        }),
      );
      await writeFile(
        join(output, 'online-assets.json'),
        JSON.stringify({ excluded, platform: android ? 'android' : 'web' }),
      );
      if (android) {
        // Capacitor's asset copier excludes hidden directories such as .vite.
        await writeFile(
          join(output, 'native-release.json'),
          JSON.stringify({
            release: process.env.GAME_RELEASE ?? 'development',
            engine,
            content: content.version,
            application_id: process.env.ANDROID_APPLICATION_ID ?? 'dev.game.training',
            version: process.env.ANDROID_VERSION ?? '1.0.0',
            web_bundle_version: Number(process.env.WEB_BUNDLE_VERSION ?? 0),
          }),
        );
        return;
      }
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
        .filter(
          (file) =>
            !file.endsWith('sw.js') &&
            !file.endsWith('.map') &&
            !file.endsWith('online-assets.json') &&
            !excluded.includes(file.slice(output.length + 1).replaceAll('\\', '/')),
        )
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
