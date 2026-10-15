//! Generator tabel rozwoju postaci (`docs/03-tresc-gry.md`, rozdz. 3.2, Z-GRW). To narzędzie
//! projektanta: wynik wkleja się do pliku postaci jako zwykłą tabelę `growth`.

use std::fmt::Write as _;

use crate::compile::MAX_LEVEL;
use crate::model::{GrowthSrc, SchoolKey, SchoolLevels};

/// Parametry krzywej.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveSpec {
    /// Wagi rozkładu punktów walki na AF, OF, AM i OM (np. 35, 25, 25, 15).
    pub weights: [u32; 4],
    /// Suma przyrostów AF + OF + AM + OM na poziomach 2–20 (typowo 14–20).
    pub combat: u32,
    /// Suma przyrostów W (typowo 55–90).
    pub stamina: u32,
    /// Suma przyrostów Ż (typowo 55–95).
    pub hp: u32,
    /// Podwyżki szkół: (poziom postaci, szkoła, nowy poziom szkoły).
    pub schools: Vec<(u8, SchoolKey, u8)>,
}

/// Zakresy przyrostów z poziomu 1 na 20 z danych referencyjnych (rozdz. 3.2).
pub const COMBAT_RANGE: (i32, i32) = (14, 20);
pub const STAMINA_RANGE: (i32, i32) = (55, 90);
pub const HP_RANGE: (i32, i32) = (55, 95);

/// Rozkłada `total` równomiernie na `count` poziomów (algorytm Bresenhama).
fn spread(total: u32, count: u32) -> Vec<i32> {
    (0..count)
        .map(|index| {
            let share = |i: u32| i64::from(i) * i64::from(total) / i64::from(count);
            i32::try_from(share(index + 1) - share(index)).unwrap_or(0)
        })
        .collect()
}

/// Tabela `growth` dla poziomów 2–20, deterministyczna dla tych samych parametrów.
#[must_use]
pub fn generate(spec: &CurveSpec) -> Vec<GrowthSrc> {
    let levels = u32::from(MAX_LEVEL - 1);
    let combat = spread(spec.combat, levels);
    let stamina = spread(spec.stamina, levels);
    let hp = spread(spec.hp, levels);
    let total_weight: i64 = spec
        .weights
        .iter()
        .map(|weight| i64::from(*weight))
        .sum::<i64>()
        .max(1);
    // Płynny przydział ważony (jak w równoważeniu obciążenia): statystyki dostają punkty
    // w proporcji do wag, przemiennie, a nie seriami.
    let mut credit = [0_i64; 4];
    let mut growth = Vec::new();
    for (index, level) in (2..=MAX_LEVEL).enumerate() {
        let mut d = [0, 0, 0, 0, stamina[index], hp[index]];
        for _ in 0..combat[index] {
            for (slot, weight) in credit.iter_mut().zip(spec.weights) {
                *slot += i64::from(weight);
            }
            let best = (0..4).fold(0, |best, slot| {
                if credit[slot] > credit[best] {
                    slot
                } else {
                    best
                }
            });
            credit[best] -= total_weight;
            d[best] += 1;
        }
        let schools: SchoolLevels = spec
            .schools
            .iter()
            .filter(|(at, _, _)| *at == level)
            .map(|(_, school, new_level)| (*school, *new_level))
            .collect();
        growth.push(GrowthSrc {
            level,
            d,
            schools: (!schools.is_empty()).then_some(schools),
        });
    }
    growth
}

/// Tabela w formacie pliku postaci.
#[must_use]
pub fn to_toml(growth: &[GrowthSrc]) -> String {
    let mut out = String::from("growth = [\n  # przyrosty: [AF, OF, AM, OM, W, Ż]\n");
    for step in growth {
        let d = step.d.map(|value| value.to_string()).join(", ");
        let _ = write!(out, "  {{ level = {}, d = [{d}]", step.level);
        if let Some(schools) = &step.schools {
            let list: Vec<String> = schools
                .iter()
                .map(|(school, level)| format!("{} = {level}", school.name()))
                .collect();
            let _ = write!(out, ", schools = {{ {} }}", list.join(", "));
        }
        out.push_str(" },\n");
    }
    out.push_str("]\n");
    out
}

/// Sumy przyrostów tabeli: (punkty walki, W, Ż).
#[must_use]
pub fn totals(growth: &[GrowthSrc]) -> (i32, i32, i32) {
    growth
        .iter()
        .fold((0, 0, 0), |(combat, stamina, hp), step| {
            (
                combat + step.d[..4].iter().sum::<i32>(),
                stamina + step.d[4],
                hp + step.d[5],
            )
        })
}

