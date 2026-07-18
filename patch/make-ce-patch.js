#!/usr/bin/env node
// Generate + apply the unified ce.exe portion of ce-patch.
//
// ce.exe gets ONE startup routine (placed in .data slack, reached by detouring the
// CRT-startup GetCommandLineA call) that:
//   - resolves SetCurrentDirectoryA / CreateDirectoryA / RegCreateKeyExA via
//     GetProcAddress (ce.exe imports none of them),
//   - GetModuleFileNameA -> <cedir>, SetCurrentDirectory(<cedir>)  [cwd fix],
//   - CreateDirectory("logs") / ("screenshots") / ("saves")  [relative, post-chdir],
//   - self-registers HKCU\Software\Classes\cneagle -> "<cedir>\ce.exe" %1,
//   - rewrites the command line  cneagle:// -> +connect   (trailing / trimmed),
//   - returns the (rewritten) command-line pointer.
//
// Image base 0x400000: file_off = vaddr - 0x400000. The routine lives in .data
// (no .text cave needed); the old standalone-cneagle .text cave is unused here.
//
// Usage:
//   make-ce-patch.js --print           # assemble, dump bytes/tables, verify only
//   make-ce-patch.js IN.exe -o OUT.exe # apply the full ce.exe patch to a copy
import assert from 'node:assert'
import {execFileSync} from 'node:child_process'
import fs from 'node:fs'
import {parseArgs} from 'node:util'

const IMAGE_BASE = 0x400000
const HOOK_VA = 0x496987 // call dword [GetCommandLineA] in mainCRTStartup
const HOOK_LEN = 6
const CAVE_VA = 0x4d4ae0 // .data zero slack (0x4d4add..0x4d6ae0, 8195 bytes)
const DATA_CHARS_OFF = 0x28c // .data section-header Characteristics field
const DATA_CHARS_OLD = 0xc0000040
const DATA_CHARS_NEW = 0xe0000040 // + IMAGE_SCN_MEM_EXECUTE

const IAT = {
  GetCommandLineA: 0x4a10e0,
  LoadLibraryA: 0x4a117c,
  GetProcAddress: 0x4a1180,
  GetModuleFileNameA: 0x4a11b4,
  RegSetValueExA: 0x4a1008,
  RegCloseKey: 0x4a100c,
  Sleep: 0x4a1120,
  select: 0x4a12cc, // WSOCK32 select (idle throttle waits on the socket)
}

// Filename-pointer repoints: file offset of a 4-byte pointer immediate -> string label.
// Order matters (matches the emitted edit table); kept as pairs, not an object.
const REPOINTS = [
  [0x85b07, 'r_error'], // mov esi, c:\error.log   (WinMain selector, default)
  [0x85b23, 'r_host'], // mov esi, host.log       (cmdline[0]=='0')
  [0x85b2c, 'r_slave'], // mov esi, c:\slave.log   (cmdline[0]=='1')
  [0x3cdfd, 'r_chat'], // push chat.log  (x4)
  [0x3cec9, 'r_chat'],
  [0x3cefb, 'r_chat'],
  [0x3cf0a, 'r_chat'],
  [0x443de, 'r_shot'], // push shot%d.tga
  [0x41d37, 'r_sg'], // push sg%d.dat  (delayed load, FUN_00441d20)
  [0x41dd4, 'r_sg'], // push sg%d.dat  (save, menu command 5)
  [0x41e7a, 'r_sg'], // push sg%d.dat  (load, menu command 6)
  [0x42307, 'r_temp'], // mov ecx, temp.dat  (level-start temp save)
]

// Savegame relocation: sg%d.dat + temp.dat move from the install root into
// saves\. The temp.dat LOAD site builds the name with an inline 9-byte copy of
// "temp.dat"; that copy is detoured into a cave stub that stores the padded
// 16-byte saves\temp.dat by immediates into the same 0x40-byte stack buffer.
const SG_NAME = 'saves\\sg%d.dat'
const TEMP_NAME = 'saves\\temp.dat'
const TEMP16 = Buffer.concat([Buffer.from(TEMP_NAME, 'latin1'), Buffer.alloc(16)]).subarray(0, 16)
const TEMPNAME_VA = 0x441d46 // the inline copy inside FUN_00441d20
const TEMPNAME_ORIG = '8b0d84284c008b1588284c00a08c284c00894c24008954240488442408'
const TEMPNAME_RET = 0x441d63 // lea ecx,[esp] right after the copy

const MASTER_OLD = 'master.gamespy.com'
const MASTER_NEW = 'ceservers.net'

// Same-length-or-shorter byte-pattern swaps applied to ce.exe: dead hostnames
// pointed at live ones. Shorter replacements are zero-padded (C strings truncate
// at the NUL).
const DOMAIN_OLD = 'codenameeagle.com'
const DOMAIN_NEW = 'codenameeagle.net'
const CE_SWAPS = [
  [MASTER_OLD, MASTER_NEW],
  [DOMAIN_OLD, DOMAIN_NEW],
]

// original pointer value at each repoint site (for verification)
const ORIG_PTR = {
  r_error: 0x4d287c,
  r_host: 0x4d2870,
  r_slave: 0x4d2860,
  r_chat: 0x4c1f14,
  r_shot: 0x4c31ec,
  r_sg: 0x4c2890,
  r_temp: 0x4c2884,
}

// Dedicated-server idle CPU throttle. The per-tick update FUN_00477640 is
// detoured into a stub that, ONLY when dedicated mode is on (byte[0x53fa6c] & 1)
// AND numplayers <= 0, yields the CPU for up to IDLE_SLEEP_MS via a blocking
// select() on the receive socket (so an incoming packet wakes it at once).
const IDLE_TICK_VA = 0x477640 // FUN_00477640 per-tick update
const IDLE_TICK_ORIG = '558bec83e4f8' // push ebp; mov ebp,esp; and esp,-8
const IDLE_TICK_RET = 0x477646 // entry + 6 (resume after the displaced prologue)
const IDLE_NUMPLAYERS = 0x472160 // FUN_00472160: DAT_00554f00 - (DAT_0053fa6c & 1)
const IDLE_DEDIFLAG = 0x53fa6c // DAT_0053fa6c; bit 0 = dedicated mode
const IDLE_QSOCK = 0x4be214 // DAT_004be214; the engine's UDP receive socket
const IDLE_SLEEP_MS = 50 // ~20 Hz while empty

// Session-kill hook ("newest session wins"): before CE spawns its lobby
// (StartLobby FUN_00485cb0), terminate any OTHER running CE session on this
// machine - every other ce.exe (PID != ours) AND every lobby.exe.
const LOBBY_KILL_VA = 0x485cb0
const LOBBY_KILL_ORIG = 'a1142d4c00' // mov eax, dword [0x4c2d14]
const LOBBY_KILL_RET = 0x485cb5 // entry + 5 (resume after the displaced insn)

// Music hooks: detour CE's CD-music functions into cemusic.dll (file-based Ogg
// Vorbis). Each .text site is overwritten with a jmp into a cave stub that calls
// the resolved export (or runs the displaced original if the DLL is absent ->
// CD-music fallback).
const MUSIC_PLAY_VA = 0x483020 // FUN_00483020(track) __fastcall, track in cl
const MUSIC_STOP_VA = 0x483170 // FUN_00483170() stop
// Volume: hook the CD/music-specific setter FUN_00486b10. Detour displaces its
// 7-byte entry `push esi; push edi; call 0x486ad0`; the call is relocated.
const MUSIC_VOL_VA = 0x486b10
const MUSIC_VOL_AD0 = 0x486ad0 // target of the displaced relative call
const MUSIC_VOL_RET = 0x486b17 // resume point after it
const MUSIC_PLAY_ORIG = 'a1041a4d00' // mov eax,[0x4d1a04]  (music-enabled flag)
const MUSIC_STOP_ORIG = 'a1a41f5500' // mov eax,[0x551fa4]
const MUSIC_VOL_ORIG = '5657e8b9ffffff' // push esi; push edi; call 0x486ad0

