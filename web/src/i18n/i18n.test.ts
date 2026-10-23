import { FluentBundle, FluentResource } from '@fluent/bundle';
import { describe, expect, it } from 'vitest';
import { cardArgs, content, fluentKey } from '../app/content';
import enContent from '../generated/cards.en.ftl?raw';
import plContent from '../generated/cards.pl.ftl?raw';
import enBase from '../generated/en.ftl?raw';
import plBase from '../generated/pl.ftl?raw';
import '../app/cardCatalog';
import enUi from './en.ftl?raw';
import { playerName, setLanguage } from './index';
import plUi from './pl.ftl?raw';

describe('translations', () => {
  for (const [locale, source, ui] of [
    ['pl', plContent + plBase, plUi],
    ['en', enContent + enBase, enUi],
  ] as const) {
    it(`formats every card without unresolved Fluent variables (${locale})`, () => {
      const bundle = new FluentBundle(locale);
      expect(bundle.addResource(new FluentResource(`${source}\n${ui}`))).toEqual([]);
      expect(content.cards).toHaveLength(330);
      for (const card of content.cards) {
        const message = bundle.getMessage(fluentKey(card.key, 'text'));
        expect(message?.value).toBeTruthy();
        if (!message?.value) continue;
        const errors: Error[] = [];
        bundle.formatPattern(message.value, cardArgs(card), errors);
        expect(errors, card.key).toEqual([]);
      }
    });
  }
});

it('names bots in the player language and leaves people as they are', () => {
  setLanguage('pl');
  expect(playerName('[AI] Circle Adept 12')).toBe('Adept Kręgu · poz. 12');
  setLanguage('en');
  expect(playerName('[AI] Battlemaster 3')).toBe('Battlemaster · lv. 3');
  expect(playerName('[AI] Unknown 3')).toBe('[AI] Unknown 3');
  expect(playerName('Alice_7')).toBe('Alice_7');
});
