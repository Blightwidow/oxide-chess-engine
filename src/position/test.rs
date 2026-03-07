#[cfg(test)]
mod test {
    use std::rc::Rc;

    use crate::{
        bitboards::{defs::EMPTY, Bitboards},
        hash::Hasher,
        movegen::Movegen,
        position::Position,
    };

    #[test]
    fn do_undo() {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let movegen = Movegen::new(Rc::clone(&bitboards));
        let mut initial_position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));

        let fen: &str = "r3k2r/p1pNqpb1/bn2pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1";
        position.set(fen.to_string());
        initial_position.set(fen.to_string());

        for mv in movegen.legal_moves(&position) {
            position.do_move(mv);
            position.undo_move(mv);

            assert_eq!(position.board, initial_position.board);
            assert_eq!(position.side_to_move, initial_position.side_to_move);
            assert_eq!(position.by_color_bb, initial_position.by_color_bb);
            assert_eq!(position.by_type_bb, initial_position.by_type_bb);
            assert_eq!(position.pinned_bb, initial_position.pinned_bb);
            assert_eq!(position.states.last().unwrap(), initial_position.states.last().unwrap());
        }
    }

    #[test]
    fn pinned_bb() {
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let mut position = Position::new(Rc::clone(&bitboards), Rc::clone(&hasher));

        let fen: &str = "rnbqkbnr/pp1ppppp/2p5/1B6/4P3/8/PPPP1PPP/RNBQK1NR b KQkq - 1 2";
        position.set(fen.to_string());

        assert_eq!(position.pinned_bb, [EMPTY, EMPTY, EMPTY]);
    }
}