// Edit tables for the other binary (off, orig_hex, new_hex); orig "" = slack.
const LOBBY_EDITS = [
  [0xb24c, 'a1d03a4200', 'e935000000'], // player%d.txt site1: jmp over dump
  [0xb74f, '0f84f0000000', 'e9f100000090'], // player%d.txt site2: gate -> always skip
  [0x20460, '', Buffer.from('logs\\lobby.log\x00', 'latin1').toString('hex')], // logs\lobby.log string in .data slack
  [0xbe85, '10014200', '60044200'], // lobby.log: mov eax,0x420110 -> 0x420460
]

// Version bump: 1.43 -> 1.50, display AND wire protocol. The game's version int
// comes from one getter FUN_00472060 (`mov eax, 143; ret`); bumping its immediate
// bumps the wire protocol too. Also rebrands "CEBETA v%.2f" -> "CE v%.2f".
const VERSION_FN_OFF = 0x72060 // FUN_00472060: b8 8f 00 00 00 c3 (mov eax, 143; ret)
const VERSION_OLD = 143 // stock 1.43
const VERSION_NEW = 150 // v1.50 (int x 0.01)
const VERSION_STR_OFF = 0xcf30c // "CEBETA v%.2f" in .data
const VERSION_STR_OLD = Buffer.from('CEBETA v%.2f\x00', 'latin1')
const VERSION_STR_NEW = Buffer.from('CE v%.2f\x00', 'latin1')

const VERSION_EDITS = [
  [
    VERSION_STR_OFF,
    VERSION_STR_OLD.toString('hex'),
    padRight(VERSION_STR_NEW, VERSION_STR_OLD.length).toString('hex'),
  ],
  [
    VERSION_FN_OFF,
    Buffer.concat([Buffer.from([0xb8]), packU(VERSION_OLD)]).toString('hex'),
    Buffer.concat([Buffer.from([0xb8]), packU(VERSION_NEW)]).toString('hex'),
  ],
]

// Weapon-switch fire-cooldown reset (the "8 trick"). NOP the per-tick store that
// zeroes the shared shot clock during the "taking up weapon" state. The pattern
// carries 4 bytes of preceding + 6 bytes of following context to anchor the site.
const FIRECOOLDOWN_EDITS = [
  [
    0x14be4,
    '8b7c2410' + 'c7466800000000' + 'd99eb0020000',
    '8b7c2410' + '90'.repeat(7) + 'd99eb0020000',
  ], // NOP the timestamp reset @ 0x414be8
]

// Per-weapon cooldown ownership: the fire-rate gate FUN_0047ab20 is detoured at
// entry into a cave stub that reimplements it with cooldown ownership - the delay
// only applies when the held item is the one that fired last.
const FIREGATE_VA = 0x47ab20 // FUN_0047ab20, the fire-rate gate
const FIREGATE_ORIG = '538a5960f6' // push ebx; mov bl,[ecx+0x60]; (test bl,1 ...)
const TICK_NOW_VA = 0x53fab0 // DAT_0053fab0, the sim tick counter (36.4 Hz)
const OBJTAB_VA = 0x53fa98 // DAT_0053fa98, the object pointer table
const OWNER_MASK = 0x3ff // 1024 character slots (object index masked)

// Resolution-scaled aiming crosshair. Detour the 5-byte `mov eax,[0x4dde5c]` at
// 0x43814a (the Target.tga handle load, start of path 1) into a cave that draws
// the quad at +/- (screenHeight / CROSSHAIR_DIV), clamped to >= 4.
const CROSSHAIR_VA = 0x43814a // `mov eax,[0x4dde5c]` at the start of path 1
const CROSSHAIR_ORIG = 'a15cde4d00' // mov eax, dword [0x4dde5c]  (Target.tga handle)
const CROSSHAIR_RET = 0x4381a5 // path-1 epilogue (pop edi/esi/ebx; add esp,0x60; ret 4)
const CROSSHAIR_HANDLE_VA = 0x4dde5c // cached Target.tga texture handle
const TARGET_TGA_VA = 0x4c19f8 // "Target.tga"
const SCREEN_H_VA = 0x5555b4 // DAT_005555b4, render height (px); 0x5555b0 = width
const TEXLOAD_PTR = 0x53fad0 // load-texture-by-name fn ptr (ecx=name, edx=1)
const DRAW2D_PTR = 0x53fac8 // 2D-quad draw fn ptr (ecx=texture, edx=&verts)
const CROSSHAIR_DIV = 120 // half = screenHeight / 120 (~= original 4px at 480p)

// The "spontaneous explosion bug" (SEB): flip the owner==0 test in the
// out-of-world projectile branch (je 0x470d54 @ 0x470d26 -> jmp) so every
// out-of-world projectile takes the silent-removal path, restoring 1.36 behavior.
const SEB_EDITS = [
  [0x70d1e, '8b86ec020000' + '85c0' + '742c', '8b86ec020000' + '85c0' + 'eb2c'], // mov eax,[esi+2ec]; test eax,eax; je -> jmp @ 0x470d26
]

// Dedicated-server log file. The dedicated-mode status lines (server loaded,
// player joined/left/lost connection/out of sync) are emitted ONLY via
// WriteConsoleA in FUN_00443740, so they are lost on any headless host - e.g. a
// Wine container, where the dedicated console's AllocConsole yields no usable
// handle and every line is written to a NULL handle and discarded. Detour that
// one WriteConsoleA call site into a cave stub that also appends the formatted
// line to logs\server.log (open/append/close per line; the startup routine has
// already chdir'd to the gamedir and created logs\), then runs the original
// WriteConsoleA so a real console (native Windows) still works. The interactive
// prompt writes (FUN_004437c0's "Ok>") are a separate site and left alone.
const SERVLOG_VA = 0x4437ad // the `call ds:[WriteConsoleA]` inside FUN_00443740
const SERVLOG_ORIG = 'ff1594104a00' // call dword [0x4a1094] (WriteConsoleA)
const SERVLOG_RET = 0x4437b3 // site + 6 (pop edi, right after the call)
const IAT_CREATEFILEA = 0x4a1184
const IAT_WRITEFILE = 0x4a1100
const IAT_CLOSEHANDLE = 0x4a10c0

// Startup-cutscene relocation. Skip the boot reel (test esi,esi; je -> jmp @
// 0x4416bd) and play the campaign opening (intro.smk + m1c2.smk) at the
// menu->level transition when the selected level is 1 and it hasn't run before.
const BOOTCUT_EDITS = [
  [0x416bb, '85f6' + '7459', '85f6' + 'eb59'], // test esi,esi; je -> jmp @ 0x4416bd
]
const INTROCUT_VA = 0x4439b9 // lifecycle menu->level transition (FUN_004438c0)
const INTROCUT_ORIG = 'a110755500' // mov eax, dword [0x557510]  (level-selection global)
const INTROCUT_RET = 0x4439be // entry + 5 (resume after the displaced insn)
const INTROCUT_PLAY = 0x44c1a0 // play-cutscene-by-name(ecx), shared with credits/REFShowCutscene
const INTRO_SMK_VA = 0x4c2ce4 // "intro.smk" (existing .data string)
const M1C2_SMK_VA = 0x4c279b // "m1c2.smk" (tail of "cutscn\m1c2.smk")
const SELECTION_VA = 0x557510 // DAT_00557510; byte 0 = selected level number

