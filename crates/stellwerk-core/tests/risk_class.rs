//! Шкала риска обязана быть упорядочена: политики сравнивают «не выше R2», и
//! без `Ord` сравнение превращается в сравнение строк.

use std::cmp::Ordering;
use stellwerk_core::contract::RiskClass;

#[test]
fn risk_levels_are_ordered_r0_through_r5() {
    let ladder = [
        RiskClass::R0,
        RiskClass::R1,
        RiskClass::R2,
        RiskClass::R3,
        RiskClass::R4,
        RiskClass::R5,
    ];
    for pair in ladder.windows(2) {
        assert_eq!(
            pair[0].cmp(&pair[1]),
            Ordering::Less,
            "{:?} должен быть ниже {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn risk_levels_compare_equal_to_themselves() {
    for level in [RiskClass::R0, RiskClass::R3, RiskClass::R5] {
        assert_eq!(level.cmp(&level), Ordering::Equal);
    }
}

#[test]
fn risk_is_copy_and_hashable() {
    // Компилируется только если RiskClass: Copy + Hash — от этого зависят
    // ключи в сете картах и передача по ссылке без клонирования.
    let level = RiskClass::R4;
    let copy = level;
    let mut set = std::collections::HashSet::new();
    set.insert(level);
    assert!(set.contains(&copy));
    assert_eq!(level, copy);
}

#[test]
fn risk_deserializes_from_r_prefixed_string() {
    assert_eq!(
        serde_json::from_str::<RiskClass>("\"R2\"").unwrap(),
        RiskClass::R2
    );
    assert!(serde_json::from_str::<RiskClass>("\"r2\"").is_err());
    assert!(serde_json::from_str::<RiskClass>("\"R9\"").is_err());
}
