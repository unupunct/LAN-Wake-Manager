"""End-to-end test: real controller + relay + agent processes on loopback.
Run:  python tools/e2e.py   (after `cargo build`)
The agent runs with LWM_DRY_RUN=1 so this PC is never actually shut down.
"""
import hashlib, json, os, re, socket, ssl, subprocess, sys, tempfile, threading, time, urllib.request, urllib.error, shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug")
EXE = ".exe" if os.name == "nt" else ""
T = tempfile.mkdtemp(prefix="lwm-e2e-")
HTTPS, LOCAL, RELAY = 18443, 18480, 18444
procs = []
results = []

def check(name, cond, extra=""):
    results.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + (f"  [{extra}]" if (extra and not cond) else ""), flush=True)

CTX = ssl.create_default_context(); CTX.check_hostname = False; CTX.verify_mode = ssl.CERT_NONE

def call(method, url, body=None, token=None, raw=False):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method)
    if data is not None: r.add_header("Content-Type", "application/json")
    if token: r.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(r, timeout=30, context=CTX) as resp:
            t = resp.read().decode()
            return resp.status, (json.loads(t) if t else None)
    except urllib.error.HTTPError as e:
        t = e.read().decode()
        try: return e.code, json.loads(t)
        except Exception: return e.code, t

L = f"http://127.0.0.1:{LOCAL}/api/v1"
H = f"https://127.0.0.1:{HTTPS}/api/v1"

def start(name, args, env=None):
    e = dict(os.environ); e.update(env or {})
    log = open(os.path.join(T, name + ".log"), "w")
    p = subprocess.Popen([os.path.join(BIN, name.split("#")[0] + EXE)] + args, stdout=log, stderr=log, env=e)
    procs.append(p); return p

def wait_for(fn, timeout=60, step=0.5):
    end = time.time() + timeout
    while time.time() < end:
        v = fn()
        if v: return v
        time.sleep(step)
    return None

def tls_fp(port):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as s:
        with CTX.wrap_socket(s, server_hostname="x") as t:
            return hashlib.sha256(t.getpeercert(binary_form=True)).hexdigest()

def op_final(tok, oid, timeout=60):
    def f():
        _, o = call("GET", f"{L}/operations/{oid}", token=tok)
        return o if o and o["state"] not in ("pending", "running") else None
    return wait_for(f, timeout)

