# 06. Silnik gry

Silnik to biblioteka Rust, która implementuje zasady z `02-zasady-gry.md`. Używa jej serwer (arbiter), przeglądarka przez WebAssembly (trening, podglądy, powtórki), symulator balansu, AI i boty testów obciążeniowych. Uzasadnienie: ADR-001, ADR-006, ADR-007.

> Fragmenty kodu poniżej to **szkice projektowe** (sygnatury i pseudokod), nie implementacja. Ostateczne nazwy ustala implementacja w fazach F1–F2.

## 1. Zasady projektowe

1. **Czystość:** brak I/O, sieci, plików, czasu systemowego i wątków. Wszystkie wejścia są jawne (stan, ruchy, treść, ziarno).
2. **Determinizm:** te same wejścia dają bit w bit ten sam wynik, natywnie i w WASM. Tylko liczby całkowite. Brak zależności od kolejności iteracji map haszujących (`HashMap`/`HashSet` są zakazane lintem). Losowość tylko z jawnych strumieni (rozdz. 7).
3. **Brak paniki** na poprawnych danych. Niepoprawne dane wejściowe dają `Result::Err`. Kod używa `#![forbid(unsafe_code)]`.
4. **Brak alokacji w gorącej ścieżce:** `resolve_round` nie alokuje (bufory o stałej pojemności, `ArrayVec`), żeby AI mogło wykonywać tysiące symulacji na decyzję.
5. **Śledzalność:** każda funkcja reguły ma w komentarzu identyfikator `R-…` ze specyfikacji, a testy odwołują się do tych identyfikatorów.

## 2. Podział na crate'y

| Crate | Zawartość | Zależności |
|---|---|---|
| `engine` | typy, konfiguracja zasad, stan pojedynku, potok rundy, kontekst efektów, rodziny efektów, powtórki | `rand_chacha`, `blake3`, `arrayvec` |
| `engine-cards` | efekty `custom` (funkcje Rust) i rejestr implementacji | `engine` |
| `engine-content` | tablice treści wygenerowane przez `content-compiler` (karty, postacie, zasady) | `engine` |
| `engine-ai` | boty L0–L3 (`11-ai-i-symulacje.md`) | `engine`, `engine-content` |
| `engine-wasm` | eksport funkcji dla przeglądarki (`wasm-bindgen`) | wszystkie powyższe |

## 3. Model danych (szkic)

```rust
enum Side { A, B }                 // A = gracz 1, B = gracz 2
enum Kind { Phys, Mag }            // typ ataku / obrony
enum School { Combat, Divine, Element, Arcane, Necro, Tech }
struct Schools([u8; 6]);           // poziom 0..=4 dla każdej szkoły (R-ENT-03)

struct Stats {                     // statystyki efektywne (R-ENT-06)
    phys_atk: i32, phys_def: i32, mag_atk: i32, mag_def: i32,
    hp_max: i32, stamina_max: i32,
}

struct AvatarSnapshot {            // R-POJ-01
    template: AvatarId, level: u8, tier: u8,  // tier 0..=3, cień jako flaga
    is_shade: bool,
    stats: Stats, schools: Schools,
    specialization: Option<Specialization>,
    gift: Option<GiftId>,
}

struct DuelSetup {
    rules: RulesConfig,            // z content/rules.toml (02-zasady-gry.md, rozdz. 13)
    content_version: [u8; 32],
    seed: [u8; 32],
    players: [PlayerSetup; 2],
}
struct PlayerSetup { avatar: AvatarSnapshot, deck: ArrayVec<CardId, 20> }

struct DuelState {
    round: u8,                      // 1..=5
    phase: Phase,                   // AwaitingMoves | Finished(Outcome)
    initiative: Side,
    players: [PlayerState; 2],
    enchantments: ArrayVec<ActiveEnchantment, 32>,
    used_interventions: [BitSet<CardId>; 2],
    rng: RngStreams,
    cast_counter: u16,
}

struct PlayerState {
    base: Stats,                    // ze snapshotu
    permanent: StatDelta,           // trwałe zmiany z czarów zwykłych (R-CZA-01)
    hp: i32, hp_start: i32, stamina: i32, focus: i32,
    resources: [i32; 2],            // punkt rozszerzenia (np. przyszły „Eter”); dziś nieużywane
    schools: Schools, school_delta: [i8; 6],
    attacks: [Kind; 3], defenses: [Kind; 3],   // ustawienia z poprzedniej rundy
    hand: ArrayVec<CardId, 5>,
    deck: ArrayVec<CardId, 20>,     // pozostałe karty, wierzch na końcu
    flags: PlayerFlags,             // np. podwójne obrażenia, pierwsze trafienie F chybia
}

enum CardAction { None, Cast(u8), Discard(u8) }   // indeks w ręce
struct Move { attacks: [Kind; 3], defenses: [Kind; 3], card: CardAction }

enum Outcome { Win(Side, Reason), Draw(Reason) }
enum Reason { Knockout, Rounds, Forfeit, Disconnect, Timeout, ConditionalWin }
```

