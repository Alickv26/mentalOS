#!/usr/bin/env bash
# Validate the mentalOS archiso profile structure without building the ISO.
#
# This script checks that:
#   - All required profile files are present
#   - profiledef.sh is valid bash and defines required vars
#   - packages.x86_64 has no blank lines, comments, or duplicates
#   - pacman.conf has the [core] repo enabled
#   - Boot loader configs reference the expected kernel/initramfs paths
#   - customize_airootfs.sh is valid bash
#   - Helper scripts (mentalos-install, mentalos-launch) are executable
#
# Usage: ./validate-profile.sh
# Exit 0 if all checks pass, 1 otherwise.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILE_DIR="${SCRIPT_DIR}"
PASS=0
FAIL=0

ok() {
    printf '  \033[32mOK\033[0m   %s\n' "$1"
    PASS=$((PASS + 1))
}

err() {
    printf '  \033[31mFAIL\033[0m %s\n' "$1"
    FAIL=$((FAIL + 1))
}

section() {
    printf '\n\033[1m=== %s ===\033[0m\n' "$1"
}

# ─── Required files ──────────────────────────────────────────

section "Required files"

required_files=(
    "profiledef.sh"
    "packages.x86_64"
    "pacman.conf"
    "airootfs/root/customize_airootfs.sh"
    "airootfs/usr/local/bin/mentalos-install"
    "airootfs/usr/local/bin/mentalos-launch"
    "airootfs/etc/systemd/system/mentalOS.service"
    "airootfs/etc/systemd/system/openclaw.service"
    "airootfs/etc/systemd/system/ollama.service"
    "airootfs/etc/systemd/system/workspace-monitor.service"
    "build-iso.sh"
    "test-qemu.sh"
    "efiboot/loader/loader.conf"
    "efiboot/loader/entries/01-mentalos-linux.conf"
    "syslinux/syslinux.cfg"
    "syslinux/syslinux-linux.cfg"
)

# Note: mentalos-workspace-monitor binary is NOT in the airootfs tree —
# it's a Rust binary built by build-iso.sh and staged at build time.
# Check for its source in the prototype crate instead.
REPO_ROOT="$(cd "${PROFILE_DIR}/../.." && pwd)"
WORKSPACE_MONITOR_SRC="${REPO_ROOT}/mentalOS-prototype/src/bin/workspace_monitor.rs"

for f in "${required_files[@]}"; do
    if [[ -f "${PROFILE_DIR}/${f}" ]]; then
        ok "${f}"
    else
        err "missing required file: ${f}"
    fi
done

# mentalos-workspace-monitor is a Rust binary built by build-iso.sh —
# check its source exists in the prototype crate.
if [[ -f "${WORKSPACE_MONITOR_SRC}" ]]; then
    ok "mentalos-workspace-monitor.rs source present"
else
    err "missing mentalos-workspace-monitor.rs at ${WORKSPACE_MONITOR_SRC}"
fi

# Check that build-iso.sh stages the workspace-monitor binary
if grep -q 'mentalos-workspace-monitor' "${PROFILE_DIR}/build-iso.sh" 2>/dev/null; then
    ok "build-iso.sh stages mentalos-workspace-monitor"
else
    err "build-iso.sh should stage mentalos-workspace-monitor"
fi

# ─── profiledef.sh ──────────────────────────────────────────

section "profiledef.sh"

