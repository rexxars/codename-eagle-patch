#![cfg_attr(windows, windows_subsystem = "windows")]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Fixed width of the Map column. The menu's row format reserves a 19-char slot
/// for the map (content + trailing gap); we pad the name to MAP_WIDTH and the
/// menu format leaves (19 - MAP_WIDTH) = 4 spaces before the Players column, so
/// every row's Players/Type/IP align regardless of map-name length.
const MAP_WIDTH: usize = 15;

/// Live Sanity query for the central server IP list. The LAN-sweep invocation
/// fetches this and merges it with discovered LAN servers.
const QUERY_URL: &str = "https://cneagle.api.sanity.io/v2026-06-03/data/query/servers";
const GROQ: &str = r#"*[_id=="serverList"][0].servers[].ip"#;
const HTTP_TIMEOUT: Duration = Duration::from_millis(1500);

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(args) = parse_args(&argv) else {
        return; // bad invocation: emit nothing (menu shows no row, no crash)
    };

    let mut out = String::new();
    if is_lan_sweep(&args.ip) {
        // The menu fires this 255.255.255.255 sweep when the player clicks Refresh
        // list (not on browser open), so it's where we also pull the central
        // (internet) list from Sanity. Fetch concurrently with the LAN beacon listen.
        let fetch = std::thread::spawn(fetch_sanity_ips);
        let lan = discover_lan(Duration::from_millis(1500));
        let central = fetch.join().unwrap_or_default();

        // Don't double-list: the menu probes manual iplist.txt entries via its own
        // per-IP spawns, and LAN hosts are already covered by the beacon path.
        let mut seen: std::collections::HashSet<[u8; 4]> = manual_ips().into_iter().collect();
        for (ip, _) in &lan {
            seen.insert(*ip);
        }

        // Resolve every host concurrently (each query binds its own socket, no
        // shared state). LAN hosts have a beacon fallback; central hosts emit a
        // row only if they answer \status\.
        let mut handles: Vec<std::thread::JoinHandle<Option<ServerInfo>>> = Vec::new();
        for (ip, beacon) in lan {
            handles.push(std::thread::spawn(move || Some(query_host(ip, beacon))));
        }
        for ip in central {
            if !seen.insert(ip) {
                continue; // already a manual/LAN entry
            }
            handles.push(std::thread::spawn(move || match query_status(ip) {
                Some((fields, ping)) if has_useful(&fields) => {
                    Some(info_from_status(&fields, ip, ping))
                }
                _ => None,
            }));
        }
        for h in handles {
            if let Ok(Some(info)) = h.join() {
                out.push_str(&build_row(&info));
            }
        }
    } else if let Some((fields, ping)) = query_status(args.ip) {
        // An answered-but-empty reply (no map/host) is treated like no answer.
        if has_useful(&fields) {
            out.push_str(&build_row(&info_from_status(&fields, args.ip, ping)));
        }
    }

    if !out.is_empty() {
        write_rows(args.write_fd, out.as_bytes());
    }
}

/// Parse a GameSpy `\key\value\...` reply into a map. The stream is backslash-
/// delimited starting with an empty token; framing keys (`final`, `queryid`) fall
/// out harmlessly. Values never contain `\`, so splitting is unambiguous.
fn parse_status(reply: &str) -> HashMap<String, String> {
    let parts: Vec<&str> = reply.split('\\').collect();
    let mut map = HashMap::new();
    let mut i = 1; // skip leading empty token
    while i + 1 < parts.len() {
        let (key, val) = (parts[i], parts[i + 1]);
        if !key.is_empty() {
            map.entry(key.to_string()).or_insert_with(|| val.to_string());
        }
        i += 2;
    }
    map
}

/// Merge a later packet's fields into `base`; first value for a key wins.
fn merge_status(base: &mut HashMap<String, String>, more: &HashMap<String, String>) {
    for (k, v) in more {
        base.entry(k.clone()).or_insert_with(|| v.clone());
    }
}

struct ServerInfo {
    name: String,
    map: String,
    gametype: String,
    players: u32,
    max_players: u32,
    ping_ms: u32,
    ip: [u8; 4],
}

