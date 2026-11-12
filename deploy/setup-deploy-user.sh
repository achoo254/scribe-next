#!/bin/bash
# Creates the restricted `scribe-deploy` user that GitHub Actions deploys with
# (run as root on the server; the public key comes on stdin):
#   bash deploy/setup-deploy-user.sh < scribe-deploy.pub   (from a copy of deploy/ on the server)
# With empty stdin (`< /dev/null`) it keeps the installed key and only
# reinstalls the sudo command. The user can only write /opt/scribe-be/ci-incoming
# and run one root-owned command through sudo:
# /usr/local/sbin/scribe-activate-incoming <release-id>.
set -euo pipefail
USER_NAME=scribe-deploy
ROOT=/opt/scribe-be
[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }

pub="$(cat)"
id "$USER_NAME" >/dev/null 2>&1 || useradd --create-home --shell /bin/bash "$USER_NAME"
home="$(getent passwd "$USER_NAME" | cut -d: -f6)"
if [ -n "$pub" ]; then
	[[ "$pub" =~ ^ssh-ed25519\ [A-Za-z0-9+/=]+(\ .*)?$ ]] || { echo "expected one ssh-ed25519 public key on stdin" >&2; exit 2; }
	install -d -m 700 -o "$USER_NAME" -g "$USER_NAME" "$home/.ssh"
	# `restrict`: no port/agent/X11 forwarding and no pty.
	printf 'restrict %s\n' "$pub" >"$home/.ssh/authorized_keys"
	chown "$USER_NAME:$USER_NAME" "$home/.ssh/authorized_keys"
	chmod 600 "$home/.ssh/authorized_keys"
elif [ ! -s "$home/.ssh/authorized_keys" ]; then
	echo "no key installed yet; pass the public key on stdin" >&2
	exit 2
fi

install -d -m 750 -o "$USER_NAME" -g "$USER_NAME" "$ROOT/ci-incoming"
install -m 755 -o root -g root "$(dirname "$0")/activate-incoming.sh" /usr/local/sbin/scribe-activate-incoming
printf '%s ALL=(root) NOPASSWD: /usr/local/sbin/scribe-activate-incoming\n' "$USER_NAME" >/etc/sudoers.d/scribe-deploy
chmod 440 /etc/sudoers.d/scribe-deploy
visudo -cf /etc/sudoers.d/scribe-deploy
echo "setup-deploy-user: done"
