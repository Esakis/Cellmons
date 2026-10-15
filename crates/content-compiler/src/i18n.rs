//! Teksty treści w formacie Project Fluent (`content/i18n/*.ftl`): polski i angielski.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

use crate::Diagnostic;

/// Język tekstów.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Pl,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Pl, Lang::En];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
        }
    }

    /// Plik względem katalogu treści.
    #[must_use]
    pub fn file(self) -> PathBuf {
        PathBuf::from("i18n").join(format!("{}.ftl", self.code()))
    }
}

/// Teksty jednego języka: identyfikator wiadomości → użyte zmienne.
#[derive(Debug, Clone)]
pub struct Messages {
    pub file: PathBuf,
    /// Plik źródłowy bez zmian; trafia do klienta.
    pub source: String,
    pub vars: BTreeMap<String, BTreeSet<String>>,
}

/// Parsuje plik Fluent; błędy składni i powtórzone identyfikatory są błędami treści.
pub fn parse(file: &PathBuf, source: String) -> Result<Messages, Vec<Diagnostic>> {
    let resource = match fluent_syntax::parser::parse(source.as_str()) {
        Ok(resource) => resource,
        Err((_, found)) => {
            return Err(found
                .into_iter()
                .map(|error| {
                    let line = source[..error.pos.start.min(source.len())]
                        .matches('\n')
                        .count()
                        + 1;
                    Diagnostic::new(
                        file,
                        None,
                        format!("wiersz {line}: błąd składni Fluent ({:?})", error.kind),
                    )
                })
                .collect());
        }
    };
    let mut errors = Vec::new();
    let mut vars = BTreeMap::new();
    for entry in &resource.body {
        if let Entry::Message(message) = entry {
            let id = message.id.name.to_owned();
            let mut used = BTreeSet::new();
            if let Some(value) = &message.value {
                collect_pattern(value, &mut used);
            }
            for attribute in &message.attributes {
                collect_pattern(&attribute.value, &mut used);
            }
            if vars.insert(id.clone(), used).is_some() {
                errors.push(Diagnostic::new(
                    file,
                    Some(&id),
                    "tekst zdefiniowany więcej niż raz",
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(Messages {
            file: file.clone(),
            source,
            vars,
        })
    } else {
        Err(errors)
    }
}

fn collect_pattern(pattern: &Pattern<&str>, used: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression(expression, used);
        }
    }
}

fn collect_expression(expression: &Expression<&str>, used: &mut BTreeSet<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            collect_inline(selector, used);
            for variant in variants {
                collect_pattern(&variant.value, used);
            }
        }
        Expression::Inline(inline) => collect_inline(inline, used),
    }
}

fn collect_inline(inline: &InlineExpression<&str>, used: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            used.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                collect_inline(argument, used);
            }
            for argument in &arguments.named {
                collect_inline(&argument.value, used);
            }
        }
        InlineExpression::Placeable { expression } => collect_expression(expression, used),
        InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::MessageReference { .. }
        | InlineExpression::TermReference { .. } => {}
    }
}
