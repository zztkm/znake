use libc::{
    F_GETFL, F_SETFL, FD_ISSET, FD_SET, O_NONBLOCK, SIGINT, STDIN_FILENO, TCSANOW, cfmakeraw,
    fcntl, tcgetattr, tcsetattr, termios,
};

pub fn clear_screen() {
    // 画面全体を消去 / カーソルを 0, 0 に移動
    write_text(b"\x1b[2J\x1b[H");
}

pub fn move_cursor(col: usize, row: usize) {
    let mut buffer = Vec::new();
    // メモ: `ESC[{row};{column}H`
    buffer.push(b'\x1b');
    buffer.push(b'[');
    buffer.extend_from_slice(row.to_string().as_bytes());
    buffer.push(b';');
    buffer.extend_from_slice(col.to_string().as_bytes());
    buffer.push(b'H');
    write_text(&buffer);
}

// 元のターミナル設定保持変数
static mut ORIGINAL_TERMIOS: Option<termios> = None;

/// 元のターミナル設定を保持したうえで raw mode に入る
pub fn init_terminal() -> Result<(), String> {
    unsafe {
        // 元のターミナル設定を取得して保持変数に持たせる
        let mut original_termios: termios = std::mem::zeroed();
        if tcgetattr(STDIN_FILENO, &mut original_termios) == -1 {
            return Err("tcgetattr failed".to_string());
        }
        ORIGINAL_TERMIOS = Some(original_termios);

        // raw mode 設定
        let mut raw_termios = original_termios;
        cfmakeraw(&mut raw_termios);
        // cfmakeraw でシグナルを生成する ISIG が無効化される
        // このゲームは Ctrl + c で終了したいので、ISIG を有効化する必要がある
        raw_termios.c_lflag |= libc::ISIG;

        // raw_termios をターミナルに反映する
        if tcsetattr(STDIN_FILENO, TCSANOW, &raw_termios) == -1 {
            return Err("tcsetattr failed".to_string());
        }

        let old_handler = libc::signal(SIGINT, signal_handler as libc::sighandler_t);
        if old_handler == libc::SIG_ERR {
            return Err("signal failed".to_string());
        }

        let flags = fcntl(STDIN_FILENO, F_GETFL);
        if flags == -1 {
            return Err("fcntl F_GETFL failed".to_string());
        }
        if fcntl(STDIN_FILENO, F_SETFL, flags | O_NONBLOCK) == -1 {
            return Err("fcntl F_SETFL failed".to_string());
        }
    }

    // TODO: DECRQM で元のカーソル状態を取得して表示 / 非表示を切り替える
    //       ただしこの手法は DECRQM に対応しているターミナルでしか使えない
    // カーソル非表示
    write_text(b"\x1b[?25l");
    Ok(())
}

// TODO: terminal.rs 内で exit するのは良くない
extern "C" fn signal_handler(_sig: libc::c_int) {
    unsafe {
        let _ = restore_terminal();
        libc::exit(0)
    }
}

pub fn read_key_with_timeout(timeout_ms: u64) -> Option<u8> {
    let mut readfds: libc::fd_set = unsafe { std::mem::zeroed() };
    unsafe {
        FD_SET(STDIN_FILENO, &mut readfds);
    };

    let mut timeout = libc::timeval {
        tv_sec: (timeout_ms / 1000) as libc::time_t,
        tv_usec: ((timeout_ms % 1000) * 1000) as libc::suseconds_t,
    };
    let ret = unsafe {
        libc::select(
            STDIN_FILENO + 1,
            &mut readfds,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut timeout,
        )
    };

    if ret > 0 && unsafe { FD_ISSET(STDIN_FILENO, &mut readfds) } {
        let mut buf = [0u8; 1];
        let n = unsafe { libc::read(STDIN_FILENO, buf.as_mut_ptr() as *mut libc::c_void, 1) };
        if n > 0 {
            return Some(buf[0]);
        }
    }
    None
}

fn restore_terminal() -> Result<(), String> {
    unsafe {
        if let Some(original) = ORIGINAL_TERMIOS {
            if tcsetattr(STDIN_FILENO, TCSANOW, &original) == -1 {
                return Err("tcsetattr restore failed".to_string());
            }
        }
    }

    write_text(b"\x1b[?25h");
    Ok(())
}

pub fn write_text(text: &[u8]) {
    unsafe {
        libc::write(
            libc::STDOUT_FILENO,
            text.as_ptr() as *const libc::c_void,
            text.len(),
        );
    }
}
