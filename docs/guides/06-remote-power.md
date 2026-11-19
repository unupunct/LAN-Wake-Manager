# Remote shutdown and restart: prerequisites

Choose **one method per computer** (Device → edit → *Power control*). All three only execute fixed, predefined commands — there is no way to run arbitrary commands through the management API.

## 1. Agent (preferred)

* Runs as a service (Windows service / systemd), connects **out** to the controller (works through NAT, no inbound port), reports CPU/RAM/disks/uptime/temperatures (only what the machine can provide; unavailable values show as "not reported"), and executes `shutdown`/`restart`/`cancel`.
* Install ([Windows](../INSTALL-WINDOWS.md) / [Linux](../INSTALL-LINUX.md)), then enroll with the one-time command from the dashboard (Device → *Agent & power* → *Create enrollment code*; valid 30 minutes). The agent verifies the controller by certificate fingerprint.
* Needs: network path agent → controller on TCP 8443 (LAN or VPN), runs as SYSTEM/root (required to shut down). Delayed shutdown, delayed restart and *cancel* are supported (`shutdown /s /t N`, `/a` on Windows; `shutdown -h +m`, `-c` on Linux).
* Revoke an agent: Device → *Agent & power* → *Revoke*.

## 2. SSH (Linux, or Windows with OpenSSH Server)

* The **controller** runs `ssh` (the OpenSSH client must be installed on the controller) with key authentication only (`BatchMode`); no passwords are stored or accepted.
* Set user and port in the device form (power method *SSH*). Key: either upload a dedicated unencrypted private key through the API (`POST /api/v1/ssh-keys` with `{name, private_key}`, admin token; it is stored in the controller's private data folder and never returned; put its `id` into the device's `ssh.key_id`) — the dashboard has no key-upload screen yet — or leave the key empty and the default key of the controller's service account (`~/.ssh/id_*` of root/SYSTEM) is used.
* Linux target: create a user, put the public key in `~/.ssh/authorized_keys`, and allow that user to power off without a password: `echo 'wake ALL=(root) NOPASSWD: /sbin/shutdown' | sudo tee /etc/sudoers.d/lan-wake` (adjust the user name and the path from `which shutdown`).
* Windows target: *Settings → Optional features → OpenSSH Server*, start the `sshd` service, add the public key to `C:\ProgramData\ssh\administrators_authorized_keys` (for administrator accounts).
* The controller must be able to reach the target's IP (same LAN, or a Tailscale subnet route — see [01-tailscale.md](01-tailscale.md)).
* New host keys are accepted on first use (`StrictHostKeyChecking=accept-new`).

## 3. Windows Remote Management (optional, Windows controller → Windows target)

Uses PowerShell remoting with the controller service account's / your domain credentials; **no password is stored**. Works best in a domain or when both computers are in a workgroup with trusted hosts configured. Prerequisites on the target:

```powershell
Enable-PSRemoting -Force                              # starts WinRM, opens the firewall for the private network
# workgroup only (on the controller): Set-Item WSMan:\localhost\Client\TrustedHosts -Value "192.168.1.50"
```
and an account that is a local administrator on the target and is accepted by the controller service (domain setups: run the controller service as a domain account with rights). This method is the most environment-dependent; use the agent if it does not work.

## What the dashboard reports

An operation ends **Succeeded** only after the computer really went offline (shutdown) or went offline and came back (restart); if the command was accepted but the machine stays up you see *Failed — still online (a program may be blocking the shutdown)*. A pending delayed shutdown can be cancelled.
