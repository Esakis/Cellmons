import {
  create,
  type DescFile,
  type DescMessage,
  fromBinary,
  type Message,
  toBinary,
  toJson,
} from '@bufbuild/protobuf';
import { BinaryWriter, WireType } from '@bufbuild/protobuf/wire';
import { expect, it } from 'vitest';
import metadata from '../generated/online-wire.json';
import {
  type ClientMessage,
  ClientMessageSchema,
  file_game_v1_envelope,
  type ServerMessage,
  ServerMessageSchema,
} from '../generated/proto/game/v1/envelope_pb';
import { createWireCodec } from '../protobuf/codec';
import { decodeServer, encodeClient } from './wire';

let seed = 0x731b2e95;
function random() {
  seed ^= seed << 13;
  seed ^= seed >>> 17;
  seed ^= seed << 5;
  return seed >>> 0;
}
function sample(schema: DescMessage, iteration: number, depth = 0): Message {
  const result = create(schema);
  const object = result as unknown as Record<string, unknown>;
  const groups = new Map<string, number>();
  for (const field of schema.fields) {
    if (field.oneof) {
      const group = field.oneof;
      let index = groups.get(group.localName);
      if (index === undefined) {
        index = depth === 0 ? iteration % group.fields.length : random() % group.fields.length;
        groups.set(group.localName, index);
      }
      if (group.fields[index] !== field) continue;
    }
    const kind = field.fieldKind === 'list' ? field.listKind : field.fieldKind;
    if (kind === 'message' && depth >= 6) continue;
    const scalar = () => {
      if (kind === 'message') return sample(field.message as DescMessage, iteration, depth + 1);
      switch (field.scalar) {
        case 3:
          return [
            -9223372036854775808n,
            -1n,
            0n,
            1n,
            9223372036854775807n,
            BigInt.asIntN(64, (BigInt(random()) << 32n) | BigInt(random())),
          ][random() % 6];
        case 4:
          return [0n, 1n, 18446744073709551615n, (BigInt(random()) << 32n) | BigInt(random())][
            random() % 4
          ];
        case 5:
        case 17:
          return [-2147483648, -1, 0, 1, 2147483647, random() | 0][random() % 6];
        case 13:
          return [0, 1, 4294967295, random()][random() % 4];
        case 8:
          return Boolean(random() & 1);
        case 9:
          return random() % 3 ? `Zażółć 🧙 <script>text</script> ${random()}\n\0` : '';
        case 12:
          return random() % 3 ? new Uint8Array([random() & 255, 0, 255]) : new Uint8Array();
        default:
          throw new Error(`Unsupported sample field ${field.name}`);
      }
    };
    if (kind === 'message' && field.fieldKind !== 'list' && !field.oneof && random() % 4 === 0)
      continue;
    const value =
      field.fieldKind === 'list' ? Array.from({ length: random() % 4 }, scalar) : scalar();
    if (field.oneof) object[field.oneof.localName] = { case: field.localName, value };
    else object[field.localName] = value;
  }
  return result;
}
function descriptors() {
  const found = new Map<string, DescMessage>();
  const files = new Set<string>();
  function visit(file: DescFile) {
    if (files.has(file.name)) return;
    files.add(file.name);
    file.dependencies.forEach(visit);
    for (const m of file.messages) found.set(m.name, m);
  }
  visit(file_game_v1_envelope);
  return found;
}

it('matches the reference codec for every message, signed ratings and 64-bit limits', () => {
  const codec = createWireCodec(metadata);
  const all = descriptors();
  expect(Object.keys(metadata).sort()).toEqual([...all.keys()].sort());
  for (const schema of all.values())
    for (let i = 0; i < 8; i++) {
      const value = sample(schema, i);
      const bytes = toBinary(schema, value);
      expect(codec.encode({ name: schema.name }, value), schema.typeName).toEqual(bytes);
      expect(codec.decode({ name: schema.name }, bytes), schema.typeName).toEqual(
        fromBinary(schema, bytes),
      );
    }
});
it('covers every envelope variant with 1000 complete client/server exchanges', () => {
  for (let i = 0; i < 1000; i++) {
    const client = sample(ClientMessageSchema, i) as ClientMessage;
    expect(encodeClient(client)).toEqual(toBinary(ClientMessageSchema, client));
    const server = sample(ServerMessageSchema, i) as ServerMessage;
    const bytes = toBinary(ServerMessageSchema, server);
    expect(decodeServer(bytes)).toEqual(fromBinary(ServerMessageSchema, bytes));
  }
});
it('skips future fields and rejects groups, truncated values and oversized frames', () => {
  const source = create(ServerMessageSchema, { seq: 123 });
  const bytes = new BinaryWriter()
    .raw(toBinary(ServerMessageSchema, source))
    .tag(999, WireType.Varint)
    .uint64(18446744073709551615n)
    .finish();
  expect(toJson(ServerMessageSchema, decodeServer(bytes))).toEqual(
    toJson(ServerMessageSchema, fromBinary(ServerMessageSchema, bytes)),
  );
  expect(() =>
    decodeServer(
      new BinaryWriter().tag(999, WireType.StartGroup).tag(999, WireType.EndGroup).finish(),
    ),
  ).toThrow('protobuf_group');
  for (const bad of [[0], [8, 128], [26, 255]])
    expect(() => decodeServer(new Uint8Array(bad))).toThrow();
  expect(() => decodeServer(new Uint8Array(256 * 1024 + 1))).toThrow('protobuf_size');
});
