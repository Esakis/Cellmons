import en from '../generated/cards.en.ftl?raw';
import data from '../generated/cards.json';
import pl from '../generated/cards.pl.ftl?raw';
import { addTranslations } from '../i18n';
import { registerCards } from './content';

registerCards(data);
addTranslations({ pl, en });
