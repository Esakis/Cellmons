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
    combo: u8,                      // seria trafień w bieżącej rundzie
    flags: PlayerFlags,             // np. podwójne obrażenia, pierwsze trafienie F chybia
}

enum CardAction { None, Cast(u8), Discard(u8) }   // indeks w ręce
struct Move { attacks: [Kind; 3], defenses: [Kind; 3], card: CardAction }

enum Outcome { Win(Side, Reason), Draw(Reason) }
enum Reason { Knockout, Rounds, Forfeit, Disconnect, Timeout, ConditionalWin }
```

## 4. Publiczne API (szkic)

```rust
fn new_duel(setup: &DuelSetup, content: &Content) -> Result<DuelState, SetupError>;
fn validate_move(s: &DuelState, side: Side, mv: &Move) -> Result<MoveCost, MoveError>;  // R-RUN-06
fn preview(s: &DuelState, side: Side, mv: &Move) -> Preview;    // K po ruchu, koszt W czaru, maks. zużycie W
fn default_move(s: &DuelState, side: Side) -> Move;             // „bez zmian” (R-RUN-09)
fn resolve_round(s: &mut DuelState, moves: [Move; 2], out: &mut EventBuf) -> RoundSummary;
fn end_by(s: &mut DuelState, side: Side, reason: Reason, out: &mut EventBuf); // ucieczka, rozłączenie, timeout
fn view(s: &DuelState, side: Side) -> PlayerView;               // stan widoczny dla gracza (rozdz. 8)
fn state_hash(s: &DuelState) -> u64;                            // do porównań natywne/WASM i testów
fn replay(r: &ReplayRecord, content: &Content) -> Result<ReplayResult, ReplayError>;
```

Kody `MoveError` (przekazywane do protokołu): `NotEnoughFocus`, `NotEnoughStamina`, `RequirementsNotMet`, `InterventionUsed`, `InvalidHandIndex`, `WrongPhase`, `AlreadySubmitted`.

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
  reset combo for both
  for slot in 0..3:
      for side in initiative_order(state):
          attack(state, side, slot)

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
attack(state, a, slot):
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
      players[a].combo += 1
      mult = rules.combo_multipliers[min(combo, len) - 1]               # R-ATK-04
      atk  = eff_atk(a, kind); def = eff_def(d, kind)
      dmg  = max(0, atk * mult - def)
      dmg  = apply_damage_modifiers(d, dmg)          # np. podwójne obrażenia
      players[d].hp -= dmg
      players[a].stamina = max(0, stamina - atk)                        # R-ATK-04
      emit AttackHit{a, slot, kind, mult, dmg}
      triggers: on_hit(a), on_damage_taken(d, source=Attack)
  else:
      players[a].combo = 0                                              # R-ATK-05
      emit AttackBlocked{a, slot, kind}
      triggers: on_blocked(d)
```

**Decyzje silnika** (uzupełnienie specyfikacji, do zatwierdzenia przy F1):
- **Z-ENG-1:** wymuszone chybienie (efekt obrońcy) ma pierwszeństwo przed wymuszonym trafieniem (efekt atakującego).
- **Z-ENG-2:** statystyki efektywne nie spadają poniżej 0. Ujemne modyfikatory mogą obniżyć AF, OF, AM i OM najwyżej do 0, a Ż_max i W_max najwyżej do 1.
- **Z-ENG-3:** leczenie nie podnosi Ż ponad `hp_max` (zgodne z walką #26519), chyba że efekt jawnie zwiększa `hp_max`.

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

### 6.5. Zdarzenia

Każda zmiana stanu emituje zdarzenie z polem widoczności. Zdarzenia są jedynym materiałem dla animacji klienta i dla zapisu powtórki „do wyświetlenia”.

| Zdarzenie | Pola | Widoczność |
|---|---|---|
| `RoundStarted` | runda, K obu stron | publiczne |
| `MovesRevealed` | ustawienia obu stron, akcje kart | publiczne |
| `SpellCast` / `SpellCountered` / `SpellFizzled` | strona, karta, koszty | publiczne |
| `CardDiscarded` | strona, karta | publiczne |
| `StatChanged` | strona, statystyka, stara → nowa, źródło | publiczne |
| `EnchantmentAdded` / `EnchantmentRemoved` | strona, karta | publiczne |
| `AttackHit` / `AttackBlocked` | atakujący, slot, typ, mnożnik, obrażenia, koszt W | publiczne |
| `Healed` / `Damaged` | strona, wartość, źródło | publiczne |
| `CardDrawn` | strona, karta | tylko właściciel (przeciwnik dostaje `CardDrawnHidden`) |
| `DuelEnded` | wynik, powód | publiczne |

## 7. Losowość i determinizm

- Ziarno pojedynku (32 bajty) losuje serwer z systemowego źródła entropii (`getrandom`).
- Strumienie: `deck[A]`, `deck[B]`, `effects`, `fill` (dobieranie brakujących kart, R-TAL-05). Każdy powstaje z `ChaCha8Rng::from_seed(BLAKE3(seed ‖ etykieta))`.
- Tasowanie: algorytm Fishera–Yatesa na strumieniu talii gracza.
- Zakazane w crate'ach silnika (sprawdzane przez `clippy` i CI): typy zmiennoprzecinkowe (`clippy::float_arithmetic`), `HashMap`/`HashSet` (`clippy::disallowed_types`), `std::time`, `thread_rng`.
- Test **natywne = WASM**: losowe pojedynki (ziarna z listy) rozgrywane natywnie i w WASM (Node + `wasm-bindgen-test`) muszą dać ten sam `state_hash` po każdej rundzie.

## 8. Widoki i ukryte informacje

`view(state, side)` zwraca `PlayerView`, czyli to, co gracz może wiedzieć:
- własna ręka, kolejność własnej talii **niejawna** (tylko liczba kart);
- ręka przeciwnika: tylko liczba kart;
- statystyki, Ż, W, K i umagicznienia obu stron (jawne zgodnie z S2: ekran pokazuje statystyki i umagicznienia obu postaci);
- ustawienia przeciwnika z poprzedniej rundy (R-RUN-07, interfejs z S2/S3);
- inicjatywa, numer rundy;
- skrót zobowiązania do ziarna (ADR-007), bez ziarna.

`PlayerView` jest podstawą wiadomości `DuelStart` i `DuelSnapshot` (`07-protokol-sieciowy.md`).

## 9. Powtórki

```rust
struct ReplayRecord {
    engine_version: SemVer, content_version: [u8; 32],
    rules: RulesConfig, seed: [u8; 32],
    players: [PlayerSetup; 2],
    moves: ArrayVec<[Move; 2], 5>,
    end: Option<(Side, Reason)>,       // ucieczka, rozłączenie, timeout
}
```

- **Rozmiar:** ok. 200–400 B w Protobuf. Dodatkowo serwer zapisuje skompresowaną listę zdarzeń (ok. 0,5–1 KB), żeby stare powtórki dało się wyświetlić nawet po zmianie silnika.
- **Odtworzenie:** `replay()` buduje stan z `DuelSetup` i aplikuje ruchy. Wynik musi się zgadzać z zapisanym (test regresji na korpusie powtórek, `14-testy-i-jakosc.md`).
- **Zgodność:** zmiana zasad, która zmienia wynik istniejących powtórek, podbija główną wersję silnika. Serwer przechowuje powtórki z wersją i wyświetla stare z zapisanych zdarzeń.

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

Pomiary: `criterion` (natywnie), benchmark w przeglądarce przez Playwright (`13-wydajnosc.md`).

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