// ce.exe single-player crash fixes (file offsets). Two long-standing crashes:
// the FP crash near enemy bases (NaN turret aim, sanitized at two consumption
// points in .text-tail slack) and the save crash (NOP the 9 D3DGrabScreen calls).
const BASECRASH_EDITS = [
  // (file_off, orig_hex, new_hex); orig "" = .text-tail slack (expect zeros)
  [
    0xa0d96,
    '',
    '508b4204250000807f3d0000807f7507c74204000000008b4208250000807f3d0000807f7507c74208000000008b420c250000807f3d0000807f7507c7420c0000000058d94204d94208d9420ce971b7fcff',
  ], // cave A: position NaN/inf sanitizer @ 0x4a0d96 (past the RtlUnwind thunk)
  [0xa0de8, '', '8b442408250000807f3d0000807f7508c74424080000803fd9442408e89735ffffe9ae45f8ff'], // cave B: acos NaN/inf guard @ 0x4a0de8
  [0x6c550, 'd94204d94208d9420c', 'e94148030090909090'], // hook A: transform FUN_0046c550 -> cave A
  [0x253b3, 'd9442408e8e4ef0600', 'e930ba070090909090'], // hook B: acos call site -> cave B
  [0x220, '96fd0900', '00000a00'], // .text VirtualSize 0x9fd96 -> 0xa0000 (map the caves)
  [0x602ce, 'e8fd4ffeff', '9090909090'], // savefix: NOP D3DGrabScreen call 1/9
  [0x602f9, 'e8d24ffeff', '9090909090'], // 2/9
  [0x60319, 'e8b24ffeff', '9090909090'], // 3/9
  [0x60372, 'e8594ffeff', '9090909090'], // 4/9
  [0x60392, 'e8394ffeff', '9090909090'], // 5/9
  [0x603b3, 'e8184ffeff', '9090909090'], // 6/9
  [0x604c3, 'e8084efeff', '9090909090'], // 7/9
  [0x604ee, 'e8dd4dfeff', '9090909090'], // 8/9
  [0x6050e, 'e8bd4dfeff', '9090909090'], // 9/9
]

const rasmCache = new Map()
function rasm2(text) {
  if (rasmCache.has(text)) return rasmCache.get(text)
  let stdout
  try {
    stdout = execFileSync('rasm2', ['-a', 'x86', '-b', '32', text], {encoding: 'utf8'})
  } catch (e) {
    const err = (e.stderr ?? '').toString().trim()
    throw new Error(`rasm2 failed for ${JSON.stringify(text)}: ${err}`)
  }
  const h = stdout.trim()
  if (!h) throw new Error(`rasm2 failed for ${JSON.stringify(text)}: empty output`)
  const bytes = Buffer.from(h, 'hex')
  rasmCache.set(text, bytes)
  return bytes
}

// Item constructors (mirror the assembler's opcode vocabulary).
const I = (t) => ['asm', t] // asm text
const IL = (t) => ['asml', t] // asm text; {label} -> the label's cave vaddr
const RAW = (h) => ['raw', h]
const JMP = (l) => ['jmp', l]
const JE = (l) => ['je', l]
const JNE = (l) => ['jne', l]
const JG = (l) => ['jg', l]
const JGE = (l) => ['jge', l]
const JLE = (l) => ['jle', l]
const JS = (l) => ['js', l]
const JP = (l) => ['jp', l]
const CALLL = (l) => ['calll', l]
const PUSHS = (l) => ['push', l]
const MOVCXL = (l) => ['movcx', l]
const LBL = (n) => ['label', n]
const STR = (n, t) => ['str', n, t]
const BUF = (n, sz) => ['buf', n, sz] // reserve sz zero bytes, labeled (runtime scratch)
const MOVST = (l) => ['storeeax', l] // mov [abs label], eax   (a3 + abs32)
const MOVLD = (l) => ['loadeax', l] // mov eax, [abs label]   (a1 + abs32)
const CALLABS = (l) => ['callabs', l] // call dword [abs label] (ff15 + abs32)
const JMPVA = (v) => ['jmpva', v] // jmp to an absolute vaddr (e9 + rel32)
const CALLVA = (v) => ['callva', v] // call an absolute vaddr (e8 + rel32)

const CALL = (name) => I(`call dword [${hex(IAT[name])}]`)

