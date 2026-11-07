#!/bin/sh
# Records the catalog preview, asciiverse/tspace.cast: 100x30, `tspace --demo --teams`
# (eight bots, Red against Blue, the camera on the leader), with the scores card
# shown for a few seconds in the middle.
# Runs asciinema in a detached tmux pane, so nothing from this terminal gets
# in; a throwaway HOME keeps the session log out of the way.
#
#   cargo build --release && asciiverse/record-preview.sh
set -eu
cd "$(dirname "$0")/.."
out=asciiverse/tspace.cast
tmp=$(mktemp -d)
trap 'tmux -L tspace-rec kill-server 2>/dev/null || true; rm -rf "$tmp"' EXIT
rm -f "$out"
tmux -L tspace-rec new-session -d -s rec -x 100 -y 30 \
  "env -i PATH=$PWD/target/release:/usr/bin:/bin:/snap/bin HOME=$tmp USER=player LOGNAME=player \
     TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 \
   asciinema rec --window-size 100x30 --idle-time-limit 2 \
     --capture-env TERM,COLORTERM,LANG --title 'tspace' \
     --command 'tspace --demo --teams --mute' $out"
sleep 28
tmux -L tspace-rec send-keys -t rec Tab
sleep 5
tmux -L tspace-rec send-keys -t rec Tab
sleep 14
tmux -L tspace-rec send-keys -t rec C-c
sleep 2
ls -la "$out"
