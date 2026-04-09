# Auto-start sway on tty1 for autologin user
if [ -z "${DISPLAY:-}" ] && [ "${XDG_VTNR:-0}" -eq 1 ]; then
  exec sway
fi
