#!/bin/sh
set -eu

SOCK_PATH="${JANUS_GIT_SSH_AUTH_SOCK:-/var/run/janus/ssh-agent.sock}"
KEY_FILE_PATH="${JANUS_GIT_SSH_PRIVATE_KEY_FILE:-}"
KEY_B64="${JANUS_GIT_SSH_PRIVATE_KEY_B64:-}"
KEY_INLINE="${JANUS_GIT_SSH_PRIVATE_KEY:-}"
TMP_KEY_FILE=""

log() {
  printf '%s\n' "$*" >&2
}

cleanup_tmp() {
  if [ -n "$TMP_KEY_FILE" ] && [ -f "$TMP_KEY_FILE" ]; then
    rm -f "$TMP_KEY_FILE" || true
  fi
}

write_tmp_key_from_b64() {
  TMP_KEY_FILE="$(mktemp /tmp/janus-git-ssh-key.XXXXXX)"
  chmod 600 "$TMP_KEY_FILE"
  printf '%s' "$KEY_B64" | base64 -d >"$TMP_KEY_FILE" || {
    log "Janus SSH agent: failed to decode JANUS_GIT_SSH_PRIVATE_KEY_B64"
    exit 1
  }
}

write_tmp_key_from_inline() {
  TMP_KEY_FILE="$(mktemp /tmp/janus-git-ssh-key.XXXXXX)"
  chmod 600 "$TMP_KEY_FILE"
  # Normalize CRLF from env-file based key injection.
  printf '%s\n' "$KEY_INLINE" | sed 's/\r$//' >"$TMP_KEY_FILE"
}

resolve_key_file() {
  if [ -n "$KEY_FILE_PATH" ]; then
    if [ ! -r "$KEY_FILE_PATH" ]; then
      log "Janus SSH agent: JANUS_GIT_SSH_PRIVATE_KEY_FILE is not readable: $KEY_FILE_PATH"
      exit 1
    fi
    printf '%s' "$KEY_FILE_PATH"
    return
  fi

  if [ -n "$KEY_B64" ]; then
    write_tmp_key_from_b64
    printf '%s' "$TMP_KEY_FILE"
    return
  fi

  if [ -n "$KEY_INLINE" ]; then
    write_tmp_key_from_inline
    printf '%s' "$TMP_KEY_FILE"
    return
  fi

  printf ''
}

start_agent_with_key() {
  key_file="$1"
  mkdir -p "$(dirname "$SOCK_PATH")"
  rm -f "$SOCK_PATH"

  eval "$(ssh-agent -a "$SOCK_PATH" -s)" >/dev/null
  if ! ssh-add "$key_file" >/dev/null 2>&1; then
    log "Janus SSH agent: failed to add key (passphrase-protected keys are not supported in headless mode)"
    exit 1
  fi

  export JANUS_GIT_SSH_AUTH_SOCK="$SOCK_PATH"
  export SSH_AUTH_SOCK="$SOCK_PATH"
  log "Janus SSH agent: enabled at $SOCK_PATH"
}

main() {
  key_file="$(resolve_key_file)"

  if [ -n "$key_file" ]; then
    start_agent_with_key "$key_file"
  else
    log "Janus SSH agent: no key configured; set JANUS_GIT_SSH_PRIVATE_KEY_FILE or JANUS_GIT_SSH_PRIVATE_KEY_B64"
  fi

  cleanup_tmp

  # Prevent accidental exposure in child process environment.
  unset JANUS_GIT_SSH_PRIVATE_KEY
  unset JANUS_GIT_SSH_PRIVATE_KEY_B64

  exec /usr/local/bin/janusd "$@"
}

main "$@"
