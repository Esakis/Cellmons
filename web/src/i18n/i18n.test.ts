import { FluentBundle, FluentResource } from '@fluent/bundle';
import { describe, expect, it } from 'vitest';
import { cardArgs, content, fluentKey } from '../app/content';
import enContent from '../generated/cards.en.ftl?raw';
import plContent from '../generated/cards.pl.ftl?raw';
import enBase from '../generated/en.ftl?raw';
import plBase from '../generated/pl.ftl?raw';
import '../app/cardCatalog';
import enUi from './en.ftl?raw';
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
