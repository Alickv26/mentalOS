# Firejail profile for mentalOS OpenClaw
# Description: Sandbox for executing potentially unsafe AI-generated commands
# persistent local customizations
include openclaw.local
# persistent global definitions
include globals.local

# Allow network access for API calls (optional, can be disabled if local only)
protocol unix,inet,inet6
netfilter

# Blacklist sensitive directories
blacklist /boot
blacklist /root
blacklist /etc/shadow
blacklist /etc/ssh

# Restrict home directory access
# Only allow access to workspaces and config
noblacklist ${HOME}/workspaces
noblacklist ${HOME}/.config/mentalOS
noblacklist ${HOME}/.local/share/mentalOS

# Read-only access to most system files
read-only /
read-write ${HOME}/workspaces
read-only ${HOME}/.config/mentalOS

# Prevent robust privilege escalation
caps.drop all
seccomp
noroot
no3d
nodvd
nogroups
nonewprivs
nosound
notv
nou2f
novideo

# Private directories
private-dev
private-tmp

# Environment
env MENTALOS_SANDBOX=1
