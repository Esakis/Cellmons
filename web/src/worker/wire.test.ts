import {
  type DescMessage,
  fromBinary,
  type Message,
  create as referenceCreate,
  toBinary,
  toJson,
} from '@bufbuild/protobuf';
import { BinaryWriter, WireType } from '@bufbuild/protobuf/wire';
import { expect, it } from 'vitest';
import {
  file_game_v1_training,
  type TrainingReply,
  TrainingReplySchema,
  type TrainingRequest,
  TrainingRequestSchema,
} from '../generated/proto/game/v1/training_pb';
import metadata from '../generated/training-wire.json';
import { create, DuelMoveSchema, decodeReply, encodeRequest, TrainingSetupSchema } from './wire';

let seed = 0x92b714e5;
function random() {
  seed ^= seed << 13;
  seed ^= seed >>> 17;
  seed ^= seed << 5;
  return seed >>> 0;
}
function sample(schema: DescMessage, iteration: number, depth = 0): Message {
  const result = referenceCreate(schema);
  const object = result as unknown as Record<string, unknown>;
  for (const field of schema.fields) {
    if (field.oneof && field.oneof.fields[iteration % field.oneof.fields.length] !== field)
      continue;
    const kind = field.fieldKind === 'list' ? field.listKind : field.fieldKind;
    const scalar = () => {
      if (kind === 'message') return sample(field.message as DescMessage, iteration, depth + 1);
      switch (field.scalar) {
        case 5:
          return [-2147483648, -1, 0, 1, 2147483647, random() | 0][random() % 6];
        case 13:
          return [0, 1, 4294967295, random()][random() % 4];
        case 8:
          return Boolean(random() & 1);
        case 9:
          return random() % 3 ? `Zażółć 🧙 ${random()}\n` : '';
        case 12:
          return new Uint8Array([random() & 255, 0, 255]);
        default:
          throw new Error('Unsupported sample');
      }
    };
    const value =
      field.fieldKind === 'list' ? Array.from({ length: random() % 4 }, scalar) : scalar();
    if (field.oneof) object[field.oneof.localName] = { case: field.localName, value };
    else if (kind !== 'message' || depth < 5) object[field.localName] = value;
  }
  return result;
}
it('generates its compact metadata from every field of the real worker protobuf schema', () => {
  const scalar = new Map([
    [5, 'i'],
    [13, 'u'],
    [8, 'b'],
    [9, 's'],
    [12, 'y'],
  ]);
  expect(metadata).toEqual(
    Object.fromEntries(
      file_game_v1_training.messages.map((m) => [
        m.name,
        m.fields.map((f) => [
          f.number,
          f.localName,
          f.fieldKind === 'message' || (f.fieldKind === 'list' && f.listKind === 'message')
            ? f.message.name
            : scalar.get(f.scalar ?? -1),
          f.fieldKind === 'list',
          f.oneof?.localName ?? '',
        ]),
      ]),
    ),
  );
});
it('matches the reference encoder and decoder for 600 varied requests and replies', () => {
  for (let i = 0; i < 600; i++) {
    const request = sample(TrainingRequestSchema, i) as TrainingRequest;
    const encoded = encodeRequest(request);
    expect(encoded).toEqual(toBinary(TrainingRequestSchema, request));
    expect(fromBinary(TrainingRequestSchema, encoded)).toEqual(request);
    const reply = sample(TrainingReplySchema, i) as TrainingReply;
    const bytes = toBinary(TrainingReplySchema, reply);
    expect(decodeReply(bytes)).toEqual(fromBinary(TrainingReplySchema, bytes));
  }
  expect(create(DuelMoveSchema)).toEqual(
    referenceCreate(
      file_game_v1_training.messages.find((m) => m.name === 'DuelMove') as DescMessage,
    ),
  );
  expect(create(TrainingSetupSchema, { avatarId: 29 })).toEqual(
    referenceCreate(
      file_game_v1_training.messages.find((m) => m.name === 'TrainingSetup') as DescMessage,
      { avatarId: 29 },
    ),
  );
});
it('merges duplicate nested messages and accepts unpacked scalars while skipping unknown fields', () => {
  const w = new BinaryWriter();
  // Two TrainingView fragments: the same me message receives HP and then two unpacked stats.
  w.tag(3, WireType.LengthDelimited)
    .fork()
    .tag(4, WireType.LengthDelimited)
    .fork()
    .tag(1, WireType.Varint)
    .int32(-1)
    .join()
    .join();
  w.tag(3, WireType.LengthDelimited)
    .fork()
    .tag(4, WireType.LengthDelimited)
    .fork()
    .tag(5, WireType.Varint)
    .int32(-2147483648)
    .tag(5, WireType.Varint)
    .int32(2147483647)
    .join()
    .join();
  w.tag(123, WireType.LengthDelimited).string('future field');
  const bytes = w.finish();
  const actual = decodeReply(bytes);
  const reference = fromBinary(TrainingReplySchema, bytes);
  expect(toJson(TrainingReplySchema, actual)).toEqual(toJson(TrainingReplySchema, reference));
  expect(actual.view?.me?.hp).toBe(-1);
  expect(actual.view?.me?.stats).toEqual([-2147483648, 2147483647]);
});
it('rejects truncated, oversized and incorrectly typed worker replies', () => {
  for (const bytes of [[0], [8, 128], [26, 255], [26, 3, 8, 128], [10, 0]])
    expect(() => decodeReply(new Uint8Array(bytes))).toThrow();
  expect(() => decodeReply(new Uint8Array(128 * 1024 + 1))).toThrow('worker_size');
});
