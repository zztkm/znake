use znake::terminal::{
    clear_screen, init_terminal, move_cursor, read_key_with_timeout, write_text,
};

const GAME_WIDTH: usize = 40;
const GAME_HEIGHT: usize = 20;

enum GameState {
    Game,
    GameOver { score: usize },
}

enum Direction {
    Up,
    Down,
    Left,
    Right,
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

// この関数を loop ごとに呼び出すと重くなって描画した文字が点滅する
// そのため、基本は 1 度読んだら連続して呼ばないほうが良い。
// TODO: Game Over 内の表示を動的なものにしたくなったら改善を考える
fn draw_game_over_screen(score: usize) {
    let game_over_msg = b"GAME OVER!";
    let col = (GAME_WIDTH - game_over_msg.len()) / 2 + 2;
    let row = GAME_HEIGHT / 2 - 3;
    move_cursor(col, row);
    write_text(game_over_msg);

    let score_str = score.to_string();
    let score_msg = format!("Score: {}", score_str);
    move_cursor((GAME_WIDTH - score_msg.len()) / 2 + 2, row + 3);
    write_text(score_msg.as_bytes());

    let retry_msg = b"Press ENTER to retry";
    move_cursor((GAME_WIDTH - retry_msg.len()) / 2 + 2, row + 6);
    write_text(retry_msg);

    let exit_msg = b"Press ctrl+c to exit";
    move_cursor((GAME_WIDTH - exit_msg.len()) / 2 + 2, row + 7);
    write_text(exit_msg);
}

fn game_loop(znake: &mut Znake) {
    let mut food = Food::new();
    let mut state = GameState::Game;
    let mut game_over_shown = false;

    loop {
        if let Some(key) = read_key_with_timeout(50) {
            match state {
                GameState::Game => {
                    // TODO: 矢印キーにも対応する
                    znake.change_direction(key);
                }
                GameState::GameOver { score: _ } => {
                    if key == b'\n' || key == b'\r' {
                        break;
                    }
                }
            }
        }

        match state {
            GameState::Game => {
                znake.move_znake();
                if znake.segments[0] == (food.x, food.y) {
                    znake.grow();
                    food = Food::new();
                }
                if znake.check_collision() {
                    let score = znake.score();
                    state = GameState::GameOver { score };
                }
                clear_screen();
                draw_border();
                znake.draw();
                food.draw();
            }
            GameState::GameOver { score } => {
                if !game_over_shown {
                    draw_game_over_screen(score);
                    game_over_shown = true;
                }
            }
        }
    }
}

fn main_loop() {
    loop {
        // GameOver 状態中に Enter を押した場合に
        // game_loop を初期化してリスタートする
        let mut znake = Znake::new();
        game_loop(&mut znake);
    }
}

fn main() {
    let code = match init_terminal() {
        Ok(()) => {
            main_loop();
            // let _ = restore_terminal();
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    };

    std::process::exit(code);
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

    fn change_direction(&mut self, key: u8) {
        let new_direction = match key {
            b'w' | b'W' => Direction::Up,
            b's' | b'S' => Direction::Down,
            b'a' | b'A' => Direction::Left,
            b'd' | b'D' => Direction::Right,
            _ => return,
        };

        // 逆方向にはいったらあかんで
        let is_opposite = match (&self.direction, &new_direction) {
            (Direction::Up, Direction::Down) => true,
            (Direction::Down, Direction::Up) => true,
            (Direction::Left, Direction::Right) => true,
            (Direction::Right, Direction::Left) => true,
            _ => false,
        };

        if !is_opposite {
            self.direction = new_direction;
        }
    }

    fn check_collision(&self) -> bool {
        let head = self.segments[0];

        // ゲーム画面外判定
        // それぞれ + 1 しておかないと見た目と判定結果にギャップが生じるのでこうしてる
        if head.0 == 1 || head.0 == GAME_WIDTH + 1 || head.1 == 1 || head.1 == GAME_HEIGHT + 1 {
            return true;
        }

        // 胴体との衝突
        for &segment in &self.segments[1..] {
            if segment == head {
                return true;
            }
        }

        false
    }

    fn draw(&self) {
        for (i, &(x, y)) in self.segments.iter().enumerate() {
            let symbol = if i == 0 { b'@' } else { b'o' };
            move_cursor(x, y);
            write_text(&[symbol]);
        }
    }

    fn grow(&mut self) {
        let next = self.next_position();
        self.segments.insert(0, next);
    }

    fn move_znake(&mut self) {
        let next = self.next_position();

        // 頭を追加して、尾を落とす
        self.segments.insert(0, next);
        self.segments.pop();
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

    fn score(&self) -> usize {
        self.segments.len().saturating_sub(3)
    }
}

struct Food {
    pub x: usize,
    pub y: usize,
}

impl Food {
    pub fn new() -> Self {
        let seed = unsafe { libc::time(std::ptr::null_mut()) } as usize;
        let x = (seed % (GAME_WIDTH - 2)) + 2;
        let y = ((seed / GAME_WIDTH) % (GAME_HEIGHT - 2)) + 2;
        Food { x, y }
    }

    pub fn draw(&self) {
        move_cursor(self.x, self.y);
        write_text(b"*");
    }
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
