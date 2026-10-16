import metadata from '../generated/online-wire.json';
import type { ClientMessage, ServerMessage } from '../generated/proto/game/v1/envelope_pb';
import { createWireCodec, type Schema } from '../protobuf/codec';

const codec = createWireCodec(metadata);
const ClientSchema: Schema<ClientMessage> = { name: 'ClientMessage' };
const ServerSchema: Schema<ServerMessage> = { name: 'ServerMessage' };
export const encodeClient = (message: ClientMessage) => codec.encode(ClientSchema, message);
export const decodeServer = (bytes: Uint8Array) => codec.decode(ServerSchema, bytes);