/// Parsuje argumenty `curves`: `--weights 35,25,25,15 --combat 17 --stamina 70 --hp 75
/// --school 5:combat=2 …`.
pub fn parse_args(args: &[String]) -> Result<CurveSpec, String> {
    let mut spec = CurveSpec {
        weights: [1; 4],
        combat: 17,
        stamina: 70,
        hp: 75,
        schools: Vec::new(),
    };
    let mut rest = args;
    while let [flag, value, tail @ ..] = rest {
        let number = |text: &str| {
            text.parse::<u32>()
                .map_err(|_| format!("„{text}” nie jest liczbą"))
        };
        match flag.as_str() {
            "--weights" => {
                let parts: Vec<u32> = value.split(',').map(number).collect::<Result<_, _>>()?;
                spec.weights = parts
                    .try_into()
                    .map_err(|_| "--weights wymaga 4 liczb".to_owned())?;
            }
            "--combat" => spec.combat = number(value)?,
            "--stamina" => spec.stamina = number(value)?,
            "--hp" => spec.hp = number(value)?,
            "--school" => {
                let error = || {
                    format!("--school wymaga postaci poziom:szkoła=poziom_szkoły, a jest „{value}”")
                };
                let (level, change) = value.split_once(':').ok_or_else(error)?;
                let (school, new_level) = change.split_once('=').ok_or_else(error)?;
                let school = SchoolKey::ALL
                    .into_iter()
                    .find(|key| key.name() == school)
                    .ok_or_else(error)?;
                spec.schools.push((
                    level.parse().map_err(|_| error())?,
                    school,
                    new_level.parse().map_err(|_| error())?,
                ));
            }
            _ => return Err(format!("nieznany argument {flag}")),
        }
        rest = tail;
    }
    if rest.is_empty() {
        Ok(spec)
    } else {
        Err(format!("brak wartości dla {}", rest[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COMBAT_RANGE, CurveSpec, HP_RANGE, STAMINA_RANGE, generate, parse_args, to_toml, totals,
    };
    use crate::model::SchoolKey;

    fn spec() -> CurveSpec {
        CurveSpec {
            weights: [35, 25, 25, 15],
            combat: 17,
            stamina: 70,
            hp: 75,
            schools: vec![
                (5, SchoolKey::Combat, 2),
                (10, SchoolKey::Arcane, 1),
                (15, SchoolKey::Combat, 3),
            ],
        }
    }

    #[test]
    fn generated_curve_covers_every_level_and_hits_the_totals() {
        let growth = generate(&spec());
        let levels: Vec<u8> = growth.iter().map(|step| step.level).collect();
        assert_eq!(levels, (2..=20).collect::<Vec<u8>>());
        let (combat, stamina, hp) = totals(&growth);
        assert_eq!((combat, stamina, hp), (17, 70, 75));
        assert!((COMBAT_RANGE.0..=COMBAT_RANGE.1).contains(&combat));
        assert!((STAMINA_RANGE.0..=STAMINA_RANGE.1).contains(&stamina));
        assert!((HP_RANGE.0..=HP_RANGE.1).contains(&hp));
        // Punkty walki co 1–2 poziomy, W i Ż po +3…+5 na poziom.
        assert!(
            growth
                .iter()
                .all(|step| step.d[..4].iter().sum::<i32>() <= 1)
        );
        assert!(
            growth
                .iter()
                .all(|step| (3..=5).contains(&step.d[4]) && (3..=5).contains(&step.d[5]))
        );
        assert_eq!(
            growth.iter().filter(|step| step.schools.is_some()).count(),
            3
        );
    }

    #[test]
    fn combat_points_follow_the_weights() {
        let growth = generate(&spec());
        let per_stat: Vec<i32> = (0..4)
            .map(|stat| growth.iter().map(|step| step.d[stat]).sum())
            .collect();
        assert_eq!(per_stat, vec![6, 4, 4, 3]);
    }

    #[test]
    fn the_same_parameters_give_the_same_table() {
        assert_eq!(to_toml(&generate(&spec())), to_toml(&generate(&spec())));
        let args: Vec<String> =
            "--weights 35,25,25,15 --school 5:combat=2 --school 10:arcane=1 --school 15:combat=3"
                .split(' ')
                .map(ToOwned::to_owned)
                .collect();
        assert_eq!(parse_args(&args), Ok(spec()));
        assert!(parse_args(&["--school".to_owned(), "5combat".to_owned()]).is_err());
    }
}
