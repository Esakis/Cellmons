// The two offline workflows share cards, moves, dialogs and collection data.
// Loading them together avoids repeating the runtime and compression dictionary.
import '../app/cardCatalog';

export { Collection } from './collection/Collection';
export { Duel } from './training/Training';
