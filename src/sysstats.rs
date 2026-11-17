//! Whole-system CPU, RAM, GPU and network figures, sampled only while the user has them switched on
//! and the pill is visible. Pure maths is separate from the Windows calls so it can be unit tested.
#![cfg_attr(not(windows), allow(dead_code))]

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub cpu: Option<f64>,
    pub ram: Option<f64>,
    pub gpu: Option<f64>,
    /// Bytes per second, (down, up).
    pub net: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Enabled {
    pub cpu: bool,
    pub ram: bool,
    pub gpu: bool,
    pub net: bool,
}

impl Enabled {
    pub fn any(&self) -> bool {
        self.cpu || self.ram || self.gpu || self.net
    }
}

/// CPU busy % from two GetSystemTimes readings (idle, kernel, user). Kernel time includes idle time.
pub fn cpu_percent(prev: (u64, u64, u64), cur: (u64, u64, u64)) -> Option<f64> {
    let idle = cur.0.checked_sub(prev.0)?;
    let total = cur.1.checked_sub(prev.1)? + cur.2.checked_sub(prev.2)?;
    if total == 0 {
        return None;
    }
    Some((100.0 * (total.saturating_sub(idle)) as f64 / total as f64).clamp(0.0, 100.0))
}

/// Bytes per second between two counter readings; None if a counter went backwards (adapter reset).
pub fn rate(prev: u64, cur: u64, secs: f64) -> Option<f64> {
    (secs > 0.0 && cur >= prev).then(|| (cur - prev) as f64 / secs)
}

/// GPU % the way Task Manager reports it: add up each engine type across processes, take the busiest type.
pub fn gpu_percent<'a>(items: impl IntoIterator<Item = (&'a str, f64)>) -> Option<f64> {
    let mut by_type: Vec<(String, f64)> = Vec::new();
    let mut any = false;
    for (name, v) in items {
        any = true;
        let ty = name.split("_engtype_").nth(1).unwrap_or("other").to_string();
        match by_type.iter_mut().find(|(t, _)| *t == ty) {
            Some((_, sum)) => *sum += v,
            None => by_type.push((ty, v)),
        }
    }
    any.then(|| by_type.iter().map(|(_, v)| *v).fold(0.0, f64::max).clamp(0.0, 100.0))
}

pub fn format_rate(bps: f64) -> String {
    let kb = bps / 1024.0;
    if kb < 1.0 {
        "0 KB/s".into()
    } else if kb < 1000.0 {
        format!("{kb:.0} KB/s")
    } else {
        format!("{:.1} MB/s", kb / 1024.0)
    }
}

#[cfg(windows)]
pub use sampler::Sampler;