const PROG = [
  I('push ebp'),
  I('mov ebp, esp'),
  I('sub esp, 0x120'),
  I('push ebx'),
  I('push esi'),
  I('push edi'),

  CALL('GetCommandLineA'),
  I('mov dword [ebp-0x11c], eax'),

  // module path -> command string  "<path>" %1\0  at [ebp-0x110]; ebx = end ptr
  I('lea edi, [ebp-0x110]'),
  I('mov byte [edi], 0x22'),
  I('push 260'),
  I('lea eax, [edi+1]'),
  I('push eax'),
  I('push 0'),
  CALL('GetModuleFileNameA'),
  I('lea ecx, [eax+6]'),
  I('mov dword [ebp-0x118], ecx'),
  RAW('8d5c0701'), // lea ebx, [edi+eax+1]
  I('mov dword [ebx], 0x31252022'), // '"',' ','%','1'
  I('mov byte [ebx+4], 0'),

  // chdir to <cedir> (resolve SetCurrentDirectoryA; truncate at last '\'; restore)
  PUSHS('s_kernel32'),
  CALL('LoadLibraryA'),
  PUSHS('s_setcwd'),
  I('push eax'),
  CALL('GetProcAddress'),
  I('test eax, eax'),
  JE('L_mkdir'),
  I('mov esi, eax'),
  LBL('L_findbs'),
  I('dec ebx'),
  I('mov al, byte [ebx]'),
  I('cmp al, 0x5c'),
  JNE('L_findbs'),
  I('mov byte [ebx], 0'), // temp-terminate at the '\' -> buf+1 = <cedir>
  // copy <cedir> into the gamedir buffer so the stubbed Drive getter returns it
  MOVCXL('s_gamedir'), // ecx = &gamedir buffer
  I('lea edx, [edi+1]'), // edx = the dir string
  LBL('L_cpdrv'),
  I('mov al, byte [edx]'),
  I('mov byte [ecx], al'),
  I('inc ecx'),
  I('inc edx'),
  I('test al, al'),
  JNE('L_cpdrv'),
  I('lea eax, [edi+1]'),
  I('push eax'),
  I('call esi'), // SetCurrentDirectoryA(<cedir>)
  I('mov byte [ebx], 0x5c'),

  // create logs/, screenshots/ and saves/ (cwd == cedir now, so relative names)
  LBL('L_mkdir'),
  PUSHS('s_kernel32'),
  CALL('LoadLibraryA'),
  PUSHS('s_createdir'),
  I('push eax'),
  CALL('GetProcAddress'),
  I('test eax, eax'),
  JE('L_reg'),
  I('mov esi, eax'), // esi = CreateDirectoryA
  I('push 0'),
  PUSHS('s_logs'),
  I('call esi'),
  I('push 0'),
  PUSHS('s_screens'),
  I('call esi'),
  I('push 0'),
  PUSHS('s_saves'),
  I('call esi'),

  // load cemusic.dll and resolve the three music exports into g_play/g_stop/g_vol
  PUSHS('s_cemusic'),
  CALL('LoadLibraryA'),
  I('test eax, eax'),
  JE('L_reg'),
  I('mov esi, eax'), // esi = hmodule (survives GetProcAddress)
  PUSHS('s_play'),
  I('push esi'),
  CALL('GetProcAddress'),
  MOVST('g_play'),
  PUSHS('s_stop'),
  I('push esi'),
  CALL('GetProcAddress'),
  MOVST('g_stop'),
  PUSHS('s_vol'),
  I('push esi'),
  CALL('GetProcAddress'),
  MOVST('g_vol'),

  // resolve RegCreateKeyExA -> ebx
  LBL('L_reg'),
  PUSHS('s_advapi'),
  CALL('LoadLibraryA'),
  PUSHS('s_regcreate'),
  I('push eax'),
  CALL('GetProcAddress'),
  I('test eax, eax'),
  JE('L_rewrite'),
  I('mov ebx, eax'),

  // create ...\cneagle\shell\open\command; default = command string
  MOVCXL('s_key2'),
  CALLL('mk_key'),
  I('test eax, eax'),
  JNE('L_cnkey'),
  I('mov esi, dword [ebp-0x114]'),
  I('mov eax, dword [ebp-0x118]'),
  I('push eax'),
  I('lea eax, [ebp-0x110]'),
  I('push eax'),
  I('push 1'),
  I('push 0'),
  I('push 0'),
  I('push esi'),
  CALL('RegSetValueExA'),
  I('push esi'),
  CALL('RegCloseKey'),

  // (re)open cneagle key; set the two scheme values
  LBL('L_cnkey'),
  MOVCXL('s_key1'),
  CALLL('mk_key'),
  I('test eax, eax'),
  JNE('L_rewrite'),
  I('mov esi, dword [ebp-0x114]'),
  I('push 16'),
  PUSHS('s_urlce'),
  I('push 1'),
  I('push 0'),
  I('push 0'),
  I('push esi'),
  CALL('RegSetValueExA'),
  I('push 1'),
  PUSHS('s_empty'),
  I('push 1'),
  I('push 0'),
  PUSHS('s_urlproto'),
  I('push esi'),
  CALL('RegSetValueExA'),
  I('push esi'),
  CALL('RegCloseKey'),

  // rewrite command line: cneagle:// -> "+connect "
  LBL('L_rewrite'),
  I('mov esi, dword [ebp-0x11c]'),
  LBL('L_scan'),
  I('mov al, byte [esi]'),
  I('test al, al'),
  JE('L_done'),
  // case-insensitive scheme match: OR each byte with 0x20
  I('mov eax, dword [esi]'),
  I('or eax, 0x20202020'),
  I('cmp eax, 0x61656e63'),
  JNE('L_next'),
  I('mov eax, dword [esi+4]'),
  I('or eax, 0x20202020'),
  I('cmp eax, 0x3a656c67'),
  JNE('L_next'),
  RAW('668b4608'),
  RAW('660d2020'),
  RAW('663d2f2f'),
  JNE('L_next'), // mov ax,[esi+8]; or ax,0x2020; cmp ax,0x2f2f
  I('mov dword [esi], 0x6e6f632b'),
  I('mov dword [esi+4], 0x7463656e'),
  I('mov word [esi+8], 0x2020'),
  I('lea edi, [esi+10]'),
  LBL('L_end'),
  I('mov al, byte [edi]'),
  I('test al, al'),
  JE('L_trim'),
  I('inc edi'),
  JMP('L_end'),
  LBL('L_trim'),
  I('cmp byte [edi-1], 0x2f'),
  JNE('L_done'),
  I('mov byte [edi-1], 0'),
  JMP('L_done'),
  LBL('L_next'),
  I('inc esi'),
  JMP('L_scan'),

  LBL('L_done'),
  I('pop edi'),
  I('pop esi'),
  I('pop ebx'),
  I('mov eax, dword [ebp-0x11c]'),
  I('mov esp, ebp'),
  I('pop ebp'),
  I('ret'),

  // helper: RegCreateKeyExA(HKCU, ecx, 0,0,0, KEY_SET_VALUE, 0, &hKey, 0)
  LBL('mk_key'),
  I('lea eax, [ebp-0x114]'),
  I('push 0'),
  I('push eax'),
  I('push 0'),
  I('push 2'),
  I('push 0'),
  I('push 0'),
  I('push 0'),
  I('push ecx'),
  I('push 0x80000001'),
  I('call ebx'),
  I('ret'),

  // --- music hooks: reached by the .text detours at MUSIC_{PLAY,STOP,VOL}_VA ---
  LBL('h_play'),
  MOVLD('g_play'),
  I('test eax, eax'),
  JE('L_play_orig'),
  RAW('a1041a4d00'),
  I('test eax, eax'),
  JE('L_play_skip'), // DAT_004d1a04 == 0 -> no music
  I('movzx eax, cl'),
  I('push eax'),
  CALLABS('g_play'),
  I('add esp, 4'),
  LBL('L_play_skip'),
  I('ret'),
  LBL('L_play_orig'),
  RAW(MUSIC_PLAY_ORIG),
  JMPVA(MUSIC_PLAY_VA + 5),
  // stop(): cemusic_stop()
  LBL('h_stop'),
  MOVLD('g_stop'),
  I('test eax, eax'),
  JE('L_stop_orig'),
  CALLABS('g_stop'),
  I('ret'),
  LBL('L_stop_orig'),
  RAW(MUSIC_STOP_ORIG),
  JMPVA(MUSIC_STOP_VA + 5),
  // volume: forward the float fraction to cemusic_volume, preserving ecx (=hwnd)
  LBL('h_vol'),
  MOVLD('g_vol'),
  I('test eax, eax'),
  JE('L_vol_orig'),
  I('push ecx'), // save hwnd
  RAW('ff742408'), // push dword [esp+8] = entry [esp+4] = fraction
  CALLABS('g_vol'),
  I('add esp, 4'),
  I('pop ecx'),
  LBL('L_vol_orig'),
  I('push esi'),
  I('push edi'),
  CALLVA(MUSIC_VOL_AD0),
  JMPVA(MUSIC_VOL_RET),

  // --- session-kill hook: reached by the .text detour at StartLobby entry ---
  LBL('h_killlobby'),
  I('pushfd'),
  I('pushad'),
  PUSHS('s_kernel32'),
  CALL('LoadLibraryA'),
  I('test eax, eax'),
  JE('L_kill_done'),
  I('mov ebx, eax'), // ebx = kernel32 (preserved across GetProcAddress)
  PUSHS('s_createproc'),
  I('push ebx'),
  CALL('GetProcAddress'),
  MOVST('g_createproc'),
  PUSHS('s_wait'),
  I('push ebx'),
  CALL('GetProcAddress'),
  MOVST('g_wait'),
  PUSHS('s_close'),
  I('push ebx'),
  CALL('GetProcAddress'),
  MOVST('g_close'),
  PUSHS('s_getpid'),
  I('push ebx'),
  CALL('GetProcAddress'),
  MOVST('g_getpid'),
  PUSHS('s_sleep'),
  I('push ebx'),
  CALL('GetProcAddress'),
  MOVST('g_sleep'),
  MOVLD('g_createproc'),
  I('test eax, eax'),
  JE('L_kill_done'),
  // resolve user32!wsprintfA (only used to stamp our PID into the ce.exe kill filter)
  PUSHS('s_user32'),
  CALL('LoadLibraryA'),
  I('test eax, eax'),
  JE('L_after_ce'),
  PUSHS('s_wsprintf'),
  I('push eax'),
  CALL('GetProcAddress'),
  MOVST('g_wsprintf'),
  I('test eax, eax'),
  JE('L_after_ce'),
  MOVLD('g_getpid'),
  I('test eax, eax'),
  JE('L_after_ce'),
  // buf = sprintf("taskkill /F /IM ce.exe /FI \"PID ne %u\"", GetCurrentProcessId())
  CALLABS('g_getpid'),
  I('push eax'),
  PUSHS('s_fmt_cekill'),
  PUSHS('s_cekill_buf'),
  CALLABS('g_wsprintf'),
  I('add esp, 0xc'), // wsprintfA is cdecl -> caller cleans 3 args
  MOVCXL('s_cekill_buf'),
  CALLL('k_spawn'), // kill all OTHER ce.exe (excludes our PID)
  LBL('L_after_ce'),
  MOVCXL('s_taskkill'),
  CALLL('k_spawn'), // kill all lobby.exe
  // settle: let the OS reclaim the freed UDP 24711 before the new lobby binds
  MOVLD('g_sleep'),
  I('test eax, eax'),
  JE('L_kill_done'),
  I('push 0x1f4'),
  CALLABS('g_sleep'), // Sleep(500)
  LBL('L_kill_done'),
  I('popad'),
  I('popfd'),
  RAW(LOBBY_KILL_ORIG), // displaced: mov eax, dword [0x4c2d14]
  JMPVA(LOBBY_KILL_RET), // back to StartLobby entry + 5

  // k_spawn(ecx = lpCommandLine): CreateProcessA windowless + WaitForSingleObject(3s)
  LBL('k_spawn'),
  I('push ebx'),
  I('push esi'),
  I('push edi'),
  I('push ebp'),
  I('mov ebp, ecx'), // ebp = cmd ptr
  MOVLD('g_createproc'),
  I('test eax, eax'),
  JE('k_spawn_ret'),
  I('mov esi, eax'), // esi = CreateProcessA (callee-saved)
  I('sub esp, 0x54'), // PROCESS_INFORMATION(0x10) + STARTUPINFOA(0x44)
  I('mov edi, esp'),
  I('xor eax, eax'),
  I('mov ecx, 0x15'),
  I('cld'),
  RAW('f3ab'), // zero it
  I('lea ebx, [esp]'), // ebx = &pi
  I('lea edi, [esp+0x10]'), // edi = &si
  I('mov dword [edi], 0x44'), // si.cb = sizeof(STARTUPINFOA)
  I('push ebx'),
  I('push edi'),
  I('push 0'),
  I('push 0'),
  I('push 0x8000000'),
  I('push 0'),
  I('push 0'),
  I('push 0'),
  I('push ebp'),
  I('push 0'), // lpCommandLine, lpApplicationName=NULL
  I('call esi'), // CreateProcessA (stdcall, cleans 10 args)
  I('test eax, eax'),
  JE('k_spawn_free'),
  I('push 0xbb8'),
  RAW('ff33'),
  CALLABS('g_wait'), // WaitForSingleObject(pi.hProcess, 3000)
  RAW('ff33'),
  CALLABS('g_close'), // CloseHandle(pi.hProcess)
  RAW('ff7304'),
  CALLABS('g_close'), // CloseHandle(pi.hThread)  ([ebx+4])
  LBL('k_spawn_free'),
  I('add esp, 0x54'),
  LBL('k_spawn_ret'),
  I('pop ebp'),
  I('pop edi'),
  I('pop esi'),
  I('pop ebx'),
  I('ret'),

  // --- idle-throttle hook: reached by the .text detour at IDLE_TICK_VA ---
  LBL('h_idletick'),
  I('pushad'),
  I('pushfd'),
  I(`mov al, byte [${hex(IDLE_DEDIFLAG)}]`),
  I('test al, 1'),
  JE('L_tick_run'),
  CALLVA(IDLE_NUMPLAYERS),
  I('test eax, eax'),
  JG('L_tick_run'),
  // fd_set + timeval built on the stack; select is __stdcall. If the socket
  // isn't up yet (-1) fall back to a plain Sleep.
  // NB: `cmp eax, -1`, NOT `cmp eax, 0xffffffff` - rasm2 mis-encodes the latter.
  I(`mov eax, dword [${hex(IDLE_QSOCK)}]`),
  I('cmp eax, -1'),
  JE('L_idle_sleep'),
  I('sub esp, 0x110'),
  I('mov dword [esp], 1'), // fd_count = 1
  I('mov dword [esp+4], eax'), // fd_array[0] = receive socket
  I('mov dword [esp+0x108], 0'), // timeval.tv_sec = 0
  I(`mov dword [esp+0x10c], ${hex(IDLE_SLEEP_MS * 1000)}`), // timeval.tv_usec
  I('lea eax, [esp+0x108]'),
  I('push eax'), // timeout
  I('push 0'), // exceptfds = NULL
  I('push 0'), // writefds  = NULL
  I('lea eax, [esp+0xc]'),
  I('push eax'), // readfds (esp shifted by 3 pushes)
  I('push 0x40'), // nfds (ignored by winsock)
  CALL('select'),
  I('add esp, 0x110'),
  JMP('L_tick_run'),
  LBL('L_idle_sleep'),
  I(`push ${hex(IDLE_SLEEP_MS)}`),
  CALL('Sleep'),
  LBL('L_tick_run'),
  I('popfd'),
  I('popad'),
  RAW(IDLE_TICK_ORIG), // displaced: push ebp; mov ebp,esp; and esp,-8
  JMPVA(IDLE_TICK_RET), // back to the tick function + 6

  // --- temp-savegame name stub: reached by the .text detour at TEMPNAME_VA ---
  LBL('h_tempname'),
  ...Array.from({length: 4}, (_, k) => {
    const i = k * 4
    return I(`mov dword [esp+${hex(i)}], ${hex(TEMP16.readUInt32LE(i))}`)
  }),
  JMPVA(TEMPNAME_RET),

  // --- campaign-intro hook: reached by the .text detour at INTROCUT_VA ---
  LBL('h_introcut'),
  I('pushfd'),
  I('pushad'),
  I(`mov al, byte [${hex(IDLE_DEDIFLAG)}]`),
  I('test al, 1'),
  JNE('L_intro_skip'),
  I(`mov al, byte [${hex(SELECTION_VA)}]`),
  I('cmp al, 1'),
  JNE('L_intro_skip'),
  MOVLD('g_introdone'),
  I('test eax, eax'),
  JNE('L_intro_skip'),
  I('mov eax, 1'),
  MOVST('g_introdone'),
  I(`mov ecx, ${hex(INTRO_SMK_VA)}`),
  CALLVA(INTROCUT_PLAY),
  I(`mov ecx, ${hex(M1C2_SMK_VA)}`),
  CALLVA(INTROCUT_PLAY),
  LBL('L_intro_skip'),
  I('popad'),
  I('popfd'),
  RAW(INTROCUT_ORIG), // displaced: mov eax, dword [0x557510]
  JMPVA(INTROCUT_RET), // back to the transition insn + 5

  // --- fire-rate gate: reached by the .text detour at FIREGATE_VA ---
  LBL('h_firegate'),
  I('push ebx'),
  I('mov bl, byte [ecx+0x60]'),
  I('test bl, 1'),
  JE('L_fg_ret'), // trigger not pressed
  // Take-up handling: mirror stock's per-tick zeroing. Mid-take-up -> allow
  // without committing; take-up completing this tick -> fall through to the full
  // logic, commits included.
  I('mov al, byte [ecx+0x2c4]'),
  I('test al, al'),
  JE('L_fg_ready'),
  I('cmp al, 3'),
  JE('L_fg_ready'),
  I(`mov eax, dword [${hex(TICK_NOW_VA)}]`),
  I('sub eax, dword [ecx+0x2c8]'), // ticks since the switch started
  I('push eax'),
  RAW('db0424'), // fild dword [esp]  (rasm2 lacks x87)
  I('pop eax'),
  RAW('d899cc020000'), // fcomp dword [ecx+0x2cc] (duration)
  RAW('dfe0'), // fnstsw ax
  I('test ah, 5'),
  JP('L_fg_ready'), // completes this tick -> real shot
  JMP('L_fg_ret'), // still drawing -> allow, no commit
  LBL('L_fg_ready'),
  I('push esi'),
  I('push edi'),
  // resolve the held item -> edi = its project index + 1 (0 = unresolved)
  I('xor edi, edi'),
  I('mov eax, dword [ecx+0x98]'), // held-item object index
  I('test eax, eax'),
  JS('L_fg_stock'),
  I('cmp eax, 0x4000'),
  JGE('L_fg_stock'),
  I(`mov edx, dword [${hex(OBJTAB_VA)}]`), // object pointer table
  I('test edx, edx'),
  JE('L_fg_stock'),
  // NB: no scaled-index operands anywhere - rasm2 silently drops the *N scale;
  // shift instead.
  I('shl eax, 2'),
  I('mov eax, dword [edx+eax]'), // the item object
  I('test eax, eax'),
  JE('L_fg_stock'),
  I('mov edi, dword [eax+0x2b4]'), // its project index
  I('inc edi'),
  // a different item owns the cooldown -> fire without consulting the clock
  I('mov eax, dword [ecx+0x7c]'), // character object index
  I(`and eax, ${hex(OWNER_MASK)}`),
  I('add eax, eax'), // word-sized entries
  IL('movzx edx, word [eax + {g_ownertab}]'),
  I('cmp edx, edi'),
  JNE('L_fg_allow'),
  // stock check: elapsed since the last shot vs the held weapon's FireDelay
  LBL('L_fg_stock'),
  I('mov eax, dword [ecx+0x68]'), // last-shot tick
  I(`mov edx, dword [${hex(TICK_NOW_VA)}]`), // now
  I('cmp edx, eax'),
  JLE('L_fg_allow'),
  I('sub edx, eax'),
  I('cmp edx, dword [ecx+0x6c]'), // FireDelay (ticks)
  JGE('L_fg_allow'),
  I('and bl, 0xfe'), // veto: clear the trigger bit
  I('mov byte [ecx+0x60], bl'),
  JMP('L_fg_pop'),
  LBL('L_fg_allow'),
  I(`mov edx, dword [${hex(TICK_NOW_VA)}]`),
  I('mov dword [ecx+0x68], edx'), // commit the shot time
  I('test edi, edi'),
  JE('L_fg_pop'), // item unresolved -> owner untouched
  I('mov eax, dword [ecx+0x7c]'),
  I(`and eax, ${hex(OWNER_MASK)}`),
  I('add eax, eax'),
  IL('mov word [eax + {g_ownertab}], di'), // owner = the held item
  LBL('L_fg_pop'),
  I('pop edi'),
  I('pop esi'),
  LBL('L_fg_ret'),
  I('pop ebx'),
  I('ret'),

  // --- crosshair scaler: reached by the .text detour at CROSSHAIR_VA ---
  LBL('h_crosshair'),
  I(`mov eax, dword [${hex(SCREEN_H_VA)}]`), // eax = render height
  I('xor edx, edx'),
  I(`mov ecx, ${CROSSHAIR_DIV}`),
  I('div ecx'), // eax = height / DIV = half
  I('cmp eax, 4'),
  JGE('L_ch_ok'),
  I('mov eax, 4'), // clamp: never smaller than stock 8px
  LBL('L_ch_ok'),
  I('sub esi, eax'), // esi = x - half
  I('sub edi, eax'), // edi = y - half
  I('add eax, eax'), // eax = full = 2*half
  I('mov dword [esp+0x10], esi'), // x-half
  I('mov dword [esp+0x14], esi'),
  I('mov dword [esp+0x20], edi'), // y-half
  I('mov dword [esp+0x2c], edi'),
  I('mov edx, edi'),
  I('add edx, eax'), // edx = y-half + full = y+half
  //   (NOT `lea edx,[edi+eax]` - rasm2 silently drops the index register)
  I('mov dword [esp+0x24], edx'),
  I('mov dword [esp+0x28], edx'),
  I('add esi, eax'), // esi = x + half
  I('mov dword [esp+0x18], esi'),
  I('mov dword [esp+0x1c], esi'),
  // stock load-if-absent of Target.tga, then draw
  I(`mov eax, dword [${hex(CROSSHAIR_HANDLE_VA)}]`),
  I('test eax, eax'),
  JNE('L_ch_draw'),
  I('mov edx, ebx'), // ebx = 1 (mip flag, as in the original)
  I(`mov ecx, ${hex(TARGET_TGA_VA)}`),
  I(`call dword [${hex(TEXLOAD_PTR)}]`),
  I(`mov dword [${hex(CROSSHAIR_HANDLE_VA)}], eax`),
  LBL('L_ch_draw'),
  I('test eax, eax'),
  JE('L_ch_done'), // texture missing -> skip draw
  I('lea edx, [esp+0x10]'),
  I('mov ecx, eax'),
  I(`call dword [${hex(DRAW2D_PTR)}]`),
  LBL('L_ch_done'),
  JMPVA(CROSSHAIR_RET),

  // routine strings
  STR('s_advapi', 'advapi32.dll'),
  STR('s_regcreate', 'RegCreateKeyExA'),
  STR('s_kernel32', 'kernel32.dll'),
  STR('s_setcwd', 'SetCurrentDirectoryA'),
  STR('s_createdir', 'CreateDirectoryA'),
  STR('s_logs', 'logs'),
  STR('s_screens', 'screenshots'),
  STR('s_saves', 'saves'),
  STR('s_key1', 'Software\\Classes\\cneagle'),
  STR('s_urlce', 'URL:CE Protocol'),
  STR('s_urlproto', 'URL Protocol'),
  STR('s_empty', ''),
  STR('s_key2', 'Software\\Classes\\cneagle\\shell\\open\\command'),
  BUF('s_gamedir', 264), // runtime: the install dir, returned by the Drive getter
  // filename repoint targets (referenced by .text operands, not the routine)
  STR('r_error', 'logs\\error.log'),
  STR('r_host', 'logs\\host.log'),
  STR('r_slave', 'logs\\slave.log'),
  STR('r_chat', 'logs\\chat.log'),
  STR('r_shot', 'screenshots\\shot%d.tga'),
  STR('r_sg', SG_NAME),
  STR('r_temp', TEMP_NAME),
  // cemusic.dll + its export names; runtime function-pointer slots (null until resolved)
  STR('s_cemusic', 'cemusic.dll'),
  STR('s_play', 'cemusic_play'),
  STR('s_stop', 'cemusic_stop'),
  STR('s_vol', 'cemusic_volume'),
  BUF('g_play', 4),
  BUF('g_stop', 4),
  BUF('g_vol', 4),
  // session-kill hook: export names + resolved function-pointer slots + scratch
  STR('s_createproc', 'CreateProcessA'),
  STR('s_wait', 'WaitForSingleObject'),
  STR('s_close', 'CloseHandle'),
  STR('s_getpid', 'GetCurrentProcessId'),
  STR('s_sleep', 'Sleep'),
  STR('s_user32', 'user32.dll'),
  STR('s_wsprintf', 'wsprintfA'),
  STR('s_taskkill', 'taskkill /F /IM lobby.exe'),
  STR('s_fmt_cekill', 'taskkill /F /IM ce.exe /FI "PID ne %u"'),
  BUF('s_cekill_buf', 64), // runtime: the ce.exe kill command with our PID stamped in
  BUF('g_createproc', 4),
  BUF('g_wait', 4),
  BUF('g_close', 4),
  BUF('g_getpid', 4),
  BUF('g_sleep', 4),
  BUF('g_wsprintf', 4),
  BUF('g_introdone', 4), // campaign-intro hook: played-once-this-process flag
  // fire-gate cooldown owners: per character slot ([char+0x7c] & OWNER_MASK),
  // the project index + 1 of the item that last fired (0 = none yet)
  BUF('g_ownertab', (OWNER_MASK + 1) * 2),

  // --- dedicated-server log: reached by the .text detour at SERVLOG_VA ---
  // Appended at the tail so every existing cave offset is unchanged. On entry
  // (jmp, no return pushed) the five WriteConsoleA args are on the stack:
  //   [esp]=handle [esp+4]=lpBuffer [esp+8]=nChars [esp+12]=&written [esp+16]=0
  // Append lpBuffer/nChars to logs\server.log, then run the original call.
  LBL('h_servlog'),
  I('mov eax, dword [esp+4]'), // lpBuffer
  MOVST('sl_buf'),
  I('mov eax, dword [esp+8]'), // nChars
  MOVST('sl_len'),
  I('pushad'),
  // CreateFileA(name, FILE_APPEND_DATA, FILE_SHARE_READ|WRITE, 0, OPEN_ALWAYS, NORMAL, 0)
  I('push 0'),
  I('push 0x80'), // FILE_ATTRIBUTE_NORMAL
  I('push 4'), // OPEN_ALWAYS
  I('push 0'),
  I('push 3'), // FILE_SHARE_READ | FILE_SHARE_WRITE
  I('push 4'), // FILE_APPEND_DATA -> writes always land at EOF
  PUSHS('s_servlog'),
  I(`call dword [${hex(IAT_CREATEFILEA)}]`),
  I('cmp eax, -1'), // INVALID_HANDLE_VALUE (NB: -1, not 0xffffffff - rasm2 quirk)
  JE('L_sl_done'),
  I('mov esi, eax'), // hFile
  // WriteFile(hFile, lpBuffer, nChars, &written, 0)
  I('push 0'),
  PUSHS('sl_written'),
  MOVLD('sl_len'),
  I('push eax'),
  MOVLD('sl_buf'),
  I('push eax'),
  I('push esi'),
  I(`call dword [${hex(IAT_WRITEFILE)}]`),
  I('push esi'),
  I(`call dword [${hex(IAT_CLOSEHANDLE)}]`),
  LBL('L_sl_done'),
  I('popad'),
  RAW(SERVLOG_ORIG), // displaced: call ds:[WriteConsoleA]
  JMPVA(SERVLOG_RET), // back to the site + 6
  STR('s_servlog', 'logs\\server.log'),
  BUF('sl_buf', 4), // scratch: lpBuffer, stashed before pushad
  BUF('sl_len', 4), // scratch: nChars
  BUF('sl_written', 4), // WriteFile's lpNumberOfBytesWritten
]

