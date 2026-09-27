//! RED-спецификация: уровни полномочий.
//!
//! `PolicyLevel::permits` объявлен `todo!()`. Инварианты:
//!   * лестница упорядочена `L0Local < L1SharedRead < … < L5Owner`;
//!   * `ReadWorkspace` доступен с самого низкого уровня;
//!   * `ManageMembers` требует `L4Admin`;
//!   * только `L5Owner` меняет политики (`GrantRoles`);
//!   * L0 не может писать наружу своего рабочего места.

use stellwerk_core::contract::{Action, PolicyLevel, WorkspaceMembership};

const LADDER: [PolicyLevel; 6] = [
    PolicyLevel::L0Local,
    PolicyLevel::L1SharedRead,
    PolicyLevel::L2Contributor,
    PolicyLevel::L3Maintainer,
    PolicyLevel::L4Admin,
    PolicyLevel::L5Owner,
];

const ALL_ACTIONS: [Action; 6] = [
    Action::ReadWorkspace,
    Action::WriteWorkspace,
    Action::RunTask,
    Action::UpdateModule,
    Action::ManageMembers,
    Action::GrantRoles,
];

#[test]
fn levels_are_ordered_l0_through_l5() {
    for pair in LADDER.windows(2) {
        assert!(
            pair[0] < pair[1],
            "{:?} должен быть ниже {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn reading_is_allowed_for_everyone() {
    for level in LADDER {
        assert!(
            level.permits(Action::ReadWorkspace),
            "{level:?} должен читать"
        );
    }
}

#[test]
fn local_level_cannot_write_or_run() {
    // L0 — это «своё локальное рабочее место»: без записи и без запуска задач.
    assert!(!PolicyLevel::L0Local.permits(Action::WriteWorkspace));
    assert!(!PolicyLevel::L0Local.permits(Action::RunTask));
}

#[test]
fn contributor_writes_and_runs_but_does_not_administer() {
    let level = PolicyLevel::L2Contributor;
    assert!(level.permits(Action::WriteWorkspace));
    assert!(level.permits(Action::RunTask));
    assert!(!level.permits(Action::UpdateModule));
    assert!(!level.permits(Action::ManageMembers));
    assert!(!level.permits(Action::GrantRoles));
}

#[test]
fn maintainer_updates_modules_but_not_members() {
    let level = PolicyLevel::L3Maintainer;
    assert!(level.permits(Action::UpdateModule));
    assert!(!level.permits(Action::ManageMembers));
    assert!(!level.permits(Action::GrantRoles));
}

#[test]
fn admin_manages_members_but_cannot_grant_roles() {
    // Смена политик — привилегия владельца, а не администратора: иначе
    // скомпрометированный админ тихо расширяет свои права.
    let level = PolicyLevel::L4Admin;
    assert!(level.permits(Action::ManageMembers));
    assert!(!level.permits(Action::GrantRoles));
}

#[test]
fn only_owner_grants_roles() {
    for level in LADDER {
        let expected = level == PolicyLevel::L5Owner;
        assert_eq!(
            level.permits(Action::GrantRoles),
            expected,
            "{level:?} → GrantRoles"
        );
    }
}

/// Права монотонны: более высокий уровень не может потерять право,
/// которое было у нижнего.
#[test]
fn permissions_are_monotonic_in_level() {
    for action in ALL_ACTIONS {
        for pair in LADDER.windows(2) {
            if pair[0].permits(action) {
                assert!(
                    pair[1].permits(action),
                    "{:?} имеет {action:?}, но {:?} — нет",
                    pair[0],
                    pair[1]
                );
            }
        }
    }
}

#[test]
fn membership_roundtrips() {
    let member = WorkspaceMembership {
        user_id: "u-1".to_owned(),
        workspace_id: "ws-1".to_owned(),
        level: PolicyLevel::L3Maintainer,
    };
    let json = serde_json::to_string(&member).expect("must serialize");
    let restored: WorkspaceMembership = serde_json::from_str(&json).expect("must deserialize");
    assert_eq!(member, restored);
}

#[test]
fn membership_rejects_unknown_fields() {
    let bad = serde_json::json!({
        "user_id": "u", "workspace_id": "w", "level": "L0Local", "expires_at": "soon"
    });
    assert!(serde_json::from_value::<WorkspaceMembership>(bad).is_err());
}

#[test]
fn level_serializes_in_pascal_case() {
    assert_eq!(
        serde_json::to_value(PolicyLevel::L5Owner).unwrap(),
        serde_json::json!("L5Owner")
    );
    assert_eq!(
        serde_json::to_value(Action::ManageMembers).unwrap(),
        serde_json::json!("manage_members")
    );
}
