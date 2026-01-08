use std::mem;

use libc::{
    F_GETFL, F_SETFL, FD_ISSET, FD_SET, O_NONBLOCK, SIGINT, STDIN_FILENO, STDOUT_FILENO, TCSANOW,
    cfmakeraw, fcntl, tcgetattr, tcsetattr, termios,
};

const GAME_WIDTH: usize = 40;
const GAME_HEIGHT: usize = 20;

enum Direction {
    Up,
    Down,
    Left,
    Right,
}

struct Znake {
    segments: Vec<(usize, usize)>,
    direction: Direction,
}

impl Znake {
    fn new() -> Self {
        let head = (GAME_WIDTH / 2, GAME_HEIGHT / 2);
        Znake {
            // `□□頭` な初期配置
            segments: vec![head, (head.0 - 1, head.1), (head.0 - 2, head.1)],
            direction: Direction::Right,
        }
    }

    fn draw(&self) {
        for (i, &(x, y)) in self.segments.iter().enumerate() {
            let symbol = if i == 0 { b'@' } else { b'o' };
            move_cursor(x, y);
            write_text(&[symbol]);
        }
    }

    fn next_position(&self) -> (usize, usize) {
        let (x, y) = self.segments[0];

        // TODO: 範囲チェックどこでやる?
        match self.direction {
            Direction::Up => (x, y - 1),
            Direction::Down => (x, y + 1),
            Direction::Left => (x - 1, y),
            Direction::Right => (x + 1, y),
        }
    }

    fn move_znake(&mut self) {
        let next = self.next_position();

        // 頭を追加して、尾を落とす
        self.segments.insert(0, next);
        self.segments.pop();
    }
}

// 元のターミナル設定保持変数
static mut ORIGINAL_TERMIOS: Option<termios> = None;

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

fn init_terminal() -> Result<(), String> {
    unsafe {
        // 元のターミナル設定を取得して保持変数に持たせる
        let mut original_termios: termios = mem::zeroed();
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

extern "C" fn signal_handler(_sig: libc::c_int) {
    unsafe {
        let _ = restore_terminal();
        libc::exit(0);
    }
}

fn clear_screen() {
    // 画面全体を消去 / カーソルを 0, 0 に移動
    write_text(b"\x1b[2J\x1b[H");
}

fn move_cursor(col: usize, row: usize) {
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

fn draw_border() {
    let mut buffer = Vec::new();

    buffer.push(b'+');
    for _ in 0..GAME_WIDTH {
        buffer.push(b'-');
    }
    buffer.push(b'+');
    buffer.push(b'\r');
    buffer.push(b'\n');

    for _ in 0..GAME_HEIGHT {
        buffer.push(b'|');
        for _ in 0..GAME_WIDTH {
            buffer.push(b' ');
        }
        buffer.push(b'|');
        buffer.push(b'\r');
        buffer.push(b'\n');
    }

    buffer.push(b'+');
    for _ in 0..GAME_WIDTH {
        buffer.push(b'-');
    }
    buffer.push(b'+');
    buffer.push(b'\r');
    buffer.push(b'\n');

    write_text(&buffer);
}

fn write_text(text: &[u8]) {
    unsafe {
        libc::write(
            STDOUT_FILENO,
            text.as_ptr() as *const libc::c_void,
            text.len(),
        );
    }
}

fn game_loop() {
    let mut znake = Znake::new();

    loop {
        let mut readfds: libc::fd_set = unsafe { mem::zeroed() };
        unsafe {
            FD_SET(STDIN_FILENO, &mut readfds);
        };

        // 50ms
        let mut timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: 50000,
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
                let _key = buf[0];
            }
        };

        znake.move_znake();
        clear_screen();
        draw_border();
        znake.draw();
    }
}

fn main() {
    let code = match init_terminal() {
        Ok(()) => {
            game_loop();
            let _ = restore_terminal();
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    };

    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_escape_sequence_len() {
        // 何バイトなのか気になっただけ
        // よくよく考えたら LSP でバイト数表示されてたのでこのテスト意味ない...
        let seq = b"\x1b[2J\x1b[H";
        assert_eq!(seq.len(), 7)
    }
}