/// Strip characters that would break the menu's strstr/quote parse.
fn sanitize(s: &str) -> String {
    s.chars().filter(|&c| c != '"' && c != '\\' && c >= ' ').collect()
}

/// Build the exact row the menu parses. Map uses the `255"<name>"` framing the
/// shipped MENUDLL expects (it reads 255 as a throwaway int, skips `255"`, then
/// copies to the closing quote).
fn build_row(st: &ServerInfo) -> String {
    // name/map are intentionally NOT length-capped here: MENUDLL caps them on its
    // side (name -> 25 chars, map -> 24), so emitting the full string preserves the contract.
    let name = sanitize(&st.name);
    // Pad/truncate the map to a fixed-width column so the menu's fixed column
    // spacing stays aligned regardless of name length (longer names truncate).
    let truncated: String = sanitize(&st.map).chars().take(MAP_WIDTH).collect();
    let map = format!("{truncated:<width$}", width = MAP_WIDTH);
    let mut gametype = sanitize(&st.gametype).to_uppercase();
    if gametype.is_empty() {
        gametype = "CTF".to_string();
    }
    let [a, b, c, d] = st.ip;
    format!(
        "Name:\"{name}\" Ping:{ping} Map:255\"{map}\" Players:{p} MaxPlayers:{mp} \
         Spectators:0 MaxSpectators:0 Type:{gametype} IP:{a}.{b}.{c}.{d} IPXAdress:0.0.0.0.0.0\n",
        ping = st.ping_ms,
        p = st.players,
        mp = st.max_players,
    )
}

struct Beacon {
    name: String,
    players: u32,
    max_players: u32,
}

/// Parse a CE `'D'` LAN beacon (broadcast to :210). Source IP comes from recvfrom,
/// not the payload. Layout: byte0=0x44, [12]=players+1, [13]=maxplayers+1 (the
/// same reserved-host-slot `-1` the beacon uses for the live count), [14..]=name,
/// NUL-terminated. The name length is also encoded at [7] as name_len+7, but the
/// NUL is the robust delimiter (`buf[13]` is NOT the name length: reading it as
/// one truncates the name to the max-player count and drops beacons whose
/// maxplayers runs past the packet end).
fn parse_beacon(buf: &[u8]) -> Option<Beacon> {
    if buf.len() < 14 || buf[0] != 0x44 {
        return None;
    }
    let players = (buf[12] as u32).saturating_sub(1);
    let max_players = (buf[13] as u32).saturating_sub(1);
    // Name is NUL-terminated (falling back to the packet end if unterminated).
    // CE uses a single-byte codepage, so high-bit chars may decode lossily here;
    // acceptable for this fallback display name.
    let tail = &buf[14..];
    let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
    let name = String::from_utf8_lossy(&tail[..end]).trim().to_string();
    Some(Beacon {
        name,
        players,
        max_players,
    })
}

struct Args {
    write_fd: i32,
    ip: [u8; 4],
    // Parsed from the menu's invocation but not consumed by the dispatch (the IP
    // alone selects internet vs LAN). Retained to keep the 6-arg contract explicit.
    #[allow(dead_code)]
    mode: i32,
}

fn parse_args(raw: &[String]) -> Option<Args> {
    // MENUDLL sprintf's the 6 ints into one string; Rust may also receive them
    // pre-split. Join then split so both layouts collapse to the same tokens.
    // Order (confirmed live from MENUDLL): `writeFd mode o1 o2 o3 o4`.
    let joined = raw.join(" ");
    let tokens: Vec<&str> = joined.split_whitespace().collect();
    if tokens.len() != 6 {
        return None;
    }
    let write_fd: i32 = tokens[0].parse().ok()?;
    let mode: i32 = tokens[1].parse().ok()?;
    let mut ip = [0u8; 4];
    for (slot, tok) in ip.iter_mut().zip(&tokens[2..=5]) {
        let octet: u16 = tok.parse().ok()?;
        if octet > 255 {
            return None;
        }
        *slot = octet as u8;
    }
    Some(Args { write_fd, ip, mode })
}

