# ui-agent — памятка агенту

Персонаж рабочего стола Windows (свинка или гусь) и предметы вокруг него. Прежде чем
что-либо менять, прочитай две заметки в `docs/`:

- [Персонаж рабочего стола](docs/character.md) — архитектура, карта модулей,
  API `Agent`/`AgentHandle`, `Companion`, формат набора, сценки, как добавить предмет,
  ручной чек-лист для Windows.
- [Визуальный стиль рабочего стола](docs/visual-style.md) —
  **обязательна для всего, что рисуется**: плоский цвет, лёгкая мозаика, тень точками,
  перспектива «плоской земли», палитра, числа и чек-лист приёмки.

Изменил поведение, API или вид — обнови эти заметки в том же изменении.

## Коротко

- Только Windows 10/11 x64; никаких абстракций под другие ОС.
- Ядро (`animation`, `assets`, `behavior`, `motion`, `effects`, `render`, `rig`, `sim`)
  не знает о Windows и проверяется тестами на любой машине. Окна, курсор и мониторы —
  в `platform`, поток — в `runtime`; `unsafe` только в `platform`.
- Новый вид рисуется через `render/shadow.rs` (тень) и `render/pixel.rs` (мозаика),
  своих теней и стилей не заводить.
- Виды тела (`RigKind`: `goose`, `pig`) делят один скелет `rig/creature`; вид — это
  сложение в `rig/creature/build.rs` и свой рисовальщик в `render/` (`goose.rs`, `pig.rs`),
  который зовёт общий `render/creature.rs`. Новый вид — по этим трём местам плюс набор в `assets/`
  и свой каталог сценок `sim/stunt/<вид>/`.
- Сценки (`Stunt`): общие — `sim/stunt/common/`, видовые — `sim/stunt/goose/`, `sim/stunt/pig/`;
  чья сценка — `Stunt::cast`. Одна сценка — один файл; порядок добавления — в заметке.
- Зависимости: `png`, `serde`, `serde_json`, `windows-sys` — новых не добавлять без нужды.
- Окна приложений подключают персонажа только через `Companion` (`src/companion.rs`), не через
  `Agent`.
- Документация крейта — только в `docs/`; ссылок на проекты, которые его подключают, нет.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p ui-agent
cargo clippy -p ui-agent --all-targets -- -D warnings
cargo clippy -p ui-agent --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo run -p ui-agent --example preview_rig -- pig out.png # вид тела кадрами (pig | goose)
cargo run -p ui-agent --example preview_stunt -- pig oops out.png --ui 3 --crop 140,110  # сценка кадрами
./scripts/build-package.sh                                 # тестовый пакет в dist/ (оба персонажа)
cargo test --manifest-path console/Cargo.toml              # консоль пакета (свой воркспейс)
cargo clippy --manifest-path console/Cargo.toml --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

Тесты `platform`/`agent` требуют рабочего стола Windows: на другой машине они только
собираются (`cargo test -p ui-agent --target x86_64-pc-windows-gnu --no-run`). Не
объявляй их пройденными без запуска на Windows.