## 4. Publiczne API

Stan po fazie F1 (`crates/engine/src/lib.rs`). Treść gry (`Content`) jest jawnym parametrem, bo silnik nie przechowuje definicji kart w stanie. Niepoprawne wejście daje błąd, a stan się wtedy nie zmienia.

```rust
fn new_duel(setup: &DuelSetup, content: &Content) -> Result<DuelState, SetupError>;
fn validate_move(s: &DuelState, content: &Content, side: Side, mv: &Move)
    -> Result<MoveCost, MoveError>;                                // R-RUN-06
fn preview(s: &DuelState, content: &Content, side: Side, mv: &Move) -> Preview;
                                    // koszty, K po ruchu, maks. zużycie W, wynik walidacji
fn default_move(s: &DuelState, side: Side) -> Move;             // „bez zmian” (R-RUN-09)
fn resolve_round(s: &mut DuelState, content: &Content, moves: &[Move; 2], out: &mut EventBuf)
    -> Result<RoundSummary, DuelError>;
fn end_by(s: &mut DuelState, loser: Side, t: Termination, out: &mut EventBuf)
    -> Result<Outcome, DuelError>;                               // ucieczka, rozłączenie, walkower
fn state_hash(s: &DuelState) -> u64;                            // do porównań natywne/WASM i testów
fn replay(r: &ReplayRecord, content: &Content, out: &mut EventBuf)
    -> Result<DuelState, ReplayError>;
fn view(s: &DuelState, side: Side) -> PlayerView;               // faza F5: stan widoczny dla gracza (rozdz. 8)
```

Kody `MoveError` (przekazywane do protokołu): `NotEnoughFocus`, `NotEnoughStamina`, `RequirementsNotMet`, `InterventionUsed`, `InvalidHandIndex`, `WrongPhase` oraz `UnknownCard` (karta z ręki nie istnieje w przekazanej treści, czyli błąd wywołującego). Kod `AlreadySubmitted` (ruch już wysłany) zwraca serwer, nie silnik (faza F7).

## 5. Potok rundy

### 5.1. Przebieg (pseudokod)

