# mentalOS Configuration Files

## /etc/hostname
```
mentalOS
```

## /etc/hosts
```
127.0.0.1   localhost
::1         localhost
127.0.1.1   mentalOS.localdomain mentalOS
```

## /etc/locale.gen (uncomment)
```
en_US.UTF-8 UTF-8
```

## /etc/vconsole.conf
```
KEYMAP=us
FONT=
FONT_MAP=
```

## /etc/fstab (generated)
```
# <device>                                 <mount point>  <type>  <options>       <dump>  <pass>
UUID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx  /              ext4    rw,relatime     0       1
UUID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx  /boot/efi      vfat    rw,relatime,fmask=0022,dmask=0022,codepage=437,iocharset=ascii,shortname=mixed,utf8  0       2
UUID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx  /home          ext4    rw,relatime     0       2
UUID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx  none           swap    sw              0       0
```

## /etc/systemd/system/getty@tty1.service.d/override.conf
```
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin alick --noclear %I $TERM
```

## /boot/loader/loader.conf
```
default mentalOS
timeout 5
editor  no
```

## /boot/loader/entries/mentalOS.conf
```
title   mentalOS
linux   /vmlinuz-linux
initrd  /initramfs-linux.img
options root=UUID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx rw quiet splash
```

## ~/.config/sway/config
```bash
# mentalOS Sway Configuration

set $mod Mod4
set $term alacritty
set $menu wofi --show drun

default_border pixel 3
hide_edge_borders smart

# Colors
set $bg #1e1e2e
set $fg #cdd6f4
set $accent #89b4fa

client.background $bg
client.focused $accent $bg $fg $accent
client.unfocused $bg $bg $fg $bg

input * {
    xkb_layout us
    xkb_options caps:escape
}

output * bg #1e1e2e solid_color

bindsym $mod+Return exec $term
bindsym $mod+d exec $menu
bindsym $mod+Shift+e exit
bindsym $mod+l exec swaylock -f

exec_always mentalOS

bar {
    position top
    status_command while date +'%Y-%m-%d %H:%M'; do sleep 1; done
    colors {
        background $bg
        statusline $fg
        focused_workspace $accent $bg $fg
        inactive_workspace $bg $bg $fg
    }
}
```