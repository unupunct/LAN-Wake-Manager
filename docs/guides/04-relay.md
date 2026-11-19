# Wake Relay

A sleeping computer cannot receive normal network commands, and Wake-on-LAN "magic packets" are *broadcasts* that never cross routers or VPN tunnels.
So the packet must be sent **from inside the target LAN**. That is the job of the Wake Relay.

* The **controller already contains a relay for its own LAN** ("built-in relay"). A single-location setup needs nothing else.
* Install a **separate relay** on an always-on computer (PC, mini PC, home server, Raspberry Pi running 64-bit Linux on x86-64/arm64 builds) in every *other*
  LAN: office, workshop, parents' house. Each LAN is a **network location** in the dashboard with its own inventory.

What the relay does: receives authenticated wake requests from the controller → sends magic packets on the right interface/subnet (UDP port 9 and 7,
several attempts) → **checks whether the computer really becomes reachable** (ping + TCP checks) → reports the result. It also probes device availability,
scans its own subnet for discovery, and can run **wake schedules by itself** when the controller is offline.

## Install

* Windows: `LAN-Wake-Manager-Relay-Setup.msi` ([INSTALL-WINDOWS.md](../INSTALL-WINDOWS.md)).
* Linux: `lan-wake-manager-relay_*.deb` ([INSTALL-LINUX.md](../INSTALL-LINUX.md)).
* Anywhere: `lwm-relay install-service` (administrator/root).

The relay listens on TCP **8444** (HTTPS, self-signed certificate that the controller pins). It only accepts connections from private/VPN addresses.

## Pair it with the controller

1. On the relay computer: `lwm-relay pair-code` (administrator/root). You get the **TLS fingerprint** and a **one-time code** (15 minutes).
2. The controller must reach `https://<relay-address>:8444`. With Tailscale, install it on the relay computer and use its `100.x` address/MagicDNS name.
3. Dashboard → **Locations** → create the location (e.g. *Office Network*) → **Relay** → enter the URL → **Check**. The dashboard shows the relay's fingerprint:
   compare it with the one printed in step 1 (this protects against a man-in-the-middle) → enter the code → **Pair**.
4. Status shows *online* with the last contact time. Add the location's computers (Discovery or manually) — they are now woken by *that* relay.

Every relay has its own token; the controller stores it, the relay stores only a hash. Unpair in the dashboard (the location goes back to the built-in relay) or pair again with a new code.

## Settings per location

Dashboard → Locations → Edit: UDP ports (default 9, 7), number of packet attempts (3), retry interval (2 s), wake timeout (120 s), and an optional
**broadcast address override** (e.g. `192.168.1.255`). The relay normally derives the subnet broadcast from its network interfaces and also sends to
`255.255.255.255`, only on interfaces in the target's subnet; VPN (CGNAT) interfaces are skipped.

## Relay-run schedules

A schedule with *Executor = relay* (wake only, computers of exactly one location) is pushed to that relay and runs there even if the controller or the
VPN is down. Results are collected by the controller later. Shutdown/restart schedules always run on the controller (they need the agent).

## Network layout notes

* The relay computer should use **Ethernet** and be in the **same subnet/VLAN** as the computers it wakes.
* Raspberry Pi: build from source on the Pi (`cargo build --release`) or use the arm64 build if you compile it; the released `.deb` files are x86-64.