function length(it) {
  const k = it[0]
  if (k === 'raw') return Buffer.from(it[1], 'hex').length
  if (k === 'asm') return rasm2(it[1]).length
  // {label} vaddrs are always 32-bit; a same-width dummy gives the true length
  if (k === 'asml') return rasm2(it[1].replace(/\{\w+\}/g, '0x11223344')).length
  if (k === 'jmp' || k === 'calll' || k === 'jmpva' || k === 'callva') return 5
  if (
    k === 'je' ||
    k === 'jne' ||
    k === 'jg' ||
    k === 'jge' ||
    k === 'jle' ||
    k === 'js' ||
    k === 'jp'
  )
    return 6
  if (k === 'push' || k === 'movcx' || k === 'storeeax' || k === 'loadeax') return 5
  if (k === 'callabs') return 6
  if (k === 'label') return 0
  if (k === 'str') return Buffer.byteLength(it[2], 'latin1') + 1
  if (k === 'buf') return it[2]
  throw new Error(`bad item ${JSON.stringify(it)}`)
}

function assemble() {
  let off = 0
  const labels = {}
  const offsets = []
  for (const it of PROG) {
    offsets.push(off)
    if (it[0] === 'label' || it[0] === 'str' || it[0] === 'buf') labels[it[1]] = off
    off += length(it)
  }
  const parts = []
  for (let idx = 0; idx < PROG.length; idx++) {
    const it = PROG[idx]
    const o = offsets[idx]
    const k = it[0]
    if (k === 'raw') parts.push(Buffer.from(it[1], 'hex'))
    else if (k === 'asm') parts.push(rasm2(it[1]))
    else if (k === 'asml')
      parts.push(rasm2(it[1].replace(/\{(\w+)\}/g, (_, name) => hex(CAVE_VA + labels[name]))))
    else if (
      k === 'jmp' ||
      k === 'je' ||
      k === 'jne' ||
      k === 'jg' ||
      k === 'jge' ||
      k === 'jle' ||
      k === 'js' ||
      k === 'jp' ||
      k === 'calll'
    ) {
      const rel = labels[it[1]] - (o + length(it))
      const op = {
        jmp: Buffer.from([0xe9]),
        je: Buffer.from([0x0f, 0x84]),
        jne: Buffer.from([0x0f, 0x85]),
        jg: Buffer.from([0x0f, 0x8f]),
        jge: Buffer.from([0x0f, 0x8d]),
        jle: Buffer.from([0x0f, 0x8e]),
        js: Buffer.from([0x0f, 0x88]),
        jp: Buffer.from([0x0f, 0x8a]),
        calll: Buffer.from([0xe8]),
      }[k]
      parts.push(Buffer.concat([op, packI(rel)]))
    } else if (k === 'push' || k === 'movcx') {
      const va = CAVE_VA + labels[it[1]]
      parts.push(Buffer.concat([Buffer.from([k === 'push' ? 0x68 : 0xb9]), packU(va)]))
    } else if (k === 'storeeax' || k === 'loadeax' || k === 'callabs') {
      const va = CAVE_VA + labels[it[1]]
      const op = {
        storeeax: Buffer.from([0xa3]),
        loadeax: Buffer.from([0xa1]),
        callabs: Buffer.from([0xff, 0x15]),
      }[k]
      parts.push(Buffer.concat([op, packU(va)]))
    } else if (k === 'jmpva' || k === 'callva') {
      const op = Buffer.from([k === 'jmpva' ? 0xe9 : 0xe8])
      parts.push(Buffer.concat([op, packI(it[1] - (CAVE_VA + o + 5))]))
    } else if (k === 'label') {
      // no bytes
    } else if (k === 'str') {
      parts.push(Buffer.concat([Buffer.from(it[2], 'latin1'), Buffer.from([0])]))
    } else if (k === 'buf') {
      parts.push(Buffer.alloc(it[2]))
    }
  }
  const out = Buffer.concat(parts)
  const strvas = {}
  for (const [n, ofs] of Object.entries(labels)) {
    if (!n.startsWith('L_') && n !== 'mk_key') strvas[n] = CAVE_VA + ofs
  }
  return [out, strvas]
}