fn is_lan_sweep(ip: &[u8; 4]) -> bool {
    *ip == [255, 255, 255, 255]
}

/// Parse a dotted-quad string into octets (strict: exactly four 0-255 parts).
fn to_octets(s: &str) -> Option<[u8; 4]> {
    let parts: Vec<&str> = s.trim().split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut ip = [0u8; 4];
    for (slot, part) in ip.iter_mut().zip(&parts) {
        let octet: u16 = part.parse().ok()?;
        if octet > 255 {
            return None;
        }
        *slot = octet as u8;
    }
    Some(ip)
}

/// Extract deduped server IPs from a Sanity `{"result":[...]}` response.
fn parse_server_ips(json: &str) -> Vec<[u8; 4]> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = value.get("result").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<[u8; 4]> = Vec::new();
    for item in arr {
        if let Some(ip) = item.as_str().and_then(to_octets) {
            if !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
}

/// Parse the user's manual `iplist.txt`: deduped dotted-quad lines, skipping
/// comments/blank/junk. These are probed by the menu's own per-IP spawns, so we
/// only use them to dedupe the central list against (avoid double-listing).
fn parse_manual_ips(text: &str) -> Vec<[u8; 4]> {
    let mut out: Vec<[u8; 4]> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(ip) = to_octets(trimmed) {
            if !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
}

/// Extract the OS handle (as u64) for MSVCRT C-runtime fd `fd` from the
/// `lpReserved2` block, which has the shape `[i32 count][count u8 flags][count
/// handles]`. The handle width is **not** fixed: the live MENUDLL hands the
/// 64-bit child an 8-byte-handle table (`cbReserved2 == 4 + count + count*8`),
/// while a classic 32-bit CRT would write 4-byte handles — so we derive the
/// width from the block size rather than assume it. Only slots flagged FOPEN
/// (0x01) are valid; 0/INVALID are rejected.
// Only called from the Windows pipe sink; unit-tested everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
fn handle_from_reserved2(block: &[u8], fd: usize) -> Option<u64> {
    if block.len() < 4 {
        return None;
    }
    let count = u32::from_le_bytes(block[0..4].try_into().ok()?) as usize;
    if count == 0 || fd >= count {
        return None;
    }
    // region = bytes after the count header and the per-fd flag bytes.
    let region = block.len().checked_sub(4 + count)?;
    if region % count != 0 {
        return None;
    }
    let hsize = region / count; // handle width: 4 (32-bit CRT) or 8 (64-bit CRT)
    if hsize != 4 && hsize != 8 {
        return None;
    }
    // The fd must be an open slot (FOPEN bit in its flag byte).
    if block.get(4 + fd)? & 0x01 == 0 {
        return None;
    }
    let off = 4 + count + fd * hsize;
    let bytes = block.get(off..off + hsize)?;
    let h = if hsize == 4 {
        u32::from_le_bytes(bytes.try_into().ok()?) as u64
    } else {
        u64::from_le_bytes(bytes.try_into().ok()?)
    };
    if h == 0 || h == u32::MAX as u64 || h == u64::MAX {
        return None; // 0 / INVALID_HANDLE_VALUE -> not an open fd
    }
    Some(h)
}

/// Query a server's GameSpy `\status\` (UDP <ip>:4711). Returns the merged field
/// map and the measured round-trip in ms, or None on timeout/no reply.
fn query_status(ip: [u8; 4]) -> Option<(HashMap<String, String>, u32)> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    let dst = SocketAddr::from((Ipv4Addr::from(ip), 4711));
    let t0 = Instant::now();
    sock.send_to(b"\\status\\", dst).ok()?;

    let mut fields: HashMap<String, String> = HashMap::new();
    let mut ping_ms = 0u32;
    let mut buf = [0u8; 4096];
    let deadline = t0 + Duration::from_millis(1200);
    let mut got = false;
    // Collect datagrams until we see \final\ or the deadline; merge them. The
    // per-recv timeout is set to the time left before `deadline` each iteration,
    // so a blocking read can never run past the overall ~1200ms bound.
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        if remaining.is_zero() {
            break;
        }
        sock.set_read_timeout(Some(remaining)).ok();
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                if !got {
                    ping_ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;
                    got = true;
                }
                let reply = String::from_utf8_lossy(&buf[..n]);
                merge_status(&mut fields, &parse_status(&reply));
                if reply.contains("\\final\\") {
                    break;
                }
            }
            Err(_) => break, // timeout
        }
    }
    if got {
        Some((fields, ping_ms))
    } else {
        None
    }
}