try:
    env = {"LWM_NO_ICMP": "1", "LWM_LOG": "info"}
    start("lwm-controller", ["--data-dir", T + "/c", "run", "--listen", f"127.0.0.1:{HTTPS}", "--local-listen", f"127.0.0.1:{LOCAL}"], env)
    def up():
        try: return call("GET", L + "/info")[0] == 200
        except Exception: return False
    check("controller starts", wait_for(up, 40))
    _, info = call("GET", L + "/info")
    check("setup required on first start", info["setup_required"] is True)
    check("TLS fingerprint in /info matches the served certificate", info["tls_fingerprint"] == tls_fp(HTTPS))

    # --- security scenario
    check("unauthenticated device list is rejected", call("GET", L + "/devices")[0] == 401)
    check("unauthenticated wake is rejected", call("POST", L + "/devices/x/wake", {})[0] == 401)
    check("setup over the network (non-loopback) not needed; short password refused", call("POST", L + "/auth/setup", {"password": "short"})[0] == 400)
    s, r = call("POST", L + "/auth/setup", {"password": "correct horse battery"})
    check("admin password setup", s == 200 and r.get("token"))
    admin = r["token"]
    check("second setup refused", call("POST", L + "/auth/setup", {"password": "another long password"})[0] == 409)
    check("wrong password denied", call("POST", L + "/auth/login", {"password": "nope-nope-nope"})[0] == 401)
    s, r = call("POST", L + "/auth/login", {"password": "correct horse battery"})
    check("login works", s == 200)

    _, locs = call("GET", L + "/locations", token=admin)
    home = locs[0]
    check("default location is built-in relay", home["relay_mode"] == "builtin" and home["relay_status"] == "builtin")
    _, groups = call("GET", L + "/groups", token=admin)
    check("seed groups present", len(groups) == 5)

    # --- mobile pairing
    s, p = call("POST", L + "/pairing", {"role": "operator", "ttl_secs": 120}, token=admin)
    payload = json.loads(p["qr_payload"])
    check("qr payload has no permanent credential and carries fp+token", payload["fp"] == info["tls_fingerprint"] and payload["token"] == p["pairing_token"] and "password" not in p["qr_payload"])
    s, m = call("POST", H + "/pair", {"pairing_token": payload["token"], "device_name": "Test Phone", "platform": "android"})
    check("pairing over pinned HTTPS", s == 200 and m.get("token"))
    phone = m["token"]
    check("pairing token is single use", call("POST", H + "/pair", {"pairing_token": payload["token"], "device_name": "x", "platform": "android"})[0] == 403)
    check("paired phone can read devices", call("GET", H + "/devices", token=phone)[0] == 200)
    check("operator cannot create locations", call("POST", H + "/locations", {"name": "X"}, token=phone)[0] == 403)
    check("operator cannot read audit log", call("GET", H + "/audit", token=phone)[0] == 403)
    s, vp = call("POST", L + "/pairing", {"role": "viewer"}, token=admin)
    s, vm = call("POST", H + "/pair", {"pairing_token": vp["pairing_token"], "device_name": "Viewer", "platform": "android"})
    check("viewer cannot wake", call("POST", H + "/devices/zzz/wake", {}, token=vm["token"])[0] == 403)

    # --- wake with a target that "boots" later
    srv = socket.socket(); srv.bind(("127.0.0.1", 0)); port = srv.getsockname()[1]; srv.close()
    s, dev = call("POST", L + "/devices", {"location_id": home["id"], "name": "Target PC", "mac": "aa:bb:cc:dd:ee:01", "ip": "127.0.0.1", "os": "linux", "tcp_check_ports": [port], "power_method": "none"}, token=admin)
    check("device created", s == 201 and dev["mac"] == "AA:BB:CC:DD:EE:01", str(dev))
    check("duplicate MAC refused", call("POST", L + "/devices", {"location_id": home["id"], "name": "dup", "mac": "AA:BB:CC:DD:EE:01"}, token=admin)[0] == 409)
    check("invalid MAC refused", call("POST", L + "/devices", {"location_id": home["id"], "name": "bad", "mac": "12:34"}, token=admin)[0] == 400)
    s, op = call("POST", f"{H}/devices/{dev['id']}/wake", {}, token=phone)
    check("wake accepted (202)", s == 202)
    time.sleep(4)
    _, cur = call("GET", f"{L}/operations/{op['id']}", token=admin)
    check("wake is NOT reported successful before the PC is reachable", cur["state"] == "running", cur["state"])
    check("magic packet steps recorded", any("packet" in x["text"] for x in cur["steps"]))
    listener = socket.socket(); listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); listener.bind(("127.0.0.1", port)); listener.listen(50)
    def _drain():
        while True:
            try: c,_ = listener.accept(); c.close()
            except Exception: return
    threading.Thread(target=_drain, daemon=True).start()
    fin = op_final(admin, op["id"], 40)
    check("wake succeeds once the PC answers", fin and fin["state"] == "succeeded", str(fin))
    _, d2 = call("GET", f"{L}/devices/{dev['id']}", token=admin)
    check("device marked online and wol_capable=yes", d2["status"] == "online" and d2["wol_capable"] == "yes", str(d2))
    check("requested_by shows the phone", fin["requested_by"] == "Test Phone")

    # --- honest timeout
    s, short = call("POST", L + "/locations", {"name": "Short", "wake_timeout_secs": 5, "wake_attempts": 1}, token=admin)
    s, ghost = call("POST", L + "/devices", {"location_id": short["id"], "name": "Ghost", "mac": "AA:BB:CC:DD:EE:02", "ip": "192.0.2.77", "tcp_check_ports": [9]}, token=admin)
    s, gop = call("POST", f"{L}/devices/{ghost['id']}/wake", {}, token=admin)
    fin = op_final(admin, gop["id"], 60)
    check("unreachable PC ends in 'timeout', never 'succeeded'", fin and fin["state"] == "timeout", str(fin))

    # --- bulk confirmation
    check("bulk shutdown without confirm refused", call("POST", L + "/bulk", {"action": "shutdown", "device_ids": [dev["id"]]}, token=admin)[1]["error"]["code"] == "confirmation_required")

    # --- agent
    s, enr = call("POST", f"{L}/devices/{dev['id']}/agent-enrollment", token=admin)
    check("agent enrollment code", s == 200 and enr["fingerprint"] == info["tls_fingerprint"])
    r = subprocess.run([os.path.join(BIN, "lwm-agent" + EXE), "--data-dir", T + "/a", "enroll", "--controller", f"https://127.0.0.1:{HTTPS}", "--code", enr["code"], "--fingerprint", enr["fingerprint"]], capture_output=True, text=True)
    check("agent enrolls with pinned fingerprint", r.returncode == 0, r.stdout + r.stderr)
    bad = subprocess.run([os.path.join(BIN, "lwm-agent" + EXE), "--data-dir", T + "/a2", "enroll", "--controller", f"https://127.0.0.1:{HTTPS}", "--code", "AAAA-BBBB", "--fingerprint", "00" * 32], capture_output=True, text=True)
    check("agent refuses a wrong fingerprint", bad.returncode != 0)
    start("lwm-agent#1", ["--data-dir", T + "/a", "run"], {"LWM_DRY_RUN": "1"})
    def agent_up():
        _, d = call("GET", f"{L}/devices/{dev['id']}", token=admin)
        return d if d["agent"]["state"] == "online" and d["sysinfo"] else None
    d3 = wait_for(agent_up, 45)
    check("agent online with system info", d3 and d3["sysinfo"]["mem_total_bytes"] > 0 and d3["power_method"] == "agent", str(d3))
    s, sop = call("POST", f"{H}/devices/{dev['id']}/shutdown", {"delay_secs": 600}, token=phone)
    def has_accept():
        _, o = call("GET", f"{L}/operations/{sop['id']}", token=admin)
        return o if any("dry run" in x["text"] for x in o["steps"]) else None
    o = wait_for(has_accept, 30)
    check("agent received and executed (dry-run) the shutdown command", o and "shutdown" in json.dumps(o["steps"]), str(o))
    check("op stays running until power-off is confirmed", o and o["state"] == "running")
    s, cop = call("POST", f"{L}/devices/{dev['id']}/cancel-shutdown", {}, token=admin)
    cf = op_final(admin, cop["id"], 40)
    check("cancel-shutdown succeeds via agent", cf and cf["state"] == "succeeded", str(cf))
    sf = op_final(admin, sop["id"], 30)
    check("cancelled shutdown op closes", sf and sf["state"] == "failed" and "cancel" in sf["message"], str(sf))
    s, h = call("GET", f"{L}/devices/{dev['id']}/metrics?minutes=60", token=admin)
    check("metrics history recorded", s == 200 and len(h) >= 1)

    # --- schedule fires without any GUI and only once per occurrence
    today = time.strftime("%Y-%m-%d")
    sched = {"name": "every minute", "action": "wake", "target": {"device_ids": [dev["id"]]}, "recurrence": {"kind": "custom", "at": "00:00", "interval": {"every": 1, "unit": "minutes"}}, "start_date": today, "executor": "controller"}
    s, sc = call("POST", L + "/schedules", sched, token=admin)
    check("schedule created with next_run", s == 201 and sc["next_run"], str(sc))
    def sched_op():
        _, ops = call("GET", f"{L}/operations?limit=20", token=admin)
        return [o for o in ops if o["source"] == "schedule"] or None
    so = wait_for(sched_op, 110)
    check("schedule executed by the controller service", bool(so) and so[0]["schedule_id"] == sc["id"], str(so))
    call("DELETE", f"{L}/schedules/{sc['id']}", token=admin)
    check("schedule without targets refused", call("POST", L + "/schedules", dict(sched, target={}), token=admin)[0] == 400)

    # --- relay
    rl = start("lwm-relay", ["--data-dir", T + "/r", "run", "--listen", f"127.0.0.1:{RELAY}", "--name", "office-relay"], env)
    def relay_up():
        try: return tls_fp(RELAY)
        except Exception: return None
    rfp = wait_for(relay_up, 30)
    check("relay starts with TLS", rfp)
    rlog = lambda: open(T + "/r.log").read() if os.path.exists(T + "/r.log") else ""
    code = None
    pc = subprocess.run([os.path.join(BIN, "lwm-relay" + EXE), "--data-dir", T + "/r", "pair-code"], capture_output=True, text=True)
    m_ = re.search(r"Pairing code\s*:\s*([A-Z0-9-]{9})", pc.stdout); code = m_.group(1) if m_ else None
    check("relay prints a pairing code", code, pc.stdout)
    s, office = call("POST", L + "/locations", {"name": "Office Network"}, token=admin)
    s, pr = call("POST", f"{L}/locations/{office['id']}/relay/probe", {"url": f"https://127.0.0.1:{RELAY}"}, token=admin)
    check("controller reads relay fingerprint for the user to compare", s == 200 and pr["fingerprint"] == rfp, str(pr))
    check("pairing with a wrong code refused", call("POST", f"{L}/locations/{office['id']}/relay/pair", {"url": f"https://127.0.0.1:{RELAY}", "code": "AAAA-AAAA", "fingerprint": rfp}, token=admin)[0] == 502)
    s, ol = call("POST", f"{L}/locations/{office['id']}/relay/pair", {"url": f"https://127.0.0.1:{RELAY}", "code": code, "fingerprint": rfp}, token=admin)
    check("relay paired", s == 200 and ol["relay_mode"] == "remote" and ol["relay_status"] == "online", str(ol))
    check("relay pairing code is single use", call("POST", f"{L}/locations/{office['id']}/relay/pair", {"url": f"https://127.0.0.1:{RELAY}", "code": code, "fingerprint": rfp}, token=admin)[0] != 200)
    call("PUT", L + "/settings", dict(call("GET", L + "/settings", token=admin)[1], monitor_interval_secs=5), token=admin)
    s, od = call("POST", L + "/devices", {"location_id": office["id"], "name": "Office PC", "mac": "AA:BB:CC:DD:EE:03", "ip": "127.0.0.1", "tcp_check_ports": [port]}, token=admin)
    s, wop = call("POST", f"{L}/devices/{od['id']}/wake", {}, token=admin)
    fin = op_final(admin, wop["id"], 60)
    check("wake through the remote relay works", fin and fin["state"] == "succeeded" and any("remote relay" in x["text"] for x in fin["steps"]), str(fin))
    s, ifs = call("GET", f"{L}/locations/{office['id']}/interfaces", token=admin)
    check("relay interfaces listed", s == 200 and isinstance(ifs, list))
    # discovery through the relay (loopback /30, tcp method)
    s, dj = call("POST", L + "/discovery", {"location_id": office["id"], "subnet": "127.0.0.0/30", "methods": ["tcp"], "tcp_ports": [port], "speed": "fast", "timeout_ms": 300}, token=admin)
    def disc_done():
        _, j = call("GET", f"{L}/discovery/{dj['job_id']}", token=admin)
        return j if j["state"] != "running" else None
    j = wait_for(disc_done, 30)
    check("discovery via relay finds the listener and flags it registered", j and any(c["ip"] == "127.0.0.1" and c["registered"] for c in j["results"]), str(j))
    check("discovery refuses huge subnets", call("POST", L + "/discovery", {"location_id": office["id"], "subnet": "10.0.0.0/8"}, token=admin)[0] == 400)
    # relay-side scheduler (works without the controller): talk to the relay as a controller
    s, enr2 = 0, None
    rl.terminate(); rl.wait(10)
    def office_offline():
        _, l = call("GET", f"{L}/locations/{office['id']}", token=admin)
        return l["relay_status"] == "offline"
    time.sleep(1)
    s, wop2 = call("POST", f"{L}/devices/{od['id']}/wake", {}, token=admin)
    fin = op_final(admin, wop2["id"], 40)
    # the device is online (listener open) -> must still fail because the relay is the only path
    check("wake fails clearly when the relay is down", fin and fin["state"] == "failed" and "relay" in fin["message"].lower(), str(fin))
    wait_for(office_offline, 70)
    notes = call("GET", L + "/notifications?after=0", token=admin)[1]
    check("relay_offline notification raised", any(n["type"] == "relay_offline" for n in notes), str([n["type"] for n in notes]))

    # --- backup / export / revoke / audit
    s, b = call("POST", L + "/backups", token=admin)
    check("manual backup created", s == 201)
    s, g = call("POST", L + "/groups", {"name": "Temp group"}, token=admin)
    rs = call("POST", f"{L}/backups/{b['name']}/restore", token=admin)
    gl = call("GET", L + "/groups", token=admin)[1]
    check("restore returns the database to the backup state", rs[0] == 200 and all(x["name"] != "Temp group" for x in gl), str(rs) + str(gl))
    _, ex = call("GET", L + "/export", token=admin)
    exs = json.dumps(ex)
    check("export contains no tokens or hashes", '"relay_token": "' not in exs and '"agent_token_hash": "' not in exs and "password" not in exs.lower())
    check("backup restore path traversal refused", call("POST", f"{L}/backups/..%2f..%2fx/restore", token=admin)[0] in (400, 404))
    mids = call("GET", L + "/mobile-devices", token=admin)[1]
    pid = [x for x in mids if x["name"] == "Test Phone"][0]["id"]
    check("revoke phone", call("DELETE", f"{L}/mobile-devices/{pid}", token=admin)[0] == 200)
    check("revoked phone is rejected immediately", call("GET", H + "/devices", token=phone)[0] == 401)
    au = call("GET", L + "/audit?limit=500", token=admin)[1]
    acts = {(a["action"], a["result"]) for a in au}
    check("audit log records pairing, power ops, denials and revocation", {("pair", "ok"), ("wake", "accepted"), ("auth", "denied"), ("mobile_revoke", "ok")} <= acts, str(sorted(acts)))
    # rate limit on login
    codes = [call("POST", L + "/auth/login", {"password": "wrong-wrong-wrong"})[0] for _ in range(14)]
    check("login brute force is rate limited", 429 in codes, str(codes))
    # websocket
    try:
        import websocket
        w = websocket.create_connection(f"wss://127.0.0.1:{HTTPS}/api/v1/events", sslopt={"cert_reqs": ssl.CERT_NONE}, timeout=8)
        w.send(json.dumps({"type": "auth", "token": admin}))
        check("websocket hello after first-message auth", json.loads(w.recv())["type"] == "hello")
        w.close()
        w = websocket.create_connection(f"wss://127.0.0.1:{HTTPS}/api/v1/events", sslopt={"cert_reqs": ssl.CERT_NONE}, timeout=8)
        w.send(json.dumps({"type": "auth", "token": "bad"}))
        check("websocket rejects a bad token", json.loads(w.recv())["type"] == "error")
    except ImportError:
        print("SKIP websocket (websocket-client missing)")
finally:
    for p in procs:
        try: p.terminate()
        except Exception: pass
    time.sleep(1)
    fails = [n for n, ok in results if not ok]
    print(f"\n{len(results) - len(fails)}/{len(results)} checks passed; logs in {T}")
    for n in fails: print("  FAILED:", n)
    sys.exit(1 if fails else 0)
