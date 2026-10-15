//! Ten sam skrót decyzji L2/L3 natywnie i w Node/WASM.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn advanced_ai_is_identical_natively_and_in_wasm() {
    use engine::testkit::{content, random_setup, sample_cards, seeded_rng};
    use engine::{EventBuf, Side, new_duel, resolve_round, view};
    use engine_ai::{Bot, Level};

    let cards = sample_cards();
    let content = content(&cards);
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for seed in 0..4 {
        let mut state = new_duel(&random_setup(&mut seeded_rng(seed), &content), &content).unwrap();
        let mut bots = [Bot::new(Level::L2, seed), Bot::new(Level::L3, seed + 1)];
        let mut events = EventBuf::new();
        while !state.is_finished() {
            let moves =
                Side::BOTH.map(|side| bots[side.index()].choose(&view(&state, side), &content));
            events.clear();
            resolve_round(&mut state, &content, &moves, &mut events).unwrap();
            for byte in engine::state_hash(&state).to_le_bytes() {
                digest = (digest ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    // Silnik 0.2: hash zawiera skalowane efekty umagicznień.
    assert_eq!(digest, 3_663_425_226_433_934_179);
}