/// True if a `\status\` reply carries something worth showing — a non-empty
/// mapname or hostname. An answered-but-empty reply is treated like no answer.
fn has_useful(fields: &HashMap<String, String>) -> bool {
    let nonempty = |k: &str| fields.get(k).is_some_and(|v| !v.is_empty());
    nonempty("mapname") || nonempty("hostname")
}

/// Resolve a single LAN host to a ServerInfo: prefer a useful `\status\` reply,
/// else fall back to the beacon (name/players/max_players, blank map). Run on
/// its own thread during the LAN sweep.
fn query_host(ip: [u8; 4], beacon: Beacon) -> ServerInfo {
    match query_status(ip) {
        Some((fields, ping)) if has_useful(&fields) => info_from_status(&fields, ip, ping),
        _ => ServerInfo {
            // fallback: beacon data (name/players/maxplayers), blank map - the
            // beacon carries no map, and a host that doesn't answer \status\ has
            // no other source for one.
            name: beacon.name,
            map: String::new(),
            gametype: String::new(),
            players: beacon.players,
            max_players: beacon.max_players,
            ping_ms: 0,
            ip,
        },
    }
}

/// Build a ServerInfo from a status map + known ip + ping.
fn info_from_status(fields: &HashMap<String, String>, ip: [u8; 4], ping_ms: u32) -> ServerInfo {
    let get = |k: &str| fields.get(k).cloned().unwrap_or_default();
    let num = |k: &str| fields.get(k).and_then(|v| v.parse().ok()).unwrap_or(0u32);
    ServerInfo {
        name: get("hostname"),
        map: get("mapname"),
        gametype: get("gametype"),
        players: num("numplayers"),
        max_players: num("maxplayers"),
        ping_ms,
        ip,
    }
}

/// Listen for `'D'` beacons on UDP :210 for `window`, returning distinct
/// (ip, beacon) by source address.
fn discover_lan(window: Duration) -> Vec<([u8; 4], Beacon)> {
    let mut found: Vec<([u8; 4], Beacon)> = Vec::new();
    let Ok(sock) = UdpSocket::bind("0.0.0.0:210") else {
        return found;
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let deadline = Instant::now() + window;
    let mut buf = [0u8; 2048];
    while Instant::now() < deadline {
        if let Ok((n, SocketAddr::V4(src))) = sock.recv_from(&mut buf) {
            if let Some(b) = parse_beacon(&buf[..n]) {
                let ip = src.ip().octets();
                if !found.iter().any(|(seen, _)| *seen == ip) {
                    found.push((ip, b));
                }
            }
        }
    }
    found
}

/// Fetch the central internet server list from Sanity. Returns deduped octet
/// IPs, or an empty list on any failure (offline/DNS/TLS/timeout) — the browser
/// then degrades to LAN + manual servers.
fn fetch_sanity_ips() -> Vec<[u8; 4]> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_TIMEOUT)
        .timeout_read(HTTP_TIMEOUT)
        .build();
    let Ok(resp) = agent
        .get(QUERY_URL)
        .query("query", GROQ)
        .query("returnQuery", "false")
        .call()
    else {
        return Vec::new();
    };
    match resp.into_string() {
        Ok(body) => parse_server_ips(&body),
        Err(_) => Vec::new(),
    }
}

/// Read the user's manual server IPs from `iplist.txt` next to the exe (used
/// only to dedupe the central list — the menu probes these separately).
fn manual_ips() -> Vec<[u8; 4]> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("iplist.txt")))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| parse_manual_ips(&text))
        .unwrap_or_default()
}

