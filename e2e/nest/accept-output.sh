#!/bin/sh
# Unattended output chooser for xdg-desktop-portal-wlr in nested e2e.
# Newer xdpw expects "Monitor: NAME"; older builds accept bare NAME.
out="${E2E_WL_OUTPUT:-WL-1}"
printf 'Monitor: %s\n' "$out"
