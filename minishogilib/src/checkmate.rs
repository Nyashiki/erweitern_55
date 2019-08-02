use pyo3::prelude::*;

use position::*;
use r#move::*;

#[pymethods]
impl Position {
    pub fn solve_checkmate_dfs(&mut self, depth: i32) -> (bool, Move) {
        return attack(self, depth);
    }
}

/// 詰みがある場合は詰み手順を返す
fn attack(position: &mut Position, depth: i32) -> (bool, Move) {
    if depth <= 0 {
        return (false, NULL_MOVE);
    }

    let moves = position.generate_moves(); // ToDo: 王手生成ルーチン

    for m in &moves {
        position.do_move(m);

        if position.get_check() == 0 {
            position.undo_move();
            continue;
        }

        let (checkmate, _) = defense(position, depth - 1);

        position.undo_move();

        if checkmate {
            return (true, *m);
        }
    }

    return (false, NULL_MOVE);
}

fn defense(position: &mut Position, depth: i32) -> (bool, Move) {
    let moves = position.generate_moves(); // ToDo: 王手生成ルーチン

    for m in &moves {
        position.do_move(m);

        let (checkmate, _) = attack(position, depth - 1);

        position.undo_move();

        if !checkmate {
            return (false, NULL_MOVE);
        }
    }

    return (true, NULL_MOVE); // ToDo: take the longest path
}

#[test]
fn checkmate_test() {
    let mut position = Position::empty_board();

    {
        position.set_sfen("2k2/5/2P2/5/2K2 b G 1");

        let start = std::time::Instant::now();
        let (checkmate, checkmate_move) = position.solve_checkmate_dfs(7);
        let elapsed = start.elapsed();

        assert_eq!(checkmate, true);
        println!("{} ... {}.{} sec.", checkmate_move.sfen(), elapsed.as_secs(), elapsed.subsec_nanos() / 1000000);
    }

    {
        position.set_sfen("5/5/2k2/5/2K2 b 3G 1");
        position.print();

        let start = std::time::Instant::now();
        let (checkmate, checkmate_move) = position.solve_checkmate_dfs(7);
        let elapsed = start.elapsed();

        assert_eq!(checkmate, true);
        println!("{} ... {}.{} sec.", checkmate_move.sfen(), elapsed.as_secs(), elapsed.subsec_nanos() / 1000000);
    }


    {
        position.set_sfen("5/5/2k2/5/2K2 b 2G 1");
        position.print();

        let start = std::time::Instant::now();
        let (checkmate, checkmate_move) = position.solve_checkmate_dfs(7);
        let elapsed = start.elapsed();

        assert_eq!(checkmate, false);
        println!("{} ... {}.{} sec.", checkmate_move.sfen(), elapsed.as_secs(), elapsed.subsec_nanos() / 1000000);
    }

    {
        position.set_sfen("2k2/5/2B2/5/2K2 b GSBRgsr2p 1");
        position.print();

        let start = std::time::Instant::now();
        let (checkmate, checkmate_move) = position.solve_checkmate_dfs(7);
        let elapsed = start.elapsed();

        assert_eq!(checkmate, true);
        println!("{} ... {}.{} sec.", checkmate_move.sfen(), elapsed.as_secs(), elapsed.subsec_nanos() / 1000000);
    }
}
