#[cfg(test)]
use rand::seq::SliceRandom;

use pyo3::prelude::*;

use bitboard::*;
use r#move::*;
use types::*;

#[pyclass]
#[derive(Copy, Clone)]
pub struct Position {
    pub side_to_move: Color,
    pub board: [Piece; SQUARE_NB],
    pub hand: [[u8; 5]; 2],
    pub pawn_flags: [u8; 2],

    pub piece_bb: [Bitboard; Piece::BPawnX as usize + 1],
    pub player_bb: [Bitboard; 2],

    pub ply: u16,
    pub kif: [Move; MAX_PLY], // ToDo: 連続で現在の手番が何回王手しているかを持つ

    pub hash: u64,
}

#[pymethods]
impl Position {
    #[new]
    pub fn new(obj: &PyRawObject) {
        obj.init(Position::empty_board());
    }

    pub fn print(self) {
        println!("side_to_move: {:?}", self.side_to_move);

        for y in 0..5 {
            for x in 0..5 {
                print!("{}", self.board[y * 5 + x]);
            }
            println!("");
        }

        let hand_str = ["G", "S", "B", "R", "P"];

        print!("WHITE HAND: ");
        for i in 0..5 {
            print!(
                "{}: {}, ",
                hand_str[i],
                self.hand[(Color::White as usize)][i]
            );
        }
        println!("");

        print!("BLACK HAND: ");
        for i in 0..5 {
            print!(
                "{}: {}, ",
                hand_str[i],
                self.hand[(Color::Black as usize)][i]
            );
        }
        println!("");

        println!("ply: {}", self.ply);

        println!("hash: {:x}", self.calculate_hash());
    }

    pub fn set_sfen(&mut self, sfen: &str) {
        // 初期化
        for i in 0..SQUARE_NB {
            self.board[i] = Piece::NoPiece;
        }
        for i in 0..2 {
            for j in 0..5 {
                self.hand[i][j] = 0;
            }

            self.pawn_flags[i] = 0;
        }

        let mut square: usize = 0;
        let mut promote: bool = false;

        let mut sfen_split = sfen.split_whitespace();

        // sfenから盤面を設定
        for c in sfen_split.next().unwrap().chars() {
            if c == '+' {
                promote = true;
                continue;
            }

            if c == '/' {
                continue;
            }

            if c.is_ascii_digit() {
                square += ((c as u8) - ('0' as u8)) as usize;
                continue;
            }

            let mut piece = char_to_piece(c);

            if promote {
                piece = piece.get_promoted();
            }

            self.board[square] = piece;

            if piece == Piece::WPawn {
                self.pawn_flags[Color::White as usize] |= 1 << (square % 5);
            } else if piece == Piece::BPawn {
                self.pawn_flags[Color::Black as usize] |= 1 << (square % 5);
            }

            promote = false;
            square += 1;
        }

        // 手番を設定
        if sfen_split.next() == Some("b") {
            self.side_to_move = Color::White;
        } else {
            self.side_to_move = Color::Black;
        }

        // 持ち駒を設定
        let mut count: u8 = 1;
        for c in sfen_split.next().unwrap().chars() {
            if c == '-' {
                continue;
            }

            if c.is_ascii_digit() {
                count = (c as u8) - ('0' as u8);
                continue;
            }

            let piece = char_to_piece(c);
            let color = piece.get_color();
            let piece_type = piece.get_piece_type();
            let hand_index = (piece_type as usize) - 2;

            self.hand[color as usize][hand_index] = count;

            count = 1;
        }

        self.set_bitboard();
        self.hash = self.calculate_hash();

        self.ply = 0;

        // ToDo: movesに沿った局面進行
    }

    pub fn set_start_position(&mut self) {
        static START_POSITION_SFEN: &str = "rbsgk/4p/5/P4/KGSBR b - 1";

        self.set_sfen(START_POSITION_SFEN);
    }

    pub fn generate_moves(self) -> std::vec::Vec<Move> {
        return self.generate_moves_with_option(true, true, false);
    }

