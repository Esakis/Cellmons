// Shared bounded protobuf codec. Metadata is generated from the official descriptors.
import { BinaryReader, BinaryWriter, WireType } from './binary';
export type Field = readonly [number, string, string, boolean, string];
export type Schema<T> = { name: string; _type?: T };
export function createWireCodec(metadata: unknown) {
  const messages = metadata as Record<string, Field[]>;
  function initial(name: string): Record<string, unknown> {
    const result: Record<string, unknown> = { $typeName: `game.v1.${name}` };
    for (const [, key, type, list, group] of messages[name] ?? []) {
      if (group) result[group] = { case: undefined };
      else if (list) result[key] = [];
      else if (type.length === 1)
        result[key] =
          type === 's'
            ? ''
            : type === 'y'
              ? new Uint8Array()
              : type === 'b'
                ? false
                : type === 'j' || type === 'k'
                  ? 0n
                  : 0;
    }
    return result;
  }
  function create<T>(schema: Schema<T>, value?: Partial<T>): T {
    return Object.assign(initial(schema.name), value) as T;
  }
  function writeScalar(writer: BinaryWriter, type: string, value: unknown) {
    switch (type) {
      case 'z':
        writer.sint32(value as number);
        break;
      case 'j':
        writer.int64(value as bigint);
        break;
      case 'k':
        writer.uint64(value as bigint);
        break;
      case 'i':
        writer.int32(value as number);
        break;
      case 'u':
        writer.uint32(value as number);
        break;
      case 'b':
        writer.bool(value as boolean);
        break;
      case 's':
        writer.string(value as string);
        break;
      case 'y':
        writer.bytes(value as Uint8Array);
        break;
      default:
        throw new Error('protobuf_schema');
    }
  }
  function encode(
    name: string,
    value: Record<string, unknown>,
    writer: BinaryWriter,
    depth: number,
  ) {
    if (depth > 16) throw new Error('protobuf_depth');
    for (const [number, key, type, list, group] of messages[name] ?? []) {
      const oneof = group
        ? (value[group] as { case?: string; value?: unknown } | undefined)
        : undefined;
      const data = group ? (oneof?.case === key ? oneof.value : undefined) : value[key];
      if (data === undefined || data === null) continue;
      const nested = type.length > 1;
      const packed = list && !nested && type !== 's' && type !== 'y';
      if (packed) {
        if (!(data as unknown[]).length) continue;
        writer.tag(number, WireType.LengthDelimited).fork();
        for (const item of data as unknown[]) writeScalar(writer, type, item);
        writer.join();
        continue;
      }
      for (const item of list ? (data as unknown[]) : [data]) {
        if (
          !list &&
          !group &&
          !nested &&
          (item === 0 ||
            item === 0n ||
            item === false ||
            item === '' ||
            (item instanceof Uint8Array && !item.length))
        )
          continue;
        writer.tag(
          number,
          nested || type === 's' || type === 'y' ? WireType.LengthDelimited : WireType.Varint,
        );
        if (nested) {
          writer.fork();
          encode(type, item as Record<string, unknown>, writer, depth + 1);
          writer.join();
        } else writeScalar(writer, type, item);
      }
    }
  }
  function readScalar(reader: BinaryReader, type: string): unknown {
    switch (type) {
      case 'z':
        return reader.sint32();
      case 'j':
        return reader.int64();
      case 'k':
        return reader.uint64();
      case 'i':
        return reader.int32();
      case 'u':
        return reader.uint32();
      case 'b':
        return reader.bool();
      case 's':
        return reader.string();
      case 'y':
        return reader.bytes();
      default:
        throw new Error('protobuf_schema');
    }
  }
  function decode(
    name: string,
    bytes: Uint8Array,
    depth: number,
    previous?: Record<string, unknown>,
  ): Record<string, unknown> {
    if (depth > 16) throw new Error('protobuf_depth');
    const reader = new BinaryReader(bytes);
    const value = previous ?? initial(name);
    while (reader.pos < reader.len) {
      const [number, wire] = reader.tag();
      const field = messages[name]?.find((f) => f[0] === number);
      if (!field) {
        if (wire === WireType.StartGroup || wire === WireType.EndGroup)
          throw new Error('protobuf_group');
        reader.skip(wire);
        continue;
      }
      const [, key, type, list, group] = field;
      const nested = type.length > 1;
      const scalarNumber = !nested && type !== 's' && type !== 'y';
      if (list && scalarNumber && wire === WireType.LengthDelimited) {
        const packed = new BinaryReader(reader.bytes());
        while (packed.pos < packed.len) (value[key] as unknown[]).push(readScalar(packed, type));
        continue;
      }
      if (wire !== (scalarNumber ? WireType.Varint : WireType.LengthDelimited))
        throw new Error('protobuf_wire_type');
      const old = group
        ? (value[group] as { case?: string; value?: Record<string, unknown> })
        : undefined;
      const data = nested
        ? decode(
            type,
            reader.bytes(),
            depth + 1,
            list
              ? undefined
              : group
                ? old?.case === key
                  ? old.value
                  : undefined
                : (value[key] as Record<string, unknown> | undefined),
          )
        : readScalar(reader, type);
      if (group) value[group] = { case: key, value: data };
      else if (list) (value[key] as unknown[]).push(data);
      else value[key] = data;
    }
    return value;
  }

  return {
    create,
    encode<T>(schema: Schema<T>, value: T): Uint8Array<ArrayBuffer> {
      const writer = new BinaryWriter();
      encode(schema.name, value as Record<string, unknown>, writer, 0);
      return writer.finish();
    },
    decode<T>(schema: Schema<T>, bytes: Uint8Array, limit = 256 * 1024): T {
      if (bytes.length > limit) throw new Error('protobuf_size');
      return decode(schema.name, bytes, 0) as T;
    },
  };
}
