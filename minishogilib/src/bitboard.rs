use bitintr::Pext;

use types::*;
use position::*;

pub type Bitboard = u32;

lazy_static! {
    /// 近接の利きを保持するbitboard
    /// ADJACENT_ATTACK[piece][square]として参照する
    static ref ADJACENT_ATTACK: [[Bitboard; Piece::BPawnX as usize + 1]; SQUARE_NB] = {
        let mut aa: [[Bitboard; Piece::BPawnX as usize + 1]; SQUARE_NB] = [[0; Piece::BPawnX as usize + 1]; SQUARE_NB];

        let mut position: Position = Position::empty_board();

        for i in 0..SQUARE_NB {
            for piece in PIECE_ALL.iter() {
                position.board[i] = *piece;
                position.side_to_move = piece.get_color();

                let moves = position.generate_moves(true, false);

                for m in moves {
                    if m.amount != 1 {
                        continue;
                    }

                    aa[i][*piece as usize] |= 1 << m.to;
                }
            }
            position.board[i] = Piece::NoPiece;
        }

        return aa;
    };

    /// 角の左下--右上方向の利きを参照するために用いるmask
    static ref BISHOP_MASK1: [u32; SQUARE_NB] = {
        let mut m: [u32; SQUARE_NB] = [0; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left_bottom = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y < 4 && x > 0 {
                    y += 1;
                    x -= 1;
                }

                5 * y + x
            };

            let right_top = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y > 0 && x < 4 {
                    y -= 1;
                    x += 1;
                }

                5 * y + x
            };

            let mut square = left_bottom;
            loop {
                if square == right_top {
                    break;
                }
                m[i] |= 1 << square;
                square -= 4;
            }
        }

        return m;
    };

    /// 角の左上--右下方向の利きを参照するために用いるmask
    static ref BISHOP_MASK2: [u32; SQUARE_NB] = {
        let mut m: [u32; SQUARE_NB] = [0; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left_top = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y > 0 && x > 0 {
                    y -= 1;
                    x -= 1;
                }

                5 * y + x
            };

            let right_bottom = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y < 4 && x < 4 {
                    y += 1;
                    x += 1;
                }

                5 * y + x
            };

            let mut square = left_top;
            loop {
                if square == right_bottom {
                    break;
                }
                m[i] |= 1 << square;
                square += 6;
            }
        }

        return m;
    };

    /// 飛車の横方向の利きを参照するために用いるmask
    static ref ROOK_MASK1: [u32; SQUARE_NB] = {
        let mut m: [u32; SQUARE_NB] = [0; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left: usize = (i / 5) * 5;

            for j in 0..5 {
                m[i] |= 1 << (left + j);
            }
        }

        return m;
    };

    /// 飛車の縦方向の利きを参照するために用いるmask
    static ref ROOK_MASK2: [u32; SQUARE_NB] = {
        let mut m: [u32; SQUARE_NB] = [0; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let top: usize = i % 5;

            for j in 0..5 {
                m[i] |= 1 << (top + 5 * j);
            }
        }

        return m;
    };

    /// 角の左下--右上方向の利きを保持するbitboard
    /// BISHOP_ATTACK1[pext((player_bb[WHITE] | player_bb[BLACK]), mask)][bishop_square]として参照する
    static ref BISHOP_ATTACK1: [[Bitboard; 32]; SQUARE_NB] = {
        let mut ba: [[Bitboard; 32]; SQUARE_NB] = [[0; 32]; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left_bottom = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y < 4 && x > 0 {
                    y += 1;
                    x -= 1;
                }

                5 * y + x
            };

            let right_top = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y > 0 && x < 4 {
                    y -= 1;
                    x += 1;
                }

                5 * y + x
            };

            for piece_bb in 0..32 {
                let mut position: Position = Position::empty_board();

                for j in 0..5 {
                    if left_bottom - 4 * j == right_top {
                        break
                    }

                    if piece_bb & (1 << j) != 0 {
                        position.board[left_bottom - 4 * j] = Piece::BPawn;
                    }
                }
                position.board[i] = Piece::WBishop;

                let moves = position.generate_moves(true, false);

                for m in moves {
                    // 左下--右上方向の合法手のみ取りだす
                    if m.direction == Direction::SW || m.direction == Direction::NE {
                        ba[piece_bb][i] |= 1 << m.to;
                    }
                }
            }
        }

        return ba;
    };

    /// 角の左上--右下方向の利きを保持するbitboard
    /// BISHOP_ATTACK2[pext((player_bb[WHITE] | player_bb[BLACK]), mask)][bishop_square]として参照する
    static ref BISHOP_ATTACK2: [[Bitboard; 32]; SQUARE_NB] = {
        let mut ba: [[Bitboard; 32]; SQUARE_NB] = [[0; 32]; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left_top = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y > 0 && x > 0 {
                    y -= 1;
                    x -= 1;
                }

                5 * y + x
            };

            let right_bottom = {
                let mut y = i / 5;
                let mut x = i % 5;

                while y < 4 && x < 4 {
                    y += 1;
                    x += 1;
                }

                5 * y + x
            };

            for piece_bb in 0..32 {
                let mut position: Position = Position::empty_board();

                for j in 0..5 {
                    if left_top + 6 * j == right_bottom {
                        break
                    }

                    if piece_bb & (1 << j) != 0 {
                        position.board[left_top + 6 * j] = Piece::BPawn;
                    }
                }
                position.board[i] = Piece::WBishop;

                let moves = position.generate_moves(true, false);

                for m in moves {
                    // 左上--右下方向の合法手のみ取りだす
                    if m.direction == Direction::NW || m.direction == Direction::SE {
                        ba[piece_bb][i] |= 1 << m.to;
                    }
                }
            }
        }

        return ba;
    };

    /// 飛車の横方向の利きを保持するbitboard
    /// ROOK_ATTACK1[pext((player_bb[WHITE] | player_bb[BLACK]), mask)][rook_square]として参照する
    static ref ROOK_ATTACK1: [[Bitboard; 32]; SQUARE_NB] = {
        let mut ra: [[Bitboard; 32]; SQUARE_NB] = [[0; 32]; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let left: usize = (i / 5) * 5;

            for piece_bb in 0..32 {
                let mut position: Position = Position::empty_board();

                for j in 0..5 {
                    if piece_bb & (1 << j) != 0 {
                        position.board[left + j] = Piece::BPawn;
                    }
                }
                position.board[i] = Piece::WRook;

                let moves = position.generate_moves(true, false);

                for m in moves {
                    // 横方向の合法手のみ取りだす
                    if m.direction == Direction::E || m.direction == Direction::W {
                        ra[piece_bb][i] |= 1 << m.to;
                    }
                }
            }
        }

        return ra;
    };

    /// 飛車の縦方向の利きを保持するbitboard
    /// ROOK_ATTACK2[pext((player_bb[WHITE] | player_bb[BLACK]), mask)][rook_square]として参照する
    static ref ROOK_ATTACK2: [[Bitboard; 32]; SQUARE_NB] = {
        let mut ra: [[Bitboard; 32]; SQUARE_NB] = [[0; 32]; SQUARE_NB];

        for i in 0..SQUARE_NB {
            let top: usize = i % 5;

            for piece_bb in 0..32 {
                let mut position: Position = Position::empty_board();

                for j in 0..5 {
                    if piece_bb & (1 << j) != 0 {
                        position.board[top + 5 * j] = Piece::BPawn;
                    }
                }
                position.board[i] = Piece::WRook;

                let moves = position.generate_moves(true, false);

                for m in moves {
                    // 縦方向の合法手のみ取りだす
                    if m.direction == Direction::N || m.direction == Direction::S {
                        ra[piece_bb][i] |= 1 << m.to;
                    }
                }
            }
        }

        return ra;
    };
}

