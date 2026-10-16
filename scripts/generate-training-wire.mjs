// Compact metadata for worker and WebSocket codecs. The full runtime is the test oracle.
import { writeFile } from 'node:fs/promises';
import { registerHooks } from 'node:module';

// Buf emits extensionless imports for the bundler; this Node-only build step resolves those TS files.
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      specifier.startsWith('./') &&
      context.parentURL?.includes('/generated/proto/') &&
      !specifier.endsWith('.ts')
    )
      return { url: new URL(`${specifier}.ts`, context.parentURL).href, shortCircuit: true };
    return nextResolve(specifier, context);
  },
});
const { file_game_v1_envelope } = await import('../web/src/generated/proto/game/v1/envelope_pb.ts');
const { file_game_v1_training } = await import('../web/src/generated/proto/game/v1/training_pb.ts');

const scalar = new Map([
  [5, 'i'],
  [13, 'u'],
  [8, 'b'],
  [9, 's'],
  [12, 'y'],
  [3, 'j'],
  [4, 'k'],
  [17, 'z'],
]);
function catalogue(root) {
  const result = {};
  const seen = new Set();
  function visit(file) {
    if (seen.has(file.name)) return;
    seen.add(file.name);
    for (const dependency of file.dependencies) visit(dependency);
    for (const message of file.messages) {
      result[message.name] = message.fields.map((field) => {
        const kind = field.fieldKind === 'list' ? field.listKind : field.fieldKind;
        const type =
          kind === 'message'
            ? field.message.name
            : kind === 'enum'
              ? 'i'
              : scalar.get(field.scalar);
        if (!type || !['message', 'scalar', 'enum'].includes(kind))
          throw new Error(`Unsupported worker field ${field.name}`);
        return [
          field.number,
          field.localName,
          type,
          field.fieldKind === 'list',
          field.oneof?.localName ?? '',
        ];
      });
    }
  }
  visit(root);
  return result;
}
await writeFile(
  new URL('../web/src/generated/training-wire.json', import.meta.url),
  `${JSON.stringify(catalogue(file_game_v1_training))}\n`,
);
await writeFile(
  new URL('../web/src/generated/online-wire.json', import.meta.url),
  `${JSON.stringify(catalogue(file_game_v1_envelope))}\n`,
);
