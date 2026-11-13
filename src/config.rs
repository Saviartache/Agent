//! Настройки. `MotionConfig` и `BehaviorConfig` приходят из манифеста персонажа,
//! `AgentConfig` — от приложения. Расстояния и скорости — в логических пикселях
//! (96 DPI): на масштаб монитора их умножает симуляция.

use serde::Deserialize;

use crate::math::Vec2;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MotionConfig {
    /// px/с
    pub walk_speed: f32,
    pub run_speed: f32,
    /// px/с²
    pub acceleration: f32,
    pub deceleration: f32,
    /// С этого расстояния до цели начинается торможение.
    pub arrive_radius: f32,
    /// Ближе этого цель считается достигнутой.
    pub arrive_tolerance: f32,
    /// Полоса у края рабочей области, из которой персонажа выталкивает.
    pub edge_margin: f32,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            walk_speed: 70.0,
            run_speed: 230.0,
            acceleration: 420.0,
            deceleration: 640.0,
            arrive_radius: 90.0,
            arrive_tolerance: 4.0,
            edge_margin: 56.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BehaviorConfig {
    /// Веса намерений в свободном режиме.
    pub wander_weight: f32,
    pub rest_weight: f32,
    pub observe_cursor_weight: f32,
    pub approach_cursor_weight: f32,
    pub chase_cursor_weight: f32,
    pub nap_weight: f32,
    /// Пределы длительности одного намерения, с.
    pub wander_secs: [f32; 2],
    pub rest_secs: [f32; 2],
    pub observe_secs: [f32; 2],
    /// Сколько идти к курсору, прежде чем сдаться.
    pub approach_secs: [f32; 2],
    pub chase_secs: [f32; 2],
    pub nap_secs: [f32; 2],
    /// Пауза перед повтором намерения, с.
    pub approach_cooldown_secs: f32,
    pub chase_cooldown_secs: f32,
    pub nap_cooldown_secs: f32,
    pub react_cooldown_secs: f32,
    /// Курсор ближе этого замечен, px.
    pub notice_radius: f32,
    /// Курсор ближе этого — контакт, px.
    pub contact_radius: f32,
    /// Скорость курсора, считающаяся резким движением, px/с.
    pub startle_speed: f32,
    /// Пауза между сценками, которые персонаж затевает сам, с; `[0, 0]` — никогда.
    pub stunt_gap_secs: [f32; 2],
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            wander_weight: 5.0,
            rest_weight: 3.0,
            observe_cursor_weight: 2.0,
            approach_cursor_weight: 1.5,
            chase_cursor_weight: 0.6,
            nap_weight: 0.3,
            wander_secs: [4.0, 11.0],
            rest_secs: [2.0, 6.0],
            observe_secs: [1.5, 4.0],
            approach_secs: [6.0, 12.0],
            chase_secs: [3.0, 7.0],
            nap_secs: [12.0, 30.0],
            approach_cooldown_secs: 20.0,
            chase_cooldown_secs: 45.0,
            nap_cooldown_secs: 120.0,
            react_cooldown_secs: 3.0,
            notice_radius: 280.0,
            contact_radius: 36.0,
            startle_speed: 2600.0,
            stunt_gap_secs: [150.0, 400.0],
        }
    }
}

/// Перехват системного курсора (сценка `CursorHeist`). Выключен, пока пользователь явно не включит.
#[derive(Clone, Debug, PartialEq)]
pub struct CursorGrabConfig {
    pub enabled: bool,
    /// Сколько персонаж носится с курсором, с; движок не держит его дольше 6 с при любом значении.
    pub max_secs: f32,
    pub cooldown_secs: f32,
}

impl Default for CursorGrabConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_secs: 5.0,
            cooldown_secs: 120.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentConfig {
    /// Множитель размера поверх масштаба монитора и манифеста.
    pub scale: f32,
    /// Частота кадров, пока персонаж виден.
    pub fps: u32,
    /// Зерно поведения; `None` — от системных часов.
    pub seed: Option<u64>,
    /// Точка появления в координатах виртуального экрана; `None` — движок выберет сам.
    pub start: Option<Vec2>,
    pub footprints: bool,
    pub particles: bool,
    pub draggable: bool,
    /// Реакция на щелчок по персонажу.
    pub click_reaction: bool,
    /// Разрешает намерения, в которых персонаж идёт к курсору.
    pub cursor_interest: bool,
    pub cursor_grab: CursorGrabConfig,
    /// Меню по правой кнопке: пауза, скрыть, выход.
    pub context_menu: bool,
    /// Прятаться, пока на экране полноэкранное приложение или презентация.
    pub hide_in_fullscreen: bool,
    /// Разрешает персонажу самому затевать сценки (`Stunt::roster`);
    /// по команде приложения они играются при любом значении.
    pub stunts: bool,
    /// Разрешает озорные сценки (`Stunt::naughty`): туалетный и театральный юмор.
    /// Выключено: на рабочем месте они неуместны.
    pub naughty: bool,
    /// Сколько карточек уведомлений живёт одновременно.
    pub max_items: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            scale: 1.0,
            fps: 60,
            seed: None,
            start: None,
            footprints: true,
            particles: true,
            draggable: true,
            click_reaction: true,
            cursor_interest: true,
            cursor_grab: CursorGrabConfig::default(),
            context_menu: true,
            hide_in_fullscreen: true,
            stunts: true,
            naughty: false,
            max_items: 3,
        }
    }
}
