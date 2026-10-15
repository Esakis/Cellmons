export type ChatCommand =
  | { kind: 'send'; channel: 'room' | 'private' | 'clan'; to: string; text: string }
  | { kind: 'social'; relation: 'friend' | 'ignore'; add: boolean; player: string }
  | { kind: 'list' }
  | { kind: 'afk' }
  | { kind: 'who'; player: string }
  | { kind: 'help' };
export function parseCommand(input: string, lastPrivate: string): ChatCommand {
  const text = input.trim();
  if (!text) throw new Error('INVALID_MESSAGE');
  if (!text.startsWith('/')) return { kind: 'send', channel: 'room', to: '', text };
  const parts = /^\/(\S+)(?:\s+([\s\S]*))?$/.exec(text);
  const command = parts?.[1]?.toLowerCase();
  const args = parts?.[2]?.trim() ?? '';
  if (command === 't') {
    const match = /^([A-Za-z0-9_-]{3,20})\s*,\s*([\s\S]+)$/.exec(args);
    if (match?.[1] && match[2])
      return { kind: 'send', channel: 'private', to: match[1], text: match[2] };
  }
  if (command === 'r' && lastPrivate && args)
    return { kind: 'send', channel: 'private', to: lastPrivate, text: args };
  if (command === 'c' && args) return { kind: 'send', channel: 'clan', to: '', text: args };
  if (command === 'f' || command === 'i') {
    const match = /^(add|del)\s+([A-Za-z0-9_-]{3,20})$/i.exec(args);
    if (match?.[1] && match[2])
      return {
        kind: 'social',
        relation: command === 'f' ? 'friend' : 'ignore',
        add: match[1].toLowerCase() === 'add',
        player: match[2],
      };
    if (/^list(?:\s+all)?$/i.test(args)) return { kind: 'list' };
  }
  if (command === 'afk' && !args) return { kind: 'afk' };
  if (command === 'w' && /^[A-Za-z0-9_-]{3,20}$/.test(args)) return { kind: 'who', player: args };
  if (command === 'help' && !args) return { kind: 'help' };
  throw new Error('INVALID_COMMAND');
}
