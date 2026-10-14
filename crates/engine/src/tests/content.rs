//! Treść gry widziana przez silnik.

use crate::card::{Content, ContentError};
use crate::testkit::{TEST_CONTENT_VERSION, noop_card};
use crate::types::{School, Schools};

#[test]
fn cards_are_found_by_id() {
    let cards = [noop_card(1, 1, 4), noop_card(5, 2, 8), noop_card(9, 0, 0)];
    let content = Content::new(TEST_CONTENT_VERSION, &cards).expect("poprawna treść");
    assert_eq!(content.get(5), Some(&cards[1]));
    assert_eq!(content.get(4), None);
    assert_eq!(content.cards().len(), 3);
    assert_eq!(content.version(), &TEST_CONTENT_VERSION);
}

#[test]
fn ids_must_be_strictly_increasing() {
    let duplicate = [noop_card(1, 1, 4), noop_card(1, 2, 8)];
    let unsorted = [noop_card(2, 1, 4), noop_card(1, 2, 8)];
    assert_eq!(
        Content::new(TEST_CONTENT_VERSION, &duplicate).err(),
        Some(ContentError::UnsortedId(1))
    );
    assert_eq!(
        Content::new(TEST_CONTENT_VERSION, &unsorted).err(),
        Some(ContentError::UnsortedId(1))
    );
}

#[test]
fn requirements_above_level_four_are_rejected() {
    let mut card = noop_card(3, 1, 4);
    card.requires = Schools::NONE.with(School::Arcane, 5);
    assert_eq!(
        Content::new(TEST_CONTENT_VERSION, &[card]).err(),
        Some(ContentError::InvalidRequirement(3))
    );
}
