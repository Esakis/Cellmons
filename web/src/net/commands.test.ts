import { describe, expect, it } from 'vitest';
import { parseCommand } from './commands';

describe('chat commands', () => {
  it('keeps plain text literal and parses private, reply and clan channels', () => {
    expect(parseCommand('<script>hello</script>', '')).toMatchObject({
      channel: 'room',
      text: '<script>hello</script>',
    });
    expect(parseCommand('/t Player_1, hello, again', '')).toMatchObject({
      channel: 'private',
      to: 'Player_1',
      text: 'hello, again',
    });
    expect(parseCommand('/r hello', 'LastOne')).toMatchObject({
      channel: 'private',
      to: 'LastOne',
    });
    expect(parseCommand('/c hello', '')).toMatchObject({ channel: 'clan' });
  });
  it('validates contacts and incomplete or unknown commands', () => {
    expect(parseCommand('/f add Player_1', '')).toEqual({
      kind: 'social',
      relation: 'friend',
      add: true,
      player: 'Player_1',
    });
    expect(parseCommand('/i del Player_1', '')).toMatchObject({ relation: 'ignore', add: false });
    for (const input of [
      '/r no conversation',
      '/t bad user, hello',
      '/unknown',
      '/afk extra',
      '/f add',
    ])
      expect(() => parseCommand(input, '')).toThrow('INVALID_COMMAND');
  });
});