pub fn init() {
    lazy_static::initialize(&BISHOP_MASK1);
    lazy_static::initialize(&BISHOP_MASK2);
    lazy_static::initialize(&ROOK_MASK1);
    lazy_static::initialize(&ROOK_MASK2);

    lazy_static::initialize(&ADJACENT_ATTACK);
    lazy_static::initialize(&BISHOP_ATTACK1);
    lazy_static::initialize(&BISHOP_ATTACK2);
    lazy_static::initialize(&ROOK_ATTACK1);
    lazy_static::initialize(&ROOK_ATTACK2);
}

pub fn adjacent_attack(piece: Piece, square: usize) -> Bitboard {
    ADJACENT_ATTACK[piece as usize][square]
}

pub fn bishop_attack(piece_bb: Bitboard, square: usize) -> Bitboard {
    BISHOP_ATTACK1[piece_bb.pext(BISHOP_MASK1[square]) as usize][square] | BISHOP_ATTACK2[piece_bb.pext(BISHOP_MASK2[square]) as usize][square]
}

pub fn rook_attack(piece_bb: Bitboard, square: usize) -> Bitboard {
    ROOK_ATTACK1[piece_bb.pext(ROOK_MASK1[square]) as usize][square] | ROOK_ATTACK2[piece_bb.pext(ROOK_MASK2[square]) as usize][square]
}

/// 一番末尾の1の場所を返す
pub fn get_square(bb: Bitboard) -> usize {
    bb.trailing_zeros() as usize
}
