"""Starts a real controller + relay + agent on loopback and fills them with demo data (used for screenshots).
Machines are simulated by TCP listeners on 127.0.0.x:4455 ("online") - everything else is the real software.
Usage: python tools/demo_seed.py   (keeps running; Ctrl+C stops everything)
"""
import json, os, re, socket, ssl, subprocess, sys, tempfile, threading, time, urllib.request, urllib.error

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug")
EXE = ".exe" if os.name == "nt" else ""
T = os.environ.get("DEMO_DIR") or tempfile.mkdtemp(prefix="lwm-demo-")
HTTPS, LOCAL, RELAY = 18443, 18480, 18444
PORT = 4455
CTX = ssl.create_default_context(); CTX.check_hostname = False; CTX.verify_mode = ssl.CERT_NONE
procs = []

def call(method, url, body=None, token=None):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method)
    if data is not None: r.add_header("Content-Type", "application/json")
    if token: r.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(r, timeout=30, context=CTX) as resp:
            t = resp.read().decode(); return json.loads(t) if t else None
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {url}: {e.code} {e.read().decode()}")

def start(name, args, env=None):
    e = dict(os.environ); e.update(env or {})
    p = subprocess.Popen([os.path.join(BIN, name + EXE)] + args, stdout=open(os.path.join(T, name + ".log"), "a"), stderr=subprocess.STDOUT, env=e)
    procs.append(p); return p

def listen(ip):
    s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind((ip, PORT)); s.listen(50)
    def run():
        while True:
            try: c, _ = s.accept(); c.close()
            except Exception: return
    threading.Thread(target=run, daemon=True).start()

L = f"http://127.0.0.1:{LOCAL}/api/v1"
env = {"LWM_NO_ICMP": "1", "LWM_DRY_RUN": "1"}
start("lwm-controller", ["--data-dir", T + "/c", "run", "--listen", f"127.0.0.1:{HTTPS}", "--local-listen", f"127.0.0.1:{LOCAL}"], env)
start("lwm-relay", ["--data-dir", T + "/r", "run", "--listen", f"127.0.0.1:{RELAY}", "--name", "office-nuc"], env)
for _ in range(60):
    try: call("GET", L + "/info"); break
    except Exception: time.sleep(0.5)
tok = call("POST", L + "/auth/setup", {"password": "demo-password-123"})["token"]
info = call("GET", L + "/info")
call("PUT", L + "/settings", dict(call("GET", L + "/settings", token=tok), name="Home controller", monitor_interval_secs=5), tok)
home = call("GET", L + "/locations", token=tok)[0]
call("PUT", f"{L}/locations/{home['id']}", {"name": "Home Network", "wake_timeout_secs": 15, "wake_attempts": 2}, tok)
office = call("POST", L + "/locations", {"name": "Office Network", "color": "#22d3ee"}, tok)
# pair the office relay
pc = subprocess.run([os.path.join(BIN, "lwm-relay" + EXE), "--data-dir", T + "/r", "pair-code"], capture_output=True, text=True).stdout
code = re.search(r"Pairing code\s*:\s*([A-Z0-9-]{9})", pc).group(1)
probe = call("POST", f"{L}/locations/{office['id']}/relay/probe", {"url": f"https://127.0.0.1:{RELAY}"}, tok)
call("POST", f"{L}/locations/{office['id']}/relay/pair", {"url": f"https://127.0.0.1:{RELAY}", "code": code, "fingerprint": probe["fingerprint"]}, tok)
g = {x["name"]: x["id"] for x in call("GET", L + "/groups", token=tok)}
D = [
    (home, "Gaming PC", "AA:11:22:33:44:01", "127.0.0.11", "windows", "pc", ["Gaming PCs"], True),
    (home, "Workstation", "AA:11:22:33:44:02", "127.0.0.12", "windows", "pc", ["Workstations"], True),
    (home, "Media Server", "AA:11:22:33:44:03", "127.0.0.13", "linux", "server", ["Servers"], True),
    (home, "NAS", "AA:11:22:33:44:04", "127.0.0.14", "linux", "nas", ["NAS"], True),
    (home, "Guest Laptop", "AA:11:22:33:44:05", "127.0.0.15", "windows", "laptop", ["Workstations"], False),
    (home, "Basement PC", "AA:11:22:33:44:06", "127.0.0.16", "linux", "pc", ["Workstations"], False),
    (office, "Office PC 1", "BB:11:22:33:44:01", "127.0.0.21", "windows", "pc", ["Office Computers"], True),
    (office, "Office PC 2", "BB:11:22:33:44:02", "127.0.0.22", "windows", "pc", ["Office Computers"], False),
    (office, "Print Server", "BB:11:22:33:44:03", "127.0.0.23", "linux", "server", ["Servers", "Office Computers"], True),
]
ids = {}
for loc, name, mac, ip, os_, typ, groups, up in D:
    d = call("POST", L + "/devices", {"location_id": loc["id"], "name": name, "mac": mac, "ip": ip, "os": os_, "device_type": typ, "group_ids": [g[x] for x in groups], "tcp_check_ports": [PORT]}, tok)
    ids[name] = d["id"]
    if up: listen(ip)
# agent on the workstation
enr = call("POST", f"{L}/devices/{ids['Workstation']}/agent-enrollment", None, tok)
subprocess.run([os.path.join(BIN, "lwm-agent" + EXE), "--data-dir", T + "/a", "enroll", "--controller", f"https://127.0.0.1:{HTTPS}", "--code", enr["code"], "--fingerprint", enr["fingerprint"]], check=True, capture_output=True)
start("lwm-agent", ["--data-dir", T + "/a", "run"], env)
# schedules
today = time.strftime("%Y-%m-%d")
call("POST", L + "/schedules", {"name": "Wake office computers 07:30", "action": "wake", "target": {"group_ids": [g["Office Computers"]]}, "recurrence": {"kind": "weekly", "at": "07:30", "weekdays": [1, 2, 3, 4, 5]}, "executor": "relay"}, tok)
call("POST", L + "/schedules", {"name": "Shut down workstations 23:00", "action": "shutdown", "target": {"group_ids": [g["Workstations"]]}, "recurrence": {"kind": "daily", "at": "23:00"}, "retry": {"max_attempts": 3, "interval_secs": 120}}, tok)
call("POST", L + "/schedules", {"name": "Restart media server (Sunday)", "action": "restart", "target": {"device_ids": [ids["Media Server"]]}, "recurrence": {"kind": "weekly", "at": "03:00", "weekdays": [7]}}, tok)
call("POST", L + "/wireguard-profiles", {"name": "Home WireGuard", "controller_address": "10.8.0.1", "port": 8443, "endpoint": "vpn.example.net:51820", "allowed_ips": "10.8.0.0/24", "notes": "existing server"}, tok)
# history
for n in ["Gaming PC", "Media Server", "Office PC 1"]:
    call("POST", f"{L}/devices/{ids[n]}/wake", {}, tok)
call("POST", f"{L}/devices/{ids['Guest Laptop']}/wake", {}, tok)  # never boots -> honest timeout
time.sleep(3)
# a paired phone
p = call("POST", L + "/pairing", {"role": "operator"}, tok)
pl = json.loads(p["qr_payload"])
call("POST", f"https://127.0.0.1:{HTTPS}/api/v1/pair", {"pairing_token": pl["token"], "device_name": "Pixel 8", "platform": "android"})
print("READY", T, flush=True)
try:
    while True: time.sleep(3600)
except KeyboardInterrupt:
    pass
finally:
    for p in procs: p.terminate()
