//! System information collection for the agent. Values that cannot be read are reported as `None`/empty.
use crate::models::*;
use sysinfo::{Components, Disks, Networks, System};

pub struct Collector {
    sys: System,
}
impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        Self { sys }
    }
    /// CPU usage needs two samples; the first call after `new()` may be 0 and is reported as None.
    pub fn collect(&mut self, first: bool) -> SysInfo {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        let cpus = self.sys.cpus();
        let usage = if first || cpus.is_empty() { None } else { Some(self.sys.global_cpu_usage()) };
        let disks = Disks::new_with_refreshed_list();
        let nets = Networks::new_with_refreshed_list();
        let comps = Components::new_with_refreshed_list();
        let ips: std::collections::HashMap<String, Vec<String>> = {
            let mut m: std::collections::HashMap<String, Vec<String>> = Default::default();
            for i in if_addrs::get_if_addrs().unwrap_or_default() {
                if !i.is_loopback() {
                    m.entry(i.name.clone()).or_default().push(i.ip().to_string());
                }
            }
            m
        };
        let total_mem = self.sys.total_memory();
        SysInfo {
            hostname: System::host_name(),
            os_name: System::name(),
            os_version: System::long_os_version().or_else(System::os_version),
            kernel: System::kernel_version(),
            arch: Some(std::env::consts::ARCH.to_string()),
            uptime_secs: Some(System::uptime()),
            cpu_model: cpus.first().map(|c| c.brand().trim().to_string()).filter(|s| !s.is_empty()),
            cpu_cores: if cpus.is_empty() { None } else { Some(cpus.len() as u32) },
            cpu_usage_percent: usage,
            mem_total_bytes: if total_mem > 0 { Some(total_mem) } else { None },
            mem_used_bytes: if total_mem > 0 { Some(self.sys.used_memory()) } else { None },
            disks: disks
                .iter()
                .filter(|d| d.total_space() > 0)
                .map(|d| DiskInfo { mount: d.mount_point().to_string_lossy().to_string(), total_bytes: d.total_space(), free_bytes: d.available_space() })
                .collect(),
            networks: nets
                .iter()
                .filter_map(|(name, d)| {
                    let ip = ips.get(name).cloned().unwrap_or_default();
                    if ip.is_empty() {
                        return None;
                    }
                    let mac = d.mac_address().to_string();
                    Some(NetInfo { name: name.clone(), ips: ip, mac: if mac == "00:00:00:00:00:00" { None } else { Some(mac.to_uppercase()) } })
                })
                .collect(),
            temperatures: comps.iter().filter_map(|c| c.temperature().filter(|t| t.is_finite() && *t > 0.0).map(|t| TempInfo { label: c.label().to_string(), celsius: t })).collect(),
            reported_at: crate::util::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collects_real_values() {
        let mut c = Collector::new();
        let s = c.collect(true);
        assert!(s.cpu_usage_percent.is_none(), "first sample must not be invented");
        std::thread::sleep(std::time::Duration::from_millis(400));
        let s = c.collect(false);
        assert!(s.mem_total_bytes.unwrap() > 0);
        assert!(s.uptime_secs.is_some());
        assert!(s.cpu_cores.unwrap() >= 1);
    }
}