    pub fn do_move(&mut self, m: &Move) {
        assert!(m.capture_piece.get_piece_type() != PieceType::King);

        if m.amount == 0 {
            // 持ち駒を打つ場合

            self.board[m.to as usize] = m.piece;
            self.hand[self.side_to_move as usize][m.piece.get_piece_type() as usize - 2] -= 1;

            // Bitboardの更新
            self.piece_bb[m.piece as usize] |= 1 << m.to;
            self.player_bb[self.side_to_move as usize] |= 1 << m.to;

            // 二歩フラグの更新
            if m.piece.get_piece_type() == PieceType::Pawn {
                self.pawn_flags[self.side_to_move as usize] |= 1 << (m.to % 5);
            }

            // hash値の更新
            self.hash ^= ::zobrist::BOARD_TABLE[m.to][m.piece as usize];
        } else {
            // 盤上の駒を動かす場合

            if m.capture_piece != Piece::NoPiece {
                self.hand[self.side_to_move as usize]
                    [m.capture_piece.get_piece_type().get_raw() as usize - 2] += 1;

                // Bitboardの更新
                self.piece_bb[m.capture_piece as usize] ^= 1 << m.to;
                self.player_bb[self.side_to_move.get_op_color() as usize] ^= 1 << m.to;

                // 二歩フラグの更新
                if m.capture_piece.get_piece_type() == PieceType::Pawn {
                    self.pawn_flags[self.side_to_move.get_op_color() as usize] ^= 1 << (m.to % 5);
                }

                // hashの更新
                self.hash ^= ::zobrist::BOARD_TABLE[m.to][m.capture_piece as usize];
            }

            if m.promotion {
                self.board[m.to as usize] = m.piece.get_promoted();
                // 二歩フラグの更新
                if m.piece.get_piece_type() == PieceType::Pawn {
                    self.pawn_flags[self.side_to_move as usize] ^= 1 << (m.to % 5);
                }
            } else {
                self.board[m.to as usize] = m.piece;
            }

            self.board[m.from as usize] = Piece::NoPiece;

            // Bitboardの更新
            // 移動先
            self.piece_bb[self.board[m.to as usize] as usize] |= 1 << m.to;
            self.player_bb[self.side_to_move as usize] |= 1 << m.to;
            // 移動元
            self.piece_bb[m.piece as usize] ^= 1 << m.from;
            self.player_bb[self.side_to_move as usize] ^= 1 << m.from;

            // hash値の更新
            self.hash ^= ::zobrist::BOARD_TABLE[m.from][m.piece as usize];
            self.hash ^= ::zobrist::BOARD_TABLE[m.to][self.board[m.to] as usize];
        }

        // 棋譜に登録
        self.kif[self.ply as usize] = *m;

        // 1手進める
        self.ply += 1;

        // 手番を変える
        self.side_to_move = self.side_to_move.get_op_color();
    }

