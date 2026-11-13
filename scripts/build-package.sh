#!/usr/bin/env bash
#
# build-package.sh — собрать пакет с персонажем рабочего стола для Windows x64.
#
#   1) cargo build --release --target x86_64-pc-windows-gnu -p ui-agent --example desktop_agent
#      (персонаж по умолчанию вшивается через UI_AGENT_RIG)
#   2) upx --best --lzma (жмём КОПИЮ в staging, не трогая cargo-output)
#   3) архив dist/ui-agent-<персонаж>-win64.zip (ui-agent-<персонаж>.exe + README.txt)
#
# Использование:
#   ./scripts/build-package.sh pig              # свинка, со сжатием
#   ./scripts/build-package.sh goose --no-upx   # гусь, без сжатия

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET="x86_64-pc-windows-gnu"
RIG=""
USE_UPX=1

fail() { printf '  ✗ %s\n' "$*" >&2; exit 1; }
step() { printf '  ▸ %s\n' "$*"; }

for arg in "$@"; do
    case "$arg" in
        goose|pig) RIG="$arg" ;;
        --no-upx) USE_UPX=0 ;;
        -h|--help)
            sed -n '3,/^$/{/^$/!p;}' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) fail "неизвестный аргумент: $arg" ;;
    esac
done

case "$RIG" in
    goose) NAME="гусь"; OTHER="pig" ;;
    pig) NAME="свинка"; OTHER="goose" ;;
    *) fail "укажи персонажа: goose или pig" ;;
esac

STAGE="dist/ui-agent-$RIG"
ARCHIVE="$STAGE-win64.zip"
EXE="$STAGE/ui-agent-$RIG.exe"

step "проверяю инструментарий"
command -v cargo >/dev/null || fail "cargo не найден"
command -v zip >/dev/null || fail "zip не найден"
command -v x86_64-w64-mingw32-gcc >/dev/null || fail "нет mingw-w64 линкера (brew install mingw-w64)"
rustup target list --installed 2>/dev/null | grep -qx "$TARGET" \
    || fail "нет rust-target $TARGET → rustup target add $TARGET"
if [ "$USE_UPX" -eq 1 ]; then
    command -v upx >/dev/null || fail "upx не найден (brew install upx), либо запусти с --no-upx"
fi

step "cargo build --release --target $TARGET -p ui-agent --example desktop_agent ($RIG)"
UI_AGENT_RIG="$RIG" cargo build --release --target "$TARGET" -p ui-agent --example desktop_agent

# В submodule-е target общий с хозяином
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
[ -n "$TARGET_DIR" ] || fail "cargo metadata не назвал target_directory"

step "staging $STAGE"
rm -rf "$STAGE" "$ARCHIVE"
mkdir -p "$STAGE"
cp "$TARGET_DIR/$TARGET/release/examples/desktop_agent.exe" "$EXE"

if [ "$USE_UPX" -eq 1 ]; then
    step "upx --best --lzma $(basename "$EXE")"
    upx --best --lzma "$EXE" >/dev/null
    upx -t "$EXE" >/dev/null || fail "upx: проверка упакованного exe не прошла"
fi

cat > "$STAGE/README.txt" <<TEXT
Персонаж рабочего стола: $NAME (ui-agent-$RIG.exe), Windows 10/11 x64

Запуск: двойной щелчок или из консоли. Персонаж появляется поверх рабочего стола,
ходит сам, замечает курсор, его можно щёлкнуть и перетащить.

Управление:
  правая кнопка по персонажу   меню: Пауза / Скрыть / Выход
  в консоли (запускать из cmd): help — список команд. Главное:
    all                    прогнать все действия, состояния, режимы, карточки и сценку
    play <действие>        одно действие; имена — actions
    hold <действие>        держать действие как своё состояние; release — отпустить
    leaves                 куча листьев, в которую персонаж вбегает с разбега
    random                 что-нибудь случайное
    status <состояние>     normal | thinking | busy | success | warning | error
    mode <режим>           wander | stay | follow | chase | sleep
    notify <тон> <текст>   info | success | warning | error
    move <x> <y>, spawn, despawn, stop
    p — пауза, h — скрыть/показать, q — выход

Ключи:
  --grab          разрешить персонажу ненадолго уводить курсор (по умолчанию выключено;
                  отпускает по кнопке мыши или Esc)
  --rig $OTHER    другой персонаж
  --pack <папка>  свой набор спрайтов
TEXT

step "архив $ARCHIVE"
( cd dist && zip -r -9 "$(basename "$ARCHIVE")" "$(basename "$STAGE")" >/dev/null )

printf '  ✓ %s (%s)\n' "$ARCHIVE" "$(du -h "$ARCHIVE" | cut -f1 | tr -d ' ')"