PROFILEDEF="${PROFILE_DIR}/profiledef.sh"
if [[ -f "${PROFILEDEF}" ]]; then
    if bash -n "${PROFILEDEF}"; then
        ok "profiledef.sh is valid bash"
    else
        err "profiledef.sh has bash syntax errors"
    fi

    # Check for required variable assignments
    for var in iso_name iso_label iso_publisher iso_application iso_version arch pacman_conf; do
        if grep -q "^${var}=" "${PROFILEDEF}"; then
            ok "profiledef.sh defines ${var}"
        else
            err "profiledef.sh missing ${var}="
        fi
    done

    # Check bootmodes is a non-empty array
    if grep -q "^bootmodes=(" "${PROFILEDEF}"; then
        if [[ "$(grep -c "'" "${PROFILEDEF}" 2>/dev/null || echo 0)" -ge 2 ]]; then
            ok "profiledef.sh defines bootmodes array"
        else
            err "profiledef.sh bootmodes array looks empty"
        fi
    else
        err "profiledef.sh missing bootmodes array"
    fi

    # Check file_permissions is defined (key for /etc/shadow)
    if grep -q "^file_permissions=(" "${PROFILEDEF}"; then
        if grep -q '/etc/shadow' "${PROFILEDEF}"; then
            ok "profiledef.sh sets /etc/shadow permissions"
        else
            err "profiledef.sh should set /etc/shadow permissions"
        fi
    else
        err "profiledef.sh missing file_permissions array"
    fi
fi

# ─── packages.x86_64 ────────────────────────────────────────

section "packages.x86_64"

PKGS="${PROFILE_DIR}/packages.x86_64"
if [[ -f "${PKGS}" ]]; then
    # No blank lines or comments
    if grep -qE '^\s*$' "${PKGS}"; then
        err "packages.x86_64 contains blank lines"
    else
        ok "packages.x86_64 has no blank lines"
    fi

    if grep -qE '^\s*#' "${PKGS}"; then
        err "packages.x86_64 contains comments (archiso rejects them)"
    else
        ok "packages.x86_64 has no comments"
    fi

    # Check for duplicate packages
    duplicates=$(sort "${PKGS}" | uniq -d)
    if [[ -z "${duplicates}" ]]; then
        ok "packages.x86_64 has no duplicates"
    else
        err "packages.x86_64 has duplicates: ${duplicates}"
    fi

    # Check for essential packages
    for pkg in base linux systemd networkmanager; do
        if grep -qx "${pkg}" "${PKGS}"; then
            ok "packages.x86_64 includes ${pkg}"
        else
            err "packages.x86_64 missing essential package: ${pkg}"
        fi
    done

    # Check for mentalOS-specific packages
    for pkg in gtk4 sway firejail; do
        if grep -qx "${pkg}" "${PKGS}"; then
            ok "packages.x86_64 includes ${pkg}"
        else
            err "packages.x86_64 missing mentalOS package: ${pkg}"
        fi
    done
fi

# ─── pacman.conf ─────────────────────────────────────────────

section "pacman.conf"

PACMAN_CONF="${PROFILE_DIR}/pacman.conf"
if [[ -f "${PACMAN_CONF}" ]]; then
    if bash -n "${PACMAN_CONF}" 2>/dev/null || grep -q '\[' "${PACMAN_CONF}"; then
        ok "pacman.conf is parseable"
    else
        err "pacman.conf has syntax errors"
    fi

    if grep -q '^\[core\]' "${PACMAN_CONF}"; then
        ok "pacman.conf has [core] repo"
    else
        err "pacman.conf missing [core] repo"
    fi

    if grep -q '^\[extra\]' "${PACMAN_CONF}"; then
        ok "pacman.conf has [extra] repo"
    else
        err "pacman.conf missing [extra] repo"
    fi
fi

# ─── Boot loader configs ─────────────────────────────────────

section "Boot loader configs"

EFI_ENTRY="${PROFILE_DIR}/efiboot/loader/entries/01-mentalos-linux.conf"
if [[ -f "${EFI_ENTRY}" ]]; then
    if grep -q '^title' "${EFI_ENTRY}"; then
        ok "efiboot entry has title"
    else
        err "efiboot entry missing title"
    fi

    if grep -q '^linux' "${EFI_ENTRY}" && grep -q 'vmlinuz-linux' "${EFI_ENTRY}"; then
        ok "efiboot entry references vmlinuz-linux"
    else
        err "efiboot entry should reference vmlinuz-linux"
    fi

    if grep -q '^initrd' "${EFI_ENTRY}" && grep -q 'initramfs-linux.img' "${EFI_ENTRY}"; then
        ok "efiboot entry references initramfs-linux.img"
    else
        err "efiboot entry should reference initramfs-linux.img"
    fi

    if grep -q 'archisobasedir' "${EFI_ENTRY}" && grep -q 'archisosearchuuid' "${EFI_ENTRY}"; then
        ok "efiboot entry has archiso kernel options"
    else
        err "efiboot entry missing archiso kernel options"
    fi
fi

SYSLINUX_CFG="${PROFILE_DIR}/syslinux/syslinux.cfg"
if [[ -f "${SYSLINUX_CFG}" ]]; then
    if grep -qi 'MENU TITLE mentalOS' "${SYSLINUX_CFG}"; then
        ok "syslinux.cfg has mentalOS menu title"
    else
        err "syslinux.cfg missing mentalOS menu title"
    fi

    if grep -q 'TIMEOUT' "${SYSLINUX_CFG}"; then
        ok "syslinux.cfg has TIMEOUT"
    else
        err "syslinux.cfg missing TIMEOUT"
    fi
fi

# ─── Systemd service units ───────────────────────────────────

section "Systemd service units"

service_files=(
    "airootfs/etc/systemd/system/mentalOS.service"
    "airootfs/etc/systemd/system/openclaw.service"
    "airootfs/etc/systemd/system/ollama.service"
    "airootfs/etc/systemd/system/workspace-monitor.service"
)

for svc in "${service_files[@]}"; do
    path="${PROFILE_DIR}/${svc}"
    if [[ ! -f "${path}" ]]; then
        err "missing service file: ${svc}"
        continue
    fi

    name=$(basename "${svc}")

    # Must be a valid INI-ish systemd unit (look for [Unit] and [Service] sections)
    if grep -q '^\[Unit\]' "${path}" && grep -q '^\[Service\]' "${path}"; then
        ok "${name} has [Unit] and [Service] sections"
    else
        err "${name} missing [Unit] or [Service] section"
    fi

    # Must have Description=
    if grep -q '^Description=' "${path}"; then
        ok "${name} has Description="
    else
        err "${name} missing Description="
    fi

    # Must have ExecStart=
    if grep -q '^ExecStart=' "${path}"; then
        ok "${name} has ExecStart="
    else
        err "${name} missing ExecStart="
    fi

    # Must have Restart= for resilience
    if grep -q '^Restart=' "${path}"; then
        ok "${name} has Restart="
    else
        err "${name} missing Restart= (services should auto-restart on failure)"
    fi

    # Must be enabled via [Install] section
    if grep -q '^\[Install\]' "${path}" && grep -q '^WantedBy=' "${path}"; then
        ok "${name} has [Install] WantedBy="
    else
        err "${name} missing [Install] WantedBy="
    fi
done

# Check that mentalOS.service depends on openclaw and ollama
MENTALOS_SVC="${PROFILE_DIR}/airootfs/etc/systemd/system/mentalOS.service"
if [[ -f "${MENTALOS_SVC}" ]]; then
    if grep -q '^Wants=openclaw.service ollama.service' "${MENTALOS_SVC}"; then
        ok "mentalOS.service Wants= openclaw + ollama"
    else
        err "mentalOS.service should Wants= openclaw.service ollama.service"
    fi

    if grep -q '^After=graphical.target' "${MENTALOS_SVC}"; then
        ok "mentalOS.service starts After= graphical.target"
    else
        err "mentalOS.service should start After= graphical.target"
    fi

    if grep -q '^WantedBy=graphical.target' "${MENTALOS_SVC}"; then
        ok "mentalOS.service WantedBy= graphical.target"
    else
        err "mentalOS.service should be WantedBy= graphical.target"
    fi
fi

# Check that openclaw.service and ollama.service are WantedBy= multi-user.target
for svc_name in openclaw ollama workspace-monitor; do
    svc_path="${PROFILE_DIR}/airootfs/etc/systemd/system/${svc_name}.service"
    if [[ -f "${svc_path}" ]]; then
        if grep -q '^WantedBy=multi-user.target' "${svc_path}"; then
            ok "${svc_name}.service WantedBy= multi-user.target"
        else
            err "${svc_name}.service should be WantedBy= multi-user.target"
        fi
    fi
done

# Check that customize_airootfs.sh enables all 4 services
CUSTOMIZE="${PROFILE_DIR}/airootfs/root/customize_airootfs.sh"
if [[ -f "${CUSTOMIZE}" ]]; then
    for svc in mentalOS openclaw ollama workspace-monitor; do
        if grep -q "systemctl enable ${svc}.service" "${CUSTOMIZE}"; then
            ok "customize_airootfs.sh enables ${svc}.service"
        else
            err "customize_airootfs.sh should enable ${svc}.service"
        fi
    done
fi

# ─── Shell scripts syntax ────────────────────────────────────

section "Shell script syntax"

shell_scripts=(
    "profiledef.sh"
    "airootfs/root/customize_airootfs.sh"
    "airootfs/usr/local/bin/mentalos-install"
    "airootfs/usr/local/bin/mentalos-launch"
    "build-iso.sh"
    "test-qemu.sh"
    "validate-profile.sh"
)

for s in "${shell_scripts[@]}"; do
    script="${PROFILE_DIR}/${s}"
    if [[ -f "${script}" ]]; then
        if bash -n "${script}"; then
            ok "${s} is valid bash"
        else
            err "${s} has bash syntax errors"
        fi

        if [[ -x "${script}" ]]; then
            ok "${s} is executable"
        else
            err "${s} is not executable (chmod +x needed)"
        fi
    fi
done

# ─── Summary ─────────────────────────────────────────────────

printf '\n'
printf '\033[1mValidation summary:\033[0m %d passed, %d failed\n' "${PASS}" "${FAIL}"

if [[ "${FAIL}" -gt 0 ]]; then
    exit 1
fi
exit 0