/// Write `bytes` to the menu's inherited pipe (CRT fd `write_fd`), or stdout when
/// running standalone (no lpReserved2 / non-Windows).
fn write_rows(write_fd: i32, bytes: &[u8]) {
    #[cfg(windows)]
    {
        if let Some(h) = pipe_handle(write_fd) {
            unsafe {
                use windows_sys::Win32::Storage::FileSystem::WriteFile;
                let mut written = 0u32;
                let _ = WriteFile(
                    h,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    &mut written,
                    std::ptr::null_mut(),
                );
            }
            return;
        }
    }
    let _ = write_fd;
    use std::io::Write;
    let _ = std::io::stdout().write_all(bytes);
}

#[cfg(windows)]
fn pipe_handle(write_fd: i32) -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::System::Threading::{GetStartupInfoW, STARTUPINFOW};
    if write_fd < 0 {
        return None;
    }
    unsafe {
        let mut si: STARTUPINFOW = std::mem::zeroed();
        GetStartupInfoW(&mut si);
        if si.cbReserved2 == 0 || si.lpReserved2.is_null() {
            return None;
        }
        let block = std::slice::from_raw_parts(si.lpReserved2, si.cbReserved2 as usize);
        handle_from_reserved2(block, write_fd as usize)
            .map(|h| h as windows_sys::Win32::Foundation::HANDLE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_fields() {
        let reply = "\\gamename\\cneagle\\gamever\\cneagle1.43\\location\\1\\hostname\\CE2022\\hostport\\24711\\mapname\\No mans land\\gametype\\ctf\\numplayers\\0\\maxplayers\\28\\gamemode\\openplaying\\final\\\\queryid\\22605.1";
        let m = parse_status(reply);
        assert_eq!(m.get("mapname").map(String::as_str), Some("No mans land"));
        assert_eq!(m.get("hostname").map(String::as_str), Some("CE2022"));
        assert_eq!(m.get("gametype").map(String::as_str), Some("ctf"));
        assert_eq!(m.get("numplayers").map(String::as_str), Some("0"));
        assert_eq!(m.get("maxplayers").map(String::as_str), Some("28"));
    }

    #[test]
    fn parse_status_merges_fragments_first_wins() {
        // Two datagrams parsed and merged; first value for a key wins.
        let mut m = parse_status("\\mapname\\A\\final\\\\queryid\\1.1");
        merge_status(&mut m, &parse_status("\\mapname\\B\\hostname\\H\\final\\\\queryid\\1.2"));
        assert_eq!(m.get("mapname").map(String::as_str), Some("A"));
        assert_eq!(m.get("hostname").map(String::as_str), Some("H"));
    }

    #[test]
    fn builds_row_with_real_map_name() {
        let st = ServerInfo {
            name: "CE2022".into(),
            map: "No mans land".into(),
            gametype: "ctf".into(),
            players: 3,
            max_players: 28,
            ping_ms: 42,
            ip: [89, 38, 98, 12],
        };
        // map is padded to MAP_WIDTH (15): "No mans land" (12) + 3 trailing spaces.
        assert_eq!(
            build_row(&st),
            "Name:\"CE2022\" Ping:42 Map:255\"No mans land   \" Players:3 MaxPlayers:28 \
             Spectators:0 MaxSpectators:0 Type:CTF IP:89.38.98.12 IPXAdress:0.0.0.0.0.0\n"
        );
    }

    #[test]
    fn row_sanitizes_quotes_and_blank_map() {
        let st = ServerInfo {
            name: "ab\"cd".into(),
            map: "".into(),
            gametype: "".into(),
            players: 0,
            max_players: 8,
            ping_ms: 0,
            ip: [10, 0, 0, 5],
        };
        let row = build_row(&st);
        assert!(row.contains("Name:\"abcd\"")); // quote stripped
        assert!(row.contains("Map:255\"               \"")); // blank map -> 15 spaces (LAN fallback)
        assert!(row.contains("Type:CTF")); // empty gametype defaults
        assert!(row.ends_with("IPXAdress:0.0.0.0.0.0\n"));
    }

    // Real beacons captured off the LAN with `nc`/a UDP listener on :210.
    // byte[7]=name_len+7, byte[12]=players+1, byte[13]=maxplayers+1, name from
    // byte 14 is NUL-terminated. byte[13] is deliberately NOT the name length -
    // an in-game host named "CodenameEagle.net US West" (25 chars) with 8 max
    // players sends byte[13]=9, and the old buf[13]-as-name_len parse truncated
    // the name to 9 bytes ("CodenameE"). See git history / iplist README.

    #[test]
    fn parses_real_ingame_host_beacon() {
        // 192.168.5.33 "CodenameEagle.net US West", 0/8 players.
        let buf: &[u8] = &[
            0x44, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x20, 0x87, 0x60, 0xff, 0x01, 0x01, 0x09,
            b'C', b'o', b'd', b'e', b'n', b'a', b'm', b'e', b'E', b'a', b'g', b'l', b'e', b'.',
            b'n', b'e', b't', b' ', b'U', b'S', b' ', b'W', b'e', b's', b't', 0x00,
        ];
        let b = parse_beacon(buf).unwrap();
        assert_eq!(b.name, "CodenameEagle.net US West"); // full name, not "CodenameE"
        assert_eq!(b.players, 0);
        assert_eq!(b.max_players, 8);
    }

    #[test]
    fn parses_real_short_name_beacon() {
        // 192.168.5.35 "LOCALDEV", byte[13]=16 -> the old parse computed
        // end=14+16=30 > 23 (packet len) and dropped this beacon entirely.
        let buf: &[u8] = &[
            0x44, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x0f, 0x87, 0x60, 0xff, 0x01, 0x01, 0x10,
            b'L', b'O', b'C', b'A', b'L', b'D', b'E', b'V', 0x00,
        ];
        let b = parse_beacon(buf).unwrap();
        assert_eq!(b.name, "LOCALDEV");
        assert_eq!(b.players, 0);
        assert_eq!(b.max_players, 15);
    }

    #[test]
    fn beacon_name_falls_back_to_packet_end_when_unterminated() {
        // No trailing NUL: name runs to the end of the datagram.
        let buf: &[u8] = &[
            0x44, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x0b, 0x87, 0x60, 0xff, 0x01, 0x03, 0x05,
            b'A', b'B', b'C', b'D',
        ];
        let b = parse_beacon(buf).unwrap();
        assert_eq!(b.name, "ABCD");
        assert_eq!(b.players, 2);
        assert_eq!(b.max_players, 4);
    }

    #[test]
    fn rejects_non_beacon() {
        assert!(parse_beacon(&[0x03, 0, 0, 0]).is_none());
        assert!(parse_beacon(&[]).is_none());
    }

    #[test]
    fn parses_six_int_args() {
        // MENUDLL sprintf's one string "writeFd mode o1 o2 o3 o4"; Rust may receive it
        // pre-split or as one token — handle both by joining then splitting.
        let a = parse_args(&[
            "8".into(),
            "1".into(),
            "89".into(),
            "38".into(),
            "98".into(),
            "12".into(),
        ])
        .unwrap();
        assert_eq!(a.write_fd, 8);
        assert_eq!(a.mode, 1);
        assert_eq!(a.ip, [89, 38, 98, 12]);
        assert!(!is_lan_sweep(&a.ip));

        let b = parse_args(&["8 1 255 255 255 255".into()]).unwrap();
        assert!(is_lan_sweep(&b.ip));
    }

    #[test]
    fn rejects_wrong_arg_count() {
        assert!(parse_args(&["1".into(), "2".into()]).is_none());
        assert!(parse_args(&["3 89 300 98 12 1".into()]).is_none()); // octet out of range
        assert!(parse_args(&["3 89 x 98 12 1".into()]).is_none()); // non-numeric token
    }

    #[test]
    fn to_octets_strict() {
        assert_eq!(to_octets("89.38.98.12"), Some([89, 38, 98, 12]));
        assert_eq!(to_octets("255.255.255.255"), Some([255, 255, 255, 255]));
        assert_eq!(to_octets(" 10.0.0.1 "), Some([10, 0, 0, 1]));
        assert_eq!(to_octets("256.0.0.1"), None);
        assert_eq!(to_octets("1.2.3"), None);
        assert_eq!(to_octets("a.b.c.d"), None);
    }

    #[test]
    fn parse_server_ips_from_sanity() {
        let json = r#"{"result":["89.38.98.12","10.0.0.5","89.38.98.12","not-an-ip"],"ms":2}"#;
        assert_eq!(parse_server_ips(json), vec![[89, 38, 98, 12], [10, 0, 0, 5]]);
        assert_eq!(parse_server_ips("garbage"), Vec::<[u8; 4]>::new());
        assert_eq!(parse_server_ips(r#"{"result":null}"#), Vec::<[u8; 4]>::new());
    }

    #[test]
    fn parse_manual_ips_skips_comments_and_junk() {
        let txt = "# my servers\n89.38.98.12\n\nexample.com\n10.0.0.5\n89.38.98.12\n";
        assert_eq!(parse_manual_ips(txt), vec![[89, 38, 98, 12], [10, 0, 0, 5]]);
    }

    #[test]
    fn extracts_handle_for_fd() {
        let count: u32 = 4;
        let mut block = Vec::new();
        block.extend_from_slice(&count.to_le_bytes());
        block.extend_from_slice(&[0x01, 0x01, 0x01, 0x09]); // flag byte per fd (value irrelevant here)
        for h in [0x10u32, 0x14, 0x18, 0xABCD] {
            // fd0..fd3 handles
            block.extend_from_slice(&h.to_le_bytes());
        }
        assert_eq!(handle_from_reserved2(&block, 3), Some(0xABCD));
        assert_eq!(handle_from_reserved2(&block, 0), Some(0x10));
    }

    #[test]
    fn rejects_bad_fd_or_block() {
        let block = [4u32.to_le_bytes()].concat(); // count=4 but no data
        assert_eq!(handle_from_reserved2(&block, 0), None);
        assert_eq!(handle_from_reserved2(&[], 0), None);
        // fd out of range
        let mut b = Vec::new();
        b.extend_from_slice(&1u32.to_le_bytes());
        b.push(0x01);
        b.extend_from_slice(&0u32.to_le_bytes()); // handle 0 = invalid
        assert_eq!(handle_from_reserved2(&b, 5), None);
        assert_eq!(handle_from_reserved2(&b, 0), None); // handle value 0 rejected
    }

    #[test]
    fn extracts_8byte_handle_from_live_block() {
        // Real lpReserved2 captured from the running game (cbReserved2=85): 9 fds,
        // 9 flag bytes, then 8-byte handles. fd7/fd8 are the _pipe ends (flag 0x89
        // = FOPEN|FPIPE|FTEXT); fd8 is the write end the menu passes to iplist.
        let count: u32 = 9;
        let mut block = Vec::new();
        block.extend_from_slice(&count.to_le_bytes());
        block.extend_from_slice(&[0xc1, 0xc1, 0xc1, 0x81, 0x01, 0x01, 0x81, 0x89, 0x89]);
        for h in [
            0xFFFF_FFFFu64, // fd0 stdin  (INVALID)
            0xFFFF_FFFF,    // fd1 stdout (INVALID)
            0xFFFF_FFFF,    // fd2 stderr (INVALID)
            0x4b8,          // fd3
            0x5a4,          // fd4
            0x5a8,          // fd5
            0x4b4,          // fd6
            0x5f4,          // fd7 pipe read
            0x5f0,          // fd8 pipe write
        ] {
            block.extend_from_slice(&h.to_le_bytes());
        }
        assert_eq!(block.len(), 85);
        assert_eq!(handle_from_reserved2(&block, 8), Some(0x5f0)); // the pipe write end
        assert_eq!(handle_from_reserved2(&block, 7), Some(0x5f4));
        assert_eq!(handle_from_reserved2(&block, 3), Some(0x4b8));
        assert_eq!(handle_from_reserved2(&block, 0), None); // INVALID_HANDLE rejected
    }
}
