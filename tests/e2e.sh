#!/bin/sh
# End-to-end test of the daemon without root: TTO_PREFIX relocates every
# path into a scratch dir, and a decoy "ollama" (really /bin/sleep) stands
# in for a local model. Run from the repo root after `cargo build`.
set -eu
TTO=${TTO:-target/debug/tto}
P=$(mktemp -d)
export TTO_PREFIX="$P"
mkdir -p "$P/etc" "$P/var/run" "$P/var/db/tto"
printf '127.0.0.1 localhost\n::1 localhost\n' > "$P/etc/hosts"
cp "$P/etc/hosts" "$P/hosts.orig"

# decoy local model
mkdir -p "$P/bin"; cp /bin/sleep "$P/bin/ollama"
"$P/bin/ollama" 600 & DECOY=$!

"$TTO" daemon 2> "$P/daemon.log" & DAEMON=$!
trap 'kill $DAEMON $DECOY 2>/dev/null; rm -rf "$P"' EXIT
for _ in $(seq 50); do [ -S "$P/var/run/tto.sock" ] && break; sleep 0.1; done
[ -S "$P/var/run/tto.sock" ] || { echo "daemon never listened"; cat "$P/daemon.log"; exit 1; }

echo "== status before"
"$TTO" status | grep -q "Everything is on"

echo "== stop is not a thing"
"$TTO" stop 2>&1 | grep -q "There is no stop"
"$TTO" cancel 2>&1 | grep -q "There is no stop"

echo "== oversized and unterminated requests are refused, daemon stays up"
python3 - "$P/var/run/tto.sock" <<'PY'
import socket,sys
s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(b"x"*10000); s.settimeout(3)
r=s.recv(4096); assert b"too long" in r, r
PY
"$TTO" status | grep -q "Everything is on"

echo "== off 1m, local only"
"$TTO" off 1m --only local -y | grep -q "Off. Back at"
sleep 1.5
cmp -s "$P/etc/hosts" "$P/hosts.orig" || { echo "local-only block must not touch hosts"; exit 1; }
kill -0 $DECOY 2>/dev/null && { echo "decoy ollama still alive"; exit 1; }
echo "decoy killed, hosts untouched"

echo "== widen to chat: hosts gains domains"
"$TTO" off 1m --only chat -y >/dev/null
sleep 1.5
grep -q ">>> tto" "$P/etc/hosts" || { echo "hosts section missing"; exit 1; }
grep -q "0.0.0.0 claude.ai" "$P/etc/hosts"
grep -q "0.0.0.0 api.anthropic.com" "$P/etc/hosts" && { echo "coding domains leaked into chat block"; exit 1; }
"$TTO" status | grep -q "local, chat"

echo "== a shorter request never shortens"
UNTIL_BEFORE=$(python3 -c "import json;print(json.load(open('$P/var/db/tto/state.json'))['until'])")
"$TTO" off 5h --only chat -y >/dev/null
UNTIL_AFTER=$(python3 -c "import json;print(json.load(open('$P/var/db/tto/state.json'))['until'])")
[ "$UNTIL_AFTER" -gt "$UNTIL_BEFORE" ] || { echo "extend failed"; exit 1; }
"$TTO" off 1m --only chat -y >/dev/null
UNTIL_AGAIN=$(python3 -c "import json;print(json.load(open('$P/var/db/tto/state.json'))['until'])")
[ "$UNTIL_AGAIN" -eq "$UNTIL_AFTER" ] || { echo "block got shorter!"; exit 1; }

echo "== restart the daemon mid-block: still blocked"
kill $DAEMON; wait $DAEMON 2>/dev/null || true
"$TTO" daemon 2>> "$P/daemon.log" & DAEMON=$!
for _ in $(seq 50); do [ -S "$P/var/run/tto.sock" ] && break; sleep 0.1; done
"$TTO" status | grep -q "Off ("

echo "== expiry restores hosts exactly"
python3 - "$P/var/db/tto/state.json" <<'PY'
import json,sys,time
p=sys.argv[1]; s=json.load(open(p)); s['until']=int(time.time())+2; json.dump(s,open(p,'w'))
PY
kill $DAEMON; wait $DAEMON 2>/dev/null || true
"$TTO" daemon 2>> "$P/daemon.log" & DAEMON=$!
sleep 4
cmp -s "$P/etc/hosts" "$P/hosts.orig" || { echo "hosts not restored"; diff "$P/etc/hosts" "$P/hosts.orig"; exit 1; }
"$TTO" status | grep -q "Everything is on"
echo "== all good"
