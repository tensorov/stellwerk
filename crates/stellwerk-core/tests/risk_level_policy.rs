//! RED-спецификация: `RiskClass::required_level` — таблица рисков.
//!
//! `RiskClass::required_level` объявлен `todo!()`. Инварианты:
//!   * R0 → L0Local, R1 → L2Contributor, R2 → L3Maintainer,
//!     R3 → L4Admin, R4 → L5Owner, R5 → L5Owner;
//!   * лестница не убывает: R0 < R1 < R2 < R3 ≤ R4 == R5;
//!   * R4 и R5 требуют L5Owner (Deny-вердикт при любом уровне).

use stellwerk_core::contract::{PolicyLevel, RiskClass};

const RISK_LADDER: [(RiskClass, PolicyLevel); 6] = [
    (RiskClass::R0, PolicyLevel::L0Local),
    (RiskClass::R1, PolicyLevel::L2Contributor),
    (RiskClass::R2, PolicyLevel::L3Maintainer),
    (RiskClass::R3, PolicyLevel::L4Admin),
    (RiskClass::R4, PolicyLevel::L5Owner),
    (RiskClass::R5, PolicyLevel::L5Owner),
];

#[test]
fn r0_requires_local_only() {
    assert_eq!(RiskClass::R0.required_level(), PolicyLevel::L0Local);
}

#[test]
fn r1_requires_contributor() {
    assert_eq!(RiskClass::R1.required_level(), PolicyLevel::L2Contributor);
}

#[test]
fn r2_requires_maintainer() {
    assert_eq!(RiskClass::R2.required_level(), PolicyLevel::L3Maintainer);
}

#[test]
fn r3_requires_admin() {
    assert_eq!(RiskClass::R3.required_level(), PolicyLevel::L4Admin);
}

#[test]
fn r4_requires_owner() {
    assert_eq!(RiskClass::R4.required_level(), PolicyLevel::L5Owner);
}

#[test]
fn r5_requires_owner() {
    assert_eq!(RiskClass::R5.required_level(), PolicyLevel::L5Owner);
}

/// Лестница required_level не убывает по мере роста риска.
#[test]
fn required_level_monotonic() {
    for pair in RISK_LADDER.windows(2) {
        let (rc_a, lvl_a) = pair[0];
        let (rc_b, lvl_b) = pair[1];
        assert!(
            lvl_a <= lvl_b,
            "{rc_a:?} → {lvl_a:?} должен быть ≤ {rc_b:?} → {lvl_b:?}"
        );
    }
}

/// R4 и R5 — абсолютный Deny: требуют L5Owner.
#[test]
fn r4_r5_require_owner() {
    assert_eq!(RiskClass::R4.required_level(), PolicyLevel::L5Owner);
    assert_eq!(RiskClass::R5.required_level(), PolicyLevel::L5Owner);
}
