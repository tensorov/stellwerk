//! Tests for `permission_for` — mapping RiskClass → Permission.
//!
//! RED: all tests expect a specific `Permission` value, but the stub returns
//! `PermissionError::NotImplemented`, so every test fails.
//!
//! Ожидаемая таблица вердиктов (packet-02 реализует):
//! | RiskClass | Permission | основание                                    |
//! |-----------|------------|----------------------------------------------|
//! | R0        | Allow      | M1-06, без изменений                         |
//! | R1        | Allow      | M1-06, без изменений                         |
//! | R2        | Ask        | M1-06, без изменений                         |
//! | R3        | Ask        | M1-06, без изменений (уровень стал L5Owner)  |
//! | R4        | Ask        | В1: был Deny, изменён                        |
//! | R5        | Deny       | В1: безусловный, не suspend'ится никогда     |

use stellwerk_core::contract::RiskClass;
use stellwerk_gateway::permission::{permission_for, Permission, ToolRisk};

// ═══════════════════════════════════════════════════════════════════════════
// Individual risk-class tests (RED: expect specific Permission)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn r0_is_allow() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R0)).unwrap();
    assert_eq!(result, Permission::Allow);
}

#[test]
fn r1_is_allow() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R1)).unwrap();
    assert_eq!(result, Permission::Allow);
}

#[test]
fn r2_is_ask() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R2)).unwrap();
    assert_eq!(result, Permission::Ask);
}

#[test]
fn r3_is_ask() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R3)).unwrap();
    assert_eq!(result, Permission::Ask);
}

#[test]
fn r4_is_ask() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R4)).unwrap();
    assert_eq!(result, Permission::Ask);
}

#[test]
fn r5_is_deny() {
    let result = permission_for(ToolRisk::Classified(RiskClass::R5)).unwrap();
    assert_eq!(result, Permission::Deny);
}

// ═══════════════════════════════════════════════════════════════════════════
// Completeness: all six RiskClass variants are covered by the table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn all_risk_classes_covered() {
    let all_classes = [
        RiskClass::R0,
        RiskClass::R1,
        RiskClass::R2,
        RiskClass::R3,
        RiskClass::R4,
        RiskClass::R5,
    ];
    for rc in &all_classes {
        let result = permission_for(ToolRisk::Classified(*rc)).unwrap();
        // Each class must map to exactly one permission; the specific value
        // is checked by the individual tests above. Here we just ensure
        // every variant is exercised (no silent skip).
        let _ = result;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Unclassified tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn unclassified_is_deny() {
    // Unclassified → Deny (неинтерактивный worker, нет L5Owner)
    let result = permission_for(ToolRisk::Unclassified).unwrap();
    assert_eq!(result, Permission::Deny);
}

#[test]
fn unclassified_never_allows() {
    // Unclassified must never return Allow regardless of context.
    let result = permission_for(ToolRisk::Unclassified).unwrap();
    assert_ne!(result, Permission::Allow);
}
