//! Pokrycie reguł testami (`docs/14-testy-i-jakosc.md`, rozdz. 1): każda reguła z zakresu silnika
//! z `docs/02-zasady-gry.md` ma test, którego nazwa zaczyna się od jej identyfikatora,
//! np. `fn r_atk_02_…` dla R-ATK-02.

use std::fs;
use std::path::Path;

/// Rodziny reguł, które implementuje silnik.
const FAMILIES: &[&str] = &["R-POJ-", "R-RUN-", "R-ATK-", "R-KON-"];

/// Reguły spoza tych rodzin, które silnik też sprawdza.
const EXTRA: &[&str] = &["R-TAL-01", "R-TAL-02", "R-TAL-05"];

/// Reguły z tych rodzin realizowane później albo poza silnikiem.
const DEFERRED: &[(&str, &str)] = &[
    ("R-POJ-07", "dar jako umagicznienie, faza F2.3"),
    (
        "R-RUN-07",
        "zatwierdzenie ruchu i informacja dla przeciwnika, serwer w fazie F7",
    ),
    ("R-RUN-08", "limity czasu na ruch, serwer w fazie F7"),
    ("R-RUN-13", "efekty umagicznień na koniec rundy, faza F2.3"),
    ("R-KON-06", "wygrane warunkowe z efektów kart, faza F2.4"),
];

/// Identyfikatory reguł w kolejności z dokumentu (definicje mają postać `**R-XXX-NN.`).
fn rule_ids(spec: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for (start, _) in spec.match_indices("**R-") {
        let id: String = spec[start + 2..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '-')
            .collect();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// Treść wszystkich plików `.rs` w katalogu (rekurencyjnie).
fn sources(dir: &Path, out: &mut String) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("wpis katalogu").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push_str(&fs::read_to_string(&path).expect("plik źródłowy"));
        }
    }
}

#[test]
fn every_engine_rule_has_a_test() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = fs::read_to_string(root.join("../../docs/02-zasady-gry.md"))
        .expect("docs/02-zasady-gry.md");
    let rules = rule_ids(&spec);
    let mut code = String::new();
    sources(&root.join("src"), &mut code);

    for (id, _) in DEFERRED {
        assert!(
            rules.iter().any(|rule| rule == id),
            "{id} nie ma w docs/02-zasady-gry.md"
        );
    }
    let required: Vec<&String> = rules
        .iter()
        .filter(|id| {
            FAMILIES.iter().any(|family| id.starts_with(family)) || EXTRA.contains(&id.as_str())
        })
        .filter(|id| !DEFERRED.iter().any(|(deferred, _)| deferred == id))
        .collect();
    assert!(
        required.len() >= 30,
        "za mało reguł w specyfikacji: {}",
        required.len()
    );

    let missing: Vec<&String> = required
        .into_iter()
        .filter(|id| !code.contains(&format!("fn {}_", id.to_lowercase().replace('-', "_"))))
        .collect();
    assert!(missing.is_empty(), "reguły bez testów: {missing:?}");
}
