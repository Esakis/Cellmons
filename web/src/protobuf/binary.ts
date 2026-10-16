// Only the proto3 wire types used by our generated schemas. Unknown fixed fields can be skipped.
// Cross-checked against the official protobuf implementation in wire.test.ts.
export const WireType = { Varint: 0, LengthDelimited: 2, StartGroup: 3, EndGroup: 4 } as const;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });

export class BinaryWriter {
  private data: number[] = [];
  private stack: number[][] = [];
  tag(field: number, wire: number) {
    return this.uint32(field * 8 + wire);
  }
  uint32(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffffffff)
      throw new Error('protobuf_uint32');
    while (value > 127) {
      this.data.push((value & 127) | 128);
      value >>>= 7;
    }
    this.data.push(value);
    return this;
  }
  uint64(value: bigint) {
    if (value < 0n || value > 0xffffffffffffffffn) throw new Error('protobuf_uint64');
    while (value > 127n) {
      this.data.push(Number(value & 127n) | 128);
      value >>= 7n;
    }
    this.data.push(Number(value));
    return this;
  }
  int64(value: bigint) {
    if (value < -0x8000000000000000n || value > 0x7fffffffffffffffn)
      throw new Error('protobuf_int64');
    return this.uint64(BigInt.asUintN(64, value));
  }
  int32(value: number) {
    if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff)
      throw new Error('protobuf_int32');
    return value < 0 ? this.int64(BigInt(value)) : this.uint32(value);
  }
  sint32(value: number) {
    if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff)
      throw new Error('protobuf_int32');
    return this.uint32(((value << 1) ^ (value >> 31)) >>> 0);
  }
  bool(value: boolean) {
    return this.uint32(value ? 1 : 0);
  }
  string(value: string) {
    return this.bytes(encoder.encode(value));
  }
  bytes(value: Uint8Array) {
    this.uint32(value.length);
    for (const byte of value) this.data.push(byte);
    return this;
  }
  fork() {
    this.stack.push(this.data);
    this.data = [];
    return this;
  }
  join() {
    const nested = this.data;
    const parent = this.stack.pop();
    if (!parent) throw new Error('protobuf_writer');
    this.data = parent;
    this.uint32(nested.length);
    for (const byte of nested) this.data.push(byte);
    return this;
  }
  finish(): Uint8Array<ArrayBuffer> {
    if (this.stack.length) throw new Error('protobuf_writer');
    return new Uint8Array(this.data);
  }
}

export class BinaryReader {
  pos = 0;
  readonly len: number;
  constructor(private data: Uint8Array) {
    this.len = data.length;
  }
  private byte() {
    const byte = this.data[this.pos++];
    if (byte === undefined) throw new Error('protobuf_truncated');
    return byte;
  }
  uint32() {
    let value = 0;
    for (let i = 0; i < 10; i++) {
      const byte = this.byte();
      if (i < 5) value |= (byte & 127) << (i * 7);
      if (i === 9 && byte > 1) throw new Error('protobuf_varint');
      if (!(byte & 128)) return value >>> 0;
    }
    throw new Error('protobuf_varint');
  }
  uint64() {
    let value = 0n;
    for (let i = 0; i < 10; i++) {
      const byte = this.byte();
      if (i === 9 && byte > 1) throw new Error('protobuf_varint');
      value |= BigInt(byte & 127) << BigInt(i * 7);
      if (!(byte & 128)) return value;
    }
    throw new Error('protobuf_varint');
  }
  int64() {
    return BigInt.asIntN(64, this.uint64());
  }
  int32() {
    return this.uint32() | 0;
  }
  sint32() {
    const value = this.uint32();
    return (value >>> 1) ^ -(value & 1);
  }
  bool() {
    return this.uint64() !== 0n;
  }
  private take(length: number) {
    const end = this.pos + length;
    if (end > this.len) throw new Error('protobuf_truncated');
    const bytes = this.data.subarray(this.pos, end);
    this.pos = end;
    return bytes;
  }
  bytes() {
    return this.take(this.uint32());
  }
  string() {
    return decoder.decode(this.bytes());
  }
  tag(): [number, number] {
    const tag = this.uint32();
    if (!(tag >>> 3) || (tag & 7) > 5) throw new Error('protobuf_tag');
    return [tag >>> 3, tag & 7];
  }
  skip(wire: number) {
    if (wire === 0) this.uint64();
    else if (wire === 1) this.take(8);
    else if (wire === 2) this.bytes();
    else if (wire === 5) this.take(4);
    else throw new Error('protobuf_wire_type');
  }
}
