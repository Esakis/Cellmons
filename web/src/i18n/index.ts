import { FluentBundle, FluentResource, type FluentVariable } from '@fluent/bundle';
import { createSignal } from 'solid-js';
import { readPreference, writePreference } from '../app/preferences';
import contentEn from '../generated/en.ftl?raw';
import contentPl from '../generated/pl.ftl?raw';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';

const bundles = {
  pl: new FluentBundle('pl', { useIsolating: false }),
  en: new FluentBundle('en', { useIsolating: false }),
};
for (const [locale, ui, content] of [
  ['pl', pl, contentPl],
  ['en', en, contentEn],
] as const) {
  for (const source of [ui, content]) {
    const errors = bundles[locale].addResource(new FluentResource(source));
    if (errors.length) throw new Error(`Invalid ${locale} translations: ${errors.join(', ')}`);
  }
}
const initial = readPreference('language', navigator.language.startsWith('pl') ? 'pl' : 'en');
export const [language, setLanguageSignal] = createSignal<'pl' | 'en'>(
  initial === 'pl' ? 'pl' : 'en',
);
export function setLanguage(value: string) {
  if (value !== 'pl' && value !== 'en') return;
  setLanguageSignal(value);
  document.documentElement.lang = value;
  writePreference('language', value);
}
export function t(key: string, args?: Record<string, FluentVariable>): string {
  const bundle = bundles[language()];
  const message = bundle.getMessage(key);
  if (!message?.value) return key;
  return bundle.formatPattern(message.value, args);
}
export const hasTranslation = (key: string) => bundles.en.hasMessage(key);

// Bots come from the server as "[AI] Circle Adept 12" (the English profile name and the
// level); players read the profile's name in their own language.
const botProfiles = ['novice', 'apprentice', 'tactician', 'archmage'];
export function playerName(login: string) {
  const bot = /^\[AI\] (.+) (\d+)$/.exec(login);
  if (!bot) return login;
  const key = botProfiles.find((name) => {
    const message = bundles.en.getMessage(name);
    return message?.value && bundles.en.formatPattern(message.value) === bot[1];
  });
  return key ? t('bot-name', { name: t(key), level: Number(bot[2]) }) : login;
}

export function addTranslations(resources: { pl: string; en: string }) {
  for (const locale of ['pl', 'en'] as const) {
    const errors = bundles[locale].addResource(new FluentResource(resources[locale]));
    if (errors.length) throw new Error(`Invalid ${locale} feature translations`);
  }
}
