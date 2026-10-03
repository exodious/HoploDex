#!/usr/bin/env bash
# For examples/pdf_spike.sh: describes each WebKit web process while the
# preview is open (whether it is confined, and what it can see), compared
# with the spike's own process.
#   pdf_spike_procs.sh SPIKE_PID CANARY_FILE
SPIKE=$1
CANARY=$2
# Only the spike's own descendants, never another app's WebKit processes.
descendants() { for c in $(pgrep -P "$1"); do echo "$c"; descendants "$c"; done; }
KIDS=$(descendants "$SPIKE")
echo "SPIKE PROCS descendants: $(for k in $KIDS; do printf '%s ' "$(cat /proc/$k/comm)"; done)"
for pid in $KIDS; do
  case "$(tr '\0' ' ' < /proc/$pid/cmdline)" in *WebKitWebProcess*|*WebKitNetworkProcess*) ;; *) continue ;; esac
  name=$(tr '\0' ' ' < /proc/$pid/cmdline | cut -c1-60)
  echo "SPIKE PROCS pid $pid: $name"
  for ns in net mnt pid user ipc; do
    mine=$(readlink /proc/$SPIKE/ns/$ns)
    theirs=$(readlink /proc/$pid/ns/$ns 2>&1)
    [ "$mine" = "$theirs" ] && same=shared || same=SEPARATE
    echo "SPIKE PROCS   ns $ns: $same ($theirs)"
  done
  grep -E "^(Seccomp|NoNewPrivs):" /proc/$pid/status | sed 's/^/SPIKE PROCS   /'
  # What the process sees of the filesystem: the canary in HOME and a few dirs.
  root=/proc/$pid/root
  if [ -e "$root$CANARY" ]; then echo "SPIKE PROCS   canary in HOME: VISIBLE"; else echo "SPIKE PROCS   canary in HOME: not visible ($(ls -d $root$(dirname $CANARY) 2>&1 | tail -c 60))"; fi
  for d in /workspace /home /tmp /etc/passwd /run/user; do
    if [ -e "$root$d" ]; then echo "SPIKE PROCS   $d: present ($(ls $root$d 2>/dev/null | head -3 | tr '\n' ' '))"; else echo "SPIKE PROCS   $d: absent"; fi
  done
done
