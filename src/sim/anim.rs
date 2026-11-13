//! Проигрыватель действий: переводит действие мозга в клип и сообщает, доиграло ли оно.

use crate::{
    action::{ActionId, ClipId},
    animation::{AnimEvent, Player},
    assets::Character,
    rig::Gesture,
};

pub(super) struct Animator {
    player: Player,
    /// Нужен проигрывателю, но события клипов симуляции не требуются.
    events: Vec<AnimEvent>,
    requested: ActionId,
    /// Действие, чей клип на самом деле идёт: высокий приоритет может не уступить.
    shown_action: ActionId,
    /// Сколько клип действия показывается с тех пор, как действие стало текущим.
    shown: f32,
    done: bool,
}

impl Animator {
    pub fn new(character: &Character) -> Self {
        let idle = character.actions().idle;
        Self {
            player: Player::new(character.action(idle).clip),
            events: Vec::new(),
            requested: idle,
            shown_action: idle,
            shown: 0.0,
            done: false,
        }
    }

    pub fn update(&mut self, character: &Character, action: ActionId, speed: f32, dt: f32) {
        if action != self.requested {
            self.requested = action;
            self.shown = 0.0;
        }
        let entry = character.action(action);
        if self.player.play(entry.clip, entry.priority) {
            self.shown_action = action;
        }
        self.player.set_speed(speed);
        self.events.clear();
        self.player.advance(dt, character, &mut self.events);
        self.done = self.is_done(character, entry.clip, dt);
    }

    /// Разовый клип доигран; зацикленный — показан целиком хотя бы раз
    /// (набор мог отобразить разовое действие на цикл, и мозг не должен ждать таймаута).
    fn is_done(&mut self, character: &Character, clip: ClipId, dt: f32) -> bool {
        if self.player.clip() != clip {
            self.shown = 0.0;
            return false;
        }
        self.shown += dt;
        let clip = character.clip(clip);
        if clip.looping {
            self.shown >= clip.duration()
        } else {
            self.player.finished()
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn clip(&self) -> ClipId {
        self.player.clip()
    }

    pub fn frame(&self) -> usize {
        self.player.frame_index()
    }

    /// Жест показанного сейчас действия и доля его клипа — для процедурного тела.
    pub fn gesture(&self, character: &Character) -> (Gesture, f32) {
        let name = &character.action(self.shown_action).name;
        (Gesture::from_action(name), self.player.progress(character))
    }
}
