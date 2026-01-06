# Roadmap for Capyshell

## Todo for first release

The first release will include a taskbar and a basic app launcher. It is not planned to include the panels yet.

### additions in the first release, somewhat in order

[x] system information (battery, clock, volume, network, bluetooth)
[x] media player
[x] show workspaces
[x] add active window
[x] fix memory leak for media player
[ ] distribution icon on left side (will later open a menu)
[ ] fix music player animated playing bars
[ ] split up the project in different crates
[ ] improve icon & app finding crate
[ ] create crate for shared window manager communication -> 1 crate communicating with the current window manager. This crate will then share generalized data to each crate that asks for information. This means that most of the window manager information is only fetched once and updated effeciently
[ ] create selector for media source for media player
[ ] add app basic app launcher (directly create a standard menu for this to share other menu items)
[ ] add generalized config file with following configs
    - show seconds
    - DD/MM/YYYY or MM/DD/YYYY
    - show amount of bluetooth connections (only when multiple connections are active)
    - custom (distribution) icon
    - workspace configs
        - show const amount of workspaces
        - show all workspaces or only per monitor
    - active window configs
        - show active window on all monitor
        - show each active window on each monitor
        - only show active window on active monitor
    - default media player (or none)
[ ] update readme & add documentation & examples
[ ] measure performance (& maybe optimize if possible)
[ ] create website with svlendid

after this, a first release will be done

### features after first release

[ ] -(right) config menu to config volume, bluetooth, network, vpn (& monitor system?)
[ ] -(left) notification menu, system health, ?
[ ] -(shared with app launcher) power menu
[ ] -(shared with app launcher) programming environment launcher -> launch different projects, these projects automatically open the right editor & can automatically start a docker container (if needed)
[ ] nofitication popup

there will even be more features, but my brain still has to find them...
if you have a feature request, feel free to leave it as an 'issue' with a feature request tag
