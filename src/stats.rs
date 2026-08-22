use crate::version;
use std::time::Instant;

#[derive(Clone, Copy)]
struct Raw {
    wall: Instant,
    cpu_secs: f64,
    uptime_secs: u64,
    mem_bytes: u64,
    threads: u64,
    io_read: u64,
    io_write: u64,
}

impl Default for Raw {
    fn default() -> Self {
        Raw {
            wall: Instant::now(),
            cpu_secs: 0.0,
            uptime_secs: 0,
            mem_bytes: 0,
            threads: 0,
            io_read: 0,
            io_write: 0,
        }
    }
}

pub struct Sample {
    pub uptime_secs: u64,
    pub cpu_percent: f64,
    pub mem_bytes: u64,
    pub threads: u64,
    pub io_read: u64,
    pub io_write: u64,
}

pub fn snapshot(pid: u32) -> Sample {
    let a = imp::raw(pid);
    std::thread::sleep(std::time::Duration::from_millis(600));
    let b = imp::raw(pid);
    let wall = b.wall.duration_since(a.wall).as_secs_f64();
    let cpu_delta = (b.cpu_secs - a.cpu_secs).max(0.0);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0);
    Sample {
        uptime_secs: b.uptime_secs,
        cpu_percent: if wall > 0.0 {
            (cpu_delta / wall) * 100.0 / cores
        } else {
            0.0
        },
        mem_bytes: b.mem_bytes,
        threads: b.threads,
        io_read: b.io_read,
        io_write: b.io_write,
    }
}

pub fn print_snapshot(pid: u32, mem_max: Option<&str>) {
    let s = snapshot(pid);
    let max = mem_max.map(|m| format!(" / {m}")).unwrap_or_default();
    println!(
        "[rustmc] pid {pid} | up {} | cpu {:>5.1}% | mem {}{max} | threads {} | io r {} w {}",
        version::human_uptime(s.uptime_secs),
        s.cpu_percent,
        version::human_bytes(s.mem_bytes),
        s.threads,
        version::human_bytes(s.io_read),
        version::human_bytes(s.io_write),
    );
}

#[cfg(windows)]
mod imp {
    use super::Raw;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use windows_sys::Win32::System::Threading::{
        GetProcessIoCounters, GetProcessTimes, OpenProcess, IO_COUNTERS,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    fn ft(f: FILETIME) -> u64 {
        ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
    }

    fn thread_count(pid: u32) -> u64 {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                return 0;
            }
            let mut te = std::mem::zeroed::<THREADENTRY32>();
            te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            let mut count = 0u64;
            if Thread32First(snap, &mut te) != 0 {
                loop {
                    if te.th32OwnerProcessID == pid {
                        count += 1;
                    }
                    if Thread32Next(snap, &mut te) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
            count
        }
    }

    pub fn raw(pid: u32) -> Raw {
        let mut raw = Raw {
            wall: Instant::now(),
            ..Default::default()
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !h.is_null() {
                let (mut c, mut e, mut k, mut u) = (
                    std::mem::zeroed::<FILETIME>(),
                    std::mem::zeroed::<FILETIME>(),
                    std::mem::zeroed::<FILETIME>(),
                    std::mem::zeroed::<FILETIME>(),
                );
                if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                    raw.cpu_secs = (ft(k) + ft(u)) as f64 / 10_000_000.0;
                    let mut now = std::mem::zeroed::<FILETIME>();
                    GetSystemTimeAsFileTime(&mut now);
                    raw.uptime_secs = ft(now).saturating_sub(ft(c)) / 10_000_000;
                }
                let mut mem = std::mem::zeroed::<PROCESS_MEMORY_COUNTERS_EX>();
                mem.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
                if GetProcessMemoryInfo(
                    h,
                    &mut mem as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
                    std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                ) != 0
                {
                    raw.mem_bytes = mem.WorkingSetSize as u64;
                }
                let mut io = std::mem::zeroed::<IO_COUNTERS>();
                if GetProcessIoCounters(h, &mut io) != 0 {
                    raw.io_read = io.ReadTransferCount;
                    raw.io_write = io.WriteTransferCount;
                }
                CloseHandle(h);
            }
        }
        raw.threads = thread_count(pid);
        raw
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::Raw;
    use std::time::Instant;

    const HZ: f64 = 100.0;

    fn page_size() -> u64 {
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
    }

    pub fn raw(pid: u32) -> Raw {
        let mut raw = Raw {
            wall: Instant::now(),
            ..Default::default()
        };
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            if let Some(rp) = stat.rfind(')') {
                let f: Vec<&str> = stat[rp + 2..].split_whitespace().collect();
                let get = |i: usize| -> u64 { f.get(i).and_then(|v| v.parse().ok()).unwrap_or(0) };
                raw.cpu_secs = (get(11) + get(12)) as f64 / HZ;
                raw.threads = get(17);
                raw.mem_bytes = get(21) * page_size();
                let start = get(19);
                if let Ok(up) = std::fs::read_to_string("/proc/uptime") {
                    if let Ok(boot) = up.split_whitespace().next().unwrap_or("0").parse::<f64>() {
                        let secs = boot - start as f64 / HZ;
                        raw.uptime_secs = secs.max(0.0) as u64;
                    }
                }
            }
        }
        if let Ok(io) = std::fs::read_to_string(format!("/proc/{pid}/io")) {
            for line in io.lines() {
                if let Some(v) = line.strip_prefix("read_bytes: ") {
                    raw.io_read = v.parse().unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("write_bytes: ") {
                    raw.io_write = v.parse().unwrap_or(0);
                }
            }
        }
        raw
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    use super::Raw;

    pub fn raw(_pid: u32) -> Raw {
        Raw::default()
    }
}
