import type {
  DuelMove,
  TrainingForfeit,
  TrainingReply,
  TrainingRequest,
  TrainingSetup,
} from '../generated/proto/game/v1/training_pb';
import raw from '../generated/training-wire.json';
import { createWireCodec, type Schema } from '../protobuf/codec';

const codec = createWireCodec(raw);
export const DuelMoveSchema: Schema<DuelMove> = { name: 'DuelMove' };
export const TrainingSetupSchema: Schema<TrainingSetup> = { name: 'TrainingSetup' };
export const TrainingForfeitSchema: Schema<TrainingForfeit> = { name: 'TrainingForfeit' };
const RequestSchema: Schema<TrainingRequest> = { name: 'TrainingRequest' };
const ReplySchema: Schema<TrainingReply> = { name: 'TrainingReply' };
export const create = codec.create;
export const encodeRequest = (value: TrainingRequest) => codec.encode(RequestSchema, value);
export function decodeReply(bytes: Uint8Array): TrainingReply {
  if (bytes.length > 128 * 1024) throw new Error('worker_size');
  return codec.decode(ReplySchema, bytes);
}
