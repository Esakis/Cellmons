import en from '../generated/paths.en.ftl?raw';
import data from '../generated/paths.json';
import pl from '../generated/paths.pl.ftl?raw';
import { addTranslations } from '../i18n';
import { registerPaths } from './content';

registerPaths(data);
addTranslations({ pl, en });
