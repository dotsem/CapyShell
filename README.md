# CapyShell

CapyShell is a modern, minimalistic, and highly customizable window manager for Linux targeting tiling Wayland window managers.

> [!NOTE]
> This project aims to support Hyprland, Sway, Niri, (maybe more)
> It currently only supports Hyprland.

> [!IMPORTANT]
> This project is still in alpha phase, new features are still being added. View [the roadmap](https://github.com/dotsem/CapyShell/blob/master/ROADMAP.md) to see which features are in the process of being added for the first release.

## How does it work?

CapyShell is built with Rust & Slint. It is built with multiple "capy" crates that can be used to create your own desktop widgets, panels, ... (whatever your heart may desire)
Capyshell uses the [Spell Framework](https://github.com/VimYoung/Spell) to bring the Slint windows to your desktop (with layer shell protocol).

## Features

CapyShell includes multiple "panels" who substitute your taskbar, notification center, settings, etc.

### Taskbar

The taskbar includes your workspaces, media controls, active application and system info

#### Workspaces

<img src="docs/readme/image.png" alt="Workspaces">

The workspaces are displayed as a row of buttons, each representing a workspace. The active workspace is highlighted with the primary color. The icon of the active app is shown in the center of the workspace button.
The icons are fetched with a custom icon finder that should find every icon on your system. If not, please open an issue on GitHub with app that is missing the icon, installation method (pacman, aur, flatpak, etc.) & the path the icon should be found on.

#### Media controls

wip

#### Active application 

wip

#### battery

wip

#### Time

wip

#### Systray

wip

#### Volume

wip

#### Network

wip

#### Bluetooth

wip


     