```
resolve_round(state, moves):
  require phase == AwaitingMoves
  for side in [A, B]:
      cost = change_cost(state.players[side], moves[side])          # R-RUN-05
      state.players[side].focus -= cost
      state.players[side].attacks/defenses = moves[side].settings
  emit MovesRevealed(moves)                                          # R-RUN-10

  # Faza czarów (R-RUN-11)
  for ench in state.enchantments ordered by cast_order:              # R-CZA-02
      ench.on_round(ctx)
  countered = resolve_counters(state, moves)                         # R-CZA-05
  for side in initiative_order(state):                               # R-POJ-05
      match moves[side].card:
          Cast(i)    -> if side in countered: pay_and_fizzle(side, i)
                        else cast(side, i)                           # R-RUN-12
          Discard(i) -> discard(side, i)
          None       -> ()

  # Faza ataków (R-ATK-01..07)
  combo = [0, 0]                       # seria trafień istnieje tylko w fazie ataków (R-RUN-03)
  for slot in 0..3:
      for side in initiative_order(state):
          attack(state, side, slot, combo[side])

  # Koniec rundy
  for ench in enchantments: ench.on_round_end(ctx)                   # R-RUN-13
  draw_replacements(state)                                           # R-RUN-14
  outcome = check_conditional_wins(state)                            # R-KON-06
         or check_knockout(state)                                    # R-KON-01
         or (round == rules.rounds ? compare_hp_percent(state) : none)  # R-KON-02/03
  if outcome: phase = Finished(outcome); emit DuelEnded
  else: round += 1; start_round(state)                               # +K (R-RUN-01), emit RoundStarted
```

### 5.2. Atak

```
attack(state, a, slot, combo):
  d = other(a)
  kind = players[a].attacks[slot]
  ov = collect_overrides(state, a, d, slot, kind)    # efekty: ForceMiss / ForceHit / Unblockable
  hit = match ov:
      ForceMiss   -> false                           # Z-ENG-1: chybienie wymuszone ma pierwszeństwo
      ForceHit    -> true
      Unblockable -> true
      Normal      -> players[d].stamina < rules.block_min_stamina       # R-ATK-03
                     or players[d].defenses[slot] != kind               # R-ATK-02
  if hit:
      combo += 1
      mult = rules.combo_multipliers[min(combo, len) - 1]               # R-ATK-04
      atk  = eff_atk(a, kind); def = eff_def(d, kind)
      dmg  = max(0, atk * mult - def)
      dmg  = apply_damage_modifiers(d, dmg)          # np. podwójne obrażenia
      players[d].hp -= dmg
      players[a].stamina = max(0, stamina - atk)                        # R-ATK-04
      emit AttackHit{a, slot, kind, mult, dmg}
      triggers: on_hit(a), on_damage_taken(d, source=Attack)
  else:
      combo = 0                                                         # R-ATK-05
      emit AttackBlocked{a, slot, kind}
      triggers: on_blocked(d)
```

