import {
  BinaryReader as ReferenceReader,
  BinaryWriter as ReferenceWriter,
} from '@bufbuild/protobuf/wire';
import { expect, it } from 'vitest';
import { BinaryReader, BinaryWriter } from './binary';

it('matches reference primitives for signed, unsigned and zigzag boundary values', () => {
  let seed = 0x73bcef15;
  const next = () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return seed >>> 0;
  };
  const limits = [0, 1, 127, 128, 255, 16383, 16384, 2147483647, 2147483648, 4294967295];
  for (let i = 0; i < 2000; i++) {
    const unsigned = limits[i] ?? next();
    const signed = unsigned | 0;
    const wide =
      i < 2 ? ([0n, 0xffffffffffffffffn][i] as bigint) : (BigInt(next()) << 32n) | BigInt(next());
    const negative = BigInt.asIntN(64, wide);
    const make = <W extends BinaryWriter | ReferenceWriter>(w: W) =>
      w
        .uint32(unsigned)
        .int32(signed)
        .sint32(signed)
        .uint64(wide)
        .int64(negative)
        .bool(Boolean(i & 1))
        .finish();
    const bytes = make(new BinaryWriter());
    expect(bytes).toEqual(make(new ReferenceWriter()));
    const read = <R extends BinaryReader | ReferenceReader>(r: R) => [
      r.uint32(),
      r.int32(),
      r.sint32(),
      r.uint64(),
      r.int64(),
      r.bool(),
      r.pos,
    ];
    expect(read(new BinaryReader(bytes))).toEqual(read(new ReferenceReader(bytes)));
  }
});
it('preserves UTF-8 behavior and rejects invalid text, truncated fields and varint overflow', () => {
  for (const text of ['', 'Zażółć 🧙\n\0', '\uFEFFhello', 'Łańcuch\u0301']) {
    const bytes = new ReferenceWriter().string(text).finish();
    expect(new BinaryWriter().string(text).finish()).toEqual(bytes);
    expect(new BinaryReader(bytes).string()).toBe(new ReferenceReader(bytes).string());
  }
  for (const bytes of [
    [1, 255],
    [2, 194],
    [2, 192, 128],
    [1, 128],
  ])
    expect(() => new BinaryReader(new Uint8Array(bytes)).string()).toThrow();
  for (let length = 0; length < 10; length++)
    expect(() => new BinaryReader(new Uint8Array(length).fill(128)).uint64()).toThrow();
  const overflow = new Uint8Array(10).fill(255);
  expect(() => new BinaryReader(overflow).uint64()).toThrow('protobuf_varint');
  expect(() => new BinaryReader(overflow).uint32()).toThrow('protobuf_varint');
  // A noncanonical but valid ten-byte varint is still accepted.
  const zero = new Uint8Array(10).fill(128);
  zero[9] = 0;
  expect(new BinaryReader(zero).uint64()).toBe(0n);
  // Bool is true for any nonzero 64-bit value, including ones outside uint32.
  expect(new BinaryReader(new ReferenceWriter().uint64(1n << 40n).finish()).bool()).toBe(true);
});
it('skips unknown fixed fields without reading beyond the frame', () => {
  for (const [wire, size] of [
    [1, 8],
    [5, 4],
  ] as const) {
    const bytes = new Uint8Array(size + 1);
    bytes[size] = 9;
    const reader = new BinaryReader(bytes);
    reader.skip(wire);
    expect(reader.uint32()).toBe(9);
    expect(() => new BinaryReader(bytes.subarray(0, size - 1)).skip(wire)).toThrow();
  }
  for (const value of [-1, 4294967296, Number.NaN, 1.5])
    expect(() => new BinaryWriter().uint32(value)).toThrow();
  for (const value of [-2147483649, 2147483648, Number.NaN])
    expect(() => new BinaryWriter().int32(value)).toThrow();
  expect(() => new BinaryWriter().uint64(-1n)).toThrow();
  expect(() => new BinaryWriter().uint64(1n << 64n)).toThrow();
  expect(() => new BinaryWriter().int64(1n << 63n)).toThrow();
});
