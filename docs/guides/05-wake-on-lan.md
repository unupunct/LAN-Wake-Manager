# Wake-on-LAN: BIOS and network adapter configuration

Wake-on-LAN (WoL) needs the right hardware settings **on the computer you want to wake**. LAN Wake Manager can only send the packet and check the result;
it cannot change these settings. Do this once per computer, then use *Wake* in the dashboard. The computer's **troubleshooting report**
(device → *Troubleshoot*) checks what it can from the software side.

## Required

1. **Use Ethernet.** Most Wi-Fi adapters cannot wake a powered-off PC.
2. **BIOS/UEFI** (press Del/F2 at boot): enable *Wake on LAN*, *Power On By PCI-E/PCI*, *Resume by LAN*… (name varies). Disable *ErP / EuP Ready* and *Deep Sleep* power saving if present.
3. **Windows**
   * Device Manager → *Network adapters* → your Ethernet adapter → *Properties*:
     * *Power Management* tab: tick **Allow this device to wake the computer** and **Only allow a magic packet to wake the computer**.
     * *Advanced* tab: **Wake on Magic Packet = Enabled** (and *Shutdown Wake-On-Lan = Enabled* if present).
   * Control Panel → Power Options → *Choose what the power buttons do* → *Change settings that are currently unavailable* → **untick "Turn on fast startup"**
     (fast startup puts many adapters in a state that cannot wake).
4. **Linux**: `sudo ethtool <iface>` must show `Wake-on: g`. If it shows `d`: `sudo ethtool -s <iface> wol g`. Make it permanent with a NetworkManager
   connection setting (`nmcli connection modify <name> 802-3-ethernet.wake-on-lan magic`) or a systemd `.link` file (`[Link] WakeOnLan=magic`).
5. **MAC address**: enter the Ethernet adapter's MAC exactly (`ipconfig /all` on Windows, `ip link` on Linux). If a computer has several adapters, use the wired one.

## Network side

* The packet is a **broadcast** and must be sent inside the PC's LAN: the controller's built-in relay or a [Wake Relay](04-relay.md) on the same subnet. A packet sent over a VPN from a phone cannot wake anything by itself — that is why the app asks the relay.
* Some routers/switches drop broadcasts to sleeping ports or have "EEE/Green Ethernet" issues: if wake works only sometimes, disable Green Ethernet/EEE on the switch port.
* Set the computer's **IP address** and, ideally, a DHCP reservation, so that the relay can verify the start-up. Without an IP the packet is sent but the result stays "not verified".
* TCP check ports (Device → edit → *Reachability TCP ports*): choose ports that are open **when the PC is up**, e.g. 445 or 3389 (Windows), 22 (Linux). Windows blocks ping by default, so a TCP port or the agent gives the most reliable confirmation.

## How the result is decided

*Succeeded* only when the relay (or the agent heartbeat) proves the computer is up. If the time-out passes you get **Timeout — packet sent, computer did not come online**, with the steps that were performed. A computer that was already on is reported as such without sending anything.
After the first successful wake the computer is marked "WoL confirmed".

## Quick checklist when a wake fails

1. Does the computer wake from the *relay computer* with another tool? (e.g. `wakeonlan <mac>` on Linux.) If not: BIOS/adapter issue.
2. Is the relay on the same subnet (Troubleshoot → "Same subnet as relay")?
3. Was the PC shut down with Fast Startup, or unplugged/battery-removed for a long time (some boards forget WoL after a power loss)?
4. Is the MAC the wired adapter's?