    pub fn undo_move(&mut self) {
        assert!(self.ply > 0);

        // 手数を戻す
        let m = self.kif[(self.ply - 1) as usize];
        self.ply -= 1;

        // 手番を戻す
        self.side_to_move = self.side_to_move.get_op_color();

        if m.amount == 0 {
            // 持ち駒を打った場合

            self.board[m.to as usize] = Piece::NoPiece;
            self.hand[self.side_to_move as usize][m.piece.get_piece_type() as usize - 2] += 1;

            // Bitboardのundo
            self.piece_bb[m.piece as usize] ^= 1 << m.to;
            self.player_bb[self.side_to_move as usize] ^= 1 << m.to;

            // 二歩フラグのundo
            if m.piece.get_piece_type() == PieceType::Pawn {
                self.pawn_flags[self.side_to_move as usize] ^= 1 << (m.to % 5);
            }

            // hash値のundo
            self.hash ^= ::zobrist::BOARD_TABLE[m.to][m.piece as usize];
        } else {
            // 盤上の駒を動かした場合

            // Bitboardのundo
            // 移動先
            assert!(self.board[m.to as usize] != Piece::NoPiece);
            self.piece_bb[self.board[m.to as usize] as usize] ^= 1 << m.to;
            self.player_bb[self.side_to_move as usize] ^= 1 << m.to;
            // 移動元
            self.piece_bb[m.piece as usize] |= 1 << m.from;
            self.player_bb[self.side_to_move as usize] |= 1 << m.from;

            // hash値のundo
            self.hash ^= ::zobrist::BOARD_TABLE[m.to][self.board[m.to] as usize];
            self.hash ^= ::zobrist::BOARD_TABLE[m.from][m.piece as usize];

            self.board[m.to as usize] = m.capture_piece;
            self.board[m.from as usize] = m.piece;

            // 二歩フラグのundo
            if m.piece.get_piece_type() == PieceType::Pawn && m.promotion {
                self.pawn_flags[self.side_to_move as usize] |= 1 << (m.to % 5);
            }

            // 相手の駒を取っていた場合には、持ち駒から減らす
            if m.capture_piece != Piece::NoPiece {
                self.hand[self.side_to_move as usize]
                    [m.capture_piece.get_piece_type().get_raw() as usize - 2] -= 1;

                // Bitboardのundo
                self.piece_bb[m.capture_piece as usize] |= 1 << m.to;
                self.player_bb[self.side_to_move.get_op_color() as usize] |= 1 << m.to;

                // 二歩フラグのundo
                if m.capture_piece.get_piece_type() == PieceType::Pawn {
                    self.pawn_flags[self.side_to_move.get_op_color() as usize] |= 1 << (m.to % 5);
                }

                // hashのundo
                self.hash ^= ::zobrist::BOARD_TABLE[m.to][m.capture_piece as usize];
            }
        }
    }
}

impl Position {
    pub fn empty_board() -> Position {
        Position {
            side_to_move: Color::NoColor,
            board: [Piece::NoPiece; SQUARE_NB],
            hand: [[0; 5]; 2],
            pawn_flags: [0; 2],
            piece_bb: [0; Piece::BPawnX as usize + 1],
            player_bb: [0; 2],
            ply: 0,
            kif: [NULL_MOVE; MAX_PLY],
            hash: 0,
        }
    }

    /// 盤上の駒からbitboardを設定する
    fn set_bitboard(&mut self) {
        // 初期化
        for i in 0..Piece::BPawnX as usize + 1 {
            self.piece_bb[i] = 0
        }
        self.player_bb[Color::White as usize] = 0;
        self.player_bb[Color::Black as usize] = 0;

        // 盤上の駒に対応する場所のbitを立てる
        for i in 0..SQUARE_NB {
            if self.board[i] != Piece::NoPiece {
                self.piece_bb[self.board[i] as usize] |= 1 << i;
                self.player_bb[self.board[i].get_color() as usize] |= 1 << i;
            }
        }
    }

    fn calculate_hash(self) -> u64 {
        let mut hash: u64 = 0;

        for i in 0..SQUARE_NB {
            if self.board[i] != Piece::NoPiece {
                hash ^= ::zobrist::BOARD_TABLE[i][self.board[i] as usize];
            }
        }

        return hash;
    }