function le32(v) {
  return packU(v).toString('hex')
}

function ceEdits(cave, detour, strvas) {
  // ce.exe edit list (off, orig_hex, new_hex); first entry = the .data routine.
  const e = [
    [CAVE_VA - IMAGE_BASE, '', cave.toString('hex')], // cave (slack)
    [HOOK_VA - IMAGE_BASE, 'ff15e0104a00', detour.toString('hex')], // detour
    [DATA_CHARS_OFF, le32(DATA_CHARS_OLD), le32(DATA_CHARS_NEW)], // .data exec flag
    // FUN_0044c120 (the registry "Drive" getter): stub it to return our gamedir
    // buffer (the cave fills it with <cedir> at startup) instead of reading the
    // registry.
    [0x4c120, '83ec0c8d4424', 'b8' + le32(strvas['s_gamedir']) + 'c3'],
  ]
  for (const [off, lbl] of REPOINTS) {
    e.push([off, le32(ORIG_PTR[lbl]), le32(strvas[lbl])]) // 12 repoints
  }
  // music detours: overwrite each .text entry with a jmp into its cave stub
  const detourVa = (site, target, total) => {
    const b = Buffer.concat([
      Buffer.from([0xe9]),
      packI(target - (site + 5)),
      Buffer.alloc(total - 5, 0x90),
    ])
    return b.toString('hex')
  }
  e.push([
    MUSIC_PLAY_VA - IMAGE_BASE,
    MUSIC_PLAY_ORIG,
    detourVa(MUSIC_PLAY_VA, strvas['h_play'], 5),
  ])
  e.push([
    MUSIC_STOP_VA - IMAGE_BASE,
    MUSIC_STOP_ORIG,
    detourVa(MUSIC_STOP_VA, strvas['h_stop'], 5),
  ])
  e.push([MUSIC_VOL_VA - IMAGE_BASE, MUSIC_VOL_ORIG, detourVa(MUSIC_VOL_VA, strvas['h_vol'], 7)])
  // lobby-kill: detour StartLobby's 5-byte entry insn into its cave stub
  e.push([
    LOBBY_KILL_VA - IMAGE_BASE,
    LOBBY_KILL_ORIG,
    detourVa(LOBBY_KILL_VA, strvas['h_killlobby'], 5),
  ])
  // idle-throttle: detour the tick function's 6-byte prologue into its cave stub
  e.push([
    IDLE_TICK_VA - IMAGE_BASE,
    IDLE_TICK_ORIG,
    detourVa(IDLE_TICK_VA, strvas['h_idletick'], 6),
  ])
  // temp-savegame name: detour the 29-byte inline "temp.dat" copy into its cave stub
  e.push([TEMPNAME_VA - IMAGE_BASE, TEMPNAME_ORIG, detourVa(TEMPNAME_VA, strvas['h_tempname'], 29)])
  // campaign-intro: detour the menu->level transition insn into its cave stub
  e.push([INTROCUT_VA - IMAGE_BASE, INTROCUT_ORIG, detourVa(INTROCUT_VA, strvas['h_introcut'], 5)])
  // fire-rate gate: detour the entry into the ownership-aware reimplementation
  e.push([FIREGATE_VA - IMAGE_BASE, FIREGATE_ORIG, detourVa(FIREGATE_VA, strvas['h_firegate'], 5)])
  // crosshair: detour path-1's Target.tga-handle load into the resolution scaler
  e.push([
    CROSSHAIR_VA - IMAGE_BASE,
    CROSSHAIR_ORIG,
    detourVa(CROSSHAIR_VA, strvas['h_crosshair'], 5),
  ])
  // dedicated-server log: detour FUN_00443740's WriteConsoleA (6-byte call) into
  // the cave stub that also appends the line to logs\server.log
  e.push([SERVLOG_VA - IMAGE_BASE, SERVLOG_ORIG, detourVa(SERVLOG_VA, strvas['h_servlog'], 6)])
  e.push(...BOOTCUT_EDITS) // startup-cutscene relocation: skip the boot reel
  e.push(...BASECRASH_EDITS) // SP base-crash (NaN turret aim) + save-crash fixes
  e.push(...VERSION_EDITS) // version bump: CEBETA v1.43 -> CE v1.50 (display + wire)
  e.push(...FIRECOOLDOWN_EDITS) // weapon-switch fire-cooldown reset (the "8 trick")
  e.push(...SEB_EDITS) // spontaneous-explosion bug: out-of-world shot -> silent removal
  return e
}

