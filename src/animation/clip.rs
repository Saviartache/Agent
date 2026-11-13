//! Клип — готовая к проигрыванию последовательность кадров одного листа.

use crate::math::Vec2;

/// Индекс листа спрайтов в таблице персонажа.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetId(pub u16);

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub sheet: SheetId,
    /// x, y, ширина, высота в пикселях листа.
    pub rect: [u32; 4],
    /// Секунды.
    pub duration: f32,
    /// Сдвиг кадра относительно якоря, px.
    pub offset: Vec2,
}

/// Событие на границе кадра: срабатывает, когда кадр `frame` начинается.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    pub frame: u16,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    pub frames: Vec<Frame>,
    pub looping: bool,
    /// Точка кадра, стоящая на позиции персонажа (ноги).
    pub anchor: Vec2,
    /// Упорядочены по кадру.
    pub markers: Vec<Marker>,
}

impl Clip {
    pub fn duration(&self) -> f32 {
        self.frames.iter().map(|frame| frame.duration).sum()
    }
}
