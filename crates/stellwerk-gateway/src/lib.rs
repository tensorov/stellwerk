//! stellwerk-gateway: виртуальный MCP-сервер — решает, что можно, спрашивает,
//! когда надо, и пишет чек.
//!
//! Зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
//! `cargo add` запрещён.
//!
//! Решения принимаются ТОЛЬКО через предикаты контракта:
//! `RiskClass::required_level`, `PolicyLevel::permits`,
//! `DataClass::permits_model` (все реализованы в M1).

pub mod gate;
pub mod permission;
pub mod shaping;