    pub fn generate_moves_with_option(
        self,
        is_board: bool,
        is_hand: bool,
        allow_illegal: bool,
    ) -> std::vec::Vec<Move> {
        // 近接駒による王手をされているか
        let mut adjacent_check_bb: Bitboard = 0;
        let mut adjacent_check_count: u8 = 0;

        let king_square =
            get_square(self.piece_bb[PieceType::King.get_piece(self.side_to_move) as usize]);

        if !allow_illegal {
            assert!(king_square < SQUARE_NB);

            for piece_type in PIECE_TYPE_ALL.iter() {
                let check_bb =
                    adjacent_attack(king_square, piece_type.get_piece(self.side_to_move))
                        & self.piece_bb
                            [piece_type.get_piece(self.side_to_move.get_op_color()) as usize];

                if check_bb != 0 {
                    adjacent_check_count += 1;
                    adjacent_check_bb |= check_bb;
                }
            }
        }

        let mut moves: Vec<Move> = Vec::new();

        if is_board {
            for i in 0..SQUARE_NB {
                if self.board[i].get_color() != self.side_to_move {
                    continue;
                }

                const MOVE_TOS: [i8; 8] = [-5, -4, 1, 6, 5, 4, -1, -6];

                // 飛び駒以外の駒の移動
                for move_dir in self.board[i].get_move_dirs() {
                    // これ以上左に行けない
                    if i % 5 == 0
                        && (move_dir == Direction::SW
                            || move_dir == Direction::W
                            || move_dir == Direction::NW)
                    {
                        continue;
                    }

                    // これ以上上に行けない
                    if i / 5 == 0
                        && (move_dir == Direction::N
                            || move_dir == Direction::NE
                            || move_dir == Direction::NW)
                    {
                        continue;
                    }

                    // これ以上右に行けない
                    if i % 5 == 4
                        && (move_dir == Direction::NE
                            || move_dir == Direction::E
                            || move_dir == Direction::SE)
                    {
                        continue;
                    }

                    // これ以上下に行けない
                    if i / 5 == 4
                        && (move_dir == Direction::SE
                            || move_dir == Direction::S
                            || move_dir == Direction::SW)
                    {
                        continue;
                    }

                    let move_to = ((i as i8) + MOVE_TOS[move_dir as usize]) as usize;

                    let capture_piece = self.board[move_to];

                    // 行き先に自分の駒がある場合には動かせない
                    if capture_piece.get_color() == self.side_to_move {
                        continue;
                    }

                    // 行き場のない歩の不成を禁止
                    if !((self.board[i] == Piece::WPawn && move_to < 5)
                        || (self.board[i] == Piece::BPawn && move_to >= 20))
                    {
                        moves.push(Move::board_move(
                            self.board[i],
                            i,
                            move_dir,
                            1,
                            move_to,
                            false,
                            capture_piece,
                        ));
                    }

                    // 成る手の生成
                    if self.board[i].is_raw()
                        && self.board[i].is_promotable()
                        && ((self.side_to_move == Color::White && (move_to < 5 || i < 5))
                            || (self.side_to_move == Color::Black && (move_to >= 20 || i >= 20)))
                    {
                        moves.push(Move::board_move(
                            self.board[i],
                            i,
                            move_dir,
                            1,
                            move_to,
                            true,
                            capture_piece,
                        ));
                    }
                }

                // 飛び駒の移動
                // 角、馬
                if self.board[i].get_piece_type() == PieceType::Bishop
                    || self.board[i].get_piece_type() == PieceType::BishopX
                {
                    const MOVE_DIRS: [Direction; 4] =
                        [Direction::NE, Direction::SE, Direction::SW, Direction::NW];

                    for move_dir in &MOVE_DIRS {
                        // これ以上左に行けない
                        if i % 5 == 0 && (*move_dir == Direction::SW || *move_dir == Direction::NW)
                        {
                            continue;
                        }

                        // これ以上上に行けない
                        if i / 5 == 0 && (*move_dir == Direction::NE || *move_dir == Direction::NW)
                        {
                            continue;
                        }

                        // これ以上右に行けない
                        if i % 5 == 4 && (*move_dir == Direction::NE || *move_dir == Direction::SE)
                        {
                            continue;
                        }

                        // これ以上下に行けない
                        if i / 5 == 4 && (*move_dir == Direction::SE || *move_dir == Direction::SW)
                        {
                            continue;
                        }

                        for amount in 1..5 {
                            let move_to = ((i as i8)
                                + MOVE_TOS[*move_dir as usize] * (amount as i8))
                                as usize;

                            let capture_piece = self.board[move_to];

                            // 自分の駒があったらそれ以上進めない
                            if capture_piece.get_color() == self.side_to_move {
                                break;
                            }

                            moves.push(Move::board_move(
                                self.board[i],
                                i,
                                *move_dir,
                                amount,
                                move_to,
                                false,
                                capture_piece,
                            ));
                            // 成る手の生成
                            if (self.board[i] == Piece::WBishop && (move_to < 5 || i < 5))
                                || (self.board[i] == Piece::BBishop && (move_to >= 20 || i >= 20))
                            {
                                moves.push(Move::board_move(
                                    self.board[i],
                                    i,
                                    *move_dir,
                                    amount,
                                    move_to,
                                    true,
                                    capture_piece,
                                ));
                            }

                            // 端まで到達したらそれ以上進めない
                            if move_to / 5 == 0
                                || move_to / 5 == 4
                                || move_to % 5 == 0
                                || move_to % 5 == 4
                            {
                                break;
                            }

                            // 相手の駒があったらそれ以上進めない
                            if capture_piece.get_color() == self.side_to_move.get_op_color() {
                                break;
                            }
                        }
                    }
                }

                // 飛、龍
                if self.board[i].get_piece_type() == PieceType::Rook
                    || self.board[i].get_piece_type() == PieceType::RookX
                {
                    const MOVE_DIRS: [Direction; 4] =
                        [Direction::N, Direction::E, Direction::S, Direction::W];

                    for move_dir in &MOVE_DIRS {
                        // これ以上左に行けない
                        if i % 5 == 0 && *move_dir == Direction::W {
                            continue;
                        }

                        // これ以上上に行けない
                        if i / 5 == 0 && *move_dir == Direction::N {
                            continue;
                        }

                        // これ以上右に行けない
                        if i % 5 == 4 && *move_dir == Direction::E {
                            continue;
                        }

                        // これ以上下に行けない
                        if i / 5 == 4 && *move_dir == Direction::S {
                            continue;
                        }

                        for amount in 1..5 {
                            let move_to = ((i as i8)
                                + MOVE_TOS[*move_dir as usize] * (amount as i8))
                                as usize;

                            let capture_piece = self.board[move_to as usize];

                            // 自分の駒があったらそれ以上進めない
                            if capture_piece.get_color() == self.side_to_move {
                                break;
                            }

                            moves.push(Move::board_move(
                                self.board[i],
                                i,
                                *move_dir,
                                amount,
                                move_to,
                                false,
                                capture_piece,
                            ));
                            // 成る手の生成
                            if (self.board[i] == Piece::WRook && (move_to < 5 || i < 5))
                                || (self.board[i] == Piece::BRook && (move_to >= 20 || i >= 20))
                            {
                                moves.push(Move::board_move(
                                    self.board[i],
                                    i,
                                    *move_dir,
                                    amount,
                                    move_to,
                                    true,
                                    capture_piece,
                                ));
                            }

                            // 端まで到達したらそれ以上進めない
                            if (*move_dir == Direction::N && move_to / 5 == 0)
                                || (*move_dir == Direction::E && move_to % 5 == 4)
                                || (*move_dir == Direction::S && move_to / 5 == 4)
                                || (*move_dir == Direction::W && move_to % 5 == 0)
                            {
                                break;
                            }

                            // 相手の駒があったらそれ以上進めない
                            if self.board[move_to as usize].get_color()
                                == self.side_to_move.get_op_color()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 近接駒に王手されている場合、持ち駒を打つ手は全て非合法手
        if is_hand && adjacent_check_count == 0 {
            // 駒のない升を列挙
            let mut empty_squares: Vec<usize> = Vec::new();
            for i in 0..SQUARE_NB {
                if self.board[i] == Piece::NoPiece {
                    empty_squares.push(i);
                }
            }

            for piece_type in HAND_PIECE_TYPE_ALL.iter() {
                if self.hand[self.side_to_move as usize][*piece_type as usize - 2] > 0 {
                    for target in &empty_squares {
                        // 二歩は禁じ手
                        if *piece_type == PieceType::Pawn
                            && self.pawn_flags[self.side_to_move as usize] & (1 << (target % 5))
                                != 0
                        {
                            continue;
                        }

                        // 行き場のない駒を打たない
                        if *piece_type == PieceType::Pawn
                            && ((self.side_to_move == Color::White && *target < 5)
                                || (self.side_to_move == Color::Black && *target >= 20))
                        {
                            continue;
                        }

                        moves.push(Move::hand_move(
                            piece_type.get_piece(self.side_to_move),
                            *target,
                        ));
                    }
                }
            }
        }

        // 非合法手を取り除く
        if !allow_illegal {
            let mut index: usize = 0;

            loop {
                if index == moves.len() {
                    break;
                }

                let is_legal = |m: Move| -> bool {
                    if m.amount == 0 {
                        // 持ち駒を打つ場合
                        let player_bb: Bitboard = self.player_bb[Color::White as usize]
                            | self.player_bb[Color::Black as usize]
                            | (1 << m.to);

                        // 角による王手
                        let bishop_check_bb = bishop_attack(king_square, player_bb);
                        if bishop_check_bb
                            & self.piece_bb[PieceType::Bishop
                                .get_piece(self.side_to_move.get_op_color())
                                as usize]
                            != 0
                            || bishop_check_bb
                                & self.piece_bb[PieceType::BishopX
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                        {
                            return false;
                        }

                        // 飛車による王手
                        let rook_check_bb = rook_attack(king_square, player_bb);
                        if rook_check_bb
                            & self.piece_bb[PieceType::Rook
                                .get_piece(self.side_to_move.get_op_color())
                                as usize]
                            != 0
                            || rook_check_bb
                                & self.piece_bb[PieceType::RookX
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                        {
                            return false;
                        }
                    } else {
                        // 盤上の駒を動かす場合
                        if m.piece.get_piece_type() == PieceType::King {
                            // 王を動かす場合
                            let player_bb: Bitboard = (self.player_bb[Color::White as usize]
                                | self.player_bb[Color::Black as usize]
                                | (1 << m.to))
                                ^ (1 << m.from);

                            // 角による王手
                            let bishop_check_bb = bishop_attack(m.to as usize, player_bb);

                            if bishop_check_bb
                                & self.piece_bb[PieceType::Bishop
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                                || bishop_check_bb
                                    & self.piece_bb[PieceType::BishopX
                                        .get_piece(self.side_to_move.get_op_color())
                                        as usize]
                                    != 0
                            {
                                return false;
                            }

                            // 飛車による王手
                            let rook_check_bb = rook_attack(m.to as usize, player_bb);

                            if rook_check_bb
                                & self.piece_bb[PieceType::Rook
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                                || rook_check_bb
                                    & self.piece_bb[PieceType::RookX
                                        .get_piece(self.side_to_move.get_op_color())
                                        as usize]
                                    != 0
                            {
                                return false;
                            }

                            // 近接王手
                            for piece_type in PIECE_TYPE_ALL.iter() {
                                let check_bb = adjacent_attack(
                                    m.to as usize,
                                    piece_type.get_piece(self.side_to_move),
                                ) & self.piece_bb[piece_type
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize];

                                if check_bb != 0 {
                                    return false;
                                }
                            }
                        } else {
                            // 王以外を動かす場合
                            if adjacent_check_count > 1 {
                                // 近接駒に両王手されている場合は玉を動かさないといけない
                                return false;
                            } else if adjacent_check_count == 1 {
                                // 王手している近接駒を取る手でないといけない
                                if adjacent_check_bb & (1 << m.to) == 0 {
                                    return false;
                                }
                            }

                            let player_bb: Bitboard = (self.player_bb[Color::White as usize]
                                | self.player_bb[Color::Black as usize]
                                | (1 << m.to))
                                ^ (1 << m.from);

                            // 角による王手
                            let bishop_check_bb =
                                bishop_attack(king_square, player_bb) & !(1 << m.to);
                            if bishop_check_bb
                                & self.piece_bb[PieceType::Bishop
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                                || bishop_check_bb
                                    & self.piece_bb[PieceType::BishopX
                                        .get_piece(self.side_to_move.get_op_color())
                                        as usize]
                                    != 0
                            {
                                return false;
                            }

                            // 飛車による王手
                            let rook_check_bb = rook_attack(king_square, player_bb) & !(1 << m.to);

                            if rook_check_bb
                                & self.piece_bb[PieceType::Rook
                                    .get_piece(self.side_to_move.get_op_color())
                                    as usize]
                                != 0
                                || rook_check_bb
                                    & self.piece_bb[PieceType::RookX
                                        .get_piece(self.side_to_move.get_op_color())
                                        as usize]
                                    != 0
                            {
                                return false;
                            }
                        }
                    }

                    return true;
                }(moves[index]);

                if !is_legal {
                    moves.swap_remove(index);

                    continue;
                }

                index += 1;
            }
        }

        return moves;
    }
}

fn char_to_piece(c: char) -> Piece {
    match c {
        'K' => Piece::WKing,
        'G' => Piece::WGold,
        'S' => Piece::WSilver,
        'B' => Piece::WBishop,
        'R' => Piece::WRook,
        'P' => Piece::WPawn,

        'k' => Piece::BKing,
        'g' => Piece::BGold,
        's' => Piece::BSilver,
        'b' => Piece::BBishop,
        'r' => Piece::BRook,
        'p' => Piece::BPawn,

        _ => Piece::NoPiece,
    }
}

#[test]
fn pawn_flags_test() {
    const LOOP_NUM: i32 = 100000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            let mut pawn_flag: [[bool; 5]; 2] = [[false; 5]; 2];

            // 二歩フラグの差分更新が正しく動作していることを確認する
            for i in 0..SQUARE_NB {
                if position.board[i] == Piece::WPawn {
                    pawn_flag[Color::White as usize][(i % 5) as usize] = true;
                } else if position.board[i] == Piece::BPawn {
                    pawn_flag[Color::Black as usize][(i % 5) as usize] = true;
                }
            }
            for i in 0..5 {
                assert_eq!(
                    pawn_flag[Color::White as usize][i],
                    (position.pawn_flags[Color::White as usize] & (1 << i)) != 0
                );
                assert_eq!(
                    pawn_flag[Color::Black as usize][i],
                    (position.pawn_flags[Color::Black as usize] & (1 << i)) != 0
                );
            }

            let moves = position.generate_moves();
            if moves.len() == 0 {
                break;
            }

            // ランダムに局面を進める
            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}

#[test]
fn move_do_undo_test() {
    const LOOP_NUM: i32 = 10000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            let moves = position.generate_moves();

            for m in &moves {
                let mut temp_position = position;

                if m.capture_piece.get_piece_type() == PieceType::King {
                    continue;
                }

                temp_position.do_move(m);
                temp_position.undo_move();

                // do_move -> undo_moveで元の局面と一致するはず
                assert_eq!(position.side_to_move, temp_position.side_to_move);
                for i in 0..SQUARE_NB {
                    assert_eq!(position.board[i], temp_position.board[i]);
                }
                for i in 0..2 {
                    for j in 0..5 {
                        assert_eq!(position.hand[i][j], temp_position.hand[i][j]);
                    }
                }

                for i in 0..Piece::BPawnX as usize + 1 {
                    assert_eq!(position.piece_bb[i], temp_position.piece_bb[i]);
                }
                for i in 0..2 {
                    assert_eq!(position.player_bb[i], temp_position.player_bb[i]);
                }

                for i in 0..2 {
                    assert_eq!(position.pawn_flags[i], temp_position.pawn_flags[i]);
                }

                assert_eq!(position.ply, temp_position.ply);

                for i in 0..position.ply as usize {
                    assert!(position.kif[i] == temp_position.kif[i]);
                }

                assert_eq!(position.hash, temp_position.hash);
            }

            if moves.len() == 0 {
                break;
            }

            // ランダムに局面を進める
            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}

#[test]
fn bitboard_test() {
    const LOOP_NUM: i32 = 100000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            for i in 0..SQUARE_NB {
                if position.board[i] == Piece::NoPiece {
                    continue;
                }

                assert!(position.piece_bb[position.board[i] as usize] & (1 << i) != 0);
            }

            let moves = position.generate_moves();
            if moves.len() == 0 {
                break;
            }

            // ランダムに局面を進める
            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}

#[test]
fn no_legal_move_test() {
    ::bitboard::init();

    static CHECKMATE_SFEN1: &str = "5/5/2p2/2g2/2K2 b P 1";
    static CHECKMATE_SFEN2: &str = "4k/1s1gp/p4/g1BS1/1KR2 b BRg 1";
    static CHECKMATE_SFEN3: &str = "4k/2G2/5/5/4R w - 1";
    static CHECKMATE_SFEN4: &str = "r4/5/5/2g2/K4 b - 1";
    static CHECKMATE_SFEN5: &str = "2G1k/5/4P/5/B4 w - 1";
    static CHECKMATE_SFEN6: &str = "4b/5/p4/5/K1g2 b - 1";
    static CHECKMATE_SFEN7: &str = "k1G2/5/P4/5/4B w - 1";
    static CHECKMATE_SFEN8: &str = "b4/5/4p/5/2g1K b - 1";
    static CHECKMATE_SFEN9: &str = "R4/2G1k/5/4P/1B3 w - 1";
    static CHECKMATE_SFEN10: &str = "r4/2g1K/5/4g/1b3 b - 1";

    let mut position = Position::empty_board();

    position.set_sfen(CHECKMATE_SFEN1);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN2);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN3);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN4);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN5);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN6);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN7);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN8);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN9);
    assert_eq!(position.generate_moves().len(), 0);

    position.set_sfen(CHECKMATE_SFEN10);
    assert_eq!(position.generate_moves().len(), 0);
}

#[test]
fn not_checkmate_positions() {
    ::bitboard::init();

    static NOT_CHECKMATE_SFEN1: &str = "rb1gk/1s2R/5/P1B2/KGS2 w P 1";

    let mut position = Position::empty_board();

    position.set_sfen(NOT_CHECKMATE_SFEN1);
    assert!(position.generate_moves().len() > 0);
}

#[test]
fn no_king_capture_move_in_legal_moves_test() {
    const LOOP_NUM: i32 = 100000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            let moves = position.generate_moves();

            for m in &moves {
                // 玉が取られる手は生成しないはず
                // -> 玉が取れる局面に遭遇しないはず
                assert!(m.capture_piece.get_piece_type() != PieceType::King);
            }

            // ランダムに局面を進める
            if moves.len() == 0 {
                break;
            }
            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}

#[test]
fn generate_moves_test() {
    const LOOP_NUM: i32 = 10000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            let moves = position.generate_moves();
            let allow_illegal_moves = position.generate_moves_with_option(true, true, true);

            let mut legal_move_count = allow_illegal_moves.len();
            for m in allow_illegal_moves {
                position.do_move(&m);

                let all_moves = position.generate_moves_with_option(true, true, true);

                for m2 in all_moves {
                    if m2.capture_piece.get_piece_type() == PieceType::King {
                        legal_move_count -= 1;
                        break;
                    }
                }

                position.undo_move();
            }

            assert_eq!(moves.len(), legal_move_count);

            // ランダムに局面を進める
            if moves.len() == 0 {
                break;
            }
            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}

#[test]
fn hash_test() {
    const LOOP_NUM: i32 = 100000;

    let mut position = Position::empty_board();

    let mut rng = rand::thread_rng();

    for _ in 0..LOOP_NUM {
        position.set_start_position();

        while position.ply < MAX_PLY as u16 {
            let moves = position.generate_moves();

            if moves.len() == 0 {
                break;
            }

            // 差分計算と全計算の値が一致することを確認する
            assert_eq!(position.hash, position.calculate_hash());

            let random_move = moves.choose(&mut rng).unwrap();
            position.do_move(random_move);
        }
    }
}
