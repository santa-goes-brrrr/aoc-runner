#!/usr/bin/env bash
set -euo pipefail

output="${1:-target/aoc-evaluator/roi-counter.so}"
mkdir -p "$(dirname "$output")"

read -ra glib_cflags <<<"$(pkg-config --cflags glib-2.0)"

cc -std=c11 -O2 -Wall -Wextra -Werror -fPIC -fvisibility=hidden -shared \
  "${glib_cflags[@]}" \
  evaluator/roi-counter.c \
  -o "$output"