**Decyzje silnika** (uzupełnienie specyfikacji, do zatwierdzenia przy F1):
- **Z-ENG-1:** wymuszone chybienie (efekt obrońcy) ma pierwszeństwo przed wymuszonym trafieniem (efekt atakującego).
- **Z-ENG-2:** statystyki efektywne nie spadają poniżej 0. Ujemne modyfikatory mogą obniżyć AF, OF, AM i OM najwyżej do 0, a Ż_max i W_max najwyżej do 1.
- **Z-ENG-3:** leczenie nie podnosi Ż ponad `hp_max` (zgodne z walką #26519), chyba że efekt jawnie zwiększa `hp_max`.
- **Z-ENG-4:** limit K (`duel.focus_cap`, w zasadach klasycznych brak limitu) ogranicza tylko przyrost z R-RUN-01. K zdobyta ponad limitem, np. z efektu karty, zostaje.

**Decyzje silnika z fazy F2** (zatwierdzone przez właściciela 2026-10-15):
- **Z-ENG-5:** efekt przywracający W (rodzina `stamina`, `drain` z puli W) nie podnosi W ponad W_max, tak jak leczenie przy Z-ENG-3. W nie spada poniżej 0.
- **Z-ENG-6:** efekty wykonane przez umagicznienie nie wyzwalają kolejnych umagicznień (np. obrażenia z umagicznienia nie uruchamiają `OnDamageTaken`). Wyklucza to nieskończone łańcuchy i uzależnienie wyniku od kolejności rejestracji.
- Dar (R-POJ-07, Z-GFT) jest zwykłym umagicznieniem zarejestrowanym przed 1. rundą, więc można go rozproszyć.
- Czas trwania `WhileActive` (modyfikator działa, dopóki umagicznienie jest aktywne) ma sens tylko w umagicznieniach. W czarze natychmiastowym taki efekt nic nie robi.
- Sprzężenie (R-CZA-06) mnoży wartości efektów `StatMod`, `Heal`, `Damage`, `Stamina`, `Focus` i `Drain` przez poziom szkoły rzucającego, gdy ten poziom sięga progu sprzężenia. Pozostałe rodziny działają bez zmian.
- `SwapChoices` zamienia ataki z obronami po ujawnieniu ruchów. Zamienione ustawienia są punktem odniesienia przy liczeniu kosztu zmian w następnej rundzie (R-RUN-05).

## 6. System efektów

### 6.1. Definicja karty w czasie wykonania

```rust
struct CardDef {
    id: CardId, rarity: Rarity,
    requires: Schools,               // wymagania (R-ENT-07)
    cost: Cost { focus: u8, stamina: u16 },
    kind: CardKind,                  // Instant | Enchantment
    keywords: Keywords,              // Intervention | Counter | Coupling{school, level} | Random
    effect: EffectSpec,
}
enum EffectSpec { Family(FamilyEffect), Custom { id: CustomId, params: [i32; 4] } }
```

`FamilyEffect` to enum odpowiadający rodzinom z `03-tresc-gry.md`, rozdz. 5.3 (`StatMod`, `StatSet`, `SwapStats`, `Heal`, `Damage`, `Stamina`, `Focus`, `Drain`, `ForceHit`, `ForceMiss`, `Unblockable`, `SwapChoices`, `DamageTakenMult`, `SchoolMod`, `Hand`, `Dispel`, `Counter`, `RandomSpell`, `Enchant`). Rodziny można składać (lista efektów wykonywanych po kolei).

### 6.2. Kontekst efektu

Efekty `custom` dostają kontekst z bezpiecznym API, a nie bezpośredni dostęp do stanu:

```rust
trait EffectCtx {
    fn caster(&self) -> Side;  fn target(&self, t: Target) -> Side;
    fn stat(&self, side: Side, s: StatKey) -> i32;           // wartość efektywna
    fn modify_stat(&mut self, side: Side, s: StatKey, delta: i32, dur: Duration);
    fn set_stat(&mut self, side: Side, s: StatKey, value: i32, dur: Duration);
    fn heal(&mut self, side: Side, amount: i32);
    fn damage(&mut self, side: Side, amount: i32, src: DamageSource);
    fn add_stamina(&mut self, side: Side, delta: i32);
    fn add_focus(&mut self, side: Side, delta: i32);
    fn school_level(&self, side: Side, s: School) -> u8;
    fn add_override(&mut self, side: Side, o: AttackOverride, scope: Scope);  // np. ForceMiss na pierwszy atak F
    fn swap_choices(&mut self, side: Side);
    fn discard_random(&mut self, side: Side, n: u8);
    fn draw(&mut self, side: Side, n: u8);
    fn dispel(&mut self, filter: DispelFilter);
    fn register_enchantment(&mut self, e: EnchantSpec);
    fn rng(&mut self) -> &mut impl RngCore;                 // strumień „effects”
    fn emit(&mut self, e: Event);
}
```

### 6.3. Umagicznienia

```rust
struct ActiveEnchantment {
    owner: Side, source: CardId, cast_order: u16,
    spec: EnchantSpec,               // wyzwalacze + efekty (rodziny lub custom)
    state: [i32; 4],                 // licznik, akumulator itp.
}
```

Wyzwalacze: `OnRound` (faza czarów, R-CZA-02), `BeforeAttack`, `OnHit`, `OnBlocked`, `OnDamageTaken`, `OnRoundEnd`. Kolejność wywołań przy tym samym wyzwalaczu: rosnąco według `cast_order`. Rozproszenie usuwa wpisy z listy i emituje `EnchantmentRemoved`.

### 6.4. Warstwy statystyk

`efektywna = max(0, baza + trwałe_zmiany + Σ modyfikatory_umagicznień + modyfikatory_rundy)` (Z-ENG-2). Wartości liczone leniwie z flagą „brudne” przy każdej zmianie listy umagicznień lub modyfikatorów. Specjalizacja (R-CZA-09) i sprzężenia (R-CZA-06) modyfikują koszty karty w `validate_move` i przy rzucaniu.

### 6.4a. Implementacja w fazie F2

- `effect.rs`: enum `Effect` z rodzinami `StatMod`, `StatSet`, `SwapStats`, `Heal`, `Damage`, `Stamina`, `Focus`, `Drain`, `ForceHit`, `ForceMiss`, `Unblockable`, `SwapChoices`, `DamageTakenMult`, `SchoolMod`, `Hand`, `Dispel` i `ConditionalWin` (R-KON-06). Przeciwzaklęcie i karta losowa są słowami kluczowymi (`Keywords`), a nie rodzinami. Efekty `custom` dojdą razem z treścią w F3.
- Czas trwania: `Duel`, `Round`, `WhileActive`. Wyzwalacze umagicznień: `OnRound`, `OnHit`, `OnBlocked`, `OnDamageTaken`, `OnRoundEnd`; `OnHit` i `OnBlocked` mogą mieć filtr ataku (`AttackFilter`). Wyzwalacz `BeforeAttack` ze szkicu nie był potrzebny: wymuszone trafienia i chybienia ustawiają efekty rundy.
- `enchant.rs`: lista aktywnych umagicznień (najwyżej `MAX_ENCHANTMENTS`), wywoływanych w kolejności rzucenia; premie ciągłe liczone w warstwach statystyk, szkół i mnożnika obrażeń.
- `card.rs`: `CardKind` (`Instant` albo `Enchantment(Trigger)`), `Keywords` (interwencja, przeciwzaklęcie, sprzężenie, karta losowa) i opcjonalny `Condition` (R-CZA-08).
- Zagrana karta schodzi z ręki przy ujawnieniu ruchów, a wygrane warunkowe są sprawdzane przed nokautem (R-KON-06).

### 6.5. Zdarzenia

Każda zmiana stanu emituje zdarzenie. Zdarzenia są jedynym materiałem dla animacji klienta i dla zapisu powtórki „do wyświetlenia”. Silnik zapisuje w nich pełne dane; to, co widzi każdy gracz (kolumna „Widoczność”), wybiera warstwa widoków (faza F5) i protokół (faza F7).

| Zdarzenie | Pola | Widoczność |
|---|---|---|
| `RoundStarted` | runda, K obu stron | publiczne |
| `MovesRevealed` | ustawienia obu stron, akcje kart, K pobrana za zmiany | publiczne |
| `SpellCast` / `SpellCountered` | strona, karta, koszty | publiczne |
| `SpellFizzled` | strona, karta (czar przepada bez kosztów, Z-SPW) | publiczne |
| `CardDiscarded` | strona, karta | publiczne |
| `StatChanged` | strona, statystyka, stara → nowa, źródło | publiczne |
| `EnchantmentAdded` / `EnchantmentRemoved` | strona, karta | publiczne |
| `AttackHit` / `AttackBlocked` | atakujący, slot, typ, mnożnik, obrażenia, koszt W | publiczne |
| `Healed` / `Damaged` | strona, wartość, źródło | publiczne |
| `CardDrawn` | strona, karta | tylko właściciel (przeciwnik dostaje `CardDrawnHidden`) |
| `DuelEnded` | wynik, powód | publiczne |

W fazie F2 doszły m.in. `ResourceChanged`, `AttackMissed`, `ChoicesSwapped`, `AttackOverride`, `DamageTakenChanged`, `SchoolChanged`, `RandomSpell`, `SpellFailed` i `ForcedDiscard`. Pełna lista z opisami jest w `crates/engine/src/event.rs`.

## 7. Losowość i determinizm

- Ziarno pojedynku (32 bajty) losuje serwer z systemowego źródła entropii (`getrandom`).
- Strumienie: `deck/A` i `deck/B` (uzupełnienie brakujących kart, R-TAL-05, a potem tasowanie talii gracza), `initiative` (R-POJ-05) i od fazy F2 `effects`. Każdy powstaje z `ChaCha8Rng::from_seed(K)`, gdzie `K` to BLAKE3 w trybie wyprowadzania klucza (stały kontekst silnika) z `seed ‖ etykieta`. Osobny strumień na zadanie sprawia, że dodatkowe losowanie w jednym miejscu nie przesuwa pozostałych.
- Tasowanie: algorytm Fishera–Yatesa na strumieniu talii gracza. Losowanie liczby z przedziału (metoda Lemire'a, bez obciążenia) i tasowanie są w kodzie silnika, a nie w bibliotece `rand`, żeby jej aktualizacja nie zmieniała przebiegu zapisanych pojedynków.
- Zakazane w crate'ach silnika (sprawdzane przez `clippy` i CI): typy zmiennoprzecinkowe (`clippy::float_arithmetic`), `HashMap`/`HashSet` (`clippy::disallowed_types`), `std::time`, `thread_rng`.
- Test **natywne = WASM** (`crates/engine-wasm/tests/native_wasm.rs`): 10 000 losowych pojedynków z `engine::testkit` daje ten sam skrót wszystkich wartości `state_hash` po każdej rundzie natywnie i w WASM (Node + `wasm-bindgen-test-runner`). Oczekiwany skrót jest zapisany w teście; zmienia się razem z zasadami, kodowaniem stanu albo losowaniem w `testkit`.
- Przypięte wartości w testach silnika (`stream_derivation_is_pinned`, `state_hash_is_pinned`) wykrywają zmianę strumieni lub kodowania stanu, np. po aktualizacji `rand_chacha` albo `blake3`.

## 8. Widoki i ukryte informacje

`view(state, side)` zwraca `PlayerView`, czyli to, co gracz może wiedzieć:
- własna ręka, kolejność własnej talii **niejawna** (tylko liczba kart);
- ręka przeciwnika: tylko liczba kart;
- statystyki, Ż, W, K i umagicznienia obu stron (jawne zgodnie z S2: ekran pokazuje statystyki i umagicznienia obu postaci);
- ustawienia przeciwnika z poprzedniej rundy (R-RUN-07, interfejs z S2/S3);
- inicjatywa, numer rundy;
- skrót zobowiązania do ziarna (ADR-007), bez ziarna.

`PlayerView` jest podstawą wiadomości `DuelStart` i `DuelSnapshot` (`07-protokol-sieciowy.md`).

Implementacja (F2.6): `engine::view` i `PlayerView` z metodami `validate_move`, `preview` i `default_move`, które dają te same wyniki co funkcje na pełnym stanie (test `checking_moves_on_the_view_gives_the_same_answers_as_on_the_full_state`). Na widoku działają boty z `engine-ai`. Skrót zobowiązania do ziarna dojdzie razem z protokołem w F7.

Od F4 `PlayerView::sample_state(content, opponent_hand, decks, seed)` tworzy hipotetyczny stan na potrzeby L3. Prywatny snapshot przechowuje wyłącznie jawne warstwy statystyk, umagicznienia i efekty rundy; obie ręce i talie są w nim wyczyszczone, a RNG nie jest kopiowany. Karty ukryte i nowe ziarno podaje bot. Metoda sprawdza długości i identyfikatory kart. Test odtwarzania zachowuje pełne warstwy efektów bez podwójnego naliczania premii ciągłych.

## 9. Powtórki

```rust
struct ReplayRecord {
    engine_version: Version, content_version: [u8; 32],
    setup: DuelSetup,                   // zasady, ziarno, postacie i talie
    moves: ArrayVec<[Move; 2], MAX_ROUNDS>,
    end: Option<(Side, Termination)>,   // przegrywający i powód: ucieczka, rozłączenie, walkower
}
```

- **Rozmiar:** ok. 200–400 B w Protobuf. Dodatkowo serwer zapisuje skompresowaną listę zdarzeń (ok. 0,5–1 KB), żeby stare powtórki dało się wyświetlić nawet po zmianie silnika.
- **Odtworzenie:** `replay()` buduje stan z `DuelSetup` i aplikuje ruchy. Wynik musi się zgadzać z zapisanym (test regresji na korpusie powtórek, `14-testy-i-jakosc.md`).
- **Zgodność:** zmiana zasad, która zmienia wynik istniejących powtórek, podbija główną wersję silnika. Powtórkę odtwarza silnik zgodny według reguł Cargo: przy wersji 0.x z tą samą częścią minor, od 1.0 z tą samą częścią major (`Version::can_replay`). Serwer przechowuje powtórki z wersją i wyświetla stare z zapisanych zdarzeń.

**Poprawki po przeglądzie F4 (silnik 0.2.0).** `stat_set` i `swap_stats` odejmują aktywne premie ciągłe przy wyznaczaniu warstwy trwałej, więc nie naliczają ich drugi raz. Hash stanu koduje również wyzwalacz i pełne, przeskalowane efekty każdego umagicznienia. Wymuszenia ataków są scalane według strony, wyniku i filtra (najwyżej 96 kombinacji); ponowne wyzwolenie zużytego `first_only` uzbraja je ponownie. Warunki wygranej są deduplikowane (najwyżej 3 na stronę). Żaden z tych buforów nie odrzuca poprawnych efektów z powodu liczby aktywnych umagicznień. Te poprawki zmieniają część wyników i skróty, dlatego powtórki 0.1.x wymagają poprzedniego silnika.

## 10. Wydajność

| Miara | Cel (natywnie, 1 rdzeń) | Cel (WASM, przeglądarka średniej klasy) |
|---|---|---|
| `resolve_round`, mediana | ≤ 5 µs | ≤ 20 µs |
| pełny pojedynek 5 rund (bez AI) | ≤ 30 µs | ≤ 120 µs |
| pamięć `DuelState` (bez listy zdarzeń) | ≤ 4 KB | ≤ 4 KB |
| rozmiar `engine-wasm` po `wasm-opt -Oz` | — | ≤ 250 KB (≤ 90 KB po Brotli) |

Techniki:
- struktury o stałym rozmiarze (`ArrayVec`), brak `Box<dyn …>` w gorącej ścieżce (dispatch przez `match` na enumach);
- tablice treści jako `static` wygenerowane w czasie kompilacji;
- ponownie używany bufor zdarzeń (`EventBuf::clear`);
- profil `release` z `lto = "fat"`, `codegen-units = 1`; dla WASM dodatkowo `opt-level = "z"`, `panic = "abort"`, bez formatowania tekstów w silniku (kody zamiast komunikatów).

Pomiary: `criterion` (natywnie, `cargo bench -p engine`), benchmark w przeglądarce przez Playwright (`13-wydajnosc.md`).

Pomiar po fazie F1 (2026-10-14, natywnie, Windows 10, karty bez efektów): mediana `resolve_round` 58 ns, pełny pojedynek 5 rund razem z `new_duel` 1,3 µs.

Pomiar po fazie F2 (2026-10-15, natywnie, Windows 10, karty z efektami i umagicznieniem): mediana `resolve_round` 0,23 µs. `sim run` z botami L1 rozgrywa pojedynek w ok. 7,5 µs razem z losowaniem wejścia i decyzjami botów.

## 11. Testowalność

**Konstruktor scenariuszy** pozwala opisać test bez pełnej treści gry. Szkic testu „TR-01” (walka #26519, `01-badania-i-zrodla.md`, rozdz. 5.2):

```rust
scenario()
    .rules(classic())
    .player_a(stats(4, 2, 9, 3, hp = 75, st = 74))
    .player_b(stats(8, 3, 7, 4, hp = 66, st = 101))
    .initiative(A)
    .hand_b([noop_spell(focus = 2, stamina = 8)])        // „Mass”
    .round(mv("MFF", "MMF", none), mv("FFF", "MMM", cast(0)))
    .expect_a(hp = 55, st = 66, focus = 1)
    .expect_b(hp = 60, st = 77, focus = 2)
    // … rundy 2 i 3
```

Inicjatywa nie wpływa na wyniki rund 1–3 tej walki, więc test przyjmuje ją arbitralnie.

Implementacja: `engine::testkit::Scenario` (cecha `testkit`) buduje stan początkowy z jawnie podaną inicjatywą, ręką i talią, bez losowania. Testy rozgrywają rundy przez `resolve_round` i porównują `RoundSummary` oraz zdarzenia. Ten sam moduł ma losowe pojedynki dla testów determinizmu i symulatora.

Rodzaje testów silnika (szczegóły w `14-testy-i-jakosc.md`): jednostkowe dla każdej reguły `R-…`, scenariuszowe (co najmniej jeden na kartę), właściwościowe (niezmienniki), fuzzing, porównanie natywne = WASM, regresja na korpusie powtórek.

## 12. Jak dodać kartę

1. Wpis w `content/cards/<set>/<szkoła>.toml` (klucz, koszty, wymagania, rzadkość, efekt).
2. Rodzina sparametryzowana: tylko parametry. Efekt `custom`: funkcja w `engine-cards/src/custom/<klucz>.rs` i wpis w rejestrze (kompilator treści sprawdza, czy implementacja istnieje).
3. Teksty w `content/i18n/pl.ftl` i `en.ftl` (zmienne z parametrów efektu).
4. Test scenariuszowy w `engine-cards/tests/cards/<klucz>.rs`: co najmniej przypadek podstawowy i jeden brzegowy (np. brak W, przeciwzaklęcie, rozproszenie).
5. `cargo run -p sim -- card <klucz>`: raport wpływu na balans (`11-ai-i-symulacje.md`).
6. PR z checklistą z `14-testy-i-jakosc.md`, rozdz. 9.

## 13. Interfejs WebAssembly

Silnik w przeglądarce działa w Web Workerze. Komunikacja wątek główny ↔ worker odbywa się przez wiadomości (`postMessage`) z danymi binarnymi Protobuf (te same typy co protokół sieciowy). Dzięki temu nie ma ręcznego mapowania obiektów i kosztownych przejść między JS a WASM.

| Funkcja eksportowana | Zastosowanie |
|---|---|
| `init(content_bytes)` | załadowanie treści (ten sam `content.<hash>` co klient) |
| `training_start(setup_bytes) -> view_bytes` | nowy pojedynek treningowy offline (ziarno z `crypto.getRandomValues`) |
| `training_move(move_bytes) -> events_bytes` | ruch gracza, ruch AI, rozstrzygnięcie |
| `preview(view_bytes, move_bytes) -> preview_bytes` | koszty i przewidywane zużycie W (ekran walki online) |
| `validate_deck(deck_bytes, avatar_bytes) -> result_bytes` | konstruktor talii |
| `replay(record_bytes) -> events_bytes` | odtwarzanie powtórki |

W pojedynkach online worker dostaje tylko `PlayerView`, więc nie zna ziarna ani talii przeciwnika. Podgląd w przeglądarce nie może ujawnić ukrytych informacji.
