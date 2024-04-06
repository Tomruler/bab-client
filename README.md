# Beyond All Buttplug GUI Client (WIP)

![](bab_logo.png)<br>
Released under GPL V2+ License
# Note

This is an experimental dev environment. Users should go to the main repository at [Beyond All Buttplug](https://github.com/Tomruler/beyondallbuttplug)

# Misc Useful Information

- Rust libraries are often flagged by antiviruses. If you want to develop in Rust, use Linux or whitelist your project folders, .cargo and .rustup
- The client stops working if you minimize the window. It's piggybacked off the GUI code in a single thread, which stops running whenever the window is shrunk down. It will still work if you simply don't minimize it and leave it in the background though.