#[cfg(windows)]
mod sampler {
    use super::*;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IF_TABLE2};
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::System::Performance::*;
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::GetSystemTimes;

    const GPU_COUNTER: &str = "\\GPU Engine(*)\\Utilization Percentage";

    pub struct Sampler {
        cpu_prev: Option<(u64, u64, u64)>,
        net_prev: Option<(u64, u64, Instant)>,
        gpu_query: PDH_HQUERY,
        gpu_counter: PDH_HCOUNTER,
    }

    fn ft(f: FILETIME) -> u64 {
        (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64
    }

    impl Sampler {
        pub fn new() -> Self {
            Sampler { cpu_prev: None, net_prev: None, gpu_query: null_mut(), gpu_counter: null_mut() }
        }

        /// Drop history and the GPU query, for example when stats are switched off or the pill is hidden.
        pub fn reset(&mut self) {
            self.cpu_prev = None;
            self.net_prev = None;
            if !self.gpu_query.is_null() {
                unsafe { PdhCloseQuery(self.gpu_query) };
                self.gpu_query = null_mut();
                self.gpu_counter = null_mut();
            }
        }

        fn cpu(&mut self) -> Option<f64> {
            let (mut i, mut k, mut u): (FILETIME, FILETIME, FILETIME) = unsafe { (zeroed(), zeroed(), zeroed()) };
            if unsafe { GetSystemTimes(&mut i, &mut k, &mut u) } == 0 {
                return None;
            }
            let cur = (ft(i), ft(k), ft(u));
            self.cpu_prev.replace(cur).and_then(|p| cpu_percent(p, cur))
        }

        fn ram() -> Option<f64> {
            let mut m: MEMORYSTATUSEX = unsafe { zeroed() };
            m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
            if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 || m.ullTotalPhys == 0 {
                return None;
            }
            Some(100.0 * (m.ullTotalPhys - m.ullAvailPhys) as f64 / m.ullTotalPhys as f64)
        }

        fn net(&mut self) -> Option<(f64, f64)> {
            let mut table: *mut MIB_IF_TABLE2 = null_mut();
            if unsafe { GetIfTable2(&mut table) } != 0 || table.is_null() {
                return None;
            }
            let (mut down, mut up) = (0u64, 0u64);
            unsafe {
                let n = (*table).NumEntries as usize;
                let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
                for r in rows {
                    let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
                    let filter = r.InterfaceAndOperStatusFlags._bitfield & 2 != 0;
                    if hardware && !filter && r.OperStatus == IfOperStatusUp && r.Type != IF_TYPE_SOFTWARE_LOOPBACK {
                        down += r.InOctets;
                        up += r.OutOctets;
                    }
                }
                FreeMibTable(table as _);
            }
            let now = Instant::now();
            let prev = self.net_prev.replace((down, up, now))?;
            let secs = now.duration_since(prev.2).as_secs_f64();
            Some((rate(prev.0, down, secs)?, rate(prev.1, up, secs)?))
        }

        fn gpu(&mut self) -> Option<f64> {
            unsafe {
                if self.gpu_query.is_null() {
                    let path: Vec<u16> = GPU_COUNTER.encode_utf16().chain(Some(0)).collect();
                    if PdhOpenQueryW(null(), 0, &mut self.gpu_query) != 0 {
                        self.gpu_query = null_mut();
                        return None;
                    }
                    if PdhAddEnglishCounterW(self.gpu_query, path.as_ptr(), 0, &mut self.gpu_counter) != 0 {
                        self.reset();
                        return None;
                    }
                    PdhCollectQueryData(self.gpu_query);
                    return None;
                }
                if PdhCollectQueryData(self.gpu_query) != 0 {
                    return None;
                }
                let (mut size, mut count) = (0u32, 0u32);
                let r = PdhGetFormattedCounterArrayW(self.gpu_counter, PDH_FMT_DOUBLE, &mut size, &mut count, null_mut());
                if r != PDH_MORE_DATA || size == 0 {
                    return Some(0.0);
                }
                let mut buf = vec![0u64; (size as usize).div_ceil(8)];
                let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                if PdhGetFormattedCounterArrayW(self.gpu_counter, PDH_FMT_DOUBLE, &mut size, &mut count, items) != 0 {
                    return None;
                }
                let items = std::slice::from_raw_parts(items, count as usize);
                let names: Vec<(String, f64)> = items
                    .iter()
                    .filter(|it| it.FmtValue.CStatus == 0)
                    .map(|it| {
                        let mut len = 0;
                        while *it.szName.add(len) != 0 {
                            len += 1;
                        }
                        let name = String::from_utf16_lossy(std::slice::from_raw_parts(it.szName, len));
                        (name, it.FmtValue.Anonymous.doubleValue)
                    })
                    .collect();
                gpu_percent(names.iter().map(|(n, v)| (n.as_str(), *v))).or(Some(0.0))
            }
        }

        pub fn sample(&mut self, on: Enabled) -> Stats {
            if !on.gpu && !self.gpu_query.is_null() {
                unsafe { PdhCloseQuery(self.gpu_query) };
                self.gpu_query = null_mut();
            }
            Stats {
                cpu: if on.cpu { self.cpu() } else { None },
                ram: if on.ram { Self::ram() } else { None },
                gpu: if on.gpu { self.gpu() } else { None },
                net: if on.net { self.net() } else { None },
            }
        }
    }

    impl Drop for Sampler {
        fn drop(&mut self) {
            self.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_from_system_times() {
        assert_eq!(cpu_percent((100, 200, 100), (175, 300, 200)), Some(62.5));
        assert_eq!(cpu_percent((100, 200, 100), (100, 200, 100)), None);
        assert_eq!(cpu_percent((100, 200, 100), (50, 300, 200)), None);
    }

    #[test]
    fn rates() {
        assert_eq!(rate(1000, 3048, 2.0), Some(1024.0));
        assert_eq!(rate(5000, 10, 2.0), None);
        assert_eq!(format_rate(0.0), "0 KB/s");
        assert_eq!(format_rate(340.0 * 1024.0), "340 KB/s");
        assert_eq!(format_rate(1.25 * 1024.0 * 1024.0), "1.2 MB/s");
    }

    #[test]
    fn gpu_takes_busiest_engine_type() {
        let items = [
            ("pid_1_luid_0x0_phys_0_eng_0_engtype_3D", 20.0),
            ("pid_2_luid_0x0_phys_0_eng_0_engtype_3D", 15.0),
            ("pid_3_luid_0x0_phys_0_eng_4_engtype_VideoDecode", 30.0),
        ];
        assert_eq!(gpu_percent(items), Some(35.0));
        assert_eq!(gpu_percent(std::iter::empty()), None);
    }
}