function rows(edits) {
  return edits.map(([off, o, n]) => `    (${hex(off)}, "${o}", "${n}"),\n`).join('')
}

function emitRust(cave, detour, strvas) {
  const out = []
  out.push('// Generated by make-ce-patch.js --rust - do not hand-edit.')
  out.push('// Each edit is (file_offset, orig_hex, new_hex); orig "" means slack (expect zeros).')
  out.push(
    `pub const CE_EDITS: &[(usize, &str, &str)] = &[\n${rows(ceEdits(cave, detour, strvas))}];`,
  )
  out.push(`pub const LOBBY_EDITS: &[(usize, &str, &str)] = &[\n${rows(LOBBY_EDITS)}];`)
  out.push('// Same-length-or-shorter byte-pattern swaps (dead hostnames -> live ones),')
  out.push('// applied to ce.exe. Shorter replacements are zero-padded (C strings truncate).')
  const swapRows = CE_SWAPS.map(([o, n]) => `    (b"${o}", b"${n}"),\n`).join('')
  out.push(`pub const CE_SWAPS: &[(&[u8], &[u8])] = &[\n${swapRows}];`)
  out.push('// Byte-patched binaries: (filename, hostname_swaps, edits). menudll.dll is')
  out.push("// not here - it's a bundled prebuilt binary, installed like iplist.exe.")
  out.push('pub const FILES: &[(&str, &[(&[u8], &[u8])], &[(usize, &str, &str)])] = &[')
  out.push('    ("ce.exe", CE_SWAPS, CE_EDITS),')
  out.push('    ("lobby.exe", &[], LOBBY_EDITS),')
  out.push('];')
  out.push('// Prebuilt binaries the patcher installs (copied from beside the tool).')
  out.push('pub const BUNDLED: &[&str] = &["iplist.exe", "menudll.dll", "cemusic.dll"];')
  process.stdout.write(out.join('\n') + '\n')
}

