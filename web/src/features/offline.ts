// The two offline workflows share cards, moves, dialogs and collection data.
// Loading them together avoids repeating the runtime and compression dictionary.
// The online duel takes the same duel board from here for that reason.
import '../app/cardCatalog';

export { Collection } from './collection/Collection';
export {
  boardStyles,
  CardDialog,
  ChronicleDialog,
  cardName,
  createSimulation,
  DecisionTable,
  errorText,
  eventText,
  loadPictures,
  quietEvents,
  type RoundRecord,
  SimulationBoard,
  useDuelKeys,
} from './training/Board';
export { Duel } from './training/Training';