function main() {
  const {values, positionals} = parseArgs({
    options: {
      out: {type: 'string', short: 'o'},
      print: {type: 'boolean'},
      rust: {type: 'boolean'},
    },
    allowPositionals: true,
  })
  const infile = positionals[0]

  const [cave, strvas] = assemble()
  const caveEnd = CAVE_VA + cave.length
  if (caveEnd > 0x4d6ae0) {
    throw new Error(`cave overflows .data slack: ends ${hex(caveEnd)} > 0x4d6ae0`)
  }
  const rel = CAVE_VA - (HOOK_VA + 5)
  const detour = Buffer.concat([Buffer.from([0xe8]), packI(rel), Buffer.from([0x90])])
  assert(detour.length === HOOK_LEN)

  if (values.rust) {
    emitRust(cave, detour, strvas)
    return
  }

  console.log(
    `cave @ ${hex(CAVE_VA)} (.data)  len=${cave.length}  ends ${hex(caveEnd)}  (${0x4d6ae0 - caveEnd} slack left)`,
  )
  console.log(`detour @ ${hex(HOOK_VA)}: ${detour.toString('hex')}  (call ${hex(CAVE_VA)}; nop)`)
  console.log(
    `data exec flag: file ${hex(DATA_CHARS_OFF)}  ${hex(DATA_CHARS_OLD)} -> ${hex(DATA_CHARS_NEW)}`,
  )
  console.log('repoints (file off -> new string vaddr):')
  for (const [off, lbl] of REPOINTS) {
    console.log(`  ${hex08(off)} -> ${hex(strvas[lbl])}  (${lbl})`)
  }
  for (const [old, nw] of CE_SWAPS) {
    console.log(`hostname swap: ${old} -> ${nw}`)
  }
  console.log('\ncave bytes:')
  const h = cave.toString('hex')
  for (let i = 0; i < h.length; i += 64) console.log(' ' + h.slice(i, i + 64))

  if (values.print || !infile) return
  if (!values.out) throw new Error('need -o OUT.exe')
  const data = fs.readFileSync(infile)
  // apply the single-source edit list (cave, detour, .data flag, Drive stub, repoints)
  for (const [off, origHex, newHex] of ceEdits(cave, detour, strvas)) {
    const nw = Buffer.from(newHex, 'hex')
    if (origHex) {
      assert(
        data.subarray(off, off + nw.length).equals(Buffer.from(origHex, 'hex')),
        `orig mismatch @ ${hex(off)}`,
      )
    }
    nw.copy(data, off)
  }
  // dead-hostname swaps (pattern), zero-padded when the replacement is shorter
  for (const [old, nw] of CE_SWAPS) {
    const oldBuf = Buffer.from(old, 'latin1')
    const newBuf = Buffer.from(nw, 'latin1')
    let i = data.indexOf(oldBuf)
    while (i !== -1) {
      newBuf.copy(data, i)
      data.fill(0, i + newBuf.length, i + oldBuf.length)
      i = data.indexOf(oldBuf, i + oldBuf.length)
    }
  }
  fs.writeFileSync(values.out, data)
  console.log(`\npatched -> ${values.out}`)
}

function hex(v) {
  return '0x' + (v >>> 0).toString(16)
}

// hex with 0x prefix, zero-padded so the whole field is 8 chars (matches %#08x).
function hex08(v) {
  return '0x' + v.toString(16).padStart(6, '0')
}

function packU(v) {
  const b = Buffer.alloc(4)
  b.writeUInt32LE(v >>> 0)
  return b
}

function packI(v) {
  const b = Buffer.alloc(4)
  b.writeInt32LE(v)
  return b
}

function padRight(buf, len, byte = 0) {
  if (buf.length >= len) return buf
  return Buffer.concat([buf, Buffer.alloc(len - buf.length, byte)])
}

main()